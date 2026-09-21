//! Schema validation, timeline integrity checks, canonical ordering and canonical JSON.
//!
//! Integrity errors carry `ACV` codes: ACV001 approval after merge, ACV003 invalid actor
//! relationships, ACV004 inconsistent timeline or duplicated/ambiguous events.

use crate::model::*;
use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Validate `value` against one of the embedded schemas: `events`, `manifest`, `policy`,
/// `provenance` or `statement`.
pub fn schema(value: &Value, name: &str) -> Result<()> {
    let raw = match name {
        "events" => include_str!("../../schemas/change-events.schema.json"),
        "manifest" => include_str!("../../schemas/manifest.schema.json"),
        "policy" => include_str!("../../schemas/policy.schema.json"),
        "provenance" => include_str!("../../schemas/provenance.schema.json"),
        "statement" => include_str!("../../schemas/statement.schema.json"),
        _ => bail!("unknown schema"),
    };
    let definition: Value = serde_json::from_str(raw)?;
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&definition)?;
    if let Some(error) = validator.iter_errors(value).next() {
        // Do not echo untrusted input values (which may contain secrets).
        bail!("schema validation failed at {}", error.instance_path);
    }
    Ok(())
}

/// Parse an RFC 3339 timestamp with any offset into a UTC instant.
pub fn timestamp(s: &str) -> Result<Timestamp> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .context("ACV004 invalid timestamp")
}

