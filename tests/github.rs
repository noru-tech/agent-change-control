//! The GitHub collector against a loopback HTTP server that replays fixture responses.

mod common;

use agent_change_control::collectors::github::{Github, Sources, UNAVAILABLE_ACTOR};
use agent_change_control::model::{
    ActorKind, Confidence, Events, EvidenceKind, Policy, ReviewState, RuleId, Status,
};
use agent_change_control::normalize::timestamp;
use agent_change_control::provenance::agent_trace::AgentTraces;
use agent_change_control::provenance::attestations::Attestations;
use agent_change_control::provenance::trailer_registry;
use agent_change_control::{Exit, exit_for, manifest};
use predicates::prelude::*;
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
        "rate-limit-header" => {
            return ("403 Forbidden", "X-RateLimit-Remaining: 0\r\n", "{}".into());
        }
        "forbidden" => return ("403 Forbidden", "", "{}".into()),
        "not-found" => return ("404 Not Found", "", "{}".into()),
        "redirect" => return ("302 Found", "", "{}".into()),
        _ => {}
    }
    if path == "/rate_limit" {
        let remaining = if mode == "rate-exhausted" { 0 } else { 59 };
        let core =
            format!("{{\"limit\":60,\"remaining\":{remaining},\"reset\":1790000000,\"used\":1}}");
        return (
            "200 OK",
            "",
            format!("{{\"resources\":{{\"core\":{core}}},\"rate\":{core}}}"),
        );
    }
    if path == "/repos/noru-tech/agent-change-control/releases/latest" {
        let tag = if mode == "newer-release" {
            "v99.0.0".to_string()
        } else {
            format!("v{}", env!("CARGO_PKG_VERSION"))
        };
        return ("200 OK", "", format!("{{\"tag_name\":\"{tag}\"}}"));
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
        let body = match mode {
            "trailer-bot-author" => edited(COMMITS, |v| {
                v[0]["author"]["login"] = "acme-agent[bot]".into();
                v[0]["author"]["type"] = "Bot".into();
                v[0]["author"]["id"] = 7.into();
            }),
            "trailer-two-agents" => edited(COMMITS, |v| {
                v[0]["commit"]["message"] = "x\n\nCo-Authored-By: A <noreply@anthropic.com>\nCo-Authored-By: C <1+Copilot@users.noreply.github.com>".into();
            }),
            "trace-only" => edited(COMMITS, |v| v[0]["commit"]["message"] = "Plain".into()),
            _ => COMMITS.into(),
        };
        return ("200 OK", "", body);
    }
    if path.starts_with("/users/") {
        return ("200 OK", "", USER.into());
    }
    *pull_reads += 1;
    let body = match mode {
        "moving" if *pull_reads > 1 => edited(PULL, |v| v["head"]["sha"] = "moved".into()),
        "deleted-merger" => edited(PULL, |v| v["merged_by"] = Value::Null),
        // No declaration in the body: only the commit trailer speaks to authorship.
        "trailer-only" | "trailer-bot-author" | "trailer-two-agents" | "trailers-ignored"
        | "trace-only" | "trace-and-trailer" | "attested-only" => {
            edited(PULL, |v| v["body"] = "Plain description".into())
        }
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
    let registry = (mode != "trailers-ignored").then(|| trailer_registry(&BTreeMap::new()));
    let traces = matches!(mode, "trace-only" | "trace-and-trailer")
        .then(|| AgentTraces::load(&[fixture("agent-trace")]).unwrap());
    let attestations = match mode {
        "review-human" | "review-agent" | "review-kind-mismatch" | "review-human-with-operator" => {
            let dir = match mode {
                "review-human" => "review-human",
                "review-human-with-operator" => "review-human-with-operator",
                _ => "review-agent",
            };
            let mut a =
                Attestations::load(&[], Some("gh attestation verify (test)".into())).unwrap();
            a.load_verification(&fixture(&format!("attestations/{dir}/verification.json")))
                .unwrap();
            Some(a)
        }
        "review-unmatched" => Some(
            Attestations::load(
                &[fixture("attestations/review-unmatched")],
                Some("x".into()),
            )
            .unwrap(),
        ),
        "attested" | "attested-only" => Some(
            Attestations::load(
                &[fixture("attestations/agree")],
                Some("gh attestation verify (test)".into()),
            )
            .unwrap(),
        ),
        "attested-unverified" => {
            Some(Attestations::load(&[fixture("attestations/agree")], None).unwrap())
        }
        "attested-conflict" => {
            Some(Attestations::load(&[fixture("attestations/conflict")], Some("x".into())).unwrap())
        }
        _ => None,
    };
    let vendors = agent_change_control::provenance::vendor_registry(&BTreeMap::from([(
        "review-agent".to_string(),
        "acme".to_string(),
    )]));
    Github::with_base(None, pages, &server.base)?.collect(
        "acme/api",
        from,
        to,
        None,
        Sources {
            known,
            trailers: registry.as_ref(),
            traces: traces.as_ref(),
            attestations: attestations.as_ref(),
            vendors: &vendors,
        },
    )
}

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
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
fn verified_attestations_are_signed_evidence_and_must_agree_with_declarations() {
    // The declaration (claude-code, alice) and the DSSE-wrapped provenance attestation agree:
    // the operator is explicit, and the attestation's evidence is signed because the container
    // carried a signature and the caller stated who verified it.
    let e = collect("attested").unwrap();
    let c = &e.changes[0];
    let op = c.agent_operator.as_ref().unwrap();
    assert_eq!(op.actor_id.as_deref(), Some("github:alice"));
    assert_eq!(op.confidence, Confidence::Explicit);
    let signed: Vec<_> = op
        .provenance
        .iter()
        .filter(|p| p.kind == EvidenceKind::Signed)
        .collect();
    assert_eq!(signed.len(), 1);
    assert_eq!(signed[0].source, "attestation");
    let record = &e.attestations[&signed[0].r#ref];
    assert!(record.signed);
    assert_eq!(
        record.verified_by.as_deref(),
        Some("gh attestation verify (test)")
    );
    assert!(record.file.ends_with("provenance.dsse.json#1"));
    assert!(
        op.provenance
            .iter()
            .any(|p| p.kind == EvidenceKind::Declared)
    );
    let m = manifest::evaluate(e.clone(), Policy::default()).unwrap();
    assert!(m.findings.is_empty());
    // A policy that requires signed authorship evidence is satisfied here...
    let strict = Policy {
        minimum_authorship_evidence: EvidenceKind::Signed,
        ..Policy::default()
    };
    let m = manifest::evaluate(e, strict.clone()).unwrap();
    assert!(m.findings.is_empty());
    // ...and not by the same attestation without a stated verifier: the claims are declared.
    let e = collect("attested-unverified").unwrap();
    let op = e.changes[0].agent_operator.as_ref().unwrap();
    assert!(op.provenance.iter().all(|p| p.kind != EvidenceKind::Signed));
    assert!(
        e.attestations
            .values()
            .all(|a| a.signed && a.verified_by.is_none())
    );
    let m = manifest::evaluate(e, strict).unwrap();
    let acc006 = m
        .findings
        .iter()
        .find(|f| f.rule_id == RuleId::Acc006)
        .unwrap();
    assert!(acc006.explanation.contains("below the policy minimum"));
    assert!(
        m.assessments
            .iter()
            .any(|a| a.rule_id == RuleId::Acc001 && a.status == Status::Unknown)
    );
    // An attestation alone (no declaration in the body) establishes explicit authorship.
    let e = collect("attested-only").unwrap();
    let c = &e.changes[0];
    assert_eq!(c.author.actor_id, "agent:claude-code");
    assert_eq!(
        c.agent_operator.as_ref().unwrap().confidence,
        Confidence::Explicit
    );
    assert!(
        c.author
            .provenance
            .iter()
            .all(|p| p.source == "attestation")
    );
    // An attestation naming another agent than the declaration is an error, never a guess.
    let err = collect("attested-conflict").unwrap_err();
    assert!(
        err.to_string()
            .contains("conflicting attested and declared agent")
    );
}

#[test]
fn review_attestations_upgrade_forge_reviews_and_never_create_them() {
    // bob's forge approval of the head, attested by bob and verified: signed review evidence,
    // no agent facts (bob is human), the label collected, vendors on agents.
    let e = collect("review-human").unwrap();
    let c = &e.changes[0];
    assert_eq!(c.labels, vec!["low-risk".to_string()]);
    assert_eq!(
        e.actors["agent:claude-code"].vendor.as_deref(),
        Some("anthropic")
    );
    assert!(e.actors["github:bob"].vendor.is_none());
    let review = &c.reviews[0];
    assert_eq!(review.actor_id, "github:bob");
    assert!(review.agent.is_none());
    let signed: Vec<_> = review
        .provenance
        .iter()
        .filter(|p| p.kind == EvidenceKind::Signed)
        .collect();
    assert_eq!(signed.len(), 1);
    let record = &e.attestations[&signed[0].r#ref];
    assert!(record.matched);
    assert_eq!(
        record.signer.as_ref().map(|s| s.identity.as_str()),
        Some("bob@example.com")
    );
    // The strict review policy is now satisfiable by a human review.
    let strict = Policy {
        minimum_review_evidence: EvidenceKind::Signed,
        ..Policy::default()
    };
    let m = manifest::evaluate(e, strict).unwrap();
    assert!(m.findings.is_empty());
    manifest::validate(&m).unwrap();

    // bob mapped as an agent reviewer: the agent facts come from the attestation, the operator
    // and instructions owner resolve to a known human, the identity is the verified signer,
    // and the vendor comes from the caller's registry.
    let known = BTreeMap::from([("bob".to_string(), "review-agent".to_string())]);
    let e = collect_with("review-agent", &known).unwrap();
    assert_eq!(e.actors["github:bob"].kind, ActorKind::Agent);
    assert_eq!(e.actors["github:bob"].vendor.as_deref(), Some("acme"));
    let agent = e.changes[0].reviews[0].agent.as_ref().unwrap();
    assert_eq!(agent.operator.as_deref(), Some("github:alice"));
    assert_eq!(agent.instructions_owner.as_deref(), Some("github:alice"));
    assert!(agent.identity.as_deref().unwrap().contains("review-bot"));
    assert_eq!(agent.model.as_deref(), Some("claude-opus-5"));
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    manifest::validate(&m).unwrap();
    // Under the default policy an agent approval still does not qualify; ACC007 records it.
    assert_eq!(
        m.findings.iter().map(|f| f.rule_id).collect::<Vec<_>>(),
        vec![RuleId::Acc001, RuleId::Acc003, RuleId::Acc007]
    );

    // A decision the forge never showed is recorded as unmatched and creates no review.
    let e = collect("review-unmatched").unwrap();
    assert_eq!(e.changes[0].reviews.len(), 1);
    assert_eq!(e.changes[0].reviews[0].state, ReviewState::Approved);
    let records: Vec<_> = e.attestations.values().collect();
    assert_eq!(records.len(), 1);
    assert!(!records[0].matched);
    manifest::validate(&manifest::evaluate(e, Policy::default()).unwrap()).unwrap();

    // An attestation calling a human account an agent is an error, not a reclassification.
    let err = collect("review-kind-mismatch").unwrap_err();
    assert!(err.to_string().contains("reviewer kind"));
}

#[test]
fn review_attestations_must_agree_with_account_mappings() {
    // bob is verified as the agent `review-agent`; a review document naming another agent for
    // the same account is an error, never a choice between the two.
    let known = BTreeMap::from([("bob".to_string(), "another-agent".to_string())]);
    let err = collect_with("review-agent", &known).unwrap_err();
    assert!(err.to_string().contains("names another agent"), "{err}");
}

#[test]
fn human_review_attestations_name_no_operator_or_instructions() {
    // Only an agent reviewer has an operator or instructions (spec §3.7); a human reviewer's
    // document that names one is an error for the change.
    let err = collect("review-human-with-operator").unwrap_err();
    assert!(
        err.to_string()
            .contains("review attestation for a human names an operator or instructions"),
        "{err}"
    );
}

#[test]
fn declarations_outrank_trailers() {
    // The clean fixture carries both a declaration and a Claude trailer.
    let e = collect("clean").unwrap();
    let op = e.changes[0].agent_operator.as_ref().unwrap();
    assert_eq!(op.confidence, Confidence::Explicit);
    assert!(
        e.changes[0]
            .author
            .provenance
            .iter()
            .all(|p| p.source != "commit_trailer")
    );
}

#[test]
fn vendor_trailers_derive_agent_authorship_and_operator() {
    let e = collect("trailer-only").unwrap();
    let c = &e.changes[0];
    assert_eq!(c.author.actor_id, "agent:claude-code");
    assert_eq!(c.forge_author.actor_id, "github:alice");
    let evidence = &c.author.provenance;
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].source, "commit_trailer");
    assert_eq!(evidence[0].kind, EvidenceKind::Derived);
    assert!(evidence[0].r#ref.ends_with("/repos/acme/api/commits/head"));
    let op = c.agent_operator.as_ref().unwrap();
    assert_eq!(op.actor_id.as_deref(), Some("github:alice"));
    assert_eq!(op.confidence, Confidence::Derived);
    // Bob approved the head: independent of the derived operator, so clean.
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    assert!(m.findings.is_empty());
    assert_eq!(m.summary.agent_authored, 1);

    // A bot committed the trailer-bearing commit: agent authorship, operator unknown.
    let e = collect("trailer-bot-author").unwrap();
    let c = &e.changes[0];
    assert_eq!(c.author.actor_id, "agent:claude-code");
    let op = c.agent_operator.as_ref().unwrap();
    assert!(op.actor_id.is_none());
    assert_eq!(op.confidence, Confidence::Unknown);
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    assert_eq!(
        m.findings.iter().map(|f| f.rule_id).collect::<Vec<_>>(),
        vec![RuleId::Acc006]
    );

    // Two vendors in the trailers: not interpreted, the forge author stays effective.
    let e = collect("trailer-two-agents").unwrap();
    assert_eq!(e.changes[0].author.actor_id, "github:alice");
    assert!(e.changes[0].agent_operator.is_none());

    // An Agent Trace record bound to the head commit names the tool; same derived tier.
    let e = collect("trace-only").unwrap();
    let c = &e.changes[0];
    assert_eq!(c.author.actor_id, "agent:cursor");
    assert_eq!(c.author.provenance.len(), 1);
    assert_eq!(c.author.provenance[0].source, "agent_trace");
    assert_eq!(c.author.provenance[0].kind, EvidenceKind::Derived);
    assert!(
        c.author.provenance[0]
            .r#ref
            .ends_with("cursor.json#550e8400-e29b-41d4-a716-446655440000")
    );
    let op = c.agent_operator.as_ref().unwrap();
    assert_eq!(op.actor_id.as_deref(), Some("github:alice"));
    assert_eq!(op.confidence, Confidence::Derived);

    // A cursor record and a Claude trailer on the same change disagree: not interpreted.
    let e = collect("trace-and-trailer").unwrap();
    assert_eq!(e.changes[0].author.actor_id, "github:alice");

    // Trailers switched off: same as before this tier existed.
    let e = collect("trailers-ignored").unwrap();
    assert_eq!(e.changes[0].author.actor_id, "github:alice");
    assert!(e.changes[0].agent_operator.is_none());
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
        vec![RuleId::Acc001, RuleId::Acc003, RuleId::Acc007]
    );
}

