//! The GitLab collector against a loopback HTTP server that serves canned API responses.

mod common;

use predicates::prelude::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const PROJECT: &str = "acme/platform/api";
const ENCODED: &str = "acme%2Fplatform%2Fapi";
const FIRST: &str = "1111111111111111111111111111111111111111";
const HEAD: &str = "2222222222222222222222222222222222222222";
const MERGE: &str = "3333333333333333333333333333333333333333";

/// Paths to responses. A path is matched with its pagination parameters removed; anything
/// unrouted is a 404.
type Routes = BTreeMap<String, Value>;

struct Server {
    base: String,
    stop: Arc<AtomicBool>,
    violated: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<String>>>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    fn start(routes: Routes) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let violated = Arc::new(AtomicBool::new(false));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let (signal, flag, log) = (stop.clone(), violated.clone(), requests.clone());
        let thread = thread::spawn(move || {
            while !signal.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => handle(stream, &routes, &flag, &log),
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base,
            stop,
            violated,
            requests,
            thread: Some(thread),
        }
    }

    fn requested(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if !thread::panicking() {
            assert!(
                !self.violated.load(Ordering::Relaxed),
                "an Authorization or PRIVATE-TOKEN header reached the loopback server"
            );
        }
    }
}

fn handle(mut stream: TcpStream, routes: &Routes, violated: &AtomicBool, log: &Mutex<Vec<String>>) {
    if stream.set_nonblocking(false).is_err() {
        return;
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut request = Vec::new();
    let mut buf = [0u8; 4096];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => request.extend_from_slice(&buf[..n]),
        }
        if request.len() > 64 * 1024 {
            return;
        }
    }
    let request = String::from_utf8_lossy(&request);
    let lower = request.to_ascii_lowercase();
    if lower.contains("authorization:") || lower.contains("private-token:") {
        violated.store(true, Ordering::Relaxed);
    }
    let Some(path) = request.split_whitespace().nth(1) else {
        return;
    };
    let key = path
        .replace("?per_page=100&page=1", "")
        .replace("&per_page=100&page=1", "");
    log.lock().unwrap().push(key.clone());
    let (status, body) = match routes.get(&key) {
        Some(body) => ("200 OK", body.to_string()),
        None => ("404 Not Found", "{}".to_string()),
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}

fn api(path: &str) -> String {
    format!("/api/v4/projects/{ENCODED}{path}")
}

fn user(id: u64, username: &str) -> Value {
    json!({"id": id, "username": username, "name": username, "state": "active"})
}

/// One merged merge request, `!7`: opened by alice, with a first push (`FIRST`) and a second
/// (`HEAD`), merged by carol into `MERGE`. `approvals` are `(user, approved_at)`.
fn routes(approvals: &[(Value, &str)], description: &str) -> Routes {
    let mut r = Routes::new();
    let mr = json!({
        "id": 7007, "iid": 7, "title": "Rotate the signing key", "description": description,
        "state": "merged", "created_at": "2026-08-10T09:00:00.000Z",
        "updated_at": "2026-08-12T15:00:05.000Z", "merged_at": "2026-08-12T15:00:00.000Z",
        "sha": HEAD, "merge_commit_sha": MERGE, "squash_commit_sha": null,
        "target_branch": "main", "labels": ["security"],
        "author": user(11, "alice"), "merge_user": user(13, "carol"),
    });
    r.insert(
        api("/merge_requests?state=merged&order_by=updated_at&sort=desc"),
        json!([mr.clone(), {
            "iid": 3, "merged_at": "2026-07-01T00:00:00.000Z", "updated_at": "2026-07-01T00:00:00.000Z"
        }]),
    );
    r.insert(api("/merge_requests/7"), mr);
    r.insert(
        api("/merge_requests/7/approvals"),
        json!({"approved": true, "approved_by": approvals.iter().map(|(u, at)| json!({"user": u, "approved_at": at})).collect::<Vec<_>>()}),
    );
    r.insert(
        api("/merge_requests/7/versions"),
        json!([
            {"id": 102, "head_commit_sha": HEAD, "created_at": "2026-08-11T10:00:00.000Z"},
            {"id": 101, "head_commit_sha": FIRST, "created_at": "2026-08-10T09:00:05.000Z"},
        ]),
    );
    r.insert(
        api("/merge_requests/7/commits"),
        json!([
            {"id": HEAD, "message": "Rotate the key\n", "author_email": "alice@example.com"},
            {"id": FIRST, "message": "Add the new key\n", "author_email": "alice@example.com"},
        ]),
    );
    for (id, name, bot) in [
        (11, "alice", false),
        (12, "bob", false),
        (13, "carol", false),
        (14, "project_5_bot_e1f2", true),
    ] {
        r.insert(
            format!("/api/v4/users/{id}"),
            json!({"id": id, "username": name, "bot": bot}),
        );
        r.insert(
            format!("/api/v4/users?username={name}"),
            json!([user(id, name)]),
        );
    }
    r
}

