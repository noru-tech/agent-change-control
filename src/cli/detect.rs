//! Zero-config defaults for the collecting commands: the repository from `GITHUB_REPOSITORY` or
//! the `origin` remote, and a window of the last 30 days ending now.
//!
//! Only collection reads the environment, `git` and the clock. The resolved values are recorded
//! in the output exactly as if they had been passed, so evaluation stays deterministic.

use crate::collectors::github::validate_repo;
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
    Remote,
}

impl Source {
    pub const fn describe(self) -> &'static str {
        match self {
            Source::Environment => "GITHUB_REPOSITORY",
            Source::Remote => "the origin remote",
        }
    }
}

/// `OWNER/REPO` from a github.com remote URL in any of its usual forms: `https://github.com/O/R`,
/// `git@github.com:O/R`, `ssh://git@github.com/O/R` and `git://github.com/O/R`, each with or
/// without `.git` and a trailing slash.
pub fn parse_remote(url: &str) -> Option<String> {
    let url = url.trim();
    let path = if let Some(rest) = url.strip_prefix("git@github.com:") {
        rest
    } else {
        let rest = ["https://", "http://", "ssh://", "git://"]
            .iter()
            .find_map(|scheme| url.strip_prefix(scheme))?;
        let (authority, path) = rest.split_once('/')?;
        let host = authority.rsplit('@').next()?;
        let host = host.split(':').next()?;
        if !host.eq_ignore_ascii_case("github.com") {
            return None;
        }
        path
    };
    let path = path.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    validate_repo(path).ok().map(|()| path.to_string())
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
    let out = std::process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    parse_remote(&String::from_utf8_lossy(&out.stdout)).map(|r| (r, Source::Remote))
}

/// The hint for a repository that was neither given nor detected.
pub const NO_REPOSITORY: &str = "pass OWNER/REPO, set GITHUB_REPOSITORY (GitHub Actions sets it for you), or run inside a clone whose origin remote is on github.com";

/// The given repository, else the detected one (reported through `note`), else a usage error.
pub fn resolve_repository(given: Option<&str>, note: impl Fn(String)) -> Result<String> {
    let repo = match given {
        Some(repo) => repo.to_string(),
        None => {
            let (repo, source) = repository().ok_or_else(|| {
                failure_with_hint(
                    Exit::Usage,
                    "no repository given and none detected",
                    NO_REPOSITORY,
                )
            })?;
            note(format!("repository: {repo} (from {})", source.describe()));
            repo
        }
    };
    validate_repo(&repo)?;
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