/// `acc` against the replay server in `mode`, through the testing-only API base variable.
fn acc_against(server: &Server) -> assert_cmd::Command {
    let mut cmd = common::acc();
    cmd.env("ACC_GITHUB_API_URL", &server.base);
    cmd
}

const SCAN: [&str; 7] = [
    "scan",
    "github",
    "acme/api",
    "--since",
    "2026-08-01",
    "--until",
    "2026-08-31",
];

#[test]
fn failures_print_a_hint_and_a_docs_link() {
    let docs = "https://github.com/noru-tech/agent-change-control/blob/main/docs/exit-codes.md";
    for (mode, code, help, anchor) in [
        (
            "unauthorized",
            5,
            "help: set GITHUB_TOKEN or GH_TOKEN",
            "#5-authentication-rejected",
        ),
        (
            "rate-limit",
            6,
            "limited to 60 an hour",
            "#6-api-permission-rate-limit-or-transport-failure",
        ),
        (
            "rate-limit-header",
            6,
            "limited to 60 an hour",
            "#6-api-permission-rate-limit-or-transport-failure",
        ),
        (
            "forbidden",
            6,
            "refused an unauthenticated request",
            "#6-api-permission-rate-limit-or-transport-failure",
        ),
        (
            "not-found",
            6,
            "a private repository looks missing",
            "#6-api-permission-rate-limit-or-transport-failure",
        ),
    ] {
        let server = Server::start(mode);
        for args in [
            &SCAN[..],
            &[
                "export",
                "github",
                "acme/api",
                "--since",
                "2026-08-01",
                "--until",
                "2026-08-31",
            ],
            &["pr", "421", "--repo", "acme/api"],
        ] {
            let out = acc_against(&server).args(args).output().unwrap();
            let stderr = String::from_utf8(out.stderr).unwrap();
            assert_eq!(out.status.code(), Some(code), "{mode} {args:?}: {stderr}");
            assert!(out.stdout.is_empty(), "{mode}");
            let lines: Vec<&str> = stderr.lines().collect();
            assert!(lines[0].starts_with("error: GitHub "), "{stderr}");
            assert!(
                lines[1].starts_with("help: ") && lines[1].contains(help),
                "{mode}: {stderr}"
            );
            assert_eq!(lines[2], format!("see: {docs}{anchor}"), "{mode}");
            assert!(!stderr.contains("127.0.0.1"), "{stderr}");
        }
    }
}

