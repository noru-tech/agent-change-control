//! Agent authorship evidence: the fenced declaration in a change description and the separately
//! published provenance-file convention.

use crate::model::*;
use anyhow::{Result, anyhow, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

const MARKER: &str = "```agent-change-control\n";

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
/// `namespace` (e.g. `github`) of the collecting forge.
///
/// Only identities already observed as humans resolve. Emails and unmatched logins remain
/// unknown; the merger, opener or first reviewer is never substituted.
pub fn apply(
    c: &mut Change,
    actors: &mut BTreeMap<String, Actor>,
    agent: String,
    operator: Option<String>,
    namespace: &str,
    source: Evidence,
) -> Result<()> {
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
        provenance: vec![source.clone()],
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
            Confidence::Explicit
        } else {
            Confidence::Unknown
        },
        actor_id: resolved,
        provenance: vec![source],
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
