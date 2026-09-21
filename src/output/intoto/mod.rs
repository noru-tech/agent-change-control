//! in-toto Statement v1 output and validation. The predicate is the manifest itself.
//!
//! Two shapes are produced. `render` writes one unsigned Statement whose subjects are every
//! evaluated change. `render_jsonl` writes JSON Lines, one Statement per change, so a signer
//! and a verifier can handle one change at a time; each line's predicate is a manifest that
//! covers that change alone, and its finding identifiers are the ones the full manifest carries.
//!
//! Subjects are git commits. A change contributes its head commit (the commit the approvals are
//! bound to) and, once merged with a known merge commit that differs from the head, the merge
//! commit as well, because after a squash or rebase merge only the merge commit is reachable
//! from the target branch. Nothing is invented: a merged change whose merge commit the forge did
//! not report has a head subject only.
//!
//! Statements are deliberately unsigned. Signing is a deployment decision: wrap the canonical
//! bytes in a DSSE envelope with the signer your organization already trusts (Sigstore, an HSM,
//! a KMS key). A verifier unwraps the envelope, checks the subject digests against the commits it
//! cares about, and runs `acc validate` on the Statement to confirm the subjects match the
//! predicate and the findings follow from the embedded facts. See `docs/in-toto.md` and
//! `spec/ai-change-provenance.md` §7.3.

use crate::model::{Change, Events, Evidence, Manifest};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// The in-toto Statement layer this renderer emits.
pub const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";
/// The predicate type: an AI Change Provenance manifest, versioned with the specification.
pub const PREDICATE_TYPE: &str = "https://noru.tech/spec/ai-change-provenance/v0.1";

/// The subject entries one change contributes: its head commit, then its merge commit when
/// merged, known and distinct from the head.
pub fn subjects(c: &Change) -> Vec<Value> {
    let mut out = vec![json!({
        "name": c.id,
        "digest": {"gitCommit": c.head_sha},
    })];
    if let Some(merge) = c.merge_commit_sha.as_ref().filter(|m| **m != c.head_sha) {
        out.push(json!({
            "name": format!("{}:merge", c.id),
            "digest": {"gitCommit": merge},
        }));
    }
    out
}

fn statement(m: &Manifest) -> Result<Value> {
    ensure!(
        !m.events.changes.is_empty(),
        "an attestation needs at least one change as its subject"
    );
    let subject: Vec<Value> = m.events.changes.iter().flat_map(subjects).collect();
    Ok(json!({
        "_type": STATEMENT_TYPE,
        "subject": subject,
        "predicateType": PREDICATE_TYPE,
        "predicate": m,
    }))
}

/// One Statement covering every change in the manifest.
pub fn render(m: &Manifest) -> Result<String> {
    crate::normalize::canonical(&statement(m)?)
}

/// JSON Lines: one Statement per change, in change ID order.
pub fn render_jsonl(m: &Manifest) -> Result<String> {
    ensure!(
        !m.events.changes.is_empty(),
        "an attestation needs at least one change as its subject"
    );
    let mut out = String::new();
    for part in per_change(m)? {
        out.push_str(&crate::normalize::canonical(&statement(&part)?)?);
    }
    Ok(out)
}

/// Every evidence entry of a change.
fn all_evidence(c: &Change) -> impl Iterator<Item = &Evidence> {
    c.provenance
        .iter()
        .chain(&c.author.provenance)
        .chain(&c.forge_author.provenance)
        .chain(c.agent_operator.iter().flat_map(|o| &o.provenance))
        .chain(c.merger.iter().flat_map(|m| &m.provenance))
        .chain(
            c.commits
                .iter()
                .filter_map(|k| k.author.as_ref())
                .flat_map(|a| &a.provenance),
        )
        .chain(c.reviews.iter().flat_map(|r| &r.provenance))
}

