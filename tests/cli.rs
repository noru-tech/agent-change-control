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
fn attestations_validate_and_are_inferred_from_their_suffix() {
    let dir = tempfile::tempdir().unwrap();
    let events = fixture("claude-operator-self-approved", "events.json");
    // A merged change with a known merge commit: two subjects, and the Statement validates.
    let statement = dir.path().join("change-control.intoto.json");
    acc()
        .args(["evaluate", "-o"])
        .arg(&statement)
        .arg(&events)
        .assert()
        .success();
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&statement).unwrap()).unwrap();
    assert_eq!(value["_type"], "https://in-toto.io/Statement/v1");
    assert_eq!(value["subject"].as_array().unwrap().len(), 2);
    assert_eq!(value["subject"][0]["digest"]["gitCommit"], "head");
    assert_eq!(value["subject"][1]["name"], "github:acme/api:pr:421:merge");
    assert_eq!(value["subject"][1]["digest"]["gitCommit"], "merge");
    acc()
        .arg("validate")
        .arg(&statement)
        .assert()
        .success()
        .stderr(predicate::str::contains("Valid attestation"));
    // JSON Lines: one Statement per change, validated as a set.
    let lines = dir.path().join("change-control.intoto.jsonl");
    acc()
        .args(["evaluate", "-o"])
        .arg(&lines)
        .arg(&events)
        .assert()
        .success();
    let text = std::fs::read_to_string(&lines).unwrap();
    assert_eq!(text.lines().count(), 1);
    assert!(text.ends_with('\n'));
    // One line is indistinguishable from one Statement, and validates as one.
    acc()
        .arg("validate")
        .arg(&lines)
        .assert()
        .success()
        .stderr(predicate::str::contains("Valid attestation"));
    // Two lines out of order are rejected; so is a Statement whose subject was edited.
    let one: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    let mut other = one.clone();
    other["predicate"]["events"]["changes"][0]["id"] = "github:acme/api:pr:9".into();
    let unordered = dir.path().join("unordered.intoto.jsonl");
    std::fs::write(
        &unordered,
        format!("{}\n{}\n", one, serde_json::to_string(&other).unwrap()),
    )
    .unwrap();
    acc()
        .arg("validate")
        .arg(&unordered)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("line 2"));
    let mut tampered = one.clone();
    tampered["subject"][0]["digest"]["gitCommit"] = "other".into();
    let path = dir.path().join("tampered.intoto.json");
    std::fs::write(&path, serde_json::to_string(&tampered).unwrap()).unwrap();
    acc()
        .arg("validate")
        .arg(&path)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("subjects"));
    // The subject form GitHub artifact attestations produce: the sha256 of the manifest file,
    // which `--format json` writes in canonical form, so the digest is reproducible.
    let manifest_path = dir.path().join("acc-manifest.json");
    acc()
        .args(["evaluate", "-o"])
        .arg(&manifest_path)
        .arg(&events)
        .assert()
        .success();
    let bytes = std::fs::read(&manifest_path).unwrap();
    let m: Manifest = serde_json::from_slice(&bytes).unwrap();
    let digest = agent_change_control::normalize::digest(&m).unwrap();
    assert_eq!(
        format!(
            "sha256:{:x}",
            <sha2::Sha256 as sha2::Digest>::digest(&bytes)
        ),
        digest,
        "the file's digest is the canonical digest"
    );
    let attested = serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": "acc-manifest.json",
            "digest": {"sha256": digest.trim_start_matches("sha256:")}
        }],
        "predicateType": "https://noru.tech/spec/ai-change-provenance/v0.1",
        "predicate": m,
    });
    let path = dir.path().join("attested.intoto.json");
    std::fs::write(&path, serde_json::to_string(&attested).unwrap()).unwrap();
    acc()
        .arg("validate")
        .arg(&path)
        .assert()
        .success()
        .stderr(predicate::str::contains("Valid attestation"));
    // A merged change without a merge commit, and an open change, have a head subject only.
    for name in ["incomplete-window", "open-pr"] {
        let out = acc()
            .args(["evaluate", "--format", "in-toto"])
            .arg(fixture(name, "events.json"))
            .output()
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["subject"].as_array().unwrap().len(), 1, "{name}");
    }
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
    // A verifier statement without attestations to apply it to is a usage error.
    acc()
        .args(["pr", "1", "--repo", "acme/api", "--verified-by", "me"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("--attestations"));
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