#[test]
fn incomplete_collection_warns_with_a_hint() {
    let server = Server::start("capped");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("m.json");
    acc_against(&server)
        .args(SCAN)
        .args(["--max-pages", "1", "-o"])
        .arg(&out)
        .assert()
        .code(4)
        .stderr(predicates::str::contains(
            "warning: collection incomplete: Pull request pagination limit reached",
        ))
        .stderr(predicates::str::contains("help: raise --max-pages"))
        .stderr(predicates::str::contains(
            "see: https://github.com/noru-tech/agent-change-control/blob/main/docs/exit-codes.md#4-collection-incomplete",
        ));
    assert!(out.exists());
    acc_against(&server)
        .args(["-q"])
        .args(SCAN)
        .args(["--max-pages", "1", "-o"])
        .arg(&out)
        .assert()
        .code(4)
        .stderr("");
    acc_against(&server)
        .args(["check"])
        .arg(&out)
        .assert()
        .code(4)
        .stderr(predicates::str::contains(
            "help: the input records an incomplete collection",
        ));
}

#[test]
fn the_testing_api_base_never_receives_a_token() {
    let server = Server::start("clean");
    acc_against(&server)
        .env("GITHUB_TOKEN", "ghp_should_never_be_sent")
        .args(SCAN)
        .assert()
        .code(2)
        .stderr(predicates::str::contains("unsupported API origin"))
        .stderr(predicates::str::contains("ghp_should_never_be_sent").not());
    common::acc()
        .env("ACC_GITHUB_API_URL", "https://evil.example")
        .args(SCAN)
        .assert()
        .code(2);
}

