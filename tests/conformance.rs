//! The evaluator conformance corpus under `conformance/` (spec §9), and the checks that keep it
//! honest.
//!
//! Every vector is generated from the table below: fixture-backed vectors take their events
//! from `tests/fixtures`, variants edit a policy or one fact, and every reject vector is one
//! mutation of an accept vector. The failing rule codes of every accept and incomplete vector
//! are declared in the table (and, for fixture-backed ones, must equal the fixture's reviewed
//! `expected-rules.json`), so the answer key is reviewed rather than copied from `acc`. The
//! assessments and manifest digest are then recorded from the reference implementation.
//!
//! `UPDATE_CORPUS=1 cargo test --test conformance` regenerates the vectors and `MANIFEST.json`;
//! without it the test requires the committed corpus to match byte for byte. Run
//! `python3 conformance/digests.py` afterwards to regenerate `CORPUS-DIGESTS.txt`.

mod common;

use agent_change_control::Exit;
use agent_change_control::cli::conformance;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const SUITE: &str = "acp-evaluator-conformance";
const SUITE_REVISION: u64 = 2;
const SPEC_VERSION: &str = "0.3";
const SPEC_PATH: &str = "spec/ai-change-provenance.md";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Accept,
    Reject,
    Incomplete,
}

impl Kind {
    fn dir(self) -> &'static str {
        match self {
            Kind::Accept => "accept",
            Kind::Reject => "reject",
            Kind::Incomplete => "incomplete",
        }
    }
}

/// How a reject vector differs from its accept parent.
enum Mutation {
    /// An edit of the parsed vector.
    Json(fn(&mut Value)),
    /// An edit of the parent's text, for violations a parsed value cannot express.
    Text(fn(&str) -> String),
}

struct Evaluated {
    id: &'static str,
    kind: Kind,
    fixture: &'static str,
    /// Edits applied to the vector `{events, policy}` built from the fixture.
    edit: Option<fn(&mut Value)>,
    fails: &'static [&'static str],
    conditions: &'static [&'static str],
}

struct Reject {
    id: &'static str,
    parent: &'static str,
    mutation: Mutation,
    codes: &'static [&'static str],
    conditions: &'static [&'static str],
}

// Spec section anchors, as GitHub renders them.
const INDEPENDENCE: &str = "#61-independence";
const RULES: &str = "#62-rules";
const POLICY: &str = "#63-policy";
const DIMENSIONS: &str = "#65-independence-dimensions";
const VALIDATION: &str = "#66-validation";
const INLINE: &str = "#31-inline-declaration";
const DERIVED: &str = "#34-derived-evidence-records-written-at-authoring-time";
const STRENGTH: &str = "#36-evidence-strength";
const COLLECTION: &str = "#4-collection";
const ACTORS: &str = "#5-actor-classification";
const NORMALIZATION: &str = "#81-normalization";
const SERIALIZATION: &str = "#82-serialization";

fn policy(v: &mut Value) -> &mut Value {
    &mut v["policy"]
}

fn change(v: &mut Value) -> &mut Value {
    &mut v["events"]["changes"][0]
}

const fn fixture(
    id: &'static str,
    fails: &'static [&'static str],
    conditions: &'static [&'static str],
) -> Evaluated {
    Evaluated {
        id,
        kind: Kind::Accept,
        fixture: id,
        edit: None,
        fails,
        conditions,
    }
}

