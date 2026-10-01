mod common;

use agent_change_control::manifest;
use agent_change_control::model::{Disposition, DispositionStatus, Manifest};
use common::{acc, fixture, fixtures, stdout};
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
        .stderr(predicate::str::contains(
            "ACV001 approval_after_merge (see https://github.com/noru-tech/agent-change-control/blob/main/docs/rules/ACV001.md)",
        ));
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
    let digest = agent_change_control::canonical::digest(&m).unwrap();
    assert_eq!(
        format!("sha256:{}", common::sha256_hex(&bytes)),
        digest,
        "the file's digest is the canonical digest"
    );
    let attested = serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": "acc-manifest.json",
            "digest": {"sha256": digest.trim_start_matches("sha256:")}
        }],
        "predicateType": "https://noru.tech/spec/ai-change-provenance/v0.3",
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
        .stderr(predicate::str::contains("--as-of"))
        .stderr(predicate::str::contains("help: pass --as-of YYYY-MM-DD"))
        .stderr(predicate::str::contains(
            "see: https://github.com/noru-tech/agent-change-control/blob/main/docs/exit-codes.md#2-invalid-command-line-arguments",
        ));
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

#[test]
fn legacy_0_2_documents_validate_with_a_note() {
    for file in [
        "manifest.json",
        "statement.intoto.json",
        "statements.intoto.jsonl",
    ] {
        acc()
            .arg("validate")
            .arg(fixture("legacy-0.2", file))
            .assert()
            .success()
            .stderr(predicate::str::contains("legacy canonicalization"));
    }
    // The dogfood shape acc 0.4.0 produced: actions/attest hashed the manifest file, whose
    // legacy bytes end in a newline, and the digest is recomputed from the predicate that way.
    let bytes = std::fs::read(fixture("legacy-0.2", "manifest.json")).unwrap();
    let m: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let attested = serde_json::json!({
        "_type": "https://in-toto.io/Statement/v1",
        "subject": [{
            "name": "acc-manifest.json",
            "digest": {"sha256": common::sha256_hex(&bytes)}
        }],
        "predicateType": "https://noru.tech/spec/ai-change-provenance/v0.2",
        "predicate": m,
    });
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("attested.intoto.json");
    std::fs::write(&path, serde_json::to_string(&attested).unwrap()).unwrap();
    acc()
        .arg("validate")
        .arg(&path)
        .assert()
        .success()
        .stderr(predicate::str::contains("legacy canonicalization"));
    // Current output does not carry the note.
    acc()
        .arg("validate")
        .arg(fixture("human-clean", "expected-manifest.json"))
        .assert()
        .success()
        .stderr(predicate::str::contains("legacy").not());
}

/// Spec §8.2: ACP documents are I-JSON with integer-only numbers in ±(2^53 − 1), no unpaired
/// surrogates, unique member names and nesting of at most 128. Each fixture differs from
/// human-clean/events.json by one violation, and both commands reject it with its code.
#[test]
fn i_json_violations_are_rejected_with_their_codes() {
    for (file, code) in [
        ("non-integer.json", "ACV005"),
        ("integer-range.json", "ACV006"),
        ("unpaired-surrogate.json", "ACV007"),
        ("duplicate-member.json", "ACV008"),
        ("depth-129.json", "ACV009"),
    ] {
        for command in ["evaluate", "validate"] {
            acc()
                .arg(command)
                .arg(fixture("ijson", file))
                .assert()
                .code(3)
                .stderr(predicate::str::contains(code));
        }
    }
    let dir = tempfile::tempdir().unwrap();
    // One level less is within the bound: the document is then refused by the schema instead.
    let text = std::fs::read_to_string(fixture("ijson", "depth-129.json"))
        .unwrap()
        .replacen("[]", "", 1);
    let path = dir.path().join("depth-128.json");
    std::fs::write(&path, text).unwrap();
    acc()
        .arg("evaluate")
        .arg(&path)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("schema validation failed"));
    // YAML is a presentation of the same data model and is held to the same numbers.
    let path = dir.path().join("float.yml");
    std::fs::write(&path, "version: '0.3'\nfail_on: 1.5\nrules: {}\n").unwrap();
    acc()
        .args(["evaluate", "--policy"])
        .arg(&path)
        .arg(fixture("human-clean", "events.json"))
        .assert()
        .code(3)
        .stderr(predicate::str::contains("ACV005"));
    // A manifest's integers are held to it as well.
    let manifest = std::fs::read_to_string(fixture("human-clean", "expected-manifest.json"))
        .unwrap()
        .replacen("\"changes\":1,", "\"changes\":1.0,", 1);
    assert!(manifest.contains("1.0"));
    let path = dir.path().join("float-manifest.json");
    std::fs::write(&path, manifest).unwrap();
    acc()
        .arg("validate")
        .arg(&path)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("ACV005"));
}