/// Run `acc` and return (stdout bytes, stderr text), requiring exit `code`.
fn output(cmd: &mut assert_cmd::Command, code: i32) -> (Vec<u8>, String) {
    let out = cmd.output().unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert_eq!(out.status.code(), Some(code), "{stderr}");
    (out.stdout, stderr)
}

#[test]
fn zero_config_scan_records_the_defaults_as_if_they_were_passed() {
    let server = Server::start("clean");
    let explicit = [
        "acme/api",
        "--since",
        "2026-08-01T23:00:00Z",
        "--until",
        "2026-08-31T23:00:00Z",
    ];
    for command in ["scan", "export"] {
        let (defaulted, stderr) = output(
            acc_against(&server)
                .env("ACC_NOW", "2026-08-31T23:00:00.75Z")
                .env("GITHUB_REPOSITORY", "acme/api")
                .args([command, "-f", "json"]),
            0,
        );
        assert!(
            stderr.contains("repository: acme/api (from GITHUB_REPOSITORY)"),
            "{stderr}"
        );
        assert!(
            stderr.contains(
                "window: 2026-08-01T23:00:00Z to 2026-08-31T23:00:00Z (UTC; the last 30 days; pass --since/--until to choose)"
            ),
            "{stderr}"
        );
        let (passed, stderr) = output(
            acc_against(&server)
                .args([command, "github"])
                .args(explicit)
                .args(["-f", "json"]),
            0,
        );
        assert!(!stderr.contains("window:"), "{stderr}");
        assert_eq!(defaulted, passed, "{command}");
        let value: Value = serde_json::from_slice(&defaulted).unwrap();
        let window = if command == "scan" {
            &value["events"]["window"]
        } else {
            &value["window"]
        };
        assert_eq!(window["from"], "2026-08-01T23:00:00Z");
        assert_eq!(window["to"], "2026-08-31T23:00:00Z");
    }
    // One edge given: the other is filled in from it.
    let (_, stderr) = output(
        acc_against(&server).args(["scan", "acme/api", "--until", "2026-08-31", "-f", "json"]),
        0,
    );
    assert!(
        stderr.contains("window: 2026-08-01T23:59:59.999999999Z to 2026-08-31T23:59:59.999999999Z (UTC; 30 days before --until"),
        "{stderr}"
    );
    output(
        acc_against(&server)
            .env("ACC_NOW", "not a time")
            .args(["scan", "acme/api", "-f", "json"]),
        2,
    );
}

