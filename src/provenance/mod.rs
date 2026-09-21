//! Agent authorship evidence: the fenced declaration in a change description, the separately
//! published provenance-file convention (bare, or as a signed attestation the caller verified),
//! and the lower tier of vendor `Co-Authored-By` trailers that agents already write into
//! commits.

pub mod agent_trace;
pub mod attestations;

use crate::model::*;
use anyhow::{Result, anyhow, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const MARKER: &str = "```agent-change-control\n";

/// Trailer identities that agent vendors write into commits they produce, keyed by the trailer
/// email after normalization (lowercase, any GitHub `ID+` prefix removed). These are not
/// deliverable addresses and are not tied to any forge account; they are the vendor tool
/// identifying itself at authoring time. Evidence derived from them is a lower tier than an
/// explicit declaration: it never overrides one and it yields `derived`, not `explicit`,
/// confidence.
pub const BUILTIN_TRAILERS: &[(&str, &str)] = &[
    ("noreply@anthropic.com", "claude-code"),
    ("copilot@users.noreply.github.com", "copilot"),
];

/// The trailer registry: the built-in vendor identities plus caller-supplied `EMAIL=AGENT`
/// mappings, which win on the same email.
pub fn trailer_registry(extra: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut registry: BTreeMap<String, String> = BUILTIN_TRAILERS
        .iter()
        .map(|(email, agent)| ((*email).to_string(), (*agent).to_string()))
        .collect();
    for (email, agent) in extra {
        registry.insert(normalize_email(email), agent.clone());
    }
    registry
}

/// Lowercase an email and drop GitHub's numeric `ID+` noreply prefix.
fn normalize_email(email: &str) -> String {
    let email = email.trim().to_ascii_lowercase();
    match email.split_once('+') {
        Some((id, rest)) if !id.is_empty() && id.chars().all(|c| c.is_ascii_digit()) => {
            rest.to_string()
        }
        _ => email,
    }
}

/// The `Co-Authored-By` trailers of a commit message as `(name, normalized email)` pairs.
///
/// Only the final paragraph is read, and only when every line in it is a `Token: value` trailer,
/// which is git's own definition of a trailer block. Prose mentioning a co-author is not a
/// trailer.
pub fn trailers(message: &str) -> Vec<(String, String)> {
    let block = message.trim_end().rsplit("\n\n").next().unwrap_or_default();
    let lines: Vec<&str> = block
        .lines()
        .map(str::trim_end)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let parsed: Vec<Option<(&str, &str)>> = lines
        .iter()
        .map(|line| {
            line.split_once(':').filter(|(token, _)| {
                !token.is_empty() && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
        })
        .collect();
    if parsed.iter().any(Option::is_none) {
        return Vec::new();
    }
    parsed
        .into_iter()
        .flatten()
        .filter(|(token, _)| token.eq_ignore_ascii_case("co-authored-by"))
        .filter_map(|(_, value)| {
            let value = value.trim();
            let (name, rest) = value.split_once('<')?;
            let email = rest.strip_suffix('>')?;
            Some((name.trim().to_string(), normalize_email(email)))
        })
        .collect()
}

/// The distinct agents a commit message's trailers identify under `registry`.
pub fn trailer_agents(message: &str, registry: &BTreeMap<String, String>) -> BTreeSet<String> {
    trailers(message)
        .into_iter()
        .filter_map(|(_, email)| registry.get(&email).cloned())
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    author: String,
    operator: Option<String>,
}

/// Extract the `(agent, operator)` declared in a change body.
///
/// Only an explicitly delimited block is accepted, never casual prose or code style. More than
/// one block, an unterminated block or an invalid one is an error, never silently ignored.
pub fn declaration(body: &str) -> Result<Option<(String, Option<String>)>> {
    let blocks: Vec<_> = body.match_indices(MARKER).collect();
    ensure!(blocks.len() <= 1, "conflicting agent provenance blocks");
    let Some((start, _)) = blocks.first() else {
        return Ok(None);
    };
    let tail = &body[start + MARKER.len()..];
    let (raw, _) = tail
        .split_once("\n```")
        .ok_or_else(|| anyhow!("unterminated agent provenance block"))?;
    let d: Declaration =
        serde_saphyr::from_str(raw).map_err(|_| anyhow!("invalid agent provenance block"))?;
    ensure!(!d.author.trim().is_empty(), "empty declared agent");
    Ok(Some((d.author, d.operator)))
}

/// Validate a provenance document against its schema and its binding to `head`, returning the
/// `(agent, operator)` it asserts.
pub fn convention(value: &Value, head: &str) -> Result<(String, Option<String>)> {
    crate::normalize::schema(value, "provenance")?;
    ensure!(
        value["change"]["head_commit"].as_str() == Some(head),
        "provenance head does not match change"
    );
    let agent = value["agent"]["name"]
        .as_str()
        .ok_or_else(|| anyhow!("provenance lacks an agent name"))?;
    let operator = value["operator"]["id"]
        .as_str()
        .ok_or_else(|| anyhow!("provenance lacks an operator"))?;
    Ok((agent.into(), Some(operator.into())))
}

/// Record `agent` as the effective author of `c` and try to resolve `operator` in the
/// `namespace` (e.g. `github`) of the collecting forge, citing `provenance`.
///
/// Only identities already observed as humans resolve, and a resolved operator gets
/// `confidence` (`explicit` for a declaration or mapping, `derived` for trailer evidence).
/// Emails and unmatched logins remain unknown; the merger, opener or first reviewer is never
/// substituted.
pub fn apply(
    c: &mut Change,
    actors: &mut BTreeMap<String, Actor>,
    agent: String,
    operator: Option<String>,
    namespace: &str,
    provenance: Vec<Evidence>,
    confidence: Confidence,
) -> Result<()> {
    ensure!(!provenance.is_empty(), "agent authorship needs evidence");
    ensure!(
        agent
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)),
        "invalid agent name"
    );
    let id = format!("agent:{}", agent.to_ascii_lowercase());
    actors.insert(
        id.clone(),
        Actor {
            kind: ActorKind::Agent,
            display_name: Some(agent),
        },
    );
    c.author = Identity {
        actor_id: id,
        provenance: provenance.clone(),
    };
    let prefix = format!("{namespace}:");
    let resolved = operator.and_then(|s| {
        let id = if s.starts_with(&prefix) {
            s.to_ascii_lowercase()
        } else {
            format!("{prefix}{}", s.to_ascii_lowercase())
        };
        actors
            .get(&id)
            .filter(|a| a.kind == ActorKind::Human)
            .map(|_| id)
    });
    c.agent_operator = Some(Operator {
        confidence: if resolved.is_some() {
            confidence
        } else {
            Confidence::Unknown
        },
        actor_id: resolved,
        provenance,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prose_is_not_a_declaration() {
        assert!(
            declaration("Claude wrote this code; Agent-Author: codex")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn exact_blocks_are_parsed() {
        let (agent, operator) =
            declaration("intro\n```agent-change-control\nauthor: codex\noperator: alice\n```\n")
                .unwrap()
                .unwrap();
        assert_eq!(agent, "codex");
        assert_eq!(operator.as_deref(), Some("alice"));
        let (agent, operator) = declaration("```agent-change-control\nauthor: codex\n```")
            .unwrap()
            .unwrap();
        assert_eq!(agent, "codex");
        assert!(operator.is_none());
    }

    #[test]
    fn trailers_are_read_from_the_final_block_only() {
        let msg = "Add limiter\n\nSee Co-Authored-By: nobody <x@y> in prose.\n\n\
                   Co-Authored-By: Claude Opus 4 <noreply@anthropic.com>\n\
                   Claude-Session: https://example.invalid/s\n";
        assert_eq!(
            trailers(msg),
            vec![(
                "Claude Opus 4".to_string(),
                "noreply@anthropic.com".to_string()
            )]
        );
        // A final paragraph with a non-trailer line is prose, not a trailer block.
        assert!(trailers("Fix\n\nthanks to\nCo-Authored-By: A <noreply@anthropic.com>").is_empty());
        assert!(trailers("Co-Authored-By: broken <no-closing").is_empty());
        assert!(trailers("").is_empty());
        // Case-insensitive token, GitHub's numeric noreply prefix stripped.
        let msg = "x\n\nco-authored-by: Copilot <198982749+Copilot@users.noreply.github.com>";
        assert_eq!(trailers(msg)[0].1, "copilot@users.noreply.github.com");
    }

    #[test]
    fn registry_maps_vendor_trailers_and_callers_extend_it() {
        let registry = trailer_registry(&BTreeMap::new());
        let msg = "x\n\nCo-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>";
        assert_eq!(
            trailer_agents(msg, &registry)
                .into_iter()
                .collect::<Vec<_>>(),
            vec!["claude-code".to_string()]
        );
        assert!(trailer_agents("x\n\nCo-Authored-By: Bob <bob@example.com>", &registry).is_empty());
        let extra = BTreeMap::from([("Bot@Example.com".to_string(), "house-agent".to_string())]);
        let registry = trailer_registry(&extra);
        assert_eq!(
            trailer_agents("x\n\nCo-Authored-By: Bot <bot@example.com>", &registry).len(),
            1
        );
        // Two vendors in one commit are two distinct agents.
        let msg = "x\n\nCo-Authored-By: A <noreply@anthropic.com>\nCo-Authored-By: C <1+copilot@users.noreply.github.com>";
        assert_eq!(trailer_agents(msg, &registry).len(), 2);
    }

    #[test]
    fn malformed_blocks_are_errors() {
        assert!(declaration("```agent-change-control\nauthor: codex\nunknown: true\n```").is_err());
        assert!(declaration("```agent-change-control\nauthor: codex").is_err());
        assert!(declaration("```agent-change-control\nauthor: ''\n```").is_err());
        assert!(
            declaration(
                "```agent-change-control\nauthor: codex\n```\n```agent-change-control\nauthor: claude-code\n```"
            )
            .is_err()
        );
    }
}
