//! `acc verify --range BASE..HEAD (--verification FILE --signer IDENTITY | --attestations PATH --verified-by TEXT)`
//!
//! The deploy gate. Every first-parent commit in `BASE..HEAD` must be the commit a change landed,
//! and that change must pass the policy according to a signed acc attestation (see
//! [`crate::coverage`]).
//!
//! acc does not check signatures. The caller verifies them with the signer's tooling first, as
//! everywhere else in acc:
//!
//! * `--verification`: the JSON `gh attestation verify --format json` writes. acc reads the
//!   certificate identity the verifier established and requires it to be one `--signer` names
//!   (and its issuer to be `--issuer`, when given), so the identity the gate trusts is written in
//!   the gate.
//! * `--attestations` with `--verified-by`: signed DSSE envelopes or Sigstore bundles the caller
//!   verified with its own identity flags (for example `cosign verify-blob-attestation
//!   --certificate-identity …`). acc records the caller's statement; an unsigned Statement is
//!   refused.
//!
//! Each Statement is then validated exactly as `acc validate` validates one (schema, subjects,
//! byte-identical re-evaluation) and judged as `acc check` judges a manifest, one change at a time,
//! under `--policy` when given and the embedded policy otherwise.
//!
//! The range comes from the local clone: `git rev-list --first-parent BASE..HEAD`. A shallow clone
//! is refused, because it cannot show that the range is whole. Nothing else reads the network or
//! the clock.
//!
//! Exit: `0` every commit covered by a passing change; `1` a commit uncovered or failing; `4` a
//! commit whose only verdicts are incomplete (takes precedence over `1`); `2` usage, including a
//! range git cannot resolve; `3` an attestation that is invalid, unsigned or signed by an identity
//! the gate does not trust.

use super::Ctx;
use super::io::{self, ResultFormat};
use crate::coverage::{self, CommitCoverage, Judged, Status};
use crate::model::Attestation;
use crate::output::intoto;
use crate::provenance::attestations::Attestations;
use crate::{Exit, failure, failure_with_hint, manifest};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SHALLOW_HINT: &str = "check out with full history (actions/checkout `fetch-depth: 0`, GitLab `GIT_DEPTH: 0`) so every commit in the range is present";

