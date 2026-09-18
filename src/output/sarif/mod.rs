//! SARIF 2.1.0 output, one result per finding.

use crate::model::{Manifest, Severity};
use crate::policy::RULES;
use anyhow::Result;
use serde_json::{Value, json};

fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::High => "error",
        Severity::Medium | Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

pub fn render(m: &Manifest) -> Result<String> {
    let rules: Vec<Value> = RULES
        .iter()
        .map(|rule| {
            json!({
                "id": rule.code(),
                "name": rule.name().as_str(),
                "shortDescription": {"text": rule.name().as_str().replace('_', " ")},
            })
        })
        .collect();
    let results: Vec<Value> = m
        .findings
        .iter()
        .map(|f| {
            let url = m
                .events
                .changes
                .iter()
                .find(|c| c.id == f.change_id)
                .map(|c| c.url.as_str())
                .unwrap_or_default();
            json!({
                "ruleId": f.rule_id,
                "level": level(f.severity),
                "message": {"text": f.explanation},
                "partialFingerprints": {"agentChangeControl/v1": f.id},
                "locations": [{"physicalLocation": {"artifactLocation": {"uri": url}}}],
                "properties": {
                    "change_id": f.change_id,
                    "provenance": f.provenance,
                    "disposition": f.disposition,
                },
            })
        })
        .collect();
    let complete = m.events.window.complete && m.events.changes.iter().all(|c| c.reviews_complete);
    let log = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {
                "name": "agent-change-control",
                "version": crate::version(),
                "rules": rules,
            }},
            "results": results,
            "invocations": [{"executionSuccessful": complete}],
            "properties": {"assessments": m.assessments},
        }],
    });
    crate::normalize::canonical(&log)
}
