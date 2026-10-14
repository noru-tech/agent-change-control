//! Zero-config defaults for the collecting commands: the forge and repository from the CI
//! environment (`GITHUB_REPOSITORY`, or GitLab's `CI_PROJECT_PATH`) or the `origin` remote, and
//! a window of the last 30 days ending now.
//!
//! Only collection reads the environment, `git` and the clock. The resolved values are recorded
//! in the output exactly as if they had been passed, so evaluation stays deterministic.

use super::forge::Kind;
use crate::collectors::github::validate_repo;
use crate::collectors::gitlab::validate_project;
use crate::model::Timestamp;
use crate::{Exit, failure, failure_with_hint};
use anyhow::Result;
use chrono::{DateTime, Days, SecondsFormat, Utc};

/// The default window length, in days.
pub const DEFAULT_DAYS: u64 = 30;

/// Testing only: an RFC 3339 instant used instead of the clock for the default window.
pub const NOW_ENV: &str = "ACC_NOW";

/// Where a detected repository came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Environment,
    GitlabEnvironment,
    Remote,
}

impl Source {
    pub const fn describe(self) -> &'static str {
        match self {
            Source::Environment => "GITHUB_REPOSITORY",
            Source::GitlabEnvironment => "CI_PROJECT_PATH",
            Source::Remote => "the origin remote",
        }
    }
}