fn evaluated() -> Vec<Evaluated> {
    vec![
        fixture("human-clean", &[], &[INDEPENDENCE, RULES]),
        fixture("human-self-approved", &["ACC002", "ACC003"], &[RULES]),
        fixture("self-and-independent", &["ACC002"], &[INDEPENDENCE, RULES]),
        fixture("claude-clean", &[], &[INLINE, INDEPENDENCE]),
        fixture(
            "claude-operator-self-approved",
            &["ACC001", "ACC002", "ACC003"],
            &[INLINE, RULES],
        ),
        fixture("claude-trailer-derived", &[], &[DERIVED, INDEPENDENCE]),
        fixture(
            "claude-trailer-self-approved",
            &["ACC001", "ACC002", "ACC003"],
            &[DERIVED, RULES],
        ),
        fixture("codex-clean", &[], &[INLINE, INDEPENDENCE]),
        fixture(
            "agent-operator-unknown",
            &["ACC006"],
            &[RULES, INDEPENDENCE],
        ),
        fixture(
            "unknown-without-review",
            &["ACC006"],
            &[RULES, INDEPENDENCE],
        ),
        fixture("bot-approval", &["ACC003"], &[ACTORS, INDEPENDENCE]),
        fixture("changes-requested", &["ACC003"], &[INDEPENDENCE]),
        fixture("dismissed-approval", &["ACC003"], &[INDEPENDENCE]),
        fixture("comment-after-approval", &[], &[INDEPENDENCE]),
        fixture("stale-approval", &["ACC003"], &[INDEPENDENCE]),
        fixture("approval-at-merge", &[], &[INDEPENDENCE]),
        fixture("open-pr", &[], &[INDEPENDENCE, RULES]),
        fixture("timezone-equivalent", &[], &[NORMALIZATION, INDEPENDENCE]),
        fixture(
            "agent-review-declared",
            &["ACC001", "ACC003", "ACC007"],
            &[RULES, INDEPENDENCE],
        ),
        fixture(
            "agent-review-same-vendor",
            &["ACC001", "ACC003", "ACC007", "ACC008"],
            &[RULES, ACTORS],
        ),
        fixture(
            "agent-review-independent",
            &["ACC007"],
            &[POLICY, DIMENSIONS, STRENGTH],
        ),
        fixture(
            "agent-review-dependent-operator",
            &["ACC001", "ACC003", "ACC007", "ACC009"],
            &[POLICY, DIMENSIONS],
        ),
        fixture(
            "agent-review-same-identity",
            &["ACC001", "ACC003", "ACC007", "ACC009"],
            &[POLICY, DIMENSIONS],
        ),
        fixture(
            "agent-review-unknown-operator",
            &["ACC007"],
            &[POLICY, DIMENSIONS, INDEPENDENCE],
        ),
        fixture(
            "agent-review-unknown-vendor",
            &["ACC007"],
            &[POLICY, DIMENSIONS, RULES],
        ),
        fixture(
            "agent-review-unsigned",
            &["ACC001", "ACC003", "ACC007", "ACC010"],
            &[POLICY, RULES, STRENGTH],
        ),
        fixture(
            "agent-review-out-of-scope",
            &["ACC001", "ACC003", "ACC007"],
            &[POLICY],
        ),
        // Evidence minimums: each tier decides an outcome somewhere.
        Evaluated {
            id: "claude-trailer-derived--minimum-authorship-declared",
            kind: Kind::Accept,
            fixture: "claude-trailer-derived",
            edit: Some(|v| policy(v)["minimum_authorship_evidence"] = "declared".into()),
            fails: &["ACC006"],
            conditions: &[STRENGTH, POLICY, INDEPENDENCE],
        },
        Evaluated {
            id: "claude-clean--minimum-authorship-observed",
            kind: Kind::Accept,
            fixture: "claude-clean",
            edit: Some(|v| policy(v)["minimum_authorship_evidence"] = "observed".into()),
            fails: &["ACC006"],
            conditions: &[STRENGTH, POLICY, INDEPENDENCE],
        },
        Evaluated {
            id: "claude-clean--minimum-review-signed",
            kind: Kind::Accept,
            fixture: "claude-clean",
            edit: Some(|v| policy(v)["minimum_review_evidence"] = "signed".into()),
            fails: &["ACC001", "ACC003"],
            conditions: &[STRENGTH, POLICY, INDEPENDENCE],
        },
        Evaluated {
            id: "claude-clean--signed-authorship",
            kind: Kind::Accept,
            fixture: "claude-clean",
            edit: Some(|v| {
                sign_authorship(v);
                policy(v)["minimum_authorship_evidence"] = "signed".into();
            }),
            fails: &[],
            conditions: &[STRENGTH, POLICY],
        },
        Evaluated {
            id: "human-self-approved--rule-disabled",
            kind: Kind::Accept,
            fixture: "human-self-approved",
            edit: Some(|v| policy(v)["rules"]["approver_is_author"]["enabled"] = false.into()),
            fails: &["ACC003"],
            conditions: &[POLICY],
        },
        // The opt-in instructions dimension (spec §6.5): independent, dependent and unknown.
        Evaluated {
            id: "agent-review-independent--instructions-required",
            kind: Kind::Accept,
            fixture: "agent-review-independent",
            edit: Some(require_instructions),
            fails: &["ACC007"],
            conditions: &[POLICY, DIMENSIONS],
        },
        Evaluated {
            id: "agent-review-instructions-dependent",
            kind: Kind::Accept,
            fixture: "agent-review-independent",
            edit: Some(|v| {
                require_instructions(v);
                change(v)["reviews"][0]["agent"]["instructions_owner"] = "github:alice".into();
            }),
            fails: &["ACC001", "ACC003", "ACC007", "ACC009"],
            conditions: &[POLICY, DIMENSIONS],
        },
        Evaluated {
            id: "agent-review-instructions-unknown",
            kind: Kind::Accept,
            fixture: "agent-review-independent",
            edit: Some(|v| {
                require_instructions(v);
                change(v)["reviews"][0]["agent"]["instructions_owner"] = Value::Null;
            }),
            fails: &["ACC007"],
            conditions: &[POLICY, DIMENSIONS, INDEPENDENCE],
        },
        // Incomplete collection: exit 4, never a clean result, even with an approval on record.
        Evaluated {
            id: "incomplete-window",
            kind: Kind::Incomplete,
            fixture: "incomplete-window",
            edit: None,
            fails: &[],
            conditions: &[COLLECTION, RULES],
        },
        Evaluated {
            id: "partial-with-approval",
            kind: Kind::Incomplete,
            fixture: "partial-with-approval",
            edit: None,
            fails: &[],
            conditions: &[COLLECTION, INDEPENDENCE],
        },
        Evaluated {
            id: "claude-clean--reviews-incomplete",
            kind: Kind::Incomplete,
            fixture: "claude-clean",
            edit: Some(|v| change(v)["reviews_complete"] = false.into()),
            fails: &[],
            conditions: &[COLLECTION, INDEPENDENCE],
        },
        Evaluated {
            id: "human-self-approved--reviews-incomplete",
            kind: Kind::Incomplete,
            fixture: "human-self-approved",
            edit: Some(|v| change(v)["reviews_complete"] = false.into()),
            fails: &["ACC002"],
            conditions: &[COLLECTION, RULES],
        },
    ]
}

