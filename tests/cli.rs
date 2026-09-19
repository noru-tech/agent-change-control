mod common;

use agent_change_control::manifest;
use agent_change_control::model::{Disposition, DispositionStatus, Manifest};
use common::{acc, fixture, stdout};
use predicates::prelude::*;

#[test]
fn offline_workflow_and_exit_codes() {
    for (name, code) in [
        ("human-clean", 0),
        ("human-self-approved", 1),
        ("agent-operator-unknown", 0),
        ("incomplete-window", 4),
    ] {
        let path = fixture(name, "expected-manifest.json");
        acc()
            .arg("validate")
            .arg(&path)
            .assert()
            .success()
            .stderr(predicate::str::contains("Valid manifest"));
        acc()
            .args(["-q", "validate"])
            .arg(&path)
            .assert()
            .success()
            .stderr("");
        acc().arg("check").arg(&path).assert().code(code);
    }
    acc()
        .arg("evaluate")
        .arg(fixture("approval-after-merge", "events.json"))
        .assert()
        .code(3)
        .stderr(predicate::str::contains("ACV001"));
    acc().arg("unknown-command").assert().code(2);
    acc()
        .arg("evaluate")
        .arg(fixture("does-not-exist", "events.json"))
        .assert()
        .code(3);
    acc()
        .args(["check", "--as-of", "yesterday"])
        .arg(fixture("human-clean", "expected-manifest.json"))
        .assert()
        .code(2);
}

#[test]
fn formats_roundtrip() {
    let events = fixture("claude-operator-self-approved", "events.json");
    for format in ["json", "yaml"] {
        let out = stdout(acc().args(["evaluate", "-f", format]).arg(&events));
        let m: Manifest = if format == "json" {
            serde_json::from_str(&out).unwrap()
        } else {
            serde_saphyr::from_str(&out).unwrap()
        };
        manifest::validate(&m).unwrap();
    }
    let out = stdout(acc().args(["evaluate", "--format", "sarif"]).arg(&events));
    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["version"], "2.1.0");
    assert_eq!(value["runs"][0]["results"].as_array().unwrap().len(), 3);
    // The attestation predicate is a complete manifest: it validates on its own.
    let out = stdout(acc().args(["evaluate", "--format", "in-toto"]).arg(&events));
    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["_type"], "https://in-toto.io/Statement/v1");
    assert_eq!(value["subject"][0]["digest"]["gitCommit"], "head");
    let predicate: Manifest = serde_json::from_value(value["predicate"].clone()).unwrap();
    manifest::validate(&predicate).unwrap();
    acc()
        .args(["check", "--format", "in-toto"])
        .arg(fixture("human-clean", "expected-manifest.json"))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"predicateType\""));
}

#[test]
fn output_extension_selects_the_format() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("nested").join("manifest.yml");
    acc()
        .args(["evaluate", "-o"])
        .arg(&out)
        .arg(fixture("human-clean", "events.json"))
        .assert()
        .success();
    let m: Manifest = serde_saphyr::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
    manifest::validate(&m).unwrap();
    acc().arg("validate").arg(&out).assert().success();
}

#[test]
fn table_sarif_and_attestation_snapshots() {
    for name in [
        "claude-operator-self-approved",
        "agent-operator-unknown",
        "incomplete-window",
    ] {
        let events = fixture(name, "events.json");
        let table = acc()
            .args(["evaluate", "--format", "table"])
            .arg(&events)
            .output()
            .unwrap();
        insta::assert_snapshot!(
            format!("table_{name}"),
            String::from_utf8(table.stdout).unwrap()
        );
        // `evaluate` exits 4 for the incomplete fixture; the output is still rendered.
        let sarif = acc()
            .args(["evaluate", "--format", "sarif"])
            .arg(&events)
            .output()
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&sarif.stdout).unwrap();
        insta::assert_yaml_snapshot!(format!("sarif_{name}"), value);
        let statement = acc()
            .args(["evaluate", "--format", "in-toto"])
            .arg(&events)
            .output()
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&statement.stdout).unwrap();
        insta::assert_yaml_snapshot!(format!("intoto_{name}"), value);
    }
}

#[test]
fn dispositions_require_as_of_and_expire() {
    let raw = std::fs::read_to_string(fixture(
        "claude-operator-self-approved",
        "expected-manifest.json",
    ))
    .unwrap();
    let mut m: Manifest = serde_json::from_str(&raw).unwrap();
    for f in &mut m.findings {
        f.disposition = Disposition {
            status: DispositionStatus::Accepted,
            owner: Some("security@example.com".into()),
            decided_at: "2026-09-01".parse().ok(),
            expires_at: "2026-09-30".parse().ok(),
            rationale: Some("Emergency patch; retrospective review completed.".into()),
            remediated_at: None,
        };
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("manifest.json");
    std::fs::write(&path, serde_json::to_string(&m).unwrap()).unwrap();
    acc().arg("validate").arg(&path).assert().success();
    acc()
        .arg("check")
        .arg(&path)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--as-of"));
    acc()
        .args(["check", "--as-of", "2026-09-18"])
        .arg(&path)
        .assert()
        .code(0)
        .stdout(predicate::str::contains("FAIL"));
    acc()
        .args(["check", "--as-of", "2026-10-01"])
        .arg(&path)
        .assert()
        .code(1);
}

#[test]
fn invalid_inputs_rejected_without_panicking() {
    acc()
        .args([
            "scan",
            "github",
            "../evil",
            "--since",
            "2026-08-01",
            "--until",
            "2026-08-31",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("OWNER/REPO"));
    acc()
        .args([
            "export",
            "github",
            "acme/api",
            "--since",
            "2026-08-31",
            "--until",
            "2026-08-01",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("reversed"));
    acc()
        .args(["pr", "1"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--repo"));
}

#[test]
fn completions_and_man_pages_render() {
    acc()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("acc"));
    acc()
        .arg("manpage")
        .assert()
        .success()
        .stdout(predicate::str::contains(".TH"));
    let dir = tempfile::tempdir().unwrap();
    acc()
        .args(["manpage", "--out-dir"])
        .arg(dir.path())
        .assert()
        .success();
    assert!(dir.path().join("acc-check.1").exists());
}