/// `evaluate --conformance-json`: the corpus contract. One JSON line on stdout, the verdict in
/// the exit status, and invalid input reported as a result with its codes.
#[test]
fn conformance_json_prints_one_result_line() {
    let run = |vector: &str| {
        let out = acc()
            .args(["evaluate", "--conformance-json"])
            .arg(common::root().join("conformance").join(vector))
            .output()
            .unwrap();
        let stdout = String::from_utf8(out.stdout).unwrap();
        assert_eq!(stdout.lines().count(), 1, "{vector}: {stdout}");
        let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        (out.status.code().unwrap(), v)
    };
    let (code, v) = run("accept/human-self-approved.json");
    assert_eq!(code, 0);
    assert_eq!(v["verdict"], "evaluated");
    assert_eq!(v["codes"], serde_json::json!(["ACC002", "ACC003"]));
    assert_eq!(v["assessments"].as_array().unwrap().len(), 8);
    assert!(v["manifestDigest"].as_str().unwrap().starts_with("sha256:"));
    let (code, v) = run("incomplete/incomplete-window.json");
    assert_eq!((code, v["verdict"].as_str()), (4, Some("incomplete")));
    let (code, v) = run("reject/human-clean--depth-129.json");
    assert_eq!((code, v["verdict"].as_str()), (3, Some("invalid")));
    assert_eq!(v["codes"], serde_json::json!(["ACV009"]));
    assert!(v.get("assessments").is_none());
    let (code, v) = run("reject/human-clean--unknown-member.json");
    assert_eq!(
        (code, v["codes"].clone()),
        (3, serde_json::json!(["ACV010"]))
    );
    // A plain event export is not a vector.
    let out = acc()
        .args(["evaluate", "--conformance-json"])
        .arg(fixture("human-clean", "events.json"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&out.stdout).contains("ACV010"));
    // The vector carries its policy; --policy and output flags do not combine with it.
    acc()
        .args(["evaluate", "--conformance-json", "--policy", "x.yml"])
        .arg(fixture("human-clean", "events.json"))
        .assert()
        .code(2);
}

/// stdout of a run that may exit non-zero, with its exit code.
fn run(cmd: &mut assert_cmd::Command) -> (Vec<u8>, i32) {
    let out = cmd.output().expect("run acc");
    (out.stdout, out.status.code().expect("exit code"))
}

#[test]
fn text_is_an_alias_of_table_with_identical_bytes() {
    let events = fixture("claude-operator-self-approved", "events.json");
    let manifest = fixture("human-self-approved", "expected-manifest.json");
    let table = run(acc().args(["evaluate", "-f", "table"]).arg(&events));
    assert_eq!(
        run(acc().args(["evaluate", "--format", "text"]).arg(&events)),
        table
    );
    assert!(!table.0.is_empty());
    let table = run(acc().args(["check", "-f", "table"]).arg(&manifest));
    assert_eq!(table.1, 1);
    assert_eq!(
        run(acc().args(["check", "-f", "text"]).arg(&manifest)),
        table
    );
}

#[test]
fn verbose_and_no_color_never_change_stdout() {
    let events = fixture("claude-operator-self-approved", "events.json");
    let manifest = fixture("human-self-approved", "expected-manifest.json");
    let cases: Vec<Vec<std::ffi::OsString>> = vec![
        vec!["evaluate".into(), events.clone().into()],
        vec![
            "evaluate".into(),
            "-f".into(),
            "table".into(),
            events.clone().into(),
        ],
        vec![
            "evaluate".into(),
            "-f".into(),
            "sarif".into(),
            events.clone().into(),
        ],
        vec![
            "evaluate".into(),
            "-f".into(),
            "in-toto".into(),
            events.into(),
        ],
        vec!["check".into(), manifest.clone().into()],
        vec![
            "validate".into(),
            "-f".into(),
            "json".into(),
            manifest.into(),
        ],
    ];
    for args in cases {
        let plain = run(acc().args(&args));
        assert_eq!(run(acc().arg("-v").args(&args)), plain, "{args:?}");
        assert_eq!(run(acc().arg("--no-color").args(&args)), plain, "{args:?}");
        assert_eq!(
            run(acc().env("NO_COLOR", "1").args(&args)),
            plain,
            "{args:?}"
        );
    }
    acc()
        .args(["--verbose", "evaluate"])
        .arg(fixture("claude-clean", "events.json"))
        .assert()
        .success()
        .stderr(predicate::str::contains("acc: policy: built-in defaults"))
        .stderr(predicate::str::contains("writing json to stdout"));
    acc().args(["-q", "-v", "validate", "x"]).assert().code(2);
    // Help and usage errors, the only output clap could color, stay plain.
    for cmd in [
        acc().args(["--no-color", "--help"]).assert().success(),
        acc().env("NO_COLOR", "1").arg("--nope").assert().code(2),
        acc()
            .args(["evaluate", "--no-color", "--format", "nope", "x"])
            .assert()
            .code(2),
    ] {
        let out = cmd.get_output();
        assert!(!out.stdout.contains(&0x1b) && !out.stderr.contains(&0x1b));
    }
}