/// Require every independence dimension, including the opt-in `instructions` (spec §6.5).
fn require_instructions(v: &mut Value) {
    policy(v)["agent_review"]["require"] =
        json!(["identity", "instructions", "operator", "provider"]);
}

/// Record a verified provenance attestation as the operator's signed evidence.
fn sign_authorship(v: &mut Value) {
    let id = "attestation:0123456789abcdef";
    v["events"]["attestations"] = json!({
        id: {
            "file": "provenance.sigstore.json#1",
            "predicate_type": "https://noru.tech/spec/ai-change-provenance/provenance/v0.1",
            "payload_digest": format!("sha256:0123456789abcdef{}", "0".repeat(48)),
            "signed": true,
            "verified_by": "gh attestation verify (conformance vector)",
            "signer": null,
            "matched": true
        }
    });
    change(v)["agent_operator"]["provenance"]
        .as_array_mut()
        .unwrap()
        .push(json!({"source": "attestation", "ref": id, "kind": "signed"}));
}

/// The text of `"display_name": "Bob"` in a pretty-printed vector, the site of the lexical
/// mutations.
const BOB: &str = "\"display_name\": \"Bob\"";

fn rejects() -> Vec<Reject> {
    vec![
        Reject {
            id: "approval-after-merge",
            parent: "approval-at-merge",
            mutation: Mutation::Json(|v| {
                change(v)["reviews"][0]["at"] = "2026-08-03T12:00:01Z".into()
            }),
            codes: &["ACV001"],
            conditions: &[VALIDATION, INDEPENDENCE],
        },
        Reject {
            id: "human-clean--unresolved-author",
            parent: "human-clean",
            mutation: Mutation::Json(|v| change(v)["author"]["actor_id"] = "github:nobody".into()),
            codes: &["ACV003"],
            conditions: &[VALIDATION],
        },
        Reject {
            id: "human-clean--operator-on-human-author",
            parent: "human-clean",
            mutation: Mutation::Json(|v| {
                change(v)["agent_operator"] = json!({
                    "actor_id": "github:bob",
                    "confidence": "explicit",
                    "provenance": [{"source": "pr_metadata", "ref": "x", "kind": "declared"}]
                })
            }),
            codes: &["ACV003"],
            conditions: &[VALIDATION],
        },
        Reject {
            id: "claude-clean--signed-without-verifier",
            parent: "claude-clean--signed-authorship",
            mutation: Mutation::Json(|v| {
                v["events"]["attestations"]["attestation:0123456789abcdef"]["verified_by"] =
                    Value::Null
            }),
            codes: &["ACV003"],
            conditions: &[VALIDATION, STRENGTH],
        },
        Reject {
            id: "human-clean--review-before-opening",
            parent: "human-clean",
            mutation: Mutation::Json(|v| {
                change(v)["reviews"][0]["at"] = "2026-08-01T11:59:59Z".into()
            }),
            codes: &["ACV004"],
            conditions: &[VALIDATION],
        },
        Reject {
            id: "comment-after-approval--duplicate-review-id",
            parent: "comment-after-approval",
            mutation: Mutation::Json(|v| change(v)["reviews"][1]["id"] = "1".into()),
            codes: &["ACV004"],
            conditions: &[VALIDATION],
        },
        Reject {
            id: "open-pr--merge-commit-unmerged",
            parent: "open-pr",
            mutation: Mutation::Json(|v| change(v)["merge_commit_sha"] = "merge".into()),
            codes: &["ACV004"],
            conditions: &[VALIDATION],
        },
        Reject {
            id: "human-clean--window-reversed",
            parent: "human-clean",
            mutation: Mutation::Json(|v| {
                v["events"]["window"]["from"] = "2026-09-01T00:00:00Z".into()
            }),
            codes: &["ACV004"],
            conditions: &[VALIDATION],
        },
        Reject {
            id: "human-clean--unknown-member",
            parent: "human-clean",
            mutation: Mutation::Json(|v| v["events"]["surprise"] = true.into()),
            codes: &["ACV010"],
            conditions: &[VALIDATION],
        },
        Reject {
            id: "human-clean--unknown-actor-kind",
            parent: "human-clean",
            mutation: Mutation::Json(|v| {
                v["events"]["actors"]["github:bob"]["kind"] = "robot".into()
            }),
            codes: &["ACV010"],
            conditions: &[VALIDATION, ACTORS],
        },
        Reject {
            id: "human-clean--unknown-policy-rule",
            parent: "human-clean",
            mutation: Mutation::Json(|v| {
                policy(v)["rules"]["vibes"] = json!({"enabled": true, "severity": "high"})
            }),
            codes: &["ACV010"],
            conditions: &[VALIDATION, POLICY],
        },
        Reject {
            id: "human-clean--extra-vector-member",
            parent: "human-clean",
            mutation: Mutation::Json(|v| v["expected"] = json!({"verdict": "evaluated"})),
            codes: &["ACV010"],
            conditions: &[VALIDATION],
        },
        Reject {
            id: "agent-review-independent--lowered-agent-minimum",
            parent: "agent-review-independent",
            mutation: Mutation::Json(|v| {
                policy(v)["agent_review"]["minimum_evidence"] = "declared".into()
            }),
            codes: &["ACV010"],
            conditions: &[VALIDATION, POLICY],
        },
        Reject {
            id: "human-clean--non-integer",
            parent: "human-clean",
            mutation: Mutation::Text(|t| t.replacen(BOB, "\"display_name\": 1.5", 1)),
            codes: &["ACV005"],
            conditions: &[SERIALIZATION],
        },
        Reject {
            id: "human-clean--integer-range",
            parent: "human-clean",
            mutation: Mutation::Text(|t| t.replacen(BOB, "\"display_name\": 9007199254740992", 1)),
            codes: &["ACV006"],
            conditions: &[SERIALIZATION],
        },
        Reject {
            id: "human-clean--unpaired-surrogate",
            parent: "human-clean",
            mutation: Mutation::Text(|t| t.replacen(BOB, "\"display_name\": \"Bob\\ud800\"", 1)),
            codes: &["ACV007"],
            conditions: &[SERIALIZATION],
        },
        Reject {
            id: "human-clean--duplicate-member",
            parent: "human-clean",
            mutation: Mutation::Text(|t| {
                t.replacen(
                    BOB,
                    "\"display_name\": \"Bob\",\n        \"display_name\": \"Bob\"",
                    1,
                )
            }),
            codes: &["ACV008"],
            conditions: &[SERIALIZATION],
        },
        Reject {
            id: "human-clean--depth-129",
            parent: "human-clean",
            // The vector object is depth 1, events 2, actors 3 and github:bob 4, so 125 nested
            // arrays in place of the name reach depth 129.
            mutation: Mutation::Text(|t| {
                t.replacen(
                    BOB,
                    &format!("\"display_name\": {}{}", "[".repeat(125), "]".repeat(125)),
                    1,
                )
            }),
            codes: &["ACV009"],
            conditions: &[SERIALIZATION],
        },
    ]
}

