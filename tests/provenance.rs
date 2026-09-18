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
    assert_eq!(policy.rules.len(), 4);
    let v = json!({"version": "0.1", "rules": {"typo": {"enabled": false, "severity": "warning"}}});
    assert!(normalize::schema(&v, "policy").is_err());
}
