//! `acc release --range BASE..HEAD --subject NAME=sha256:HEX MANIFEST...`
//!
//! The release attestation: one in-toto Statement whose subjects are the release artifacts the
//! caller names (their digests, as a build produced them) and whose predicate lists every
//! first-parent commit in the range with the changes that landed it and their verdicts. It joins
//! change-level evidence to the build provenance of the same artifacts.
//!
//! Coverage is `acc verify`'s ([`crate::coverage`]): every first-parent commit in `BASE..HEAD`
//! must be the commit a change landed, and that change must pass the policy. The verdicts come
//! from the manifests given, judged one change at a time as `acc check` judges them, under
//! `--policy` when given and each manifest's own policy otherwise.
//!
//! The Statement is written whatever the verdict, because a release that shipped uncovered commits
//! is a fact worth attesting; the predicate's `verdict` says which. It is unsigned: sign it as you
//! sign acc's other attestations (see docs/signing.md).
//!
//! Exit: `0` every commit covered by a passing change; `1` a commit uncovered or failing; `4` a
//! commit whose only verdicts are incomplete (takes precedence over `1`); `2` usage, including a
//! range git cannot resolve or a malformed subject; `3` a manifest that does not validate.

use super::Ctx;
use super::io;
use super::verify::{first_parent, parse_range, resolve};
use crate::coverage::{self, CommitCoverage, Judged, Status};
use crate::manifest::{self, ChangeVerdict};
use crate::{Exit, failure_with_hint};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::PathBuf;

/// The predicate type of a release attestation, versioned on its own like the review document's.
pub const RELEASE_PREDICATE_TYPE: &str = "https://noru.tech/spec/ai-change-provenance/release/v0.1";

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Manifests (JSON or YAML) whose changes may have landed the range's commits.
    #[arg(required = true, value_name = "MANIFEST")]
    pub manifests: Vec<PathBuf>,
    /// The commits the release shipped, as BASE..HEAD: the previous release and this one, for
    /// example v1.4.0..v1.5.0.
    #[arg(long, value_name = "BASE..HEAD")]
    pub range: String,
    /// A release artifact, as NAME=sha256:HEX (repeatable; at least one): the subjects of the
    /// Statement, for example the archive and checksum your build produced.
    #[arg(long, value_name = "NAME=sha256:HEX", required = true)]
    pub subject: Vec<String>,
    /// Policy file; overrides the policy embedded in each manifest.
    #[arg(long, value_name = "FILE")]
    pub policy: Option<PathBuf>,
    /// Calendar date dispositions are evaluated on (YYYY-MM-DD). Required when a finding has a
    /// non-open disposition; the machine clock is never consulted.
    #[arg(long, value_name = "DATE")]
    pub as_of: Option<String>,
    /// The local clone the range is read from.
    #[arg(long, value_name = "DIR", default_value = ".")]
    pub checkout: PathBuf,
    /// Write the Statement to FILE instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

/// `NAME=sha256:HEX` as an in-toto subject.
fn subject(s: &str) -> Result<Value> {
    let invalid = || {
        failure_with_hint(
            Exit::Usage,
            format!("invalid --subject: {s}"),
            "use NAME=sha256:HEX with a 64-character lowercase hexadecimal digest, for example --subject acc.tar.xz=sha256:3f…",
        )
    };
    let (name, digest) = s.split_once('=').ok_or_else(invalid)?;
    let hex = digest.strip_prefix("sha256:").ok_or_else(invalid)?;
    if name.trim().is_empty()
        || hex.len() != 64
        || !hex
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
        return Err(invalid());
    }
    Ok(json!({"name": name, "digest": {"sha256": hex}}))
}

/// SHA-256 over the RFC 8785 bytes of a change's verdict: what a reader can recompute from the
/// manifest to check the predicate did not restate it.
fn verdict_digest(v: &ChangeVerdict) -> Result<String> {
    crate::canonical::digest(&json!({
        "change_id": v.change_id,
        "merge_commit_sha": v.merge_commit_sha,
        "verdict": v.verdict.as_str(),
        "failing": v.failing,
    }))
}

