//! The forge selector and collection flags shared by `scan` and `export`.

use super::io::{self, EvidenceArgs, OutputArgs};
use crate::collectors::github::{Github, Sources, validate_repo};
use crate::model::Events;
use crate::{Exit, failure};
use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Subcommand)]
pub enum Forge {
    /// A GitHub.com repository. Reads GITHUB_TOKEN or GH_TOKEN; public repositories need none.
    Github(Collect),
}

#[derive(Debug, Args)]
pub struct Collect {
    /// Repository as OWNER/REPO.
    pub repository: String,
    /// Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339.
    #[arg(long, value_name = "DATE")]
    pub since: String,
    /// End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339.
    #[arg(long, value_name = "DATE")]
    pub until: String,
    /// Policy file (defaults to .agent-change-control/policy.yml when present).
    #[arg(long, value_name = "FILE")]
    pub policy: Option<PathBuf>,
    /// Treat LOGIN as the verified account of AGENT (repeatable).
    #[arg(long, value_name = "LOGIN=AGENT")]
    pub agent_account: Vec<String>,
    #[command(flatten)]
    pub evidence: EvidenceArgs,
    /// Maximum pages per list endpoint (100 items each).
    #[arg(long, default_value_t = 100, value_name = "N")]
    pub max_pages: usize,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// Collect the pull requests merged in the window.
pub fn collect(c: &Collect) -> Result<Events> {
    validate_repo(&c.repository)?;
    let from = io::boundary(&c.since, false)?;
    let to = io::boundary(&c.until, true)?;
    if from > to {
        return Err(failure(Exit::Usage, "window is reversed"));
    }
    let known = io::known(&c.agent_account)?;
    let registry = c.evidence.registry()?;
    let traces = c.evidence.traces()?;
    Github::new(io::token(), c.max_pages)?.collect(
        &c.repository,
        from,
        to,
        None,
        Sources {
            known: &known,
            trailers: registry.as_ref(),
            traces: traces.as_ref(),
        },
    )
}