/// The `unknown` outcomes the rules can produce, as (rule, reason). Every one must be exercised
/// by some vector, because an unknown must never be rounded to pass or fail.
const UNKNOWN_PATHS: &[(&str, &str)] = &[
    ("ACC001", HUMAN_UNKNOWN),
    ("ACC002", HUMAN_UNKNOWN),
    ("ACC003", HUMAN_UNKNOWN),
    ("ACC001", HUMAN_BELOW_MINIMUM),
    ("ACC002", HUMAN_BELOW_MINIMUM),
    ("ACC003", HUMAN_BELOW_MINIMUM),
    ("ACC002", "Review collection is incomplete."),
    ("ACC001", REVIEWS_INCOMPLETE),
    ("ACC003", REVIEWS_INCOMPLETE),
    ("ACC001", AGENT_UNKNOWN),
    ("ACC003", AGENT_UNKNOWN),
    (
        "ACC008",
        "The author agent's or an approving agent's vendor is unknown; same-vendor review cannot be established.",
    ),
    (
        "ACC009",
        "An agent approval of the current head is unknown on a required dimension and dependent on none.",
    ),
];
const HUMAN_UNKNOWN: &str =
    "Effective human author is unknown; independence cannot be established.";
const HUMAN_BELOW_MINIMUM: &str = "Effective human author's evidence is below the policy minimum; independence cannot be established.";
const REVIEWS_INCOMPLETE: &str =
    "Review collection is incomplete; missing independent approval cannot be established.";