#[test]
fn validate_reports_a_result_object_in_json() {
    let clean = fixture("human-clean", "expected-manifest.json");
    acc()
        .args(["validate", "--format", "json"])
        .arg(&clean)
        .assert()
        .success()
        .stdout("{\"message\":\"Valid manifest\",\"valid\":true}\n")
        .stderr(predicate::str::contains("Valid manifest"));
    // Text keeps stdout empty, as before.
    acc()
        .args(["validate", "--format", "text"])
        .arg(&clean)
        .assert()
        .success()
        .stdout("");
    let out = acc()
        .args(["validate", "-f", "json"])
        .arg(fixtures().join("ijson/duplicate-member.json"))
        .assert()
        .code(3)
        .get_output()
        .clone();
    let object: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(object["valid"], false);
    let code = object["code"].as_str().unwrap();
    assert!(code.starts_with("ACV"), "{object}");
    assert_eq!(
        object["help_uri"],
        format!("https://github.com/noru-tech/agent-change-control/blob/main/docs/rules/{code}.md")
    );
    let out = acc()
        .args(["validate", "-f", "json", "missing.json"])
        .assert()
        .code(3)
        .get_output()
        .clone();
    let object: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(object["valid"], false);
    assert!(object.get("code").is_none() && object.get("help_uri").is_none());

    let dir = tempfile::tempdir().unwrap();
    let json = dir.path().join("result.json");
    acc()
        .args(["validate", "-o"])
        .arg(&json)
        .arg(&clean)
        .assert()
        .success()
        .stdout("");
    assert_eq!(
        std::fs::read_to_string(&json).unwrap(),
        "{\"message\":\"Valid manifest\",\"valid\":true}\n"
    );
    let text = dir.path().join("result.txt");
    acc()
        .args(["validate", "--output"])
        .arg(&text)
        .arg(&clean)
        .assert()
        .success();
    assert_eq!(std::fs::read_to_string(&text).unwrap(), "Valid manifest\n");
}

#[test]
fn completions_and_man_pages_write_to_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("completions/acc.bash");
    acc()
        .args(["completions", "bash", "--output"])
        .arg(&script)
        .assert()
        .success()
        .stdout("");
    let stdout = stdout(acc().args(["completions", "bash"]));
    assert_eq!(std::fs::read_to_string(&script).unwrap(), stdout);
    let page = dir.path().join("acc.1");
    acc().args(["manpage", "-o"]).arg(&page).assert().success();
    assert_eq!(std::fs::read_to_string(&page).unwrap(), stdout_of_manpage());
    acc()
        .args(["manpage", "-o", "x.1", "--out-dir"])
        .arg(dir.path())
        .assert()
        .code(2);
}

fn stdout_of_manpage() -> String {
    stdout(acc().arg("manpage"))
}

/// stderr of a run that must exit with `code`.
fn stderr_of(cmd: &mut assert_cmd::Command, code: i32) -> String {
    let out = cmd.output().expect("run acc");
    assert_eq!(
        out.status.code(),
        Some(code),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stderr).unwrap()
}

#[test]
fn usage_and_input_failures_print_a_hint_and_a_docs_link() {
    let docs = "https://github.com/noru-tech/agent-change-control/blob/main/docs";
    let usage = format!("see: {docs}/exit-codes.md#2-invalid-command-line-arguments");
    let clean = fixture("human-clean", "expected-manifest.json");
    let cases: Vec<(Vec<std::ffi::OsString>, i32, &str, String)> = vec![
        (
            vec!["pr".into(), "421".into()],
            2,
            "help: pass --repo OWNER/REPO, set GITHUB_REPOSITORY",
            usage.clone(),
        ),
        (
            vec![
                "scan".into(),
                "github".into(),
                "acme/api".into(),
                "--since".into(),
                "2026-08-32".into(),
                "--until".into(),
                "2026-09-01".into(),
            ],
            2,
            "help: use YYYY-MM-DD",
            usage.clone(),
        ),
        (
            vec![
                "scan".into(),
                "github".into(),
                "acme/api".into(),
                "--since".into(),
                "2026-09-01".into(),
                "--until".into(),
                "2026-08-01".into(),
            ],
            2,
            "help: --since must not be later than --until",
            usage.clone(),
        ),
        (
            vec![
                "check".into(),
                "--as-of".into(),
                "yesterday".into(),
                clean.clone().into(),
            ],
            2,
            "help: use a calendar date such as --as-of 2026-09-30",
            usage.clone(),
        ),
        (
            vec![
                "evaluate".into(),
                "--policy".into(),
                "Cargo.toml".into(),
                fixture("human-clean", "events.json").into(),
            ],
            3,
            "help: fix the file against schemas/policy.schema.json",
            format!("see: {docs}/policy.md"),
        ),
        (
            vec![
                "check".into(),
                "--policy".into(),
                "missing-policy.yml".into(),
                clean.into(),
            ],
            3,
            "help: fix the file against schemas/policy.schema.json",
            format!("see: {docs}/policy.md"),
        ),
    ];
    for (args, code, help, see) in cases {
        let stderr = stderr_of(acc().args(&args), code);
        let lines: Vec<&str> = stderr.lines().collect();
        assert!(lines[0].starts_with("error: "), "{args:?}: {stderr}");
        assert!(lines[1].starts_with(help), "{args:?}: {stderr}");
        assert_eq!(lines[2], see, "{args:?}");
    }
}
