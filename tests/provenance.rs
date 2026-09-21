use agent_change_control::model::{Policy, Severity};
use agent_change_control::{normalize, provenance};
use serde_json::json;

#[test]
fn only_explicit_blocks_count() {
    assert!(
        provenance::declaration("Claude wrote this code; Agent-Author: codex")
            .unwrap()
            .is_none()
    );
    let (agent, operator) =
        provenance::declaration("```agent-change-control\nauthor: codex\noperator: alice\n```")
            .unwrap()
            .unwrap();
    assert_eq!(agent, "codex");
    assert_eq!(operator.as_deref(), Some("alice"));
    assert!(
        provenance::declaration("```agent-change-control\nauthor: codex\nunknown: true\n```")
            .is_err()
    );
    assert!(
        provenance::declaration(
            "```agent-change-control\nauthor: codex\n```\n```agent-change-control\nauthor: claude-code\n```"
        )
        .is_err()
    );
}

#[test]
fn convention_is_schema_validated_and_head_bound() {
    let v = json!({
        "spec_version": "0.1",
        "agent": {"name": "codex", "version": null},
        "operator": {"id": "alice@example.com"},
        "session": {"id": "sha256:test", "started_at": "2026-08-01T00:00:00Z"},
        "change": {"base_commit": "base", "head_commit": "head"}
    });
    normalize::schema(&v, "provenance").unwrap();
    assert!(provenance::convention(&v, "head").is_ok());
    assert!(provenance::convention(&v, "other").is_err());
}

#[test]
fn policy_default_threshold_and_unknown_rules() {
    let v = json!({"version": "0.1", "rules": {"approver_is_author": {"enabled": false, "severity": "warning"}}});
    normalize::schema(&v, "policy").unwrap();
    let policy: Policy = serde_json::from_value(v).unwrap();
    let policy = agent_change_control::policy::resolve(policy).unwrap();
    assert_eq!(policy.fail_on, Severity::Medium);
    assert_eq!(policy.rules.len(), 8);
    let v = json!({"version": "0.1", "rules": {"typo": {"enabled": false, "severity": "warning"}}});
    assert!(normalize::schema(&v, "policy").is_err());
}

/// The shipped examples are producer-facing documentation; they must stay valid.
#[test]
fn shipped_examples_are_valid() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let body = std::fs::read_to_string(root.join("examples/pull-request-body.md")).unwrap();
    let (agent, operator) = provenance::declaration(&body).unwrap().unwrap();
    assert_eq!(agent, "claude-code");
    assert_eq!(operator.as_deref(), Some("alice"));
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("examples/provenance.json")).unwrap(),
    )
    .unwrap();
    let head = v["change"]["head_commit"].as_str().unwrap().to_string();
    let (agent, operator) = provenance::convention(&v, &head).unwrap();
    assert_eq!(agent, "codex");
    assert_eq!(operator.as_deref(), Some("github:alice"));
    // The same document as a signed attestation's predicate: bound by subject and predicate.
    let statement: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("examples/provenance.intoto.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        statement["predicateType"],
        agent_change_control::provenance::attestations::PROVENANCE_PREDICATE_TYPE
    );
    assert_eq!(statement["subject"][0]["digest"]["gitCommit"], head);
    normalize::schema(&statement, "statement").unwrap();
    assert_eq!(statement["predicate"], v);
    let attestations = agent_change_control::provenance::attestations::Attestations::load(
        &[root.join("examples/provenance.intoto.json")],
        None,
    )
    .unwrap();
    let auth = attestations.authorship(&head).unwrap().unwrap();
    assert_eq!(auth.agent, "codex");
    assert_eq!(auth.operator.as_deref(), Some("github:alice"));
    // The review document example binds to its head and names an agent reviewer.
    let review: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("examples/review.intoto.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        review["predicateType"],
        agent_change_control::provenance::attestations::REVIEW_PREDICATE_TYPE
    );
    normalize::schema(&review, "statement").unwrap();
    normalize::schema(&review["predicate"], "review").unwrap();
    let review_head = review["subject"][0]["digest"]["gitCommit"]
        .as_str()
        .unwrap()
        .to_string();
    let attestations = agent_change_control::provenance::attestations::Attestations::load(
        &[root.join("examples/review.intoto.json")],
        None,
    )
    .unwrap();
    let claims = attestations.reviews(&review_head).unwrap();
    assert_eq!(claims.len(), 1);
    assert_eq!(claims[0].agent.as_deref(), Some("claude-code-review"));
    assert_eq!(claims[0].operator.as_deref(), Some("github:carol"));
    let policy: Policy =
        serde_saphyr::from_str(&std::fs::read_to_string(root.join("examples/policy.yml")).unwrap())
            .unwrap();
    normalize::schema(&serde_json::to_value(&policy).unwrap(), "policy").unwrap();
    // The pull request template must not itself be parsed as a declaration.
    let template = std::fs::read_to_string(root.join(".github/PULL_REQUEST_TEMPLATE.md")).unwrap();
    assert!(provenance::declaration(&template).unwrap().is_none());
}
