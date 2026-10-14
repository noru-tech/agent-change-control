//! `acc release`: an in-toto Statement about release artifacts, covering a real git range with
//! the verdicts of the changes that landed it.

mod common;

use common::{acc, fixture};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

const ARTIFACT: &str =
    "acc.tar.xz=sha256:3f8a1e0c55d1a2b4c6e8f00112233445566778899aabbccddeeff00112233445";

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

    fn commit(&self, message: &str) -> String {
        self.git(&["commit", "-q", "--allow-empty", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }
}

/// A manifest of `fixture_name`'s events with its first change merged as `merge_sha`.
fn manifest(fixture_name: &str, merge_sha: &str, tmp: &Path) -> PathBuf {
    let mut events: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture(fixture_name, "events.json")).unwrap(),
    )
    .unwrap();
    events["changes"][0]["merge_commit_sha"] = json!(merge_sha);
    let input = tmp.join(format!("{fixture_name}-{merge_sha}.events.json"));
    std::fs::write(&input, serde_json::to_vec(&events).unwrap()).unwrap();
    let output = tmp.join(format!("{fixture_name}-{merge_sha}.manifest.json"));
    let out = acc()
        .arg("evaluate")
        .arg(&input)
        .args(["--format", "json", "--output"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        matches!(out.status.code(), Some(0 | 4)),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    output
}

/// Run `acc release` over `range` and return its exit code and Statement.
fn release(repo: &Repo, range: &str, manifests: &[PathBuf], extra: &[&str]) -> (i32, Value) {
    let out = acc()
        .env_remove("GIT_DIR")
        .arg("release")
        .args(manifests)
        .args(["--range", range, "--subject", ARTIFACT])
        .args(extra)
        .arg("--checkout")
        .arg(repo.path())
        .output()
        .unwrap();
    let statement = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
        panic!(
            "no Statement; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    (out.status.code().unwrap(), statement)
}

#[test]
fn a_release_whose_every_commit_a_passing_change_landed_passes() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    repo.commit("v1");
    repo.git(&["tag", "v1"]);
    let merged = repo.commit("Merge pull request #421");
    repo.git(&["tag", "v2"]);
    let (code, s) = release(
        &repo,
        "v1..v2",
        &[manifest("human-clean", &merged, tmp.path())],
        &[],
    );
    assert_eq!(code, 0);
    assert_eq!(s["_type"], "https://in-toto.io/Statement/v1");
    assert_eq!(
        s["predicateType"],
        "https://noru.tech/spec/ai-change-provenance/release/v0.1"
    );
    assert_eq!(
        s["subject"],
        json!([{"name": "acc.tar.xz", "digest": {"sha256": "3f8a1e0c55d1a2b4c6e8f00112233445566778899aabbccddeeff00112233445"}}])
    );
    let p = &s["predicate"];
    assert_eq!(p["verdict"], "pass");
    assert_eq!(p["range"]["head"], json!(merged));
    assert_eq!(p["commits"][0]["status"], "covered");
    let change = &p["commits"][0]["changes"][0];
    assert_eq!(change["change_id"], "github:acme/api:pr:421");
    assert_eq!(change["verdict"], "pass");
    assert!(
        change["verdict_digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(change["manifest_digest"], p["manifests"][0]["digest"]);
    assert_eq!(p["uncovered"], json!([]));

    // `acc validate` recognises the Statement.
    let path = tmp.path().join("release.intoto.json");
    std::fs::write(&path, serde_json::to_vec(&s).unwrap()).unwrap();
    acc()
        .arg("validate")
        .arg(&path)
        .assert()
        .success()
        .stderr(predicates::str::contains("Valid release attestation"));
}

#[test]
fn a_direct_push_or_a_failing_change_fails_the_release_but_is_still_attested() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    repo.commit("v1");
    repo.git(&["tag", "v1"]);
    let reviewed = repo.commit("Merge pull request #421");
    let self_approved = repo.commit("Merge pull request #7");
    let pushed = repo.commit("Pushed straight to main");
    let (code, s) = release(
        &repo,
        "v1..HEAD",
        &[
            manifest("human-clean", &reviewed, tmp.path()),
            manifest("human-self-approved", &self_approved, tmp.path()),
        ],
        &[],
    );
    assert_eq!(code, 1);
    let p = &s["predicate"];
    assert_eq!(p["verdict"], "fail");
    let statuses: Vec<(&str, &str)> = p["commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["commit"].as_str().unwrap(), c["status"].as_str().unwrap()))
        .collect();
    // Newest first, as `git rev-list --first-parent` prints them.
    assert_eq!(
        statuses,
        vec![
            (pushed.as_str(), "uncovered"),
            (self_approved.as_str(), "failing"),
            (reviewed.as_str(), "covered"),
        ]
    );
    assert_eq!(p["uncovered"], json!([pushed]));
    assert_eq!(p["manifests"].as_array().unwrap().len(), 2);
}

#[test]
fn subjects_are_required_and_must_be_sha256() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    repo.commit("v1");
    repo.git(&["tag", "v1"]);
    let merged = repo.commit("Merge");
    let m = manifest("human-clean", &merged, tmp.path());
    acc()
        .env_remove("GIT_DIR")
        .arg("release")
        .arg(&m)
        .args(["--range", "v1..HEAD", "--subject", "acc.tar.xz=sha1:abc"])
        .arg("--checkout")
        .arg(repo.path())
        .assert()
        .code(2)
        .stderr(predicates::str::contains("invalid --subject"));
    acc()
        .env_remove("GIT_DIR")
        .arg("release")
        .arg(&m)
        .args(["--range", "v1..HEAD"])
        .arg("--checkout")
        .arg(repo.path())
        .assert()
        .code(2);
}

#[test]
fn a_tampered_manifest_is_refused() {
    let repo = Repo::new();
    let tmp = TempDir::new().unwrap();
    repo.commit("v1");
    repo.git(&["tag", "v1"]);
    let merged = repo.commit("Merge");
    let m = manifest("human-self-approved", &merged, tmp.path());
    let mut doc: Value = serde_json::from_str(&std::fs::read_to_string(&m).unwrap()).unwrap();
    doc["findings"] = json!([]);
    std::fs::write(&m, serde_json::to_vec(&doc).unwrap()).unwrap();
    acc()
        .env_remove("GIT_DIR")
        .arg("release")
        .arg(&m)
        .args(["--range", "v1..HEAD", "--subject", ARTIFACT])
        .arg("--checkout")
        .arg(repo.path())
        .assert()
        .code(3);
}
