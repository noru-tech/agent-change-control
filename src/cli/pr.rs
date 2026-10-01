//! `acc pr NUMBER --repo OWNER/REPO`

use super::io::{self, EvidenceArgs, OutputArgs};
use super::{Ctx, forge};
use crate::collectors::github::{Sources, validate_repo};
use crate::output::Format;
use crate::{Exit, failure_with_hint, manifest};
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
    #[command(flatten)]
    pub evidence: EvidenceArgs,
    /// Maximum pages per list endpoint (100 items each).
    #[arg(long, default_value_t = 100, value_name = "N")]
    pub max_pages: usize,
    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let repo = args.repo.ok_or_else(|| {
        failure_with_hint(
            Exit::Usage,
            "pr requires --repo OWNER/REPO or GITHUB_REPOSITORY",
            "pass --repo OWNER/REPO, or set GITHUB_REPOSITORY (GitHub Actions sets it for you)",
        )
    })?;
    validate_repo(&repo)?;
    let from = DateTime::<Utc>::UNIX_EPOCH;
    let to = Utc
        .with_ymd_and_hms(9999, 12, 31, 23, 59, 59)
        .single()
        .expect("fixed far-future instant");
    let known = io::known(&args.agent_account)?;
    let registry = args.evidence.registry()?;
    let traces = args.evidence.traces()?;
    let attestations = args.evidence.attestations()?;
    let vendors = args.evidence.vendors()?;
    ctx.debug(format!("collecting {repo} pull request {}", args.number));
    forge::debug_auth(ctx);
    let events = forge::client(args.max_pages)?.collect(
        &repo,
        from,
        to,
        Some(args.number),
        Sources {
            known: &known,
            trailers: registry.as_ref(),
            traces: traces.as_ref(),
            attestations: attestations.as_ref(),
            vendors: &vendors,
        },
    )?;
    ctx.debug(format!(
        "policy: {}",
        io::policy_source(args.policy.as_deref())
    ));
    let m = manifest::evaluate(events, io::load_policy(args.policy.as_deref())?)?;
    let (_, exit) = manifest::check(&m, None, None)?;
    args.output.render(ctx, &m, Format::Table)?;
    if exit == Exit::Incomplete {
        super::warn_incomplete(
            ctx,
            &m.events,
            "raise --max-pages (at most 1000) or retry once the pull request stops changing",
        );
    }
    Ok(exit)
}
