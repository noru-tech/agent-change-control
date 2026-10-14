//! `acc pr NUMBER --repo OWNER/REPO` (or `--forge gitlab --repo GROUP/PROJECT`)

use super::forge::Kind;
use super::io::{self, EvidenceArgs, OutputArgs};
use super::{Ctx, detect, forge};
use crate::collectors::Sources;
use crate::model::RuleId;
use crate::output::Format;
use crate::{Exit, failure_with_hint, manifest};
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Pull request number (a merge request's IID on GitLab).
    pub number: u64,
    /// Repository as OWNER/REPO (GitHub) or GROUP/…/PROJECT (GitLab); defaults to
    /// GITHUB_REPOSITORY or CI_PROJECT_PATH, else the origin remote of the working directory.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: Option<String>,
    /// The forge; defaults to the one GitHub Actions or GitLab CI reports, else the origin
    /// remote's, else GitHub.
    #[arg(long, value_enum)]
    pub forge: Option<Kind>,
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
    let kind = args.forge.unwrap_or_else(detect::forge);
    let detected = match kind {
        Kind::Github => detect::repository(),
        Kind::Gitlab => detect::gitlab_project(),
    };
    let repo = match args.repo {
        Some(repo) => repo,
        None => detected
            .map(|(repo, source)| {
                ctx.note(format!("repository: {repo} (from {})", source.describe()));
                repo
            })
            .ok_or_else(|| match kind {
                Kind::Github => failure_with_hint(
                    Exit::Usage,
                    "pr requires --repo OWNER/REPO or GITHUB_REPOSITORY",
                    "pass --repo OWNER/REPO, set GITHUB_REPOSITORY (GitHub Actions sets it for you), or run inside a clone whose origin remote is on github.com",
                ),
                Kind::Gitlab => failure_with_hint(
                    Exit::Usage,
                    "pr requires --repo GROUP/PROJECT or CI_PROJECT_PATH",
                    detect::NO_PROJECT,
                ),
            })?,
    };
    let repo = detect::resolve_repository(kind, Some(&repo), |_| {})?;
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
    ctx.debug(format!(
        "policy: {}",
        io::policy_source(args.policy.as_deref())
    ));
    let mut policy = io::load_policy(args.policy.as_deref())?;
    // ACC012 and ACC013 judge a window's branch updates and deployments, not one change: `pr`
    // leaves them to `scan`, and records that it did in the policy it embeds.
    for rule in [RuleId::Acc012, RuleId::Acc013] {
        if let Some(r) = policy.rules.get_mut(&rule.name())
            && r.enabled
        {
            r.enabled = false;
            ctx.note(format!(
                "{rule} judges a window, not one change; run `acc scan` for it"
            ));
        }
    }
    let teams = manifest::teams_required_by(&policy);
    ctx.debug(format!("collecting {repo} change {}", args.number));
    forge::debug_auth(ctx, kind);
    let events = forge::Client::new(kind, args.max_pages)?.collect(
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
            checks: manifest::checks_required(&policy),
            bypass_as_of: manifest::bypass_required(&policy).then(chrono::Utc::now),
            teams: (!teams.is_empty()).then(|| (teams.as_slice(), chrono::Utc::now())),
            branches: None,
            deployments: None,
        },
    )?;
    let m = manifest::evaluate(events, policy)?;
    // A fresh collection has no dispositions, so the date only places an emergency's
    // retroactive review inside or past its window.
    let (_, exit) = manifest::check(&m, None, Some(chrono::Utc::now().date_naive()))?;
    args.output.render(ctx, &m, Format::Table)?;
    if exit == Exit::Incomplete {
        super::warn_incomplete(
            ctx,
            &m.events,
            Some(&m.policy),
            "raise --max-pages (at most 1000) or retry once the pull request stops changing",
        );
    }
    Ok(exit)
}
