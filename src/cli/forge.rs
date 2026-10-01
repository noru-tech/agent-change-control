//! The forge selector and collection flags shared by `scan` and `export`.

use super::Ctx;
use super::io::{self, EvidenceArgs, OutputArgs};
use crate::collectors::github::{Github, Sources, validate_repo};
use crate::model::Events;
use crate::{Exit, failure_with_hint};
use anyhow::Result;
use chrono::SecondsFormat;
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

/// Report under `--verbose` whether requests are authenticated, naming the variable but never
/// the token.
pub fn debug_auth(ctx: &Ctx) {
    ctx.debug(match io::token_source() {
        Some((name, _)) => format!("authenticating with the token in {name}"),
        None => "no token set: unauthenticated requests (public repositories only)".into(),
    });
}

/// What to do about an incomplete collection.
pub const RECOLLECT: &str = "raise --max-pages (at most 1000) or narrow the window, then collect again; the output records the gap";

/// Testing only: a loopback API base (`http://127.0.0.1:PORT`) for the integration tests'
/// replay server. Any other value, or a token in the environment, is refused, so a token can
/// never be sent anywhere but `api.github.com`.
pub const API_URL_ENV: &str = "ACC_GITHUB_API_URL";

/// The GitHub client for the environment's token, honoring the testing-only [`API_URL_ENV`].
pub fn client(max_pages: usize) -> Result<Github> {
    match std::env::var(API_URL_ENV) {
        Ok(base) if !base.is_empty() => Github::with_base(io::token(), max_pages, &base),
        _ => Github::new(io::token(), max_pages),
    }
}

/// Collect the pull requests merged in the window.
pub fn collect(ctx: &Ctx, c: &Collect) -> Result<Events> {
    validate_repo(&c.repository)?;
    let from = io::boundary(&c.since, false)?;
    let to = io::boundary(&c.until, true)?;
    if from > to {
        return Err(failure_with_hint(
            Exit::Usage,
            "window is reversed",
            "--since must not be later than --until",
        ));
    }
    let known = io::known(&c.agent_account)?;
    let registry = c.evidence.registry()?;
    let traces = c.evidence.traces()?;
    let attestations = c.evidence.attestations()?;
    let vendors = c.evidence.vendors()?;
    ctx.debug(format!(
        "collecting {} merged from {} to {} (at most {} pages per endpoint)",
        c.repository,
        from.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        to.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        c.max_pages
    ));
    debug_auth(ctx);
    client(c.max_pages)?.collect(
        &c.repository,
        from,
        to,
        None,
        Sources {
            known: &known,
            trailers: registry.as_ref(),
            traces: traces.as_ref(),
            attestations: attestations.as_ref(),
            vendors: &vendors,
        },
    )
}