#[derive(Debug, clap::Args)]
pub struct Args {
    /// The commits to cover, as BASE..HEAD: the commit deployed last and the one about to be.
    /// Every first-parent commit after BASE up to and including HEAD must be covered.
    #[arg(long, value_name = "BASE..HEAD")]
    pub range: String,
    /// A verifier's JSON output (`gh attestation verify --format json`), whose bundles hold acc
    /// attestations (repeatable; requires --signer).
    #[arg(long, value_name = "PATH")]
    pub verification: Vec<PathBuf>,
    /// A certificate identity the gate trusts to sign verdicts, compared exactly with the
    /// verifier's subject alternative name, for example
    /// https://github.com/acme/api/.github/workflows/attest.yml@refs/heads/main (repeatable).
    #[arg(long, value_name = "IDENTITY")]
    pub signer: Vec<String>,
    /// The OIDC issuer the signer's identity must come from, for example
    /// https://token.actions.githubusercontent.com.
    #[arg(long, value_name = "URL", requires = "signer")]
    pub issuer: Option<String>,
    /// Signed DSSE envelopes or Sigstore bundles (.json or .jsonl, files or directories) whose
    /// signatures the caller has verified (repeatable; requires --verified-by).
    #[arg(long, value_name = "PATH", requires = "verified_by")]
    pub attestations: Vec<PathBuf>,
    /// Who verified the --attestations signatures, and against which identity, recorded
    /// verbatim in the result.
    #[arg(long, value_name = "TEXT", requires = "attestations")]
    pub verified_by: Option<String>,
    /// Policy file; overrides the policy embedded in each attestation.
    #[arg(long, value_name = "FILE")]
    pub policy: Option<PathBuf>,
    /// Calendar date dispositions are evaluated on (YYYY-MM-DD). Required when a signed finding
    /// has a non-open disposition; the machine clock is never consulted.
    #[arg(long, value_name = "DATE")]
    pub as_of: Option<String>,
    /// The local clone the range is read from.
    #[arg(long, value_name = "DIR", default_value = ".")]
    pub checkout: PathBuf,
    /// Result format: `text` (one line per commit) or `json` (a result object); inferred from
    /// the --output extension when omitted.
    #[arg(short, long, value_enum)]
    pub format: Option<ResultFormat>,
    /// Write the result to FILE instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

/// `BASE..HEAD`, each side a revision git can resolve. A symmetric range (`...`) and anything
/// that looks like an option are refused before git sees them.
fn parse_range(range: &str) -> Result<(&str, &str)> {
    let invalid = || {
        failure_with_hint(
            Exit::Usage,
            format!("invalid --range: {range}"),
            "use BASE..HEAD, for example --range v1.4.0..HEAD or --range 1a2b3c4..5d6e7f8",
        )
    };
    let (base, head) = range.split_once("..").ok_or_else(invalid)?;
    let plain = |s: &str| {
        !s.is_empty()
            && !s.starts_with('-')
            && !s.starts_with('.')
            && !s.ends_with('.')
            && !s.chars().any(char::is_whitespace)
    };
    if !plain(base) || !plain(head) {
        return Err(invalid());
    }
    Ok((base, head))
}

/// Run a read-only git command in `dir` and return its stdout.
fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            failure_with_hint(
                Exit::Usage,
                format!("unable to run git: {e}"),
                "acc verify reads the range from a local clone; install git",
            )
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(failure_with_hint(
            Exit::Usage,
            format!("git {}: {}", args.join(" "), stderr.trim()),
            SHALLOW_HINT,
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The full commit SHA `rev` names.
fn resolve(dir: &Path, rev: &str) -> Result<String> {
    let sha = git(
        dir,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    )?;
    Ok(sha.trim().to_ascii_lowercase())
}

/// The first-parent commits after `base` up to and including `head`, newest first.
fn first_parent(dir: &Path, base: &str, head: &str) -> Result<Vec<String>> {
    if git(dir, &["rev-parse", "--is-shallow-repository"])?.trim() == "true" {
        return Err(failure_with_hint(
            Exit::Usage,
            "the checkout is shallow, so the range cannot be shown to be whole",
            SHALLOW_HINT,
        ));
    }
    let listed = git(
        dir,
        &["rev-list", "--first-parent", &format!("{base}..{head}")],
    )?;
    Ok(listed
        .lines()
        .map(|l| l.trim().to_ascii_lowercase())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Whether `record` was signed by an identity the gate trusts.
fn trusted(record: &Attestation, args: &Args) -> Result<()> {
    match &record.signer {
        Some(signer) => {
            if !args.signer.iter().any(|s| s == &signer.identity) {
                return Err(failure_with_hint(
                    Exit::InvalidInput,
                    format!(
                        "attestation {} was signed by {}, which no --signer names",
                        record.file, signer.identity
                    ),
                    "pass the identity of the workflow that signs your verdicts with --signer",
                ));
            }
            if let Some(issuer) = &args.issuer
                && signer.issuer.as_deref() != Some(issuer.as_str())
            {
                return Err(failure(
                    Exit::InvalidInput,
                    format!(
                        "attestation {} was signed through issuer {}, not {issuer}",
                        record.file,
                        signer.issuer.as_deref().unwrap_or("(none)")
                    ),
                ));
            }
        }
        None if !record.signed => {
            return Err(failure(
                Exit::InvalidInput,
                format!(
                    "attestation {} carries no signature; a deploy gate accepts only signed verdicts",
                    record.file
                ),
            ));
        }
        None => {}
    }
    Ok(())
}

fn attestation_object(record: &Attestation) -> Value {
    json!({
        "file": record.file,
        "payload_digest": record.payload_digest,
        "predicate_type": record.predicate_type,
        "signer": record.signer.as_ref().map(|s| json!({"identity": s.identity, "issuer": s.issuer})),
        "verified_by": if record.signer.is_some() { Value::Null } else { json!(record.verified_by) },
    })
}

fn commit_object(c: &CommitCoverage) -> Value {
    json!({
        "commit": c.commit,
        "status": c.status.as_str(),
        "changes": c.judged.iter().map(|j| json!({
            "change_id": j.change.change_id,
            "verdict": j.change.verdict.as_str(),
            "failing": j.change.failing,
            "attestation": j.source,
        })).collect::<Vec<_>>(),
    })
}

fn text(coverage: &[CommitCoverage]) -> String {
    let mut out = format!(
        "{:<12}  {:<10}  {:<32}  {}\n",
        "COMMIT", "STATUS", "CHANGE", "ATTESTATION"
    );
    for c in coverage {
        let change = c
            .judged
            .first()
            .map_or("-", |j| j.change.change_id.as_str());
        let source = c.judged.first().map_or("-", |j| j.source.as_str());
        out.push_str(&format!(
            "{:<12}  {:<10}  {:<32}  {}\n",
            &c.commit[..c.commit.len().min(12)],
            c.status.as_str(),
            change,
            source
        ));
    }
    out
}

fn summary(coverage: &[CommitCoverage], range: &str) -> String {
    let count = |s: Status| coverage.iter().filter(|c| c.status == s).count();
    if coverage.iter().all(|c| c.status == Status::Covered) {
        return format!(
            "Verified: {} commit(s) in {range}, each landed by a change that passes",
            coverage.len()
        );
    }
    format!(
        "Not verified: {} of {} commit(s) in {range} not covered ({} uncovered, {} failing, {} incomplete)",
        coverage.len() - count(Status::Covered),
        coverage.len(),
        count(Status::Uncovered),
        count(Status::Failing),
        count(Status::Incomplete),
    )
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    if args.verification.is_empty() && args.attestations.is_empty() {
        return Err(failure_with_hint(
            Exit::Usage,
            "nothing to verify against",
            "pass --verification with gh attestation verify's JSON, or --attestations with --verified-by",
        ));
    }
    if !args.verification.is_empty() && args.signer.is_empty() {
        return Err(failure_with_hint(
            Exit::Usage,
            "--verification requires --signer",
            "name the identity that signs your verdicts, for example --signer https://github.com/OWNER/REPO/.github/workflows/attest.yml@refs/heads/main",
        ));
    }
    if args.verification.is_empty() && !args.signer.is_empty() {
        return Err(failure(
            Exit::Usage,
            "--signer applies to --verification; for --attestations, pass the identity to your verifier and say so with --verified-by",
        ));
    }
    if args
        .verified_by
        .as_deref()
        .is_some_and(|v| v.trim().is_empty())
    {
        return Err(failure(Exit::Usage, "verified-by must not be empty"));
    }
    let as_of = args.as_of.as_deref().map(super::check::as_of).transpose()?;
    let policy = args
        .policy
        .as_deref()
        .map(|p| io::load_policy(Some(p)))
        .transpose()?;
    let (base, head) = parse_range(&args.range)?;

    let mut loaded = Attestations::load(&args.attestations, args.verified_by.clone())?;
    for path in &args.verification {
        loaded.load_verification(path)?;
    }
    let mut judged = Vec::new();
    let mut records = Vec::new();
    for (record, statement) in loaded.statements() {
        trusted(record, &args)?;
        let m = intoto::validate_statement(statement)
            .with_context(|| format!("attestation {}", record.file))?;
        for change in manifest::verdicts(&m, policy.clone(), as_of)
            .with_context(|| format!("attestation {}", record.file))?
        {
            judged.push(Judged {
                source: record.file.clone(),
                change,
            });
        }
        records.push(attestation_object(record));
    }
    ctx.debug(format!(
        "{} attestation(s), {} change verdict(s)",
        records.len(),
        judged.len()
    ));

    let base_sha = resolve(&args.checkout, base)?;
    let head_sha = resolve(&args.checkout, head)?;
    let commits = first_parent(&args.checkout, &base_sha, &head_sha)?;
    ctx.debug(format!(
        "{} first-parent commit(s) in {base_sha}..{head_sha}",
        commits.len()
    ));
    let coverage = coverage::cover(&commits, &judged);
    let exit = coverage::exit(&coverage);

    let rendered = match ResultFormat::resolve(args.format, args.output.as_deref()) {
        ResultFormat::Json => {
            let object = json!({
                "verified": exit == Exit::Ok,
                "range": {"base": base_sha, "head": head_sha},
                "commits": coverage.iter().map(commit_object).collect::<Vec<_>>(),
                "attestations": records,
            });
            crate::canonical::jcs_bytes(&object)? + "\n"
        }
        ResultFormat::Text => text(&coverage),
    };
    io::write(&rendered, args.output.as_deref())?;
    ctx.note(summary(&coverage, &args.range));
    Ok(exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_two_plain_revisions() {
        assert_eq!(parse_range("v1.0..HEAD").unwrap(), ("v1.0", "HEAD"));
        assert_eq!(parse_range("abc123..def456").unwrap(), ("abc123", "def456"));
        for bad in [
            "HEAD",
            "..HEAD",
            "v1..",
            "a...b",
            "--output=x..HEAD",
            "a..-b",
            "a b..c",
        ] {
            assert_eq!(
                crate::exit_for(&parse_range(bad).unwrap_err()),
                Exit::Usage,
                "{bad}"
            );
        }
    }
}