/// The actors a change refers to.
fn referenced_actors(c: &Change) -> BTreeSet<&str> {
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    ids.insert(&c.author.actor_id);
    ids.insert(&c.forge_author.actor_id);
    if let Some(id) = c
        .agent_operator
        .as_ref()
        .and_then(|o| o.actor_id.as_deref())
    {
        ids.insert(id);
    }
    if let Some(m) = &c.merger {
        ids.insert(&m.actor_id);
    }
    ids.extend(
        c.commits
            .iter()
            .filter_map(|k| k.author.as_ref())
            .map(|a| a.actor_id.as_str()),
    );
    ids.extend(c.reviews.iter().map(|r| r.actor_id.as_str()));
    ids
}

/// Split a manifest into one manifest per change. Each keeps the window and the resolved policy,
/// carries only the actors its change refers to, and is re-evaluated so that it validates on its
/// own. Finding identifiers are unchanged, since they depend only on the repository, the change,
/// the rule and the actors, and recorded dispositions are carried over by identifier.
pub fn per_change(m: &Manifest) -> Result<Vec<Manifest>> {
    m.events
        .changes
        .iter()
        .map(|c| {
            let referenced = referenced_actors(c);
            let events = Events {
                version: m.events.version.clone(),
                repository: m.events.repository.clone(),
                window: m.events.window.clone(),
                actors: m
                    .events
                    .actors
                    .iter()
                    .filter(|(id, _)| referenced.contains(id.as_str()))
                    .map(|(id, a)| (id.clone(), a.clone()))
                    .collect(),
                attestations: m
                    .events
                    .attestations
                    .iter()
                    .filter(|(id, _)| all_evidence(c).any(|ev| ev.r#ref == **id))
                    .map(|(id, a)| (id.clone(), a.clone()))
                    .collect(),
                changes: vec![c.clone()],
            };
            let mut part = crate::manifest::evaluate(events, m.policy.clone())?;
            for f in &mut part.findings {
                if let Some(old) = m.findings.iter().find(|old| old.id == f.id) {
                    f.disposition = old.disposition.clone();
                }
            }
            Ok(part)
        })
        .collect()
}

/// Whether a Statement's subjects fit its predicate. Two forms are accepted: exactly the commit
/// subjects the predicate's changes produce, or, for signers that accept only SHA-2 digests
/// (GitHub artifact attestations, `cosign attest-blob`), a single subject whose `sha256` digest
/// is over the canonical bytes of the predicate, which is the manifest as `--format json` writes
/// it. The commits are then found inside the predicate.
fn subjects_match(v: &Value, m: &Manifest) -> Result<bool> {
    let commits: Vec<Value> = m.events.changes.iter().flat_map(subjects).collect();
    if v["subject"] == Value::Array(commits) {
        return Ok(true);
    }
    let Some([one]) = v["subject"].as_array().map(Vec::as_slice) else {
        return Ok(false);
    };
    let Some(hex) = one["digest"]["sha256"].as_str() else {
        return Ok(false);
    };
    Ok(crate::normalize::digest(m)? == format!("sha256:{}", hex.to_ascii_lowercase()))
}

/// Validate one Statement: the statement schema, the predicate type, the predicate as a manifest
/// (schema, timeline and byte-identical re-evaluation), and subjects that fit the predicate (see
/// [`subjects_match`]). Returns the predicate.
pub fn validate_statement(v: &Value) -> Result<Manifest> {
    crate::normalize::schema(v, "statement")?;
    ensure!(
        v["predicateType"] == PREDICATE_TYPE,
        "unsupported predicate type; expected {PREDICATE_TYPE}"
    );
    crate::normalize::schema(&v["predicate"], "manifest")?;
    let m: Manifest = serde_json::from_value(v["predicate"].clone())?;
    crate::manifest::validate(&m)?;
    ensure!(
        subjects_match(v, &m)?,
        "statement subjects do not match the predicate's changes"
    );
    Ok(m)
}

/// Validate JSON Lines of Statements: every line is a Statement about exactly one change, and
/// the lines are in strictly ascending change ID order. Returns the predicates.
pub fn validate_jsonl(text: &str) -> Result<Vec<Manifest>> {
    let mut out: Vec<Manifest> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let n = i + 1;
        ensure!(!line.trim().is_empty(), "line {n} is empty");
        let v: Value =
            serde_json::from_str(line).with_context(|| format!("line {n} is not JSON"))?;
        let m = validate_statement(&v).with_context(|| format!("line {n}"))?;
        ensure!(
            m.events.changes.len() == 1,
            "line {n}: a JSON Lines statement covers exactly one change"
        );
        let id = &m.events.changes[0].id;
        ensure!(
            out.last()
                .is_none_or(|prev| prev.events.changes[0].id < *id),
            "line {n}: statements are not in change ID order"
        );
        out.push(m);
    }
    ensure!(!out.is_empty(), "no statements found");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn evidence() -> Evidence {
        Evidence {
            source: "github_api".into(),
            r#ref: "https://api.github.com/repos/acme/api/pulls/1".into(),
            kind: EvidenceKind::Observed,
        }
    }

    fn manifest(changes: Vec<Change>) -> Manifest {
        let mut actors = std::collections::BTreeMap::new();
        for (id, name) in [("github:alice", "Alice"), ("github:bob", "Bob")] {
            actors.insert(
                id.to_string(),
                Actor {
                    kind: ActorKind::Human,
                    display_name: Some(name.into()),
                },
            );
        }
        let events = Events {
            version: "0.1".into(),
            repository: "acme/api".into(),
            window: Window {
                from: crate::normalize::timestamp("2026-08-01T00:00:00Z").unwrap(),
                to: crate::normalize::timestamp("2026-08-31T23:59:59Z").unwrap(),
                complete: true,
                reason: None,
            },
            actors,
            attestations: std::collections::BTreeMap::new(),
            changes,
        };
        crate::manifest::evaluate(events, Policy::default()).unwrap()
    }

    /// An open change by alice, or a merged one when `merge` is given.
    fn change(number: u32, merge: Option<&str>) -> Change {
        let alice = Identity {
            actor_id: "github:alice".into(),
            provenance: vec![evidence()],
        };
        let merged = merge.map(|_| crate::normalize::timestamp("2026-08-03T00:00:00Z").unwrap());
        Change {
            id: format!("github:acme/api:pr:{number}"),
            repository: "acme/api".into(),
            forge: Forge::Github,
            title: "t".into(),
            url: format!("https://github.com/acme/api/pull/{number}"),
            opened_at: crate::normalize::timestamp("2026-08-02T00:00:00Z").unwrap(),
            merged_at: merged,
            head_sha: "0123abcd".into(),
            merge_commit_sha: merge.map(String::from),
            commits: vec![],
            forge_author: alice.clone(),
            author: alice.clone(),
            agent_operator: None,
            reviews: vec![],
            reviews_complete: true,
            merger: merged.map(|_| alice),
            provenance: vec![evidence()],
        }
    }

    #[test]
    fn empty_windows_have_no_subject_and_are_refused() {
        let err = render(&manifest(vec![])).unwrap_err();
        assert!(err.to_string().contains("subject"));
        assert!(render_jsonl(&manifest(vec![])).is_err());
    }

    #[test]
    fn subjects_are_head_commits_and_the_predicate_is_the_manifest() {
        let m = manifest(vec![change(1, None)]);
        let out = render(&m).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["_type"], STATEMENT_TYPE);
        assert_eq!(v["predicateType"], PREDICATE_TYPE);
        assert_eq!(v["subject"].as_array().unwrap().len(), 1);
        assert_eq!(v["subject"][0]["name"], "github:acme/api:pr:1");
        assert_eq!(v["subject"][0]["digest"]["gitCommit"], "0123abcd");
        let predicate: Manifest = serde_json::from_value(v["predicate"].clone()).unwrap();
        crate::manifest::validate(&predicate).unwrap();
        assert_eq!(render(&m).unwrap(), out, "byte-stable");
        validate_statement(&v).unwrap();
    }

    #[test]
    fn merged_changes_add_a_distinct_merge_commit_subject() {
        let v: Value =
            serde_json::from_str(&render(&manifest(vec![change(1, Some("m3rg3"))])).unwrap())
                .unwrap();
        let subjects = v["subject"].as_array().unwrap();
        assert_eq!(subjects.len(), 2);
        assert_eq!(subjects[0]["digest"]["gitCommit"], "0123abcd");
        assert_eq!(subjects[1]["name"], "github:acme/api:pr:1:merge");
        assert_eq!(subjects[1]["digest"]["gitCommit"], "m3rg3");
        // A merge commit equal to the head (a fast-forward) is not repeated.
        let v: Value =
            serde_json::from_str(&render(&manifest(vec![change(1, Some("0123abcd"))])).unwrap())
                .unwrap();
        assert_eq!(v["subject"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn jsonl_has_one_statement_per_change_in_id_order_with_stable_finding_ids() {
        // pr:10 sorts before pr:9 lexicographically, like everywhere else in the manifest.
        let m = manifest(vec![change(9, Some("m9")), change(10, Some("m10"))]);
        let out = render_jsonl(&m).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["subject"][0]["name"], "github:acme/api:pr:10");
        assert_eq!(first["subject"][1]["digest"]["gitCommit"], "m10");
        assert_eq!(first["predicate"]["summary"]["changes"], 1);
        // Only the actors the change refers to travel with it.
        assert!(first["predicate"]["events"]["actors"]["github:bob"].is_null());
        let parts = validate_jsonl(&out).unwrap();
        assert_eq!(parts.len(), 2);
        let ids: Vec<&str> = parts
            .iter()
            .flat_map(|p| p.findings.iter().map(|f| f.id.as_str()))
            .collect();
        let full: Vec<&str> = m.findings.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, full);
        assert_eq!(render_jsonl(&m).unwrap(), out, "byte-stable");
    }

    #[test]
    fn a_single_sha256_subject_over_the_canonical_predicate_is_accepted() {
        let m = manifest(vec![change(1, Some("m1"))]);
        let digest = crate::normalize::digest(&m).unwrap();
        let hex = digest.trim_start_matches("sha256:");
        let mut v: Value = serde_json::from_str(&render(&m).unwrap()).unwrap();
        v["subject"] = json!([{"name": "acc-manifest.json", "digest": {"sha256": hex}}]);
        validate_statement(&v).unwrap();
        // Signers may upper-case hex; a wrong digest, or two such subjects, are rejected.
        v["subject"][0]["digest"]["sha256"] = hex.to_ascii_uppercase().into();
        validate_statement(&v).unwrap();
        v["subject"][0]["digest"]["sha256"] = "0".repeat(64).into();
        assert!(validate_statement(&v).is_err());
        v["subject"] = json!([
            {"name": "a", "digest": {"sha256": hex}},
            {"name": "b", "digest": {"sha256": hex}}
        ]);
        assert!(validate_statement(&v).is_err());
        v["subject"] = json!([{"name": "a", "digest": {"sha512": hex}}]);
        assert!(validate_statement(&v).is_err());
    }

    #[test]
    fn validation_rejects_tampered_subjects_and_unordered_lines() {
        let m = manifest(vec![change(1, Some("m1"))]);
        let mut v: Value = serde_json::from_str(&render(&m).unwrap()).unwrap();
        v["subject"][0]["digest"]["gitCommit"] = "other".into();
        assert!(
            validate_statement(&v)
                .unwrap_err()
                .to_string()
                .contains("subjects")
        );
        let mut v: Value = serde_json::from_str(&render(&m).unwrap()).unwrap();
        v["predicateType"] = "https://example.com/other".into();
        assert!(
            validate_statement(&v)
                .unwrap_err()
                .to_string()
                .contains("predicate type")
        );
        let mut v: Value = serde_json::from_str(&render(&m).unwrap()).unwrap();
        v["subject"] = json!([]);
        assert!(validate_statement(&v).is_err());
        let two = manifest(vec![change(9, None), change(10, None)]);
        let out = render_jsonl(&two).unwrap();
        let reversed: String = out.lines().rev().map(|l| format!("{l}\n")).collect();
        assert!(
            validate_jsonl(&reversed)
                .unwrap_err()
                .to_string()
                .contains("order")
        );
        assert!(
            validate_jsonl(&format!("{out}\n"))
                .unwrap_err()
                .to_string()
                .contains("empty")
        );
        assert!(validate_jsonl("").is_err());
        let multi = render(&two).unwrap();
        assert!(
            validate_jsonl(&multi)
                .unwrap_err()
                .to_string()
                .contains("exactly one change")
        );
    }
}
