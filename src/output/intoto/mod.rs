//! in-toto Statement v1 output: one unsigned attestation whose subjects are the evaluated
//! changes and whose predicate is the manifest itself.
//!
//! The statement is deliberately unsigned. Signing is a deployment decision: wrap the canonical
//! bytes in a DSSE envelope with the signer your organization already trusts (Sigstore, an HSM,
//! a KMS key). A verifier unwraps the envelope, checks the subject digests against the commits it
//! cares about, and runs `acc validate` on the predicate to confirm the findings follow from the
//! embedded facts. See `spec/ai-change-provenance.md`.

use crate::model::Manifest;
use anyhow::{Result, ensure};
use serde_json::{Value, json};

/// The in-toto Statement layer this renderer emits.
pub const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";
/// The predicate type: an AI Change Provenance manifest, versioned with the specification.
pub const PREDICATE_TYPE: &str = "https://noru.tech/spec/ai-change-provenance/v0.1";

pub fn render(m: &Manifest) -> Result<String> {
    ensure!(
        !m.events.changes.is_empty(),
        "an attestation needs at least one change as its subject"
    );
    let subject: Vec<Value> = m
        .events
        .changes
        .iter()
        .map(|c| {
            json!({
                "name": c.id,
                "digest": {"gitCommit": c.head_sha},
            })
        })
        .collect();
    let statement = json!({
        "_type": STATEMENT_TYPE,
        "subject": subject,
        "predicateType": PREDICATE_TYPE,
        "predicate": m,
    });
    crate::normalize::canonical(&statement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn manifest(changes: Vec<Change>) -> Manifest {
        let mut actors = std::collections::BTreeMap::new();
        actors.insert(
            "github:alice".to_string(),
            Actor {
                kind: ActorKind::Human,
                display_name: None,
            },
        );
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
            changes,
        };
        crate::manifest::evaluate(events, Policy::default()).unwrap()
    }

    #[test]
    fn empty_windows_have_no_subject_and_are_refused() {
        let err = render(&manifest(vec![])).unwrap_err();
        assert!(err.to_string().contains("subject"));
    }

    #[test]
    fn subjects_are_head_commits_and_the_predicate_is_the_manifest() {
        let evidence = Evidence {
            source: "github_api".into(),
            r#ref: "https://api.github.com/repos/acme/api/pulls/1".into(),
            kind: EvidenceKind::Observed,
        };
        let alice = Identity {
            actor_id: "github:alice".into(),
            provenance: vec![evidence.clone()],
        };
        let change = Change {
            id: "github:acme/api:pr:1".into(),
            repository: "acme/api".into(),
            forge: Forge::Github,
            title: "t".into(),
            url: "https://github.com/acme/api/pull/1".into(),
            opened_at: crate::normalize::timestamp("2026-08-02T00:00:00Z").unwrap(),
            merged_at: None,
            head_sha: "0123abcd".into(),
            commits: vec![],
            forge_author: alice.clone(),
            author: alice,
            agent_operator: None,
            reviews: vec![],
            reviews_complete: true,
            merger: None,
            provenance: vec![evidence],
        };
        let m = manifest(vec![change]);
        let out = render(&m).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["_type"], STATEMENT_TYPE);
        assert_eq!(v["predicateType"], PREDICATE_TYPE);
        assert_eq!(v["subject"][0]["name"], "github:acme/api:pr:1");
        assert_eq!(v["subject"][0]["digest"]["gitCommit"], "0123abcd");
        let predicate: Manifest = serde_json::from_value(v["predicate"].clone()).unwrap();
        crate::manifest::validate(&predicate).unwrap();
        assert_eq!(render(&m).unwrap(), out, "byte-stable");
    }
}