/// The approval routes plus a project that requires pipelines, a pipeline on the head with
/// `status`, and two jobs, one of them allowed to fail.
fn with_checks(mut r: Routes, status: &str) -> Routes {
    r.insert(
        format!("/api/v4/projects/{ENCODED}"),
        json!({"id": 5, "path_with_namespace": PROJECT, "only_allow_merge_if_pipeline_succeeds": true}),
    );
    r.insert(
        api(&format!("/pipelines?sha={HEAD}")),
        json!([{"id": 900, "sha": HEAD}]),
    );
    r.insert(api(&format!("/pipelines?sha={MERGE}")), json!([]));
    r.insert(
        api("/pipelines/900"),
        json!({"id": 900, "sha": HEAD, "status": status, "finished_at": "2026-08-12T14:30:00.000Z"}),
    );
    r.insert(
        api("/pipelines/900/jobs?include_retried=true"),
        json!([
            {"id": 1, "name": "test", "status": "success", "allow_failure": false, "finished_at": "2026-08-12T14:20:00.000Z"},
            {"id": 2, "name": "lint", "status": "failed", "allow_failure": true, "finished_at": "2026-08-12T14:10:00.000Z"},
            {"id": 3, "name": "deploy", "status": "manual", "allow_failure": false, "finished_at": null},
        ]),
    );
    r
}

fn acc_against(server: &Server) -> assert_cmd::Command {
    let mut cmd = common::acc();
    cmd.env("ACC_GITLAB_API_URL", &server.base);
    cmd
}

const SCAN: [&str; 9] = [
    "scan",
    "gitlab",
    PROJECT,
    "--since",
    "2026-08-01",
    "--until",
    "2026-08-31",
    "-f",
    "json",
];

/// Run `acc`, require exit `code`, and parse its stdout as JSON. `scan` reports failing rules
/// in the manifest and exits 0; only an incomplete collection changes its exit code.
fn manifest(cmd: &mut assert_cmd::Command, code: i32) -> Value {
    let out = cmd.output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(code), "{stderr}");
    serde_json::from_slice(&out.stdout).unwrap_or_else(|_| panic!("not JSON; {stderr}"))
}

