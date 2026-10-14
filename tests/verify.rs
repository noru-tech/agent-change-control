//! `acc verify`: coverage of a real git range by signed verdicts.
//!
//! Every case builds a throwaway repository, evaluates a fixture whose merge commit is set to one
//! of its commits, and wraps the Statement the way `gh attestation verify --format json` reports a
//! verified bundle. acc does not check signatures, so the signature bytes are a placeholder; what
//! is under test is everything acc does with a verified bundle.

mod common;

use base64::Engine;
use common::{acc, fixture};
use predicates::prelude::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const SIGNER: &str = "https://github.com/acme/api/.github/workflows/attest.yml@refs/heads/main";
const ISSUER: &str = "https://token.actions.githubusercontent.com";

struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> Self {
        let repo = Self {
            dir: TempDir::new().unwrap(),
        };
        repo.git(&["init", "-q", "-b", "main"]);
        repo
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(self.path())
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.com",
            ])
            .args(args)
            .env_remove("GIT_DIR")
            .env("GIT_AUTHOR_DATE", "2026-08-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-08-01T00:00:00Z")
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    /// Commit on the current branch and return the new commit's SHA.
    fn commit(&self, message: &str) -> String {
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }
}

/// The in-toto Statement acc writes for `fixture`'s events, with its first change merged as
/// `merge_sha`.
fn statement(fixture_name: &str, merge_sha: &str, tmp: &Path) -> Vec<u8> {
    let mut events: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture(fixture_name, "events.json")).unwrap(),
    )
    .unwrap();
    events["changes"][0]["merge_commit_sha"] = json!(merge_sha);
    let input = tmp.join(format!("{fixture_name}-{merge_sha}.events.json"));
    std::fs::write(&input, serde_json::to_vec(&events).unwrap()).unwrap();
    let out = acc()
        .arg("evaluate")
        .arg(&input)
        .args(["--format", "in-toto"])
        .output()
        .unwrap();
    // 4 is an incomplete collection: the Statement is written all the same.
    assert!(
        matches!(out.status.code(), Some(0 | 4)),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

fn bundle(statement: &[u8]) -> Value {
    json!({
        "mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
        "verificationMaterial": {},
        "dsseEnvelope": {
            "payloadType": "application/vnd.in-toto+json",
            "payload": base64::engine::general_purpose::STANDARD.encode(statement),
            "signatures": [{"keyid": "", "sig": "c2lnbmF0dXJlLW5vdC1jaGVja2VkLWJ5LWFjYw=="}],
        },
    })
}

/// `gh attestation verify --format json` output for `statements`, signed by `identity`.
fn verification(statements: &[Vec<u8>], identity: &str, tmp: &Path, name: &str) -> PathBuf {
    let results: Vec<Value> = statements
        .iter()
        .map(|s| {
            json!({
                "attestation": {"bundle": bundle(s)},
                "verificationResult": {"signature": {"certificate": {
                    "subjectAlternativeName": identity,
                    "issuer": ISSUER,
                }}},
            })
        })
        .collect();
    let path = tmp.join(name);
    std::fs::write(&path, serde_json::to_vec(&results).unwrap()).unwrap();
    path
}

fn verify(repo: &Repo, range: &str) -> assert_cmd::Command {
    let mut cmd = acc();
    cmd.env_remove("GIT_DIR")
        .arg("verify")
        .args(["--range", range])
        .arg("--checkout")
        .arg(repo.path());
    cmd
}

fn result(cmd: &mut assert_cmd::Command, code: i32) -> Value {
    let out = cmd.args(["--format", "json"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(code),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("a JSON result")
}

#[test]
fn a_range_landed_by_passing_changes_verifies() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    let first = repo.commit("pull request 1");
    let second = repo.commit("pull request 2");
    let v = verification(
        &[
            statement("claude-clean", &first, tmp.path()),
            statement("human-clean", &second, tmp.path()),
        ],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    let r = result(
        verify(&repo, &format!("{base}..{second}"))
            .arg("--verification")
            .arg(&v)
            .args(["--signer", SIGNER, "--issuer", ISSUER]),
        0,
    );
    assert_eq!(r["verified"], true);
    assert_eq!(r["range"]["base"], base);
    assert_eq!(r["range"]["head"], second);
    let commits = r["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0]["commit"], second);
    assert!(commits.iter().all(|c| c["status"] == "covered"));
    assert_eq!(r["attestations"][0]["signer"]["identity"], SIGNER);

    // The text form names every commit, and the status line says it verified.
    verify(&repo, &format!("{base}..{second}"))
        .arg("--verification")
        .arg(&v)
        .args(["--signer", SIGNER])
        .assert()
        .success()
        .stdout(predicate::str::contains(&second[..12]).and(predicate::str::contains("covered")))
        .stderr(predicate::str::contains("Verified: 2 commit(s)"));
}

#[test]
fn a_direct_push_is_uncovered_and_fails_the_gate() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    let merged = repo.commit("pull request");
    let pushed = repo.commit("pushed straight to main");
    let v = verification(
        &[statement("claude-clean", &merged, tmp.path())],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    let r = result(
        verify(&repo, &format!("{base}..{pushed}"))
            .arg("--verification")
            .arg(&v)
            .args(["--signer", SIGNER]),
        1,
    );
    assert_eq!(r["verified"], false);
    assert_eq!(r["commits"][0]["commit"], pushed);
    assert_eq!(r["commits"][0]["status"], "uncovered");
    assert_eq!(r["commits"][1]["status"], "covered");
}

#[test]
fn a_failing_change_fails_the_gate_unless_the_policy_allows_it() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    let merged = repo.commit("self-approved pull request");
    let v = verification(
        &[statement("human-self-approved", &merged, tmp.path())],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    let range = format!("{base}..{merged}");
    let r = result(
        verify(&repo, &range)
            .arg("--verification")
            .arg(&v)
            .args(["--signer", SIGNER]),
        1,
    );
    let change = &r["commits"][0]["changes"][0];
    assert_eq!(r["commits"][0]["status"], "failing");
    assert_eq!(change["verdict"], "fail");
    assert_eq!(
        change["failing"],
        json!(["acc-bd288824f2c3dee5", "acc-0f9627abec6db796"])
    );

    // The gate's own policy replaces the signed one; the signed facts stay what they were.
    let policy = tmp.path().join("policy.yml");
    std::fs::write(
        &policy,
        "version: \"0.4\"\nrules:\n  approver_is_author:\n    enabled: false\n    severity: high\n  merged_without_independent_approval:\n    enabled: false\n    severity: high\n",
    )
    .unwrap();
    let r = result(
        verify(&repo, &range)
            .arg("--verification")
            .arg(&v)
            .args(["--signer", SIGNER])
            .arg("--policy")
            .arg(&policy),
        0,
    );
    assert_eq!(r["commits"][0]["status"], "covered");
}

#[test]
fn incomplete_facts_never_pass_and_take_precedence() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    let partial = repo.commit("pull request with an incomplete review history");
    let pushed = repo.commit("pushed straight to main");
    let v = verification(
        &[statement("partial-with-approval", &partial, tmp.path())],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    let r = result(
        verify(&repo, &format!("{base}..{pushed}"))
            .arg("--verification")
            .arg(&v)
            .args(["--signer", SIGNER]),
        4,
    );
    assert_eq!(r["commits"][0]["status"], "uncovered");
    assert_eq!(r["commits"][1]["status"], "incomplete");

    // A complete verdict on the same change wins over the incomplete one.
    let complete = verification(
        &[statement("claude-clean", &partial, tmp.path())],
        SIGNER,
        tmp.path(),
        "complete.json",
    );
    let r = result(
        verify(&repo, &format!("{base}..{partial}"))
            .arg("--verification")
            .arg(&v)
            .arg("--verification")
            .arg(&complete)
            .args(["--signer", SIGNER]),
        0,
    );
    assert_eq!(r["commits"][0]["changes"].as_array().unwrap().len(), 2);
}

#[test]
fn commits_a_merge_brings_in_are_covered_by_the_merge() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "feature"]);
    repo.commit("work on the branch");
    repo.commit("more work on the branch");
    repo.git(&["checkout", "-q", "main"]);
    repo.git(&[
        "merge",
        "-q",
        "--no-ff",
        "feature",
        "-m",
        "merge pull request",
    ]);
    let merge = repo.git(&["rev-parse", "HEAD"]);
    let v = verification(
        &[statement("claude-clean", &merge, tmp.path())],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    let r = result(
        verify(&repo, &format!("{base}..{merge}"))
            .arg("--verification")
            .arg(&v)
            .args(["--signer", SIGNER]),
        0,
    );
    assert_eq!(r["commits"].as_array().unwrap().len(), 1);
}

#[test]
fn an_empty_range_verifies_with_nothing_to_cover() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let head = repo.commit("base");
    let v = verification(
        &[statement("claude-clean", &head, tmp.path())],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    let r = result(
        verify(&repo, &format!("{head}..HEAD"))
            .arg("--verification")
            .arg(&v)
            .args(["--signer", SIGNER]),
        0,
    );
    assert_eq!(r["commits"], json!([]));
}

#[test]
fn only_the_named_signer_and_issuer_are_trusted() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    let merged = repo.commit("pull request");
    let range = format!("{base}..{merged}");
    let elsewhere = verification(
        &[statement("claude-clean", &merged, tmp.path())],
        "https://github.com/mallory/fork/.github/workflows/attest.yml@refs/heads/main",
        tmp.path(),
        "elsewhere.json",
    );
    verify(&repo, &range)
        .arg("--verification")
        .arg(&elsewhere)
        .args(["--signer", SIGNER])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("which no --signer names"));

    let v = verification(
        &[statement("claude-clean", &merged, tmp.path())],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    verify(&repo, &range)
        .arg("--verification")
        .arg(&v)
        .args(["--signer", SIGNER, "--issuer", "https://example.com/oidc"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("not https://example.com/oidc"));
}

#[test]
fn a_tampered_statement_is_invalid() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    let merged = repo.commit("self-approved pull request");
    let mut s: Value =
        serde_json::from_slice(&statement("human-self-approved", &merged, tmp.path())).unwrap();
    // Drop the findings and claim a clean summary: re-evaluation must not reproduce it.
    s["predicate"]["findings"] = json!([]);
    let v = verification(
        &[serde_json::to_vec(&s).unwrap()],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    verify(&repo, &format!("{base}..{merged}"))
        .arg("--verification")
        .arg(&v)
        .args(["--signer", SIGNER])
        .assert()
        .code(3);
}

#[test]
fn caller_verified_bundles_must_be_signed_and_say_who_verified_them() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    let merged = repo.commit("pull request");
    let range = format!("{base}..{merged}");
    let st = statement("claude-clean", &merged, tmp.path());
    let signed = tmp.path().join("signed.sigstore.json");
    std::fs::write(&signed, serde_json::to_vec(&bundle(&st)).unwrap()).unwrap();
    let r = result(
        verify(&repo, &range)
            .arg("--attestations")
            .arg(&signed)
            .args([
                "--verified-by",
                "cosign verify-blob-attestation --certificate-identity attest.yml",
            ]),
        0,
    );
    assert_eq!(
        r["attestations"][0]["verified_by"],
        "cosign verify-blob-attestation --certificate-identity attest.yml"
    );
    assert_eq!(r["attestations"][0]["signer"], Value::Null);

    let unsigned = tmp.path().join("unsigned.intoto.json");
    std::fs::write(&unsigned, &st).unwrap();
    verify(&repo, &range)
        .arg("--attestations")
        .arg(&unsigned)
        .args(["--verified-by", "nobody"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("carries no signature"));
}

#[test]
fn usage_errors() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    let base = repo.commit("base");
    let merged = repo.commit("pull request");
    let range = format!("{base}..{merged}");
    let v = verification(
        &[statement("claude-clean", &merged, tmp.path())],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    // Nothing to verify against.
    verify(&repo, &range).assert().code(2);
    // A verifier's output without the identity the gate trusts.
    verify(&repo, &range)
        .arg("--verification")
        .arg(&v)
        .assert()
        .code(2)
        .stderr(predicate::str::contains("requires --signer"));
    // Caller-verified bundles without saying who verified them.
    verify(&repo, &range)
        .arg("--attestations")
        .arg(&v)
        .assert()
        .code(2);
    // A range that is not BASE..HEAD, and one git cannot resolve.
    for bad in ["HEAD", "a...b"] {
        verify(&repo, bad)
            .arg("--verification")
            .arg(&v)
            .args(["--signer", SIGNER])
            .assert()
            .code(2);
    }
    verify(&repo, "no-such-tag..HEAD")
        .arg("--verification")
        .arg(&v)
        .args(["--signer", SIGNER])
        .assert()
        .code(2);
}

#[test]
fn a_shallow_clone_is_refused() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    repo.commit("base");
    let merged = repo.commit("pull request");
    let shallow = tmp.path().join("shallow");
    let out = Command::new("git")
        .args(["clone", "-q", "--depth", "1"])
        .arg(format!("file://{}", repo.path().display()))
        .arg(&shallow)
        .env_remove("GIT_DIR")
        .output()
        .unwrap();
    assert!(out.status.success());
    let v = verification(
        &[statement("claude-clean", &merged, tmp.path())],
        SIGNER,
        tmp.path(),
        "v.json",
    );
    acc()
        .env_remove("GIT_DIR")
        .arg("verify")
        .args(["--range", &format!("{merged}..{merged}")])
        .arg("--checkout")
        .arg(&shallow)
        .arg("--verification")
        .arg(&v)
        .args(["--signer", SIGNER])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("shallow"));
}