#[test]
fn the_repository_is_inferred_from_the_origin_remote() {
    let server = Server::start("clean");
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
    git(&["remote", "add", "origin", "git@github.com:acme/api.git"]);
    let (_, stderr) = output(
        acc_against(&server)
            .env_remove("GIT_DIR")
            .current_dir(dir.path())
            .env("ACC_NOW", "2026-08-31T23:00:00Z")
            .args(["scan", "-f", "json"]),
        0,
    );
    assert!(
        stderr.contains("repository: acme/api (from the origin remote)"),
        "{stderr}"
    );
    let (_, stderr) = output(
        acc_against(&server)
            .env_remove("GIT_DIR")
            .current_dir(dir.path())
            .args(["pr", "421", "-f", "json"]),
        0,
    );
    assert!(
        stderr.contains("repository: acme/api (from the origin remote)"),
        "{stderr}"
    );
    git(&[
        "remote",
        "set-url",
        "origin",
        "https://gitlab.com/acme/api.git",
    ]);
    let (_, stderr) = output(
        acc_against(&server)
            .env_remove("GIT_DIR")
            .current_dir(dir.path())
            .env("GIT_CEILING_DIRECTORIES", dir.path().parent().unwrap())
            .args(["scan"]),
        2,
    );
    assert!(
        stderr.contains("error: no repository given and none detected"),
        "{stderr}"
    );
    assert!(
        stderr.contains("help: pass OWNER/REPO, set GITHUB_REPOSITORY"),
        "{stderr}"
    );
}