fn commit_object(c: &CommitCoverage, digests: &[(String, String)]) -> Result<Value> {
    let changes = c
        .judged
        .iter()
        .map(|j| {
            let manifest = digests
                .iter()
                .find(|(source, _)| *source == j.source)
                .map(|(_, d)| d.clone());
            Ok(json!({
                "change_id": j.change.change_id,
                "verdict": j.change.verdict.as_str(),
                "failing": j.change.failing,
                "verdict_digest": verdict_digest(&j.change)?,
                "manifest_digest": manifest,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({"commit": c.commit, "status": c.status.as_str(), "changes": changes}))
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let subjects = args
        .subject
        .iter()
        .map(|s| subject(s))
        .collect::<Result<Vec<_>>>()?;
    let as_of = args.as_of.as_deref().map(super::check::as_of).transpose()?;
    let policy = args
        .policy
        .as_deref()
        .map(|p| io::load_policy(Some(p)))
        .transpose()?;
    let (base, head) = parse_range(&args.range)?;

    let mut judged = Vec::new();
    let mut digests = Vec::new();
    let mut sources = Vec::new();
    for path in &args.manifests {
        let source = path.display().to_string();
        let m = io::read(path, "manifest")?;
        manifest::validate(&m).with_context(|| format!("manifest {source}"))?;
        let digest = crate::canonical::digest(&m)?;
        for change in manifest::verdicts(&m, policy.clone(), as_of)
            .with_context(|| format!("manifest {source}"))?
        {
            judged.push(Judged {
                source: source.clone(),
                change,
            });
        }
        sources.push(json!({
            "digest": digest,
            "source_digest": m.generated.source_digest,
            "repository": m.events.repository,
        }));
        digests.push((source, digest));
    }

    let base_sha = resolve(&args.checkout, base)?;
    let head_sha = resolve(&args.checkout, head)?;
    let commits = first_parent(&args.checkout, &base_sha, &head_sha)?;
    ctx.debug(format!(
        "{} first-parent commit(s) in {base_sha}..{head_sha}, {} change verdict(s)",
        commits.len(),
        judged.len()
    ));
    let coverage = coverage::cover(&commits, &judged);
    let exit = coverage::exit(&coverage);
    let verdict = match exit {
        Exit::Ok => "pass",
        Exit::Incomplete => "incomplete",
        _ => "fail",
    };
    let uncovered: Vec<&str> = coverage
        .iter()
        .filter(|c| c.status == Status::Uncovered)
        .map(|c| c.commit.as_str())
        .collect();
    let statement = json!({
        "_type": crate::output::intoto::STATEMENT_TYPE,
        "subject": subjects,
        "predicateType": RELEASE_PREDICATE_TYPE,
        "predicate": {
            "range": {"base": base_sha, "head": head_sha},
            "verdict": verdict,
            "commits": coverage
                .iter()
                .map(|c| commit_object(c, &digests))
                .collect::<Result<Vec<_>>>()?,
            "uncovered": uncovered,
            "manifests": sources,
            "policy": if args.policy.is_some() { "given" } else { "embedded" },
            "as_of": as_of.map(|d| d.to_string()),
            "generated": {"tool": "agent-change-control", "version": crate::version()},
        },
    });
    crate::normalize::schema(&statement, "release")?;
    io::write(
        &(crate::canonical::jcs_bytes(&statement)? + "\n"),
        args.output.as_deref(),
    )?;
    let count = |s: Status| coverage.iter().filter(|c| c.status == s).count();
    ctx.note(format!(
        "Release {verdict}: {} of {} commit(s) in {} covered by a passing change ({} uncovered, {} failing, {} incomplete)",
        count(Status::Covered),
        coverage.len(),
        args.range,
        count(Status::Uncovered),
        count(Status::Failing),
        count(Status::Incomplete),
    ));
    Ok(exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subjects_are_a_name_and_a_sha256() {
        let hex = "a".repeat(64);
        assert_eq!(
            subject(&format!("acc.tar.xz=sha256:{hex}")).unwrap(),
            json!({"name": "acc.tar.xz", "digest": {"sha256": hex}})
        );
        for bad in [
            "acc.tar.xz",
            "=sha256:aa",
            &format!("x=sha1:{hex}"),
            &format!("x=sha256:{}", "A".repeat(64)),
            "x=sha256:abc",
        ] {
            assert_eq!(
                crate::exit_for(&subject(bad).unwrap_err()),
                Exit::Usage,
                "{bad}"
            );
        }
    }
}
