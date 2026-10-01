//! `acc doctor [--online] [--format text|json]`: is this environment ready to collect?
//!
//! Offline it reports the version, whether a token is set (never the token), whether the
//! default policy file parses, and which repository `scan` would collect. `--online` adds
//! GET-only checks against `api.github.com`: the token and the rate limit (`GET /rate_limit`)
//! and whether a newer acc has been released (`GET /repos/noru-tech/agent-change-control/
//! releases/latest`). That is acc's only update check, and it runs only when asked.
//!
//! Exit 0 when nothing is broken (warnings allowed), 2 when a check failed.

use super::Ctx;
use super::io::{self, DEFAULT_POLICY, ResultFormat};
use super::{detect, forge};
use crate::{Exit, Failure};
use anyhow::Result;
use serde::Serialize;
use std::path::Path;

/// The repository whose releases the update check reads.
pub const RELEASES_REPOSITORY: &str = "noru-tech/agent-change-control";

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Also check the token, the rate limit and the latest release on api.github.com (GET only).
    #[arg(long)]
    pub online: bool,
    /// Report format: `text` (a line per check) or `json` (one object).
    #[arg(short, long, value_enum, default_value = "text")]
    pub format: ResultFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Warn,
    Fail,
}

impl Status {
    const fn as_str(self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Warn => "warn",
            Status::Fail => "fail",
        }
    }
}

/// One check's outcome.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

impl Check {
    fn new(name: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            name,
            status,
            detail: detail.into(),
            hint: None,
        }
    }

    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// A failed check from an error, carrying the error's hint when it has one.
    fn failed(name: &'static str, err: &anyhow::Error) -> Self {
        let check = Self::new(name, Status::Fail, format!("{err:#}"));
        match err.downcast_ref::<Failure>().and_then(|f| f.hint.as_ref()) {
            Some(hint) => check.hint(hint.to_string()),
            None => check,
        }
    }
}

/// The whole report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub version: &'static str,
    pub healthy: bool,
    pub checks: Vec<Check>,
}

fn token_check() -> Check {
    match io::token_source() {
        Some((name, _)) => Check::new("token", Status::Ok, format!("{name} is set")),
        None => Check::new(
            "token",
            Status::Warn,
            "neither GITHUB_TOKEN nor GH_TOKEN is set: public repositories only, 60 requests an hour",
        )
        .hint("set GITHUB_TOKEN or GH_TOKEN to a token with read access to contents and pull requests"),
    }
}

fn policy_check() -> Check {
    let path = Path::new(DEFAULT_POLICY);
    if !path.exists() {
        return Check::new(
            "policy",
            Status::Ok,
            format!("{DEFAULT_POLICY} not found: the built-in defaults apply"),
        );
    }
    match io::load_policy(Some(path)) {
        Ok(policy) => Check::new(
            "policy",
            Status::Ok,
            format!(
                "{DEFAULT_POLICY} parses (fail_on: {})",
                format!("{:?}", policy.fail_on).to_ascii_lowercase()
            ),
        ),
        Err(err) => Check::failed("policy", &err),
    }
}

fn repository_check() -> Check {
    match detect::repository() {
        Some((repo, source)) => match crate::collectors::github::validate_repo(&repo) {
            Ok(()) => Check::new(
                "repository",
                Status::Ok,
                format!("{repo} (from {})", source.describe()),
            ),
            Err(err) => {
                Check::failed("repository", &err).hint("GITHUB_REPOSITORY must be OWNER/REPO")
            }
        },
        None => Check::new(
            "repository",
            Status::Warn,
            "none detected: scan and export need OWNER/REPO",
        )
        .hint(detect::NO_REPOSITORY),
    }
}