/// Sort and de-duplicate an evidence list, and check that attestation evidence resolves: a
/// `attestation:*` reference must name a recorded attestation, and `signed` evidence must come
/// from one that carried a signature and names its verifier.
fn evidence(v: &mut Vec<Evidence>, attestations: &BTreeMap<String, Attestation>) -> Result<()> {
    v.sort();
    v.dedup();
    for ev in v.iter() {
        let from_attestation = ev.source == crate::provenance::attestations::SOURCE;
        ensure!(
            ev.kind != EvidenceKind::Signed || from_attestation,
            "ACV003 signed evidence must come from an attestation"
        );
        if from_attestation {
            let record = attestations
                .get(&ev.r#ref)
                .ok_or_else(|| anyhow::anyhow!("ACV003 unresolved attestation"))?;
            ensure!(
                ev.kind != EvidenceKind::Signed || (record.signed && record.verified_by.is_some()),
                "ACV003 signed evidence without a verified attestation"
            );
        }
    }
    Ok(())
}

/// Validate an export and put it into canonical order.
pub fn events(mut e: Events) -> Result<Events> {
    schema(&serde_json::to_value(&e)?, "events")?;
    ensure!(e.window.from <= e.window.to, "ACV004 window is reversed");
    ensure!(
        e.window.complete == e.window.reason.is_none(),
        "window completeness and reason disagree"
    );
    for id in e.actors.keys() {
        ensure!(
            id.split_once(':')
                .is_some_and(|(namespace, name)| !namespace.is_empty() && !name.is_empty())
                && !id.chars().any(|c| c.is_whitespace() || c.is_control()),
            "ACV003 malformed actor ID"
        );
    }
    for (id, a) in &e.attestations {
        ensure!(
            id.strip_prefix("attestation:")
                .is_some_and(|hex| hex.len() == 16 && hex.chars().all(|c| c.is_ascii_hexdigit())),
            "ACV003 malformed attestation ID"
        );
        ensure!(
            a.payload_digest.starts_with("sha256:") && a.payload_digest[7..].starts_with(&id[12..]),
            "ACV003 attestation ID does not match its payload digest"
        );
        ensure!(
            a.verified_by
                .as_deref()
                .is_none_or(|v| !v.trim().is_empty()),
            "ACV003 empty attestation verifier"
        );
    }
    let attestations = e.attestations.clone();
    let mut ids = BTreeSet::new();
    for c in &mut e.changes {
        ensure!(ids.insert(c.id.clone()), "ACV004 duplicate change ID");
        ensure!(c.repository == e.repository, "ACV004 repository mismatch");
        ensure!(
            c.merged_at.is_none_or(|m| m >= c.opened_at),
            "ACV004 merge before opening"
        );
        ensure!(
            c.merged_at.is_some() == c.merger.is_some(),
            "ACV004 merge actor/time mismatch"
        );
        ensure!(
            c.merged_at.is_some() || c.merge_commit_sha.is_none(),
            "ACV004 merge commit on unmerged change"
        );
        let mut identities = vec![&mut c.author, &mut c.forge_author];
        if let Some(m) = &mut c.merger {
            identities.push(m);
        }
        for commit in &mut c.commits {
            if let Some(a) = &mut commit.author {
                identities.push(a);
            }
        }
        for a in identities {
            ensure!(
                e.actors.contains_key(&a.actor_id),
                "ACV003 unresolved actor"
            );
            evidence(&mut a.provenance, &attestations)?;
        }
        if let Some(op) = &mut c.agent_operator {
            ensure!(
                e.actors[&c.author.actor_id].kind == ActorKind::Agent,
                "ACV003 operator on non-agent change"
            );
            ensure!(
                (op.confidence == Confidence::Unknown) == op.actor_id.is_none(),
                "ACV003 inconsistent operator confidence"
            );
            if let Some(id) = &op.actor_id {
                ensure!(
                    e.actors.get(id).is_some_and(|a| a.kind == ActorKind::Human),
                    "ACV003 operator must resolve to a human"
                );
            }
            evidence(&mut op.provenance, &attestations)?;
        }
        let mut review_ids = BTreeSet::new();
        for r in &mut c.reviews {
            ensure!(
                review_ids.insert(r.id.clone()),
                "ACV004 duplicate review ID"
            );
            ensure!(
                e.actors.contains_key(&r.actor_id),
                "ACV003 unresolved review actor"
            );
            ensure!(r.at >= c.opened_at, "ACV004 review before opening");
            ensure!(
                r.state != ReviewState::Approved || c.merged_at.is_none_or(|m| r.at <= m),
                "ACV001 approval_after_merge"
            );
            evidence(&mut r.provenance, &attestations)?;
        }
        c.reviews.sort_by(|a, b| (a.at, &a.id).cmp(&(b.at, &b.id)));
        // Conflicting decisions with identical timestamps cannot be chronologically resolved.
        let mut decisions = BTreeMap::new();
        for r in &c.reviews {
            if r.state == ReviewState::Commented {
                continue;
            }
            if let Some(other) = decisions.insert((&r.actor_id, r.at), r) {
                ensure!(
                    r.state == other.state && r.commit_sha == other.commit_sha,
                    "ACV004 ambiguous simultaneous reviews"
                );
            }
        }
        c.commits.sort_by(|a, b| a.sha.cmp(&b.sha));
        ensure!(
            c.commits.windows(2).all(|w| w[0].sha != w[1].sha),
            "ACV004 duplicate commit"
        );
        evidence(&mut c.provenance, &attestations)?;
    }
    e.changes.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(e)
}

/// Canonical JSON: sorted object keys, compact separators, UTF-8, one final LF.
///
/// This canonicalization is project-specific, not an RFC 8785 claim.
pub fn canonical<T: serde::Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(&serde_json::to_value(value)?)? + "\n")
}

/// `sha256:<hex>` over [`canonical`] bytes.
pub fn digest<T: serde::Serialize>(value: &T) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!(
        "sha256:{:x}",
        Sha256::digest(canonical(value)?.as_bytes())
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_sorts_keys_and_ends_with_newline() {
        let v = serde_json::json!({"b": 1, "a": {"z": true, "y": null}});
        assert_eq!(
            canonical(&v).unwrap(),
            "{\"a\":{\"y\":null,\"z\":true},\"b\":1}\n"
        );
    }

    #[test]
    fn digest_is_prefixed_and_stable() {
        let d = digest(&serde_json::json!(["acme/api", "x"])).unwrap();
        assert!(d.starts_with("sha256:"));
        assert_eq!(d.len(), "sha256:".len() + 64);
        assert_eq!(d, digest(&serde_json::json!(["acme/api", "x"])).unwrap());
    }

    #[test]
    fn timestamps_accept_offsets_and_reject_garbage() {
        let t = timestamp("2026-08-01T14:00:00+02:00").unwrap();
        assert_eq!(t.to_rfc3339(), "2026-08-01T12:00:00+00:00");
        assert!(timestamp("2026-08-01").is_err());
    }

    #[test]
    fn unknown_schema_names_are_rejected() {
        assert!(schema(&serde_json::json!({}), "nope").is_err());
    }
}