const AGENT_UNKNOWN: &str = "An agent approved the current head, but its independence from the effective author is unknown on a required dimension.";

/// Vectors in which an evidence tier decides an outcome, by tier.
const TIER_VECTORS: &[(&str, &str)] = &[
    (
        "derived",
        "claude-trailer-derived--minimum-authorship-declared",
    ),
    ("declared", "claude-clean--minimum-authorship-observed"),
    ("observed", "claude-clean--minimum-review-signed"),
    ("signed", "claude-clean--signed-authorship"),
    ("signed", "agent-review-independent"),
];

struct Built {
    id: String,
    kind: Kind,
    text: String,
    expected: Value,
    parent: Option<String>,
    conditions: Vec<&'static str>,
}

fn root() -> PathBuf {
    common::root().join("conformance")
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap() + "\n"
}

/// The vector for a fixture: its events as recorded and its resolved policy.
fn vector_from(fixture: &str) -> Value {
    let dir = common::fixtures().join(fixture);
    let events: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("events.json")).unwrap()).unwrap();
    let path = dir.join("policy.yml");
    let policy = if path.exists() {
        let p: agent_change_control::model::Policy =
            serde_saphyr::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        agent_change_control::policy::resolve(p).unwrap()
    } else {
        agent_change_control::model::Policy::default()
    };
    json!({"events": events, "policy": policy})
}

fn run(text: &str) -> (Value, Exit) {
    let (object, exit, _) = conformance::evaluate(text).unwrap();
    (object, exit)
}

