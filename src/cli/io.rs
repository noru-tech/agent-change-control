//! Input parsing, output writing and the flags shared by several subcommands.

use crate::model::*;
use crate::output::Format;
use crate::provenance::agent_trace::AgentTraces;
use crate::provenance::attestations::Attestations;
use crate::{Exit, failure};
use anyhow::{Context, Result, anyhow, ensure};
use chrono::NaiveDate;
use clap::Args;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_INPUT: u64 = 32 * 1024 * 1024;
/// Where `scan` writes when neither `--format` nor `--output` is given.
pub const DEFAULT_MANIFEST: &str = ".agent-change-control/manifest.yml";
/// The policy file loaded automatically when present.
pub const DEFAULT_POLICY: &str = ".agent-change-control/policy.yml";

/// Shared `--format` / `--output` flags.
#[derive(Debug, Clone, Args, Default)]
pub struct OutputArgs {
    /// Output format; inferred from the --output extension when omitted.
    #[arg(short, long, value_enum)]
    pub format: Option<Format>,
    /// Write to FILE instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

impl OutputArgs {
    /// The explicit format, else the one implied by the output extension, else `default`.
    pub fn format(&self, default: Format) -> Format {
        self.format
            .or_else(|| self.output.as_deref().and_then(Format::from_path))
            .unwrap_or(default)
    }

    pub fn render(&self, m: &Manifest, default: Format) -> Result<()> {
        write(
            &crate::output::render(m, self.format(default))?,
            self.output.as_deref(),
        )
    }
}

/// Read a bounded UTF-8 text file.
pub fn read_text(path: &Path) -> Result<String> {
    let mut data = String::new();
    std::fs::File::open(path)
        .with_context(|| format!("unable to open {}", path.display()))?
        .take(MAX_INPUT + 1)
        .read_to_string(&mut data)
        .context("unable to read UTF-8 input")?;
    ensure!(data.len() as u64 <= MAX_INPUT, "input exceeds 32 MiB limit");
    Ok(data)
}

/// Parse one JSON or YAML document.
pub fn parse(data: &str) -> Result<Value> {
    // YAML 1.2 is a superset of JSON; try the strict JSON parser first.
    match serde_json::from_str(data) {
        Ok(v) => Ok(v),
        Err(_) => {
            serde_saphyr::from_str(data).map_err(|_| anyhow!("input is not valid JSON or YAML"))
        }
    }
}

/// Read a JSON or YAML document, validate it against the embedded `schema`, and deserialize it.
pub fn read<T: DeserializeOwned>(path: &Path, schema: &str) -> Result<T> {
    let value = parse(&read_text(path)?)?;
    crate::normalize::schema(&value, schema)?;
    Ok(serde_json::from_value(value)?)
}

/// Load `path`, else the default policy file when it exists, else the built-in defaults.
pub fn load_policy(path: Option<&Path>) -> Result<Policy> {
    let default = Path::new(DEFAULT_POLICY);
    match path.or_else(|| default.exists().then_some(default)) {
        Some(path) => crate::policy::resolve(read(path, "policy")?),
        None => Ok(Policy::default()),
    }
}

/// Parse repeated `LOGIN=AGENT` mappings into a lowercase login map.
pub fn known(values: &[String]) -> Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    for v in values {
        let (login, agent) = v
            .split_once('=')
            .ok_or_else(|| failure(Exit::Usage, "agent-account must be LOGIN=AGENT"))?;
        if login.is_empty() || agent.is_empty() {
            return Err(failure(Exit::Usage, "empty agent account mapping"));
        }
        if map
            .insert(login.to_ascii_lowercase(), agent.into())
            .is_some()
        {
            return Err(failure(Exit::Usage, "duplicate known agent account"));
        }
    }
    Ok(map)
}

/// Shared derived-evidence flags: the vendor trailer registry and Agent Trace records.
#[derive(Debug, Clone, Args, Default)]
pub struct EvidenceArgs {
    /// Treat commits whose Co-Authored-By trailer carries EMAIL as written by AGENT (repeatable;
    /// extends the built-in vendor registry).
    #[arg(long, value_name = "EMAIL=AGENT")]
    pub agent_trailer: Vec<String>,
    /// Do not read Co-Authored-By trailers; only declarations and account mappings establish
    /// agent authorship.
    #[arg(long)]
    pub ignore_trailers: bool,
    /// Agent Trace record files or directories, bound to commits by vcs.revision (repeatable).
    #[arg(long, value_name = "PATH")]
    pub agent_trace: Vec<PathBuf>,
    /// Attestation files or directories (in-toto Statements, DSSE envelopes or Sigstore
    /// bundles, .json or .jsonl), bound to changes by their head commit (repeatable). acc does
    /// not verify signatures; see --verified-by.
    #[arg(long, value_name = "PATH")]
    pub attestations: Vec<PathBuf>,
    /// Who verified the attestations' signatures before this run, recorded verbatim (for
    /// example "gh attestation verify, run 123"). Without it attestation claims count as
    /// declared, not signed.
    #[arg(long, value_name = "TEXT", requires = "attestations")]
    pub verified_by: Option<String>,
}

impl EvidenceArgs {
    /// The attestations, when any path was given.
    pub fn attestations(&self) -> Result<Option<Attestations>> {
        if self.attestations.is_empty() {
            return Ok(None);
        }
        if self
            .verified_by
            .as_deref()
            .is_some_and(|v| v.trim().is_empty())
        {
            return Err(failure(Exit::Usage, "verified-by must not be empty"));
        }
        Ok(Some(Attestations::load(
            &self.attestations,
            self.verified_by.clone(),
        )?))
    }

