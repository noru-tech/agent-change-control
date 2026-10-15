//! The forge selector and collection flags shared by `scan` and `export`.

use super::Ctx;
use super::detect;
use super::io::{self, EvidenceArgs, OutputArgs};
use crate::collectors::github::Github;
use crate::collectors::gitlab::Gitlab;
use crate::collectors::{DeploymentQuery, Sources};
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
    /// A GitLab.com project. Reads GITLAB_TOKEN (read_api scope); public projects need none.
    Gitlab(Collect),
}

/// A forge, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Kind {
    Github,
    Gitlab,
}

/// `scan`/`export` arguments: an optional forge and, without one, the collection flags directly,
/// so `acc scan` works with no arguments in a clone or a pipeline. The forge is then the one
/// GitHub Actions or GitLab CI reports, else the origin remote's, else GitHub.
#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Target {
    #[command(subcommand)]
    pub forge: Option<Forge>,
    #[command(flatten)]
    pub collect: Collect,
}

impl Target {
    /// The forge and the collection flags, whichever way they were given.
    pub fn into_collect(self) -> (Kind, Collect) {
        match self.forge {
            Some(Forge::Github(c)) => (Kind::Github, c),
            Some(Forge::Gitlab(c)) => (Kind::Gitlab, c),
            None => (detect::forge(), self.collect),
        }
    }
}