fn build() -> Vec<Built> {
    let mut out: Vec<Built> = Vec::new();
    for e in evaluated() {
        let mut v = vector_from(e.fixture);
        if let Some(edit) = e.edit {
            edit(&mut v);
        }
        let text = pretty(&v);
        let (expected, exit) = run(&text);
        let want = match e.kind {
            Kind::Incomplete => Exit::Incomplete,
            _ => Exit::Ok,
        };
        assert_eq!(exit, want, "{}: {expected}", e.id);
        assert_eq!(
            expected["codes"],
            json!(e.fails),
            "{}: declared failing rules",
            e.id
        );
        if e.edit.is_none() {
            let reviewed: Value = serde_json::from_str(
                &std::fs::read_to_string(common::fixture(e.fixture, "expected-rules.json"))
                    .unwrap(),
            )
            .unwrap();
            let rules: BTreeSet<&str> = reviewed["rules"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r.as_str().unwrap())
                .collect();
            assert_eq!(
                rules,
                e.fails.iter().copied().collect(),
                "{}: table disagrees with the fixture's reviewed expected-rules.json",
                e.id
            );
        }
        out.push(Built {
            id: e.id.into(),
            kind: e.kind,
            text,
            expected,
            parent: None,
            conditions: e.conditions.to_vec(),
        });
    }
    for r in rejects() {
        let parent = out
            .iter()
            .find(|b| b.id == r.parent && b.kind == Kind::Accept)
            .unwrap_or_else(|| panic!("{}: parent {} is not an accept vector", r.id, r.parent));
        let text = match r.mutation {
            Mutation::Json(f) => {
                let mut v: Value = serde_json::from_str(&parent.text).unwrap();
                f(&mut v);
                pretty(&v)
            }
            Mutation::Text(f) => {
                let t = f(&parent.text);
                assert_ne!(t, parent.text, "{}: the mutation did not apply", r.id);
                t
            }
        };
        let (object, exit) = run(&text);
        assert_eq!(exit, Exit::InvalidInput, "{}: {object}", r.id);
        let emitted: BTreeSet<&str> = object["codes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        assert!(
            r.codes.iter().any(|c| emitted.contains(c)),
            "{}: acc emitted {emitted:?}, expected one of {:?}",
            r.id,
            r.codes
        );
        out.push(Built {
            id: r.id.into(),
            kind: Kind::Reject,
            text,
            expected: json!({"verdict": "invalid", "codes": r.codes}),
            parent: Some(r.parent.into()),
            conditions: r.conditions.to_vec(),
        });
    }
    out
}

fn spec_digest() -> String {
    agent_change_control::canonical::sha256(&std::fs::read(common::root().join(SPEC_PATH)).unwrap())
}

fn manifest(built: &[Built]) -> Value {
    let count = |k: Kind| built.iter().filter(|b| b.kind == k).count();
    let vectors: Vec<Value> = built
        .iter()
        .map(|b| {
            let mut entry = Map::new();
            entry.insert("id".into(), b.id.clone().into());
            entry.insert(
                "path".into(),
                format!("{}/{}.json", b.kind.dir(), b.id).into(),
            );
            entry.insert("kind".into(), b.kind.dir().into());
            entry.insert("expected".into(), b.expected.clone());
            if let Some(p) = &b.parent {
                entry.insert("parent".into(), p.clone().into());
            }
            entry.insert("conditions".into(), json!(b.conditions));
            Value::Object(entry)
        })
        .collect();
    json!({
        "suite": SUITE,
        "suiteRevision": SUITE_REVISION,
        "specVersion": SPEC_VERSION,
        "specPath": SPEC_PATH,
        "specDigest": spec_digest(),
        "contract": "README.md#contract",
        "counts": {
            "accept": count(Kind::Accept),
            "reject": count(Kind::Reject),
            "incomplete": count(Kind::Incomplete),
        },
        "vectors": vectors,
    })
}

#[test]
fn corpus_is_generated_from_the_table() {
    let built = build();
    let manifest_text = pretty(&manifest(&built));
    let root = root();
    let update = std::env::var_os("UPDATE_CORPUS").is_some();
    if update {
        for kind in ["accept", "reject", "incomplete"] {
            let dir = root.join(kind);
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
        }
    }
    let mut expected_files = BTreeSet::new();
    for b in &built {
        let rel = format!("{}/{}.json", b.kind.dir(), b.id);
        let path = root.join(&rel);
        if update {
            std::fs::write(&path, &b.text).unwrap();
        }
        assert_eq!(
            std::fs::read_to_string(&path).unwrap_or_default(),
            b.text,
            "{rel} is stale; regenerate with UPDATE_CORPUS=1"
        );
        expected_files.insert(rel);
    }
    if update {
        std::fs::write(root.join("MANIFEST.json"), &manifest_text).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(root.join("MANIFEST.json")).unwrap_or_default(),
        manifest_text,
        "MANIFEST.json is stale; regenerate with UPDATE_CORPUS=1"
    );
    let readme = std::fs::read_to_string(root.join("README.md")).unwrap();
    assert!(
        readme.contains(&format!("suite revision {SUITE_REVISION},")),
        "conformance/README.md names another suite revision"
    );
    // No vector file exists that the table does not produce.
    for kind in ["accept", "reject", "incomplete"] {
        for entry in std::fs::read_dir(root.join(kind)).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                expected_files.contains(&format!("{kind}/{name}")),
                "{kind}/{name} is not generated by the table"
            );
        }
    }
}

