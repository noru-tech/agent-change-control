//! `acc pr NUMBER --repo OWNER/REPO`

use super::Ctx;
use super::io::{self, OutputArgs};
use crate::collectors::github::{Github, validate_repo};
use crate::output::Format;
use crate::{Exit, failure, manifest};
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Pull request number.
    pub number: u64,
    /// Repository as OWNER/REPO.
    #[arg(long, env = "GITHUB_REPOSITORY", value_name = "OWNER/REPO")]
    pub repo: Option<String>,
    /// Policy file (defaults to .agent-change-control/policy.yml when present).
    #[arg(long, value_name = "FILE")]
    pub policy: Option<PathBuf>,
    /// Treat LOGIN as the verified account of AGENT (repeatable).
    #[arg(long, value_name = "LOGIN=AGENT")]
    pub agent_account: Vec<String>,
    /// Maximum pages per list endpoint (100 items each).
    #[arg(long, default_value_t = 100, value_name = "N")]
    pub max_pages: usize,
    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn run(_ctx: &Ctx, args: Args) -> Result<Exit> {
    let repo = args.repo.ok_or_else(|| {
        failure(
            Exit::Usage,
            "pr requires --repo OWNER/REPO or GITHUB_REPOSITORY",
        )
    })?;
    validate_repo(&repo)?;
    let from = DateTime::<Utc>::UNIX_EPOCH;
    let to = Utc
        .with_ymd_and_hms(9999, 12, 31, 23, 59, 59)
        .single()
        .expect("fixed far-future instant");
    let events = Github::new(io::token(), args.max_pages)?.collect(
        &repo,
        from,
        to,
        Some(args.number),
        &io::known(&args.agent_account)?,
    )?;
    let m = manifest::evaluate(events, io::load_policy(args.policy.as_deref())?)?;
    let (_, exit) = manifest::check(&m, None, None)?;
    args.output.render(&m, Format::Table)?;
    Ok(exit)
}