    /// The Agent Trace records, when any path was given.
    pub fn traces(&self) -> Result<Option<AgentTraces>> {
        if self.agent_trace.is_empty() {
            return Ok(None);
        }
        Ok(Some(AgentTraces::load(&self.agent_trace)?))
    }

    /// The trailer registry to collect with, or `None` when trailers are ignored.
    pub fn registry(&self) -> Result<Option<BTreeMap<String, String>>> {
        if self.ignore_trailers {
            return Ok(None);
        }
        let mut extra = BTreeMap::new();
        for v in &self.agent_trailer {
            let (email, agent) = v
                .split_once('=')
                .ok_or_else(|| failure(Exit::Usage, "agent-trailer must be EMAIL=AGENT"))?;
            if email.is_empty() || agent.is_empty() || !email.contains('@') {
                return Err(failure(Exit::Usage, "invalid agent trailer mapping"));
            }
            if extra
                .insert(email.to_ascii_lowercase(), agent.to_string())
                .is_some()
            {
                return Err(failure(Exit::Usage, "duplicate agent trailer mapping"));
            }
        }
        Ok(Some(crate::provenance::trailer_registry(&extra)))
    }
}

/// Parse a window boundary: a calendar date expands to the start (or `end`) of that UTC day; an
/// RFC 3339 timestamp is taken as is.
pub fn boundary(s: &str, end: bool) -> Result<Timestamp> {
    if s.len() == 10 {
        let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| failure(Exit::Usage, "invalid date"))?;
        let time = if end {
            date.and_hms_nano_opt(23, 59, 59, 999_999_999)
        } else {
            date.and_hms_opt(0, 0, 0)
        };
        return time
            .map(|t| t.and_utc())
            .ok_or_else(|| failure(Exit::Usage, "invalid date"));
    }
    crate::normalize::timestamp(s).map_err(|_| failure(Exit::Usage, "invalid date/time"))
}

/// The GitHub token from the environment: `GITHUB_TOKEN`, else `GH_TOKEN`.
pub fn token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .find_map(|name| std::env::var(name).ok().filter(|t| !t.is_empty()))
}

/// Write to `path` (creating parent directories) or to stdout.
pub fn write(data: &str, path: Option<&Path>) -> Result<()> {
    if let Some(path) = path {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, data)
            .with_context(|| format!("unable to write {}", path.display()))?;
    } else {
        std::io::stdout().lock().write_all(data.as_bytes())?;
    }
    Ok(())
}

/// Whether any part of the collection was incomplete (exit 4 territory).
pub fn incomplete(e: &Events) -> bool {
    !e.window.complete || e.changes.iter().any(|c| !c.reviews_complete)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_expand_dates_to_utc_day_edges() {
        let from = boundary("2026-08-01", false).unwrap();
        let to = boundary("2026-08-31", true).unwrap();
        assert_eq!(from.to_rfc3339(), "2026-08-01T00:00:00+00:00");
        assert_eq!(
            serde_json::to_string(&to).unwrap(),
            "\"2026-08-31T23:59:59.999999999Z\""
        );
        let exact = boundary("2026-08-01T14:00:00+02:00", false).unwrap();
        assert_eq!(exact.to_rfc3339(), "2026-08-01T12:00:00+00:00");
        assert_eq!(
            crate::exit_for(&boundary("2026-13-01", false).unwrap_err()),
            Exit::Usage
        );
        assert_eq!(
            crate::exit_for(&boundary("yesterday", false).unwrap_err()),
            Exit::Usage
        );
    }

    #[test]
    fn trailer_flags_build_or_disable_the_registry() {
        let args = EvidenceArgs {
            agent_trailer: vec!["Bot@Example.com=house-agent".into()],
            ..Default::default()
        };
        let registry = args.registry().unwrap().unwrap();
        assert_eq!(registry["bot@example.com"], "house-agent");
        assert_eq!(registry["noreply@anthropic.com"], "claude-code");
        let off = EvidenceArgs {
            ignore_trailers: true,
            ..Default::default()
        };
        assert!(off.registry().unwrap().is_none());
        assert!(off.traces().unwrap().is_none());
        assert!(off.attestations().unwrap().is_none());
        let blank = EvidenceArgs {
            attestations: vec!["x".into()],
            verified_by: Some("  ".into()),
            ..Default::default()
        };
        assert_eq!(
            crate::exit_for(&blank.attestations().unwrap_err()),
            Exit::Usage
        );
        for bad in ["nope", "=x", "a@b=", "noat=x"] {
            let args = EvidenceArgs {
                agent_trailer: vec![bad.into()],
                ..Default::default()
            };
            assert_eq!(
                crate::exit_for(&args.registry().unwrap_err()),
                Exit::Usage,
                "{bad}"
            );
        }
    }

    #[test]
    fn agent_account_mappings_are_lowercased_and_unique() {
        let map = known(&["My-Agent[bot]=codex".into()]).unwrap();
        assert_eq!(map["my-agent[bot]"], "codex");
        assert!(known(&["nope".into()]).is_err());
        assert!(known(&["=codex".into()]).is_err());
        assert!(known(&["a=codex".into(), "A=claude-code".into()]).is_err());
    }

    #[test]
    fn output_format_is_explicit_then_inferred_then_default() {
        let args = OutputArgs {
            format: None,
            output: Some("out.sarif".into()),
        };
        assert_eq!(args.format(Format::Json), Format::Sarif);
        let args = OutputArgs {
            format: Some(Format::Table),
            output: Some("out.yml".into()),
        };
        assert_eq!(args.format(Format::Json), Format::Table);
        assert_eq!(OutputArgs::default().format(Format::Yaml), Format::Yaml);
        let args = OutputArgs {
            format: None,
            output: Some("change-control.intoto.jsonl".into()),
        };
        assert_eq!(args.format(Format::Json), Format::InTotoJsonl);
    }
}