/// The host and path of a remote URL in any of its usual forms: `https://HOST/PATH`,
/// `git@HOST:PATH`, `ssh://git@HOST[:PORT]/PATH` and `git://HOST/PATH`, with `.git` and a
/// trailing slash removed.
fn remote_parts(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let (host, path) = if let Some(rest) = url.strip_prefix("git@") {
        let (host, path) = rest.split_once(':')?;
        (host.to_string(), path)
    } else {
        let rest = ["https://", "http://", "ssh://", "git://"]
            .iter()
            .find_map(|scheme| url.strip_prefix(scheme))?;
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?;
        (host.split(':').next()?.to_string(), path)
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    Some((host.to_ascii_lowercase(), path.to_string()))
}

/// `GROUP/…/PROJECT` from a gitlab.com remote URL. Self-managed GitLab is not collected yet.
pub fn parse_gitlab_remote(url: &str) -> Option<String> {
    let (host, path) = remote_parts(url)?;
    (host == "gitlab.com" && validate_project(&path).is_ok()).then_some(path)
}

fn origin() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn set(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The GitLab project from `CI_PROJECT_PATH` in a gitlab.com pipeline, else from a gitlab.com
/// origin remote, with where it came from.
pub fn gitlab_project() -> Option<(String, Source)> {
    if set("GITLAB_CI").is_some()
        && set("CI_SERVER_HOST").is_none_or(|h| h.eq_ignore_ascii_case("gitlab.com"))
        && let Some(path) = set("CI_PROJECT_PATH")
    {
        return Some((path, Source::GitlabEnvironment));
    }
    origin()
        .as_deref()
        .and_then(parse_gitlab_remote)
        .map(|p| (p, Source::Remote))
}

/// Which forge to collect from when none is named: GitHub Actions and GitLab CI say so
/// themselves; otherwise the origin remote's host decides, and GitHub is the default.
pub fn forge() -> Kind {
    if set("GITHUB_REPOSITORY").is_some() {
        Kind::Github
    } else if set("GITLAB_CI").is_some()
        || origin().as_deref().and_then(parse_gitlab_remote).is_some()
    {
        Kind::Gitlab
    } else {
        Kind::Github
    }
}

/// `OWNER/REPO` from a github.com remote URL in any of its usual forms: `https://github.com/O/R`,
/// `git@github.com:O/R`, `ssh://git@github.com/O/R` and `git://github.com/O/R`, each with or
/// without `.git` and a trailing slash.
pub fn parse_remote(url: &str) -> Option<String> {
    let (host, path) = remote_parts(url)?;
    (host == "github.com" && validate_repo(&path).is_ok()).then_some(path)
}

/// The repository from `GITHUB_REPOSITORY`, else from `git remote get-url origin` in the working
/// directory, with where it came from.
pub fn repository() -> Option<(String, Source)> {
    if let Some(repo) = std::env::var("GITHUB_REPOSITORY")
        .ok()
        .filter(|r| !r.is_empty())
    {
        return Some((repo, Source::Environment));
    }
    origin()
        .as_deref()
        .and_then(parse_remote)
        .map(|r| (r, Source::Remote))
}

/// The hint for a repository that was neither given nor detected.
pub const NO_REPOSITORY: &str = "pass OWNER/REPO, set GITHUB_REPOSITORY (GitHub Actions sets it for you), or run inside a clone whose origin remote is on github.com";

/// The hint for a GitLab project that was neither given nor detected.
pub const NO_PROJECT: &str = "pass GROUP/PROJECT, run in a gitlab.com pipeline (GitLab sets CI_PROJECT_PATH), or run inside a clone whose origin remote is on gitlab.com";

/// The given repository, else the one detected for `kind` (reported through `note`), else a
/// usage error.
pub fn resolve_repository(
    kind: Kind,
    given: Option<&str>,
    note: impl Fn(String),
) -> Result<String> {
    let repo = match given {
        Some(repo) => repo.to_string(),
        None => {
            let (detected, hint) = match kind {
                Kind::Github => (repository(), NO_REPOSITORY),
                Kind::Gitlab => (gitlab_project(), NO_PROJECT),
            };
            let (repo, source) = detected.ok_or_else(|| {
                failure_with_hint(Exit::Usage, "no repository given and none detected", hint)
            })?;
            note(format!("repository: {repo} (from {})", source.describe()));
            repo
        }
    };
    match kind {
        Kind::Github => validate_repo(&repo)?,
        Kind::Gitlab => validate_project(&repo)?,
    }
    Ok(repo)
}

/// Now, to the second: the clock, or [`NOW_ENV`] in tests.
pub fn now() -> Result<Timestamp> {
    let now = match std::env::var(NOW_ENV) {
        Ok(s) if !s.is_empty() => crate::normalize::timestamp(&s)
            .map_err(|_| failure(Exit::Usage, "ACC_NOW must be an RFC 3339 timestamp"))?,
        _ => Utc::now(),
    };
    DateTime::from_timestamp(now.timestamp(), 0)
        .ok_or_else(|| failure(Exit::Usage, "time out of range"))
}

/// Fill in a missing window edge: the end defaults to now, the start to [`DEFAULT_DAYS`] before
/// the end.
pub fn window(
    since: Option<Timestamp>,
    until: Option<Timestamp>,
    now: impl FnOnce() -> Result<Timestamp>,
) -> Result<(Timestamp, Timestamp)> {
    let until = match until {
        Some(t) => t,
        None => now()?,
    };
    let since = match since {
        Some(t) => t,
        None => until
            .checked_sub_days(Days::new(DEFAULT_DAYS))
            .ok_or_else(|| failure(Exit::Usage, "time out of range"))?,
    };
    Ok((since, until))
}

/// An instant as it is printed in the resolved-window line.
pub fn show(t: Timestamp) -> String {
    t.to_rfc3339_opts(SecondsFormat::AutoSi, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_remotes_in_every_form() {
        for url in [
            "https://github.com/acme/api",
            "https://github.com/acme/api.git",
            "https://github.com/acme/api/",
            "https://user@github.com/acme/api.git",
            "http://github.com/acme/api",
            "git@github.com:acme/api.git",
            "git@github.com:acme/api",
            "ssh://git@github.com/acme/api.git",
            "ssh://git@github.com:22/acme/api.git",
            "git://github.com/acme/api.git",
            "https://GitHub.com/acme/api\n",
        ] {
            assert_eq!(parse_remote(url).as_deref(), Some("acme/api"), "{url}");
        }
        for url in [
            "https://gitlab.com/acme/api.git",
            "git@gitlab.com:acme/api.git",
            "https://github.com.evil.example/acme/api",
            "https://github.com/acme",
            "https://github.com/acme/api/tree/main",
            "/home/me/api",
            "",
        ] {
            assert_eq!(parse_remote(url), None, "{url}");
        }
    }

    #[test]
    fn gitlab_remotes_in_every_form() {
        for (url, path) in [
            ("https://gitlab.com/acme/api", "acme/api"),
            (
                "https://gitlab.com/acme/platform/api.git",
                "acme/platform/api",
            ),
            ("git@gitlab.com:acme/api.git", "acme/api"),
            (
                "ssh://git@gitlab.com:22/acme/platform/api.git",
                "acme/platform/api",
            ),
            ("https://oauth2:x@GitLab.com/acme/api/\n", "acme/api"),
        ] {
            assert_eq!(parse_gitlab_remote(url).as_deref(), Some(path), "{url}");
        }
        for url in [
            "https://github.com/acme/api.git",
            "https://gitlab.example.com/acme/api.git",
            "https://gitlab.com.evil.example/acme/api",
            "https://gitlab.com/acme",
            "",
        ] {
            assert_eq!(parse_gitlab_remote(url), None, "{url}");
        }
    }

    #[test]
    fn the_default_window_is_the_last_30_days_ending_now() {
        let t = |s: &str| crate::normalize::timestamp(s).unwrap();
        let now = || Ok(t("2026-10-01T12:00:00Z"));
        assert_eq!(
            window(None, None, now).unwrap(),
            (t("2026-09-01T12:00:00Z"), t("2026-10-01T12:00:00Z"))
        );
        assert_eq!(
            window(Some(t("2026-09-20T00:00:00Z")), None, now).unwrap(),
            (t("2026-09-20T00:00:00Z"), t("2026-10-01T12:00:00Z"))
        );
        assert_eq!(
            window(None, Some(t("2026-08-31T23:59:59Z")), || unreachable!()).unwrap(),
            (t("2026-08-01T23:59:59Z"), t("2026-08-31T23:59:59Z"))
        );
    }
}
