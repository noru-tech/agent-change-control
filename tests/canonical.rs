//! Spec §8.2: every byte sequence ACP hashes or signs is the RFC 8785 serialization of the
//! normalized value. One test per site, each recomputing the preimage with the JCS library
//! directly rather than through `acc`'s own routing.

mod common;

use agent_change_control::model::*;
use agent_change_control::{manifest, output};
use serde_json::Value;
use sha2::{Digest, Sha256};

fn jcs<T: serde::Serialize>(v: &T) -> String {
    serde_json_canonicalizer::to_string(v).unwrap()
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn evaluated(name: &str) -> Manifest {
    let e: Events = serde_json::from_str(
        &std::fs::read_to_string(common::fixture(name, "events.json")).unwrap(),
    )
    .unwrap();
    manifest::evaluate(e, Policy::default()).unwrap()
}

#[test]
fn finding_identifiers_hash_the_jcs_tuple() {
    let m = evaluated("claude-operator-self-approved");
    assert!(!m.findings.is_empty());
    for f in &m.findings {
        let preimage = jcs(&(&m.events.repository, &f.change_id, f.rule_id, &f.actor_ids));
        assert!(!preimage.ends_with('\n'));
        assert_eq!(f.id, format!("acc-{}", &sha256(preimage.as_bytes())[7..23]));
        // The 0.2 identifier hashed the same tuple with a trailing newline.
        let legacy = sha256(format!("{preimage}\n").as_bytes());
        assert_eq!(f.legacy_ids, vec![format!("acc-{}", &legacy[7..23])]);
    }
}

#[test]
fn source_digest_hashes_the_jcs_events() {
    let m = evaluated("claude-clean");
    assert_eq!(m.generated.source_digest, sha256(jcs(&m.events).as_bytes()));
}

#[test]
fn json_output_is_exactly_the_jcs_manifest() {
    let m = evaluated("claude-clean");
    let out = output::render(&m, output::Format::Json).unwrap();
    assert_eq!(out, jcs(&m));
    // The file's digest is the manifest's digest: no newline is appended.
    assert_eq!(sha256(out.as_bytes()), sha256(jcs(&m).as_bytes()));
}

#[test]
fn sarif_output_is_jcs() {
    let m = evaluated("claude-operator-self-approved");
    let out = output::render(&m, output::Format::Sarif).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(out, jcs(&v));
}

#[test]
fn statements_are_jcs_and_json_lines_separate_them_with_newlines() {
    let m = evaluated("claude-operator-self-approved");
    let out = output::render(&m, output::Format::InToto).unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(out, jcs(&v));
    assert_eq!(v["predicate"], serde_json::to_value(&m).unwrap());
    let lines = output::render(&m, output::Format::InTotoJsonl).unwrap();
    assert!(lines.ends_with('\n'));
    for line in lines.lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        assert_eq!(line, jcs(&v));
    }
}

#[test]
fn the_sha256_subject_form_hashes_the_jcs_predicate() {
    let m = evaluated("claude-clean");
    let mut v: Value =
        serde_json::from_str(&output::render(&m, output::Format::InToto).unwrap()).unwrap();
    let digest = sha256(jcs(&m).as_bytes());
    v["subject"] = serde_json::json!([{
        "name": "acc-manifest.json",
        "digest": {"sha256": digest.trim_start_matches("sha256:")}
    }]);
    output::intoto::validate_statement(&v).unwrap();
    // The legacy preimage (with its newline) does not match a 0.3 predicate.
    let legacy = sha256(format!("{}\n", jcs(&m)).as_bytes());
    v["subject"][0]["digest"]["sha256"] = legacy.trim_start_matches("sha256:").into();
    assert!(output::intoto::validate_statement(&v).is_err());
}