/// `(major, minor, patch)` of `v1.2.3` or `1.2.3`, ignoring any pre-release or build suffix.
pub fn parse_version(tag: &str) -> Option<(u64, u64, u64)> {
    let core = tag.strip_prefix('v').unwrap_or(tag);
    let core = core.split(['-', '+']).next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

fn online_checks(checks: &mut Vec<Check>) {
    let github = match forge::client(1) {
        Ok(github) => github,
        Err(err) => {
            checks.push(Check::failed("github", &err));
            return;
        }
    };
    let authenticated = io::token_source().is_some();
    checks.push(match github.rate_limit() {
        Ok(rate) => {
            let status = if rate.remaining == 0 {
                Status::Warn
            } else {
                Status::Ok
            };
            let reset = chrono::DateTime::from_timestamp(rate.reset as i64, 0)
                .map_or_else(|| rate.reset.to_string(), detect::show);
            Check::new(
                "github",
                status,
                format!(
                    "{}: {} of {} requests left this hour (resets {reset})",
                    if authenticated {
                        "token accepted"
                    } else {
                        "unauthenticated"
                    },
                    rate.remaining,
                    rate.limit
                ),
            )
        }
        Err(err) => Check::failed("github", &err),
    });
    let current = crate::version();
    checks.push(match github.latest_release(RELEASES_REPOSITORY) {
        Ok(tag) => match (parse_version(&tag), parse_version(current)) {
            (Some(latest), Some(mine)) if latest > mine => Check::new(
                "update",
                Status::Warn,
                format!(
                    "acc {} is available (this is {current})",
                    tag.trim_start_matches('v')
                ),
            )
            .hint("see https://github.com/noru-tech/agent-change-control/releases/latest"),
            (Some(_), Some(_)) => Check::new(
                "update",
                Status::Ok,
                format!("up to date (latest release {tag})"),
            ),
            _ => Check::new(
                "update",
                Status::Warn,
                format!("cannot compare the latest release tag {tag} with {current}"),
            ),
        },
        Err(err) => Check::failed("update", &err),
    });
}

/// Run every check.
pub fn report(online: bool) -> Report {
    let mut checks = vec![
        Check::new("version", Status::Ok, format!("acc {}", crate::version())),
        token_check(),
        policy_check(),
        repository_check(),
    ];
    if online {
        online_checks(&mut checks);
    }
    Report {
        version: crate::version(),
        healthy: checks.iter().all(|c| c.status != Status::Fail),
        checks,
    }
}

/// The text report: one line per check, a `help:` line under any with a hint, and a verdict.
pub fn render_text(report: &Report) -> String {
    let mut out = String::new();
    for c in &report.checks {
        out.push_str(&format!(
            "{:<4}  {:<10}  {}\n",
            c.status.as_str(),
            c.name,
            c.detail
        ));
        if let Some(hint) = &c.hint {
            out.push_str(&format!("{:<16}help: {hint}\n", ""));
        }
    }
    let failed = report
        .checks
        .iter()
        .filter(|c| c.status == Status::Fail)
        .count();
    out.push_str(&match failed {
        0 => "healthy\n".to_string(),
        n => format!("{n} check(s) failed\n"),
    });
    out
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    if !args.online {
        ctx.debug("offline: pass --online to check the token, the rate limit and for updates");
    }
    let report = report(args.online);
    let rendered = match args.format {
        ResultFormat::Text => render_text(&report),
        ResultFormat::Json => crate::canonical::jcs_bytes(&report)? + "\n",
    };
    io::write(&rendered, None)?;
    Ok(if report.healthy {
        Exit::Ok
    } else {
        Exit::Usage
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert_eq!(parse_version("v0.5.10"), Some((0, 5, 10)));
        assert_eq!(parse_version("1.2.3-rc.1"), Some((1, 2, 3)));
        assert!(parse_version("v0.10.0") > parse_version("0.9.9"));
        assert_eq!(parse_version("v1.2"), None);
        assert_eq!(parse_version("v1.2.3.4"), None);
        assert_eq!(parse_version("latest"), None);
    }

    #[test]
    fn the_report_is_healthy_unless_a_check_failed() {
        let report = Report {
            version: "0.0.0",
            healthy: true,
            checks: vec![
                Check::new("version", Status::Ok, "acc 0.0.0"),
                Check::new("token", Status::Warn, "unset").hint("set one"),
            ],
        };
        let text = render_text(&report);
        assert!(text.contains("warn  token       unset\n"), "{text}");
        assert!(text.contains("help: set one"), "{text}");
        assert!(text.ends_with("healthy\n"));
    }
}