/// `acc doctor` output as JSON, requiring exit `code`.
fn doctor(cmd: &mut assert_cmd::Command, code: i32) -> Value {
    let (stdout, _) = output(cmd.args(["doctor", "--online", "--format", "json"]), code);
    serde_json::from_slice(&stdout).unwrap()
}

fn check<'a>(report: &'a Value, name: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no {name} check in {report}"))
}

#[test]
fn doctor_online_checks_the_rate_limit_and_the_latest_release() {
    let server = Server::start("clean");
    let report = doctor(&mut acc_against(&server), 0);
    assert_eq!(report["healthy"], true);
    assert_eq!(check(&report, "github")["status"], "ok");
    assert_eq!(
        check(&report, "github")["detail"],
        "unauthenticated: 59 of 60 requests left this hour (resets 2026-09-21T14:13:20Z)"
    );
    assert_eq!(check(&report, "update")["status"], "ok");
    assert!(
        check(&report, "update")["detail"]
            .as_str()
            .unwrap()
            .starts_with("up to date")
    );

    let server = Server::start("newer-release");
    let report = doctor(&mut acc_against(&server), 0);
    assert_eq!(check(&report, "update")["status"], "warn");
    assert!(
        check(&report, "update")["detail"]
            .as_str()
            .unwrap()
            .starts_with("acc 99.0.0 is available")
    );

    let server = Server::start("rate-exhausted");
    let report = doctor(&mut acc_against(&server), 0);
    assert_eq!(check(&report, "github")["status"], "warn");

    let server = Server::start("unauthorized");
    let report = doctor(&mut acc_against(&server), 2);
    assert_eq!(report["healthy"], false);
    assert_eq!(check(&report, "github")["status"], "fail");
    assert_eq!(
        check(&report, "github")["detail"],
        "GitHub authentication failed"
    );
    assert!(
        check(&report, "github")["hint"]
            .as_str()
            .unwrap()
            .contains("GITHUB_TOKEN")
    );

    // Text: the same checks, one line each, and a verdict.
    let server = Server::start("clean");
    let (stdout, _) = output(acc_against(&server).args(["doctor", "--online"]), 0);
    let text = String::from_utf8(stdout).unwrap();
    assert!(
        text.contains("ok    github      unauthenticated: 59 of 60"),
        "{text}"
    );
    assert!(text.ends_with("healthy\n"), "{text}");
}

#[test]
fn doctor_never_prints_the_token() {
    let secret = "ghp_doctorMustNeverPrintThis0123456789";
    let server = Server::start("clean");
    for args in [
        &["doctor"][..],
        &["doctor", "--format", "json"],
        &["-v", "doctor", "--online"],
        &["doctor", "--online", "--format", "json"],
    ] {
        for var in ["GITHUB_TOKEN", "GH_TOKEN"] {
            let out = acc_against(&server)
                .env(var, secret)
                .args(args)
                .output()
                .unwrap();
            let all = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(!all.contains(secret), "{args:?}: {all}");
            assert!(all.contains(&format!("{var} is set")), "{args:?}: {all}");
        }
    }
}
