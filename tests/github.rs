//! The GitHub collector against a loopback HTTP server that replays fixture responses.

use agent_change_control::collectors::github::{Github, UNAVAILABLE_ACTOR};
use agent_change_control::model::{ActorKind, Events, Policy, RuleId};
use agent_change_control::normalize::timestamp;
use agent_change_control::{Exit, exit_for, manifest};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const PULLS: &str = include_str!("fixtures/github-api/pulls.json");
const PULL: &str = include_str!("fixtures/github-api/pull.json");
const REVIEWS: &str = include_str!("fixtures/github-api/reviews.json");
const COMMITS: &str = include_str!("fixtures/github-api/commits.json");
const USER: &str = include_str!("fixtures/github-api/user.json");
const NEXT: &str = "Link: <https://untrusted.invalid/next>; rel=\"next\"\r\n";

struct Server {
    base: String,
    stop: Arc<AtomicBool>,
    violated: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Server {
    fn start(mode: &'static str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let violated = Arc::new(AtomicBool::new(false));
        let (signal, flag) = (stop.clone(), violated.clone());
        let thread = thread::spawn(move || {
            let mut pull_reads = 0;
            while !signal.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => handle(stream, mode, &mut pull_reads, &flag),
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
            thread: Some(thread),
        }
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
                "an Authorization header reached the loopback server"
            );
        }
    }
}

/// Serve one request. Sockets accepted from a non-blocking listener inherit that flag on macOS,
/// so the stream is switched back to blocking before reading. Any I/O failure simply drops the
/// connection: the client then fails its own assertion instead of the server thread panicking.
fn handle(
    mut stream: TcpStream,
    mode: &'static str,
    pull_reads: &mut usize,
    violated: &AtomicBool,
) {
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
    if request.to_ascii_lowercase().contains("authorization:") {
        violated.store(true, Ordering::Relaxed);
    }
    let Some(path) = request.split_whitespace().nth(1) else {
        return;
    };
    let (status, link, body) = respond(mode, path, pull_reads);
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n{link}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.flush();
}

fn edited(json: &str, edit: impl FnOnce(&mut Value)) -> String {
    let mut v: Value = serde_json::from_str(json).unwrap();
    edit(&mut v);
    v.to_string()
}

/// A page of one pull request last updated (and merged) before the collection window.
fn older_page() -> String {
    edited(PULLS, |v| {
        v[0]["number"] = 300.into();
        v[0]["id"] = 90.into();
        v[0]["created_at"] = "2026-06-01T00:00:00Z".into();
        v[0]["updated_at"] = "2026-07-01T00:00:00Z".into();
        v[0]["merged_at"] = "2026-07-01T00:00:00Z".into();
    })
}

fn respond(mode: &str, path: &str, pull_reads: &mut usize) -> (&'static str, &'static str, String) {
    match mode {
        "unauthorized" => return ("401 Unauthorized", "", "{}".into()),
        "rate-limit" => return ("429 Too Many Requests", "", "{}".into()),
        "redirect" => return ("302 Found", "", "{}".into()),
        _ => {}
    }
    if path.starts_with("/repos/acme/api/pulls?") {
        let page = path.rsplit("page=").next().unwrap_or("1");
        return match (mode, page) {
            ("capped", _) => ("200 OK", NEXT, PULLS.into()),
            ("pagination", "1") => ("200 OK", NEXT, "[]".into()),
            ("older-page", "1") => ("200 OK", NEXT, PULLS.into()),
            ("older-page", "2") => ("200 OK", NEXT, older_page()),
            ("older-page", _) => ("500 Internal Server Error", "", "{}".into()),
            _ => ("200 OK", "", PULLS.into()),
        };
    }
    if path.contains("/reviews?") {
        let body = match mode {
            "dismissed" => REVIEWS.replace("APPROVED", "DISMISSED"),
            "deleted-reviewer" => edited(REVIEWS, |v| v[0]["user"] = Value::Null),
            _ => REVIEWS.into(),
        };
        return ("200 OK", "", body);
    }
    if path.contains("/commits?") {
        return ("200 OK", "", COMMITS.into());
    }
    if path.starts_with("/users/") {
        return ("200 OK", "", USER.into());
    }
    *pull_reads += 1;
    let body = match mode {
        "moving" if *pull_reads > 1 => edited(PULL, |v| v["head"]["sha"] = "moved".into()),
        "deleted-merger" => edited(PULL, |v| v["merged_by"] = Value::Null),
        _ => PULL.into(),
    };
    ("200 OK", "", body)
}