#[derive(Debug, Args)]
pub struct Collect {
    /// Repository as OWNER/REPO (GitHub) or GROUP/…/PROJECT (GitLab); defaults to
    /// GITHUB_REPOSITORY or CI_PROJECT_PATH, else the origin remote of the working directory.
    pub repository: Option<String>,
    /// Start of the merge window, inclusive: YYYY-MM-DD (start of that UTC day) or RFC 3339.
    /// Defaults to 30 days before --until.
    #[arg(long, value_name = "DATE")]
    pub since: Option<String>,
    /// End of the merge window, inclusive: YYYY-MM-DD (end of that UTC day) or RFC 3339.
    /// Defaults to now. A defaulted window is printed and recorded like an explicit one.
    #[arg(long, value_name = "DATE")]
    pub until: Option<String>,
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
    /// Also collect check runs, commit statuses (GitLab: pipelines and jobs) and the required
    /// checks for merged changes (ACC004). Implied when the policy enables ACC004.
    #[arg(long)]
    pub checks: bool,
    /// Also collect ruleset evaluations of merged changes (ACC005); the token needs repository
    /// administration read access. GitLab records none, so ACC005 stays unknown there. Implied
    /// when the policy enables ACC005.
    #[arg(long)]
    pub bypass: bool,
    /// Also record this reviewer team's members (ORG/SLUG, or a GitLab group path; repeatable;
    /// ACC011). The token needs to read the members. Implied for the teams the policy's tiers
    /// require.
    #[arg(long, value_name = "ORG/SLUG")]
    pub team: Vec<String>,
    /// Also record every update of the repository's default branch in the window (ACC012).
    /// Implied when the policy enables ACC012.
    #[arg(long)]
    pub branches: bool,
    /// Also record every update of this protected branch in the window (repeatable; ACC012).
    #[arg(long, value_name = "NAME")]
    pub branch: Vec<String>,
    /// Also record successful deployments to production environments and the commits each
    /// shipped (ACC013): those the forge marks as production. Implied when the policy enables
    /// ACC013.
    #[arg(long)]
    pub deployments: bool,
    /// Also record deployments to this environment as production (repeatable; ACC013).
    #[arg(long, value_name = "NAME")]
    pub environment: Vec<String>,
    /// Start of the deployment window (default: the start of the merge window). Keep it later
    /// than --since, so the changes a deployment shipped were merged inside the merge window.
    #[arg(long, value_name = "DATE")]
    pub deploy_since: Option<String>,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// Report under `--verbose` whether requests are authenticated, naming the variable but never
/// the token.
pub fn debug_auth(ctx: &Ctx, kind: Kind) {
    let source = match kind {
        Kind::Github => io::token_source(),
        Kind::Gitlab => io::gitlab_token_source(),
    };
    ctx.debug(match source {
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

/// Testing only: [`API_URL_ENV`]'s counterpart for GitLab, with the same restrictions, so a
/// token can never be sent anywhere but `gitlab.com`.
pub const GITLAB_API_URL_ENV: &str = "ACC_GITLAB_API_URL";

/// The GitHub client for the environment's token, honoring the testing-only [`API_URL_ENV`].
pub fn client(max_pages: usize) -> Result<Github> {
    match std::env::var(API_URL_ENV) {
        Ok(base) if !base.is_empty() => Github::with_base(io::token(), max_pages, &base),
        _ => Github::new(io::token(), max_pages),
    }
}

/// A collector for either forge.
pub enum Client {
    Github(Github),
    Gitlab(Gitlab),
}

impl Client {
    /// The client for `kind` and the environment's token for it.
    pub fn new(kind: Kind, max_pages: usize) -> Result<Self> {
        Ok(match kind {
            Kind::Github => Client::Github(client(max_pages)?),
            Kind::Gitlab => {
                let token = io::gitlab_token_source().map(|(_, t)| t);
                Client::Gitlab(match std::env::var(GITLAB_API_URL_ENV) {
                    Ok(base) if !base.is_empty() => Gitlab::with_base(token, max_pages, &base)?,
                    _ => Gitlab::new(token, max_pages)?,
                })
            }
        })
    }

    /// Collect the changes merged in `from..=to`, or the single change `number`.
    pub fn collect(
        &self,
        repository: &str,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
        number: Option<u64>,
        sources: Sources<'_>,
    ) -> Result<Events> {
        match self {
            Client::Github(g) => g.collect(repository, from, to, number, sources),
            Client::Gitlab(g) => g.collect(repository, from, to, number, sources),
        }
    }
}

/// The facts the opt-in rules need, beyond the changes themselves.
#[derive(Debug, Clone, Default)]
pub struct Wanted {
    pub checks: bool,
    pub bypass: bool,
    pub teams: Vec<String>,
    /// Protected branches to record; `Some(empty)` names the default branch.
    pub branches: Option<Vec<String>>,
    /// Production environments to record; `Some(empty)` means the forge's production flag.
    pub environments: Option<Vec<String>>,
}

impl Wanted {
    /// What `p` needs collected.
    pub fn for_policy(p: &crate::model::Policy) -> Self {
        use crate::manifest;
        Self {
            checks: manifest::checks_required(p),
            bypass: manifest::bypass_required(p),
            teams: manifest::teams_required_by(p),
            branches: manifest::branches_required(p)
                .then(|| p.branches.clone().unwrap_or_default().protected),
            environments: manifest::deployments_required(p)
                .then(|| p.deployments.clone().unwrap_or_default().environments),
        }
    }
}

/// Merge requested names with the flags: `None` unless something asked, the union otherwise.
fn names(wanted: &Option<Vec<String>>, flag: bool, given: &[String]) -> Option<Vec<String>> {
    if wanted.is_none() && !flag && given.is_empty() {
        return None;
    }
    let mut out: Vec<String> = wanted.iter().flatten().chain(given).cloned().collect();
    out.sort();
    out.dedup();
    Some(out)
}

/// Collect the pull requests merged in the window, and whatever `wanted` asks for whatever the
/// flags say: the caller's policy needs it.
pub fn collect(ctx: &Ctx, kind: Kind, c: &Collect, wanted: &Wanted) -> Result<Events> {
    let (checks, bypass, teams) = (wanted.checks, wanted.bypass, wanted.teams.as_slice());
    let repository = detect::resolve_repository(kind, c.repository.as_deref(), |m| ctx.note(m))?;
    let since = c
        .since
        .as_deref()
        .map(|s| io::boundary(s, false))
        .transpose()?;
    let until = c
        .until
        .as_deref()
        .map(|s| io::boundary(s, true))
        .transpose()?;
    let (from, to) = detect::window(since, until, detect::now)?;
    if since.is_none() || until.is_none() {
        ctx.note(format!(
            "window: {} to {} (UTC; {}; pass --since/--until to choose)",
            detect::show(from),
            detect::show(to),
            match (since, until) {
                (None, None) => "the last 30 days",
                (Some(_), None) => "ending now",
                _ => "30 days before --until",
            }
        ));
    }
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
        repository,
        from.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        to.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        c.max_pages
    ));
    debug_auth(ctx, kind);
    let mut team_ids: Vec<String> = teams
        .iter()
        .chain(&c.team)
        .map(|t| t.to_ascii_lowercase())
        .collect();
    team_ids.sort();
    team_ids.dedup();
    let branches = names(&wanted.branches, c.branches, &c.branch);
    let environments = names(&wanted.environments, c.deployments, &c.environment);
    let deploy_from = c
        .deploy_since
        .as_deref()
        .map(|s| io::boundary(s, false))
        .transpose()?
        .unwrap_or(from);
    if deploy_from > to {
        return Err(failure_with_hint(
            Exit::Usage,
            "deployment window is reversed",
            "--deploy-since must not be later than --until",
        ));
    }
    let now = chrono::Utc::now();
    Client::new(kind, c.max_pages)?.collect(
        &repository,
        from,
        to,
        None,
        Sources {
            known: &known,
            trailers: registry.as_ref(),
            traces: traces.as_ref(),
            attestations: attestations.as_ref(),
            vendors: &vendors,
            checks: checks || c.checks,
            bypass_as_of: (bypass || c.bypass).then(chrono::Utc::now),
            teams: (!team_ids.is_empty()).then_some((team_ids.as_slice(), now)),
            branches: branches.as_deref().map(|b| (b, now)),
            deployments: environments.as_deref().map(|environments| DeploymentQuery {
                environments,
                from: deploy_from,
                to,
            }),
        },
    )
}
