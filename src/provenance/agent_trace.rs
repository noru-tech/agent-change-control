//! Adapter for [Agent Trace](https://agent-trace.dev) records (0.1, RFC): line-level attribution
//! written by the authoring tool and bound to a VCS revision.
//!
//! Agent Trace answers *which lines came from which model*; this project answers *who is the
//! effective human behind a change and who independently approved it*. The adapter reads only
//! what the two share: a record bound to one of the change's commits whose contributors include
//! `ai` or `mixed` ranges is derived evidence that the named tool wrote (part of) the change.
//! Records carry no human identity, so the operator is derived from commit authorship exactly as
//! for trailers, or stays unknown. Storage is implementation-defined by the Agent Trace spec, so
//! the caller points at the files.

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAX_FILE: u64 = 32 * 1024 * 1024;

/// One record that attributes some of a revision to an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceHit {
    /// `<file>#<record id>`: where the record came from, for evidence references.
    pub locator: String,
    /// The tool name, normalized to an agent name, when the record names one.
    pub agent: Option<String>,
}

/// Agent Trace records indexed by `vcs.revision`.
#[derive(Debug, Default)]
pub struct AgentTraces {
    by_revision: BTreeMap<String, Vec<TraceHit>>,
}

impl AgentTraces {
    /// Load every `.json` file under `paths` (files or directories, recursively, in sorted
    /// order). A file holds one record or an array of records. Records are validated against
    /// the fields this adapter reads; a file that is not Agent Trace data is an error rather
    /// than silently ignored.
    pub fn load(paths: &[PathBuf]) -> Result<Self> {
        let mut traces = Self::default();
        for path in paths {
            traces.load_path(path)?;
        }
        Ok(traces)
    }

    fn load_path(&mut self, path: &Path) -> Result<()> {
        let meta = std::fs::metadata(path)
            .with_context(|| format!("unable to read {}", path.display()))?;
        if meta.is_dir() {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(path)?
                .map(|e| e.map(|e| e.path()))
                .collect::<std::io::Result<_>>()?;
            entries.sort();
            for entry in entries {
                if entry.is_dir() || entry.extension().is_some_and(|x| x == "json") {
                    self.load_path(&entry)?;
                }
            }
            return Ok(());
        }
        ensure!(
            meta.len() <= MAX_FILE,
            "agent trace file exceeds 32 MiB limit"
        );
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("unable to read {}", path.display()))?;
        let value: Value = serde_json::from_str(&data)
            .with_context(|| format!("{} is not valid JSON", path.display()))?;
        let records = match value {
            Value::Array(items) => items,
            v => vec![v],
        };
        for record in records {
            self.add(&record, path)?;
        }
        Ok(())
    }

    fn add(&mut self, record: &Value, path: &Path) -> Result<()> {
        let object = record
            .as_object()
            .with_context(|| format!("{}: agent trace record is not an object", path.display()))?;
        if !object.get("version").is_some_and(Value::is_string)
            || !object.get("files").is_some_and(Value::is_array)
        {
            bail!("{}: not an Agent Trace record", path.display());
        }
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .with_context(|| format!("{}: agent trace record lacks an id", path.display()))?;
        // A record that is not bound to a revision cannot be tied to a change.
        let Some(revision) = object
            .get("vcs")
            .and_then(|v| v.get("revision"))
            .and_then(Value::as_str)
        else {
            return Ok(());
        };
        let attributed = object["files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| f["conversations"].as_array())
            .flatten()
            .any(|conversation| {
                let level = contributor_is_ai(&conversation["contributor"]);
                let ranges = conversation["ranges"].as_array();
                level
                    || ranges
                        .into_iter()
                        .flatten()
                        .any(|r| r.get("contributor").is_some_and(contributor_is_ai))
            });
        if !attributed {
            return Ok(());
        }
        let agent = object
            .get("tool")
            .and_then(|t| t.get("name"))
            .and_then(Value::as_str)
            .map(agent_name)
            .filter(|n| !n.is_empty());
        self.by_revision
            .entry(revision.to_string())
            .or_default()
            .push(TraceHit {
                locator: format!("{}#{id}", path.display()),
                agent,
            });
        Ok(())
    }

    /// The records bound to `revision` that attribute lines to an agent.
    pub fn hits(&self, revision: &str) -> &[TraceHit] {
        self.by_revision
            .get(revision)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        self.by_revision.is_empty()
    }
}