#[test]
fn coverage_requirements_hold() {
    let built = build();
    let evaluated: Vec<&Built> = built.iter().filter(|b| b.kind != Kind::Reject).collect();
    let outcomes: BTreeSet<(String, String, String)> = evaluated
        .iter()
        .flat_map(|b| b.expected["assessments"].as_array().unwrap().iter())
        .map(|a| {
            (
                a["rule"].as_str().unwrap().to_string(),
                a["outcome"].as_str().unwrap().to_string(),
                String::new(),
            )
        })
        .collect();
    // Every rule fails somewhere and does not fail somewhere.
    for rule in [
        "ACC001", "ACC002", "ACC003", "ACC006", "ACC007", "ACC008", "ACC009", "ACC010",
    ] {
        assert!(
            outcomes.contains(&(rule.into(), "fail".into(), String::new())),
            "{rule} never fails"
        );
        assert!(
            outcomes.iter().any(|(r, o, _)| r == rule && o != "fail"),
            "{rule} never has a non-fail outcome"
        );
    }
    // Every unknown path is exercised. The contract's assessments carry no reason, so the
    // reasons are read from the manifests the reference implementation writes.
    let mut reached = BTreeSet::new();
    for b in &evaluated {
        let v: Value = serde_json::from_str(&b.text).unwrap();
        let events = serde_json::from_value(v["events"].clone()).unwrap();
        let policy = serde_json::from_value(v["policy"].clone()).unwrap();
        let m = agent_change_control::manifest::evaluate(events, policy).unwrap();
        for a in m.assessments {
            if a.status == agent_change_control::model::Status::Unknown {
                reached.insert((a.rule_id.code().to_string(), a.reason));
            }
        }
    }
    let listed: BTreeSet<(String, String)> = UNKNOWN_PATHS
        .iter()
        .map(|(r, why)| (r.to_string(), why.to_string()))
        .collect();
    assert_eq!(
        reached, listed,
        "the unknown paths reached differ from the list of every unknown path"
    );
    // Every evidence tier decides an outcome: the variant fails where its parent passes, or
    // (signed) passes where the tier is required.
    for (tier, id) in TIER_VECTORS {
        let variant = built
            .iter()
            .find(|b| b.id == *id)
            .unwrap_or_else(|| panic!("{tier}: vector {id} is missing"));
        // A minimum at or below the tier's own evidence changes nothing; one above it turns the
        // fixture's outcome into a failure. The signed vectors pass only because of signed
        // evidence, which their own policy requires.
        if let Some((fixture, _)) = id.split_once("--minimum-") {
            let parent = built.iter().find(|b| b.id == fixture).unwrap();
            assert_ne!(
                variant.expected["codes"], parent.expected["codes"],
                "{tier}: {id} decides nothing"
            );
        } else {
            assert_eq!(variant.expected["verdict"], "evaluated", "{id}");
        }
    }
    // Every I-JSON constraint has a reject vector, and every ACV code in use has one too.
    let reject_codes: BTreeSet<&str> = built
        .iter()
        .filter(|b| b.kind == Kind::Reject)
        .flat_map(|b| b.expected["codes"].as_array().unwrap().iter())
        .map(|c| c.as_str().unwrap())
        .collect();
    for code in [
        "ACV001", "ACV003", "ACV004", "ACV005", "ACV006", "ACV007", "ACV008", "ACV009", "ACV010",
    ] {
        assert!(reject_codes.contains(code), "no reject vector for {code}");
    }
    // Incomplete collection never yields a clean result.
    for b in built.iter().filter(|b| b.kind == Kind::Incomplete) {
        assert_eq!(b.expected["verdict"], "incomplete", "{}", b.id);
    }
    assert!(built.iter().filter(|b| b.kind == Kind::Incomplete).count() >= 3);
}

/// The number of leaf paths at which two values differ. Objects compare member by member (a
/// member present on one side only is one difference); arrays of equal length compare element
/// by element; an array that is the other plus one inserted element is one difference; any
/// other difference in type or length counts as a replaced subtree, one difference.
fn leaf_distance(a: &Value, b: &Value) -> usize {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
            keys.into_iter()
                .map(|k| match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => leaf_distance(p, q),
                    _ => 1,
                })
                .sum()
        }
        (Value::Array(x), Value::Array(y)) if x.len() == y.len() => {
            x.iter().zip(y).map(|(p, q)| leaf_distance(p, q)).sum()
        }
        (Value::Array(x), Value::Array(y)) if x.len().abs_diff(y.len()) == 1 => {
            let (long, short) = if x.len() > y.len() { (x, y) } else { (y, x) };
            let one_insert = (0..long.len()).any(|i| {
                let mut l = long.clone();
                l.remove(i);
                l == *short
            });
            if one_insert { 1 } else { 2 }
        }
        _ => usize::from(a != b),
    }
}

