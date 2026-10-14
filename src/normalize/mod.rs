//! Schema validation, timeline integrity checks and canonical ordering (ACP §8.1). Turning the
//! normalized value into bytes is [`crate::canonical`]'s job (§8.2).
//!
//! Integrity errors carry `ACV` codes: ACV001 approval after merge, ACV003 invalid actor
//! relationships, ACV004 inconsistent timeline or duplicated/ambiguous events, ACV010 a document
//! that does not conform to its schema. ACV005 to ACV009 (I-JSON) are raised while parsing, in
//! [`crate::canonical::ijson`].

mod schema_order;

use crate::model::*;
use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Validate `value` against one of the embedded schemas: `events`, `manifest`, `policy`,
/// `provenance`, `statement` or `review`.
pub fn schema(value: &Value, name: &str) -> Result<()> {
    let raw = match name {
        "events" => include_str!("../../schemas/change-events.schema.json"),
        "manifest" => include_str!("../../schemas/manifest.schema.json"),
        "policy" => include_str!("../../schemas/policy.schema.json"),
        "provenance" => include_str!("../../schemas/provenance.schema.json"),
        "statement" => include_str!("../../schemas/statement.schema.json"),
        "review" => include_str!("../../schemas/review.schema.json"),
        _ => bail!("unknown schema"),
    };
    let definition: Value = serde_json::from_str(raw)?;
    let validator = jsonschema::options()
        .should_validate_formats(true)
        .build(&definition)?;
    // Report the same violation whatever order this `jsonschema` release evaluates in.
    let first = validator
        .iter_errors(value)
        .map(|error| {
            let at = error.instance_path().to_string();
            let key = schema_order::key(&definition, &error.evaluation_path().to_string(), &at);
            (key, at)
        })
        .reduce(|first, next| if next.0 < first.0 { next } else { first });
    if let Some((_, at)) = first {
        // Do not echo untrusted input values (which may contain secrets).
        bail!("ACV010 schema validation failed at {at}");
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
        "ACV004 window completeness and reason disagree"
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
    for a in e.attestations.values() {
        ensure!(
            a.signer
                .as_ref()
                .is_none_or(|s| !s.identity.trim().is_empty()),
            "ACV003 empty attestation signer"
        );
    }
    let attestations = e.attestations.clone();
    let well_formed = |id: &str| {
        id.split_once(':')
            .is_some_and(|(namespace, name)| !namespace.is_empty() && !name.is_empty())
            && !id.chars().any(|c| c.is_whitespace() || c.is_control())
    };
    let mut team_ids = BTreeSet::new();
    for team in &mut e.teams {
        ensure!(
            team.id.split('/').count() == 2
                && team.id.split('/').all(|s| !s.is_empty())
                && team.id == team.id.to_ascii_lowercase(),
            "ACV003 malformed team ID"
        );
        ensure!(team_ids.insert(team.id.clone()), "ACV004 duplicate team");
        ensure!(
            team.members.iter().all(|m| well_formed(m)),
            "ACV003 malformed team member ID"
        );
        team.members.sort();
        team.members.dedup();
        evidence(&mut team.provenance, &attestations)?;
    }
    e.teams.sort_by(|a, b| a.id.cmp(&b.id));
    let mut ids = BTreeSet::new();
    for c in &mut e.changes {
        if let Some(r) = &mut c.risk {
            ensure!(
                (r.tier == Tier::Emergency) == r.emergency.is_some(),
                "ACV004 emergency details disagree with the risk tier"
            );
            ensure!(
                r.emergency
                    .as_ref()
                    .is_none_or(|x| well_formed(&x.declared_by)),
                "ACV003 malformed emergency declarer ID"
            );
            ensure!(
                !r.provenance.is_empty(),
                "ACV003 risk tier without evidence"
            );
            evidence(&mut r.provenance, &attestations)?;
        }
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
        c.labels.sort();
        c.labels.dedup();
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
            if let Some(agent) = &r.agent {
                ensure!(
                    e.actors[&r.actor_id].kind == ActorKind::Agent,
                    "ACV003 agent review facts on a non-agent reviewer"
                );
                for id in [&agent.operator, &agent.instructions_owner]
                    .into_iter()
                    .flatten()
                {
                    ensure!(
                        e.actors.get(id).is_some_and(|a| a.kind == ActorKind::Human),
                        "ACV003 review agent operator or instructions owner must resolve to a human"
                    );
                }
            }
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
        if let Some(k) = &mut c.checks {
            ensure!(
                c.merged_at.is_some(),
                "ACV004 check facts on unmerged change"
            );
            for run in &mut k.runs {
                ensure!(
                    run.commit_sha == c.head_sha
                        || c.merge_commit_sha.as_deref() == Some(run.commit_sha.as_str()),
                    "ACV003 check result on a commit that is neither the head nor the merge commit"
                );
                ensure!(!run.name.trim().is_empty(), "ACV010 empty check name");
                evidence(&mut run.provenance, &attestations)?;
            }
            k.runs.sort_by(|a, b| {
                (
                    &a.name,
                    &a.commit_sha,
                    a.completed_at,
                    a.kind,
                    a.conclusion,
                    &a.provenance,
                )
                    .cmp(&(
                        &b.name,
                        &b.commit_sha,
                        b.completed_at,
                        b.kind,
                        b.conclusion,
                        &b.provenance,
                    ))
            });
            ensure!(
                k.runs.windows(2).all(|w| w[0] != w[1]),
                "ACV004 duplicate check result"
            );
            ensure!(
                k.required.iter().all(|n| !n.trim().is_empty()),
                "ACV010 empty check name"
            );
            k.required.sort();
            k.required.dedup();
            k.required_sources.sort();
            k.required_sources.dedup();
            evidence(&mut k.provenance, &attestations)?;
        }
        if let Some(b) = &mut c.protection {
            ensure!(
                c.merged_at.is_some(),
                "ACV004 protection facts on unmerged change"
            );
            let mut suite_ids = BTreeSet::new();
            for suite in &mut b.suites {
                ensure!(
                    c.merge_commit_sha.as_deref() == Some(suite.after_sha.as_str()),
                    "ACV003 ruleset evaluation of a push that is not the change's merge"
                );
                ensure!(
                    !suite.id.trim().is_empty(),
                    "ACV010 empty ruleset evaluation ID"
                );
                ensure!(
                    suite_ids.insert(suite.id.clone()),
                    "ACV004 duplicate ruleset evaluation"
                );
                evidence(&mut suite.provenance, &attestations)?;
            }
            b.suites
                .sort_by(|a, b| (a.pushed_at, &a.id).cmp(&(b.pushed_at, &b.id)));
            evidence(&mut b.provenance, &attestations)?;
        }
    }
    e.changes.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;

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
