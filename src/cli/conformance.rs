//! `acc evaluate --conformance-json VECTOR`: the external-verifier contract of the ACP
//! conformance corpus (`conformance/README.md`, spec §9).
//!
//! The input is a conformance vector, a JSON object with exactly two members: `events` (a
//! normalized event export) and `policy` (a policy). The last line of stdout is one JSON object:
//!
//! ```json
//! {"verdict":"evaluated","codes":["ACC002"],"assessments":[{"change":"…","rule":"ACC002","outcome":"fail"}],"manifestDigest":"sha256:…"}
//! ```
//!
//! `verdict` is `evaluated` (exit 0), `incomplete` (exit 4) or `invalid` (exit 3). `codes` holds
//! every rule with outcome `fail`, or the `ACV` codes of invalid input. Nothing else is printed
//! to stdout, so the object is also the only line.

use super::io;
use crate::canonical::jcs_bytes;
use crate::model::{Events, Manifest, Policy, Status};
use crate::{Exit, manifest};
use anyhow::{Result, anyhow, ensure};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::Path;

/// Parse a vector: an object with exactly `events` and `policy`, each valid under its schema.
fn parse_vector(text: &str) -> Result<(Events, Policy)> {
    let mut v = io::parse(text)?;
    let members = v
        .as_object_mut()
        .ok_or_else(|| anyhow!("ACV010 a conformance vector is a JSON object"))?;
    ensure!(
        members.len() == 2 && members.contains_key("events") && members.contains_key("policy"),
        "ACV010 a conformance vector has exactly the members events and policy"
    );
    let events = members.remove("events").unwrap_or_default();
    let policy = members.remove("policy").unwrap_or_default();
    crate::normalize::schema(&events, "events")?;
    crate::normalize::schema(&policy, "policy")?;
    Ok((
        serde_json::from_value(events)?,
        serde_json::from_value(policy)?,
    ))
}

/// Every `ACVnnn` code mentioned anywhere in an error chain, in order of first appearance.
fn validation_codes(err: &anyhow::Error) -> Vec<String> {
    let text = format!("{err:#}");
    let bytes = text.as_bytes();
    let mut codes: Vec<String> = Vec::new();
    for i in 0..bytes.len().saturating_sub(5) {
        let window = &bytes[i..i + 6];
        if window.starts_with(b"ACV") && window[3..].iter().all(u8::is_ascii_digit) {
            let code = String::from_utf8_lossy(window).into_owned();
            if !codes.contains(&code) {
                codes.push(code);
            }
        }
    }
    codes
}

/// SHA-256 over the RFC 8785 bytes of the manifest without `generated.tool` and
/// `generated.version`. Those name the implementation that wrote it, not the result, so they
/// would make the digest differ between implementations and between releases of one.
pub fn manifest_digest(m: &Manifest) -> Result<String> {
    let mut v = serde_json::to_value(m)?;
    if let Some(generated) = v["generated"].as_object_mut() {
        generated.remove("tool");
        generated.remove("version");
    }
    crate::canonical::digest(&v)
}

/// The contract's result object for an evaluated manifest.
pub fn result(m: &Manifest) -> Result<(Value, Exit)> {
    let codes: BTreeSet<&str> = m
        .assessments
        .iter()
        .filter(|a| a.status == Status::Fail)
        .map(|a| a.rule_id.code())
        .collect();
    let assessments: Vec<Value> = m
        .assessments
        .iter()
        .map(|a| {
            json!({
                "change": a.change_id,
                "rule": a.rule_id.code(),
                "outcome": a.status,
            })
        })
        .collect();
    let (verdict, exit) = if io::incomplete(&m.events) {
        ("incomplete", Exit::Incomplete)
    } else {
        ("evaluated", Exit::Ok)
    };
    Ok((
        json!({
            "verdict": verdict,
            "codes": codes,
            "assessments": assessments,
            "manifestDigest": manifest_digest(m)?,
        }),
        exit,
    ))
}

/// Evaluate a vector's text into the contract's result object and exit status. Invalid input is
/// a result (`invalid`, exit 3, with its `ACV` codes), not an error; the error, if any, is
/// returned alongside for the caller to report.
pub fn evaluate(text: &str) -> Result<(Value, Exit, Option<anyhow::Error>)> {
    match parse_vector(text).and_then(|(events, policy)| manifest::evaluate(events, policy)) {
        Ok(m) => {
            let (object, exit) = result(&m)?;
            Ok((object, exit, None))
        }
        Err(err) => Ok((
            json!({"verdict": "invalid", "codes": validation_codes(&err)}),
            Exit::InvalidInput,
            Some(err),
        )),
    }
}

/// Evaluate the vector at `path` and print the result object as the only line of stdout.
pub fn run(path: &Path) -> Result<Exit> {
    let (object, exit, err) = match io::read_text(path) {
        Ok(text) => evaluate(&text)?,
        Err(err) => (
            json!({"verdict": "invalid", "codes": []}),
            Exit::InvalidInput,
            Some(err),
        ),
    };
    if let Some(err) = err {
        eprintln!("error: {err:#}");
    }
    println!("{}", jcs_bytes(&object)?);
    Ok(exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_digest_does_not_name_the_implementation() {
        let e: Events =
            serde_json::from_str(include_str!("../../tests/fixtures/human-clean/events.json"))
                .unwrap();
        let mut m = manifest::evaluate(e, Policy::default()).unwrap();
        let digest = manifest_digest(&m).unwrap();
        m.generated.tool = "another-evaluator".into();
        m.generated.version = "9.9.9".into();
        assert_eq!(manifest_digest(&m).unwrap(), digest);
        m.generated.source_digest = format!("sha256:{}", "0".repeat(64));
        assert_ne!(manifest_digest(&m).unwrap(), digest);
    }

    #[test]
    fn validation_codes_are_found_once_in_order() {
        let err = anyhow!("ACV004 duplicate review ID").context("ACV010 x ACV004");
        assert_eq!(validation_codes(&err), vec!["ACV010", "ACV004"]);
        assert!(validation_codes(&anyhow!("no code here, ACV12")).is_empty());
    }
}