fn status(m: &Value, rule: &str) -> String {
    m["assessments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["rule_id"] == rule && a["change_id"] == "gitlab:acme/platform/api:mr:7")
        .unwrap_or_else(|| panic!("no {rule} assessment"))["status"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn an_approval_of_the_head_by_another_human_qualifies() {
    let server = Server::start(routes(&[(user(12, "bob"), "2026-08-12T14:00:00.000Z")], ""));
    let m = manifest(acc_against(&server).args(SCAN), 0);
    let events = &m["events"];
    assert_eq!(events["repository"], PROJECT);
    assert_eq!(events["window"]["complete"], true);
    let changes = events["changes"].as_array().unwrap();
    assert_eq!(changes.len(), 1, "only !7 merged in the window");
    let c = &changes[0];
    assert_eq!(c["forge"], "gitlab");
    assert_eq!(
        c["url"],
        "https://gitlab.com/acme/platform/api/-/merge_requests/7"
    );
    assert_eq!(c["merge_commit_sha"], MERGE);
    assert_eq!(c["forge_author"]["actor_id"], "gitlab:alice");
    assert_eq!(c["merger"]["actor_id"], "gitlab:carol");
    assert_eq!(c["labels"], json!(["security"]));
    assert_eq!(c["reviews_complete"], true);
    let review = &c["reviews"][0];
    assert_eq!(review["actor_id"], "gitlab:bob");
    assert_eq!(review["commit_sha"], HEAD);
    // The binding is derived from the diff versions, and says which one.
    assert!(
        review["provenance"].as_array().unwrap().iter().any(
            |p| p["kind"] == "derived" && p["ref"].as_str().unwrap().ends_with("/versions#102")
        ),
        "{review}"
    );
    // GitLab's commit listing names no account, so no commit author is resolved from an email.
    assert!(
        c["commits"]
            .as_array()
            .unwrap()
            .iter()
            .all(|k| k["author"].is_null())
    );
    assert_eq!(events["actors"]["gitlab:bob"]["kind"], "human");
    assert_eq!(status(&m, "ACC003"), "pass");
    assert_eq!(status(&m, "ACC002"), "pass");
}

#[test]
fn an_approval_given_before_the_last_push_does_not_approve_the_head() {
    let server = Server::start(routes(&[(user(12, "bob"), "2026-08-10T12:00:00.000Z")], ""));
    let m = manifest(acc_against(&server).args(SCAN), 0);
    assert_eq!(m["events"]["changes"][0]["reviews"][0]["commit_sha"], FIRST);
    assert_eq!(status(&m, "ACC003"), "fail");
}

#[test]
fn an_approval_too_close_to_a_push_is_not_bound_and_the_history_is_incomplete() {
    // 400 ms after the second diff version: it could have approved either head.
    let server = Server::start(routes(&[(user(12, "bob"), "2026-08-11T10:00:00.400Z")], ""));
    let m = manifest(acc_against(&server).args(SCAN), 4);
    let c = &m["events"]["changes"][0];
    assert_eq!(c["reviews"], json!([]));
    assert_eq!(c["reviews_complete"], false);
    assert_eq!(m["events"]["window"]["complete"], false);
    assert_eq!(status(&m, "ACC003"), "unknown");
}

#[test]
fn a_bot_approval_is_never_a_human_approval() {
    let server = Server::start(routes(
        &[(user(14, "project_5_bot_e1f2"), "2026-08-12T14:00:00.000Z")],
        "",
    ));
    let m = manifest(acc_against(&server).args(SCAN), 0);
    assert_eq!(
        m["events"]["actors"]["gitlab:project_5_bot_e1f2"]["kind"],
        "bot"
    );
    assert_eq!(status(&m, "ACC003"), "fail");
}

#[test]
fn a_declared_agent_change_approved_only_by_its_operator_fails() {
    let declaration =
        "Key rotation.\n\n```agent-change-control\nauthor: claude-code\noperator: alice\n```\n";
    let server = Server::start(routes(
        &[(user(11, "alice"), "2026-08-12T14:00:00.000Z")],
        declaration,
    ));
    let m = manifest(acc_against(&server).args(SCAN), 0);
    let c = &m["events"]["changes"][0];
    assert_eq!(c["author"]["actor_id"], "agent:claude-code");
    assert_eq!(c["agent_operator"]["actor_id"], "gitlab:alice");
    assert_eq!(c["agent_operator"]["confidence"], "explicit");
    assert_eq!(status(&m, "ACC001"), "fail");
    // The operator was confirmed a human through the users API, never assumed.
    assert!(
        server
            .requested()
            .contains(&"/api/v4/users?username=alice".to_string())
    );
}

#[test]
fn pipelines_and_jobs_on_the_head_feed_acc004() {
    let approval = [(user(12, "bob"), "2026-08-12T14:00:00.000Z")];
    let server = Server::start(with_checks(routes(&approval, ""), "success"));
    let m = manifest(acc_against(&server).args(SCAN).arg("--checks"), 0);
    let checks = &m["events"]["changes"][0]["checks"];
    assert_eq!(checks["required"], json!(["pipeline"]));
    assert_eq!(checks["required_sources"], json!(["project_settings"]));
    assert_eq!(checks["runs_complete"], true);
    let runs: BTreeMap<String, (String, String)> = checks["runs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["name"].as_str().unwrap().to_string(),
                (
                    r["kind"].as_str().unwrap().to_string(),
                    r["conclusion"].as_str().unwrap().to_string(),
                ),
            )
        })
        .collect();
    assert_eq!(runs["pipeline"], ("pipeline".into(), "success".into()));
    assert_eq!(runs["test"], ("job".into(), "success".into()));
    // Allowed to fail: recorded, but not as a failure.
    assert_eq!(runs["lint"], ("job".into(), "neutral".into()));
    // A manual job that never ran has no result.
    assert!(!runs.contains_key("deploy"));

    let policy = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(policy.path(), "version: \"0.4\"\nrules:\n  merged_without_required_checks:\n    enabled: true\n    severity: high\n").unwrap();
    let m = manifest(
        acc_against(&server)
            .args(SCAN)
            .arg("--policy")
            .arg(policy.path()),
        0,
    );
    assert_eq!(status(&m, "ACC004"), "pass");

    let failed = Server::start(with_checks(routes(&approval, ""), "failed"));
    let m = manifest(
        acc_against(&failed)
            .args(SCAN)
            .arg("--policy")
            .arg(policy.path()),
        0,
    );
    assert_eq!(status(&m, "ACC004"), "fail");
}

#[test]
fn bypass_is_recorded_as_unreadable_on_gitlab() {
    let server = Server::start(routes(&[(user(12, "bob"), "2026-08-12T14:00:00.000Z")], ""));
    let m = manifest(acc_against(&server).args(SCAN).arg("--bypass"), 0);
    let protection = &m["events"]["changes"][0]["protection"];
    assert_eq!(protection["branch"], "main");
    assert_eq!(protection["complete"], false);
    assert_eq!(protection["suites"], json!([]));
}

#[test]
fn a_group_path_is_a_reviewer_team_of_its_direct_members() {
    let mut r = routes(&[(user(12, "bob"), "2026-08-12T14:00:00.000Z")], "");
    r.insert(
        "/api/v4/groups/acme%2Fsecurity/members".into(),
        json!([user(12, "Bob"), user(15, "dana"), user(16, "ghost")]),
    );
    let server = Server::start(r);
    let m = manifest(
        acc_against(&server)
            .args(SCAN)
            .args(["--team", "acme/security"]),
        0,
    );
    let team = &m["events"]["teams"][0];
    assert_eq!(team["id"], "acme/security");
    assert_eq!(team["members"], json!(["gitlab:bob", "gitlab:dana"]));
    assert_eq!(team["complete"], true);
}

#[test]
fn one_merge_request_by_its_iid() {
    let server = Server::start(routes(&[(user(12, "bob"), "2026-08-12T14:00:00.000Z")], ""));
    let m = manifest(
        acc_against(&server).args([
            "pr", "7", "--forge", "gitlab", "--repo", PROJECT, "-f", "json",
        ]),
        0,
    );
    assert_eq!(
        m["events"]["changes"][0]["id"],
        "gitlab:acme/platform/api:mr:7"
    );
    assert!(
        !server
            .requested()
            .iter()
            .any(|p| p.contains("/merge_requests?")),
        "a single merge request is read directly, not listed"
    );
}

#[test]
fn a_gitlab_pipeline_names_the_forge_and_the_project() {
    let server = Server::start(routes(&[(user(12, "bob"), "2026-08-12T14:00:00.000Z")], ""));
    let m = manifest(
        acc_against(&server)
            .env("GITLAB_CI", "true")
            .env("CI_SERVER_HOST", "gitlab.com")
            .env("CI_PROJECT_PATH", PROJECT)
            .args([
                "scan",
                "--since",
                "2026-08-01",
                "--until",
                "2026-08-31",
                "-f",
                "json",
            ]),
        0,
    );
    assert_eq!(m["events"]["repository"], PROJECT);

    // Self-managed GitLab is not collected yet, so its pipeline names nothing to collect.
    acc_against(&server)
        .env("GITLAB_CI", "true")
        .env("CI_SERVER_HOST", "gitlab.example.com")
        .env("CI_PROJECT_PATH", PROJECT)
        .args(["scan", "--since", "2026-08-01", "--until", "2026-08-31"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains(
            "no repository given and none detected",
        ))
        .stderr(predicates::str::contains("gitlab.com pipeline"));
}

#[test]
fn a_gitlab_origin_remote_selects_gitlab() {
    let server = Server::start(routes(&[(user(12, "bob"), "2026-08-12T14:00:00.000Z")], ""));
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&[
        "remote",
        "add",
        "origin",
        "git@gitlab.com:acme/platform/api.git",
    ]);
    let out = acc_against(&server)
        .env_remove("GIT_DIR")
        .current_dir(dir.path())
        .args([
            "scan",
            "--since",
            "2026-08-01",
            "--until",
            "2026-08-31",
            "-f",
            "json",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{stderr}");
    assert!(
        stderr.contains("repository: acme/platform/api (from the origin remote)"),
        "{stderr}"
    );
}

#[test]
fn the_testing_api_base_never_receives_a_token() {
    let server = Server::start(Routes::new());
    acc_against(&server)
        .env("GITLAB_TOKEN", "glpat-should-never-be-sent")
        .args(SCAN)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("unsupported API origin"))
        .stderr(predicates::str::contains("glpat-should-never-be-sent").not());
    common::acc()
        .env("ACC_GITLAB_API_URL", "https://evil.example")
        .args(SCAN)
        .assert()
        .code(2);
}

#[test]
fn doctor_reports_the_gitlab_project_and_token() {
    let out = common::acc()
        .env("GITLAB_CI", "true")
        .env("CI_SERVER_HOST", "gitlab.com")
        .env("CI_PROJECT_PATH", PROJECT)
        .args(["doctor", "--format", "json"])
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let check = |name: &str| {
        report["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap()
            .clone()
    };
    assert_eq!(
        check("repository")["detail"],
        "acme/platform/api on GitLab (from CI_PROJECT_PATH)"
    );
    assert_eq!(check("token")["status"], "warn");
    assert!(
        check("token")["hint"]
            .as_str()
            .unwrap()
            .contains("GITLAB_TOKEN")
    );
}