fn window() -> (
    agent_change_control::model::Timestamp,
    agent_change_control::model::Timestamp,
) {
    (
        timestamp("2026-08-01T00:00:00Z").unwrap(),
        timestamp("2026-08-31T23:59:59Z").unwrap(),
    )
}

fn collect_with(mode: &'static str, known: &BTreeMap<String, String>) -> anyhow::Result<Events> {
    let server = Server::start(mode);
    let pages = if mode == "capped" { 1 } else { 3 };
    let (from, to) = window();
    Github::with_base(None, pages, &server.base)?.collect("acme/api", from, to, None, known)
}

fn collect(mode: &'static str) -> anyhow::Result<Events> {
    collect_with(mode, &BTreeMap::new())
}

#[test]
fn github_export_evaluate_and_check() {
    let e = collect("clean").unwrap();
    assert!(e.window.complete);
    assert_eq!(e.changes[0].author.actor_id, "agent:claude-code");
    assert_eq!(
        e.changes[0]
            .agent_operator
            .as_ref()
            .unwrap()
            .actor_id
            .as_deref(),
        Some("github:alice")
    );
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    assert!(m.findings.is_empty());
    assert_eq!(manifest::check(&m, None, None).unwrap().1, Exit::Ok);
}

#[test]
fn pagination_uses_fixed_origin() {
    let e = collect("pagination").unwrap();
    assert_eq!(e.changes.len(), 1);
    assert!(e.window.complete);
}

#[test]
fn listing_stops_at_the_first_page_before_the_window() {
    // Page 3 answers 500; reaching it means the walk did not stop after the older page.
    let e = collect("older-page").unwrap();
    assert!(e.window.complete);
    assert_eq!(e.changes.len(), 1);
    assert_eq!(e.changes[0].id, "github:acme/api:pr:421");
}

#[test]
fn incomplete_snapshots_never_pass() {
    for mode in ["capped", "dismissed", "moving", "deleted-reviewer"] {
        let e = collect(mode).unwrap();
        assert!(!e.window.complete, "{mode}");
        let m = manifest::evaluate(e, Policy::default()).unwrap();
        assert_eq!(m.summary.clean, 0, "{mode}");
        assert_eq!(
            manifest::check(&m, None, None).unwrap().1,
            Exit::Incomplete,
            "{mode}"
        );
    }
}

#[test]
fn deleted_accounts_degrade_to_unknown_actors() {
    let e = collect("deleted-merger").unwrap();
    assert!(e.window.complete);
    let merger = e.changes[0].merger.as_ref().unwrap();
    assert_eq!(merger.actor_id, UNAVAILABLE_ACTOR);
    assert_eq!(e.actors[UNAVAILABLE_ACTOR].kind, ActorKind::Unknown);
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    assert!(m.findings.is_empty());
    let e = collect("deleted-reviewer").unwrap();
    assert_eq!(e.changes[0].reviews[0].actor_id, UNAVAILABLE_ACTOR);
    assert!(!e.changes[0].reviews_complete);
}

#[test]
fn api_errors_are_typed_and_redacted() {
    for (mode, exit) in [
        ("unauthorized", Exit::Auth),
        ("rate-limit", Exit::Api),
        ("redirect", Exit::Api),
    ] {
        let err = collect(mode).unwrap_err();
        assert_eq!(exit_for(&err), exit, "{mode}");
        let text = format!("{err:#}");
        assert!(!text.contains("untrusted.invalid"), "{text}");
        assert!(!text.contains("127.0.0.1"), "{text}");
    }
}

#[test]
fn tokens_cannot_be_sent_to_custom_hosts() {
    assert!(Github::with_base(Some("secret".into()), 1, "http://127.0.0.1:1").is_err());
    assert!(Github::with_base(None, 1, "https://evil.example").is_err());
}

#[test]
fn known_agent_reviewers_cannot_count_as_humans() {
    let known = BTreeMap::from([("bob".to_string(), "review-agent".to_string())]);
    let events = collect_with("clean", &known).unwrap();
    assert_eq!(events.actors["github:bob"].kind, ActorKind::Agent);
    let m = manifest::evaluate(events, Policy::default()).unwrap();
    assert_eq!(
        m.findings.iter().map(|f| f.rule_id).collect::<Vec<_>>(),
        vec![RuleId::Acc001, RuleId::Acc003]
    );
}