#[test]
fn every_reject_differs_from_its_accept_parent_by_one_mutation() {
    let built = build();
    let exceptions: Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("PAIRING-EXCEPTIONS.json")).unwrap(),
    )
    .unwrap();
    let exceptions = exceptions["vectors"].as_object().unwrap();
    let mut used = BTreeSet::new();
    for b in built.iter().filter(|b| b.kind == Kind::Reject) {
        let parent_id = b.parent.as_deref().expect("reject vector without a parent");
        let parent = built
            .iter()
            .find(|p| p.id == parent_id)
            .unwrap_or_else(|| panic!("{}: parent {parent_id} not in the corpus", b.id));
        assert_eq!(parent.kind, Kind::Accept, "{}: parent is not accept", b.id);
        let distance = match (
            serde_json::from_str::<Value>(&parent.text),
            serde_json::from_str::<Value>(&b.text),
        ) {
            (Ok(p), Ok(r)) => Some(leaf_distance(&p, &r)),
            _ => None,
        };
        if distance == Some(1) {
            assert!(
                !exceptions.contains_key(&b.id),
                "{}: declared as an exception but measures one mutation",
                b.id
            );
            continue;
        }
        let reason = exceptions
            .get(&b.id)
            .and_then(|e| e["reason"].as_str())
            .filter(|r| !r.trim().is_empty())
            .unwrap_or_else(|| {
                panic!(
                    "{}: {distance:?} mutations from {parent_id} and no declared exception",
                    b.id
                )
            });
        // A lexical exception is still one contiguous edit of the parent's text.
        let prefix = parent
            .text
            .bytes()
            .zip(b.text.bytes())
            .take_while(|(x, y)| x == y)
            .count();
        let suffix = parent.text[prefix..]
            .bytes()
            .rev()
            .zip(b.text[prefix..].bytes().rev())
            .take_while(|(x, y)| x == y)
            .count();
        let removed = &parent.text[prefix..parent.text.len() - suffix];
        assert!(
            !removed.contains('\n') || removed.is_empty(),
            "{}: the edit spans more than one line of the parent ({reason})",
            b.id
        );
        used.insert(b.id.clone());
    }
    for id in exceptions.keys() {
        assert!(used.contains(id), "stale pairing exception: {id}");
    }
}

/// GitHub's heading anchor: lowercase, spaces to hyphens, other punctuation dropped.
fn anchor(heading: &str) -> String {
    let mut out = String::from("#");
    for c in heading.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' {
            out.push('-');
        }
    }
    out
}

#[test]
fn conditions_name_existing_spec_sections() {
    let spec = std::fs::read_to_string(common::root().join(SPEC_PATH)).unwrap();
    let anchors: BTreeSet<String> = spec
        .lines()
        .filter_map(|l| l.strip_prefix("## ").or_else(|| l.strip_prefix("### ")))
        .map(anchor)
        .collect();
    let mut all = BTreeMap::new();
    for b in build() {
        assert!(!b.conditions.is_empty(), "{} names no condition", b.id);
        for c in b.conditions {
            all.entry(c).or_insert(b.id.clone());
        }
    }
    for (c, id) in all {
        assert!(anchors.contains(c), "{id}: no spec section {c}");
    }
}

#[test]
fn the_digest_list_matches_the_files_on_disk() {
    let root = root();
    let listed = std::fs::read_to_string(root.join("CORPUS-DIGESTS.txt")).unwrap();
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    let recomputed: String = files
        .iter()
        .map(|rel| {
            let digest =
                agent_change_control::canonical::sha256(&std::fs::read(root.join(rel)).unwrap());
            format!("{}  {rel}\n", digest.trim_start_matches("sha256:"))
        })
        .collect();
    assert_eq!(
        listed, recomputed,
        "CORPUS-DIGESTS.txt is stale; run python3 conformance/digests.py"
    );
}

/// Every corpus file except the digest list itself, relative to the corpus root.
fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            if rel != "__pycache__" {
                collect(root, &path, out);
            }
        } else if !rel.starts_with("CORPUS-DIGESTS.txt") && !rel.ends_with(".pyc") {
            out.push(rel);
        }
    }
}