fn contributor_is_ai(c: &Value) -> bool {
    matches!(c["type"].as_str(), Some("ai" | "mixed"))
}

/// A tool name as an agent name: lowercase, with anything outside `[a-z0-9._-]` collapsed to a
/// hyphen, so `Claude Code` and `claude-code` are one agent.
fn agent_name(tool: &str) -> String {
    let mut out = String::new();
    for c in tool.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() || "._-".contains(c) {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(revision: Option<&str>, tool: Option<&str>, kind: &str) -> Value {
        let mut r = json!({
            "version": "0.1.0",
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "timestamp": "2026-01-23T14:30:00Z",
            "files": [{"path": "src/a.ts", "conversations": [{
                "contributor": {"type": kind, "model_id": "anthropic/claude-opus-4-5"},
                "ranges": [{"start_line": 1, "end_line": 5}]
            }]}]
        });
        if let Some(rev) = revision {
            r["vcs"] = json!({"type": "git", "revision": rev});
        }
        if let Some(t) = tool {
            r["tool"] = json!({"name": t, "version": "2.4.0"});
        }
        r
    }

    fn write(dir: &Path, name: &str, v: &Value) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, v.to_string()).unwrap();
        p
    }

    #[test]
    fn records_bind_to_revisions_and_name_the_tool() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(
            dir.path(),
            "cursor.json",
            &record(Some("abc"), Some("Cursor"), "ai"),
        );
        let traces = AgentTraces::load(std::slice::from_ref(&p)).unwrap();
        let hits = traces.hits("abc");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].agent.as_deref(), Some("cursor"));
        assert_eq!(
            hits[0].locator,
            format!("{}#550e8400-e29b-41d4-a716-446655440000", p.display())
        );
        assert!(traces.hits("other").is_empty());
    }

    #[test]
    fn human_only_unbound_and_range_level_records() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "a.json", &record(Some("h"), Some("x"), "human"));
        write(dir.path(), "b.json", &record(None, Some("x"), "ai"));
        let mut range_override = record(Some("r"), Some("Claude Code"), "human");
        range_override["files"][0]["conversations"][0]["ranges"][0]["contributor"] =
            json!({"type": "mixed"});
        write(dir.path(), "c.json", &range_override);
        write(dir.path(), "notes.txt", &json!("ignored: not .json"));
        let sub = dir.path().join("nested");
        std::fs::create_dir(&sub).unwrap();
        write(&sub, "d.json", &json!([record(Some("n"), None, "ai")]));
        let traces = AgentTraces::load(&[dir.path().to_path_buf()]).unwrap();
        assert!(traces.hits("h").is_empty(), "human-only");
        assert!(!traces.is_empty());
        assert_eq!(traces.hits("r")[0].agent.as_deref(), Some("claude-code"));
        assert!(traces.hits("n")[0].agent.is_none(), "no tool name");
    }

    #[test]
    fn non_trace_files_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), "x.json", &json!({"hello": "world"}));
        assert!(AgentTraces::load(&[p]).is_err());
        let p = write(
            dir.path(),
            "y.json",
            &json!({"version": "0.1.0", "files": []}),
        );
        assert!(AgentTraces::load(&[p]).is_err(), "missing id");
        assert!(AgentTraces::load(&[dir.path().join("missing.json")]).is_err());
    }

    #[test]
    fn tool_names_normalize() {
        assert_eq!(agent_name("Claude Code"), "claude-code");
        assert_eq!(agent_name("  cursor "), "cursor");
        assert_eq!(agent_name("A/B  C"), "a-b-c");
        assert_eq!(agent_name("***"), "");
    }
}
