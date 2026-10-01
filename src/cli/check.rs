//! `acc check MANIFEST [--policy FILE] [--as-of DATE]`

use super::Ctx;
use super::io::{self, OutputArgs};
use crate::output::Format;
use crate::{Exit, failure_with_hint, manifest};
use anyhow::Result;
use chrono::NaiveDate;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// A manifest (JSON or YAML) written by `scan` or `evaluate`.
    pub input: PathBuf,
    /// Policy file; overrides the policy embedded in the manifest.
    #[arg(long, value_name = "FILE")]
    pub policy: Option<PathBuf>,
    /// Calendar date dispositions are evaluated on (YYYY-MM-DD). Required when any finding has
    /// a non-open disposition; the machine clock is never consulted.
    #[arg(long, value_name = "DATE")]
    pub as_of: Option<String>,
    #[command(flatten)]
    pub output: OutputArgs,
}

/// Parse `--as-of` as a calendar date (accepting exactly what clap's `NaiveDate` parser did).
pub fn as_of(s: &str) -> Result<NaiveDate> {
    s.parse::<NaiveDate>().map_err(|_| {
            failure_with_hint(
                Exit::Usage,
                format!("invalid --as-of date: {s}"),
                "use a calendar date such as --as-of 2026-09-30; dispositions are evaluated on that date and acc never reads the clock",
            )
        })
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let as_of = args.as_of.as_deref().map(as_of).transpose()?;
    let m = io::read(&args.input, "manifest")?;
    let policy = args
        .policy
        .as_deref()
        .map(|p| io::load_policy(Some(p)))
        .transpose()?;
    ctx.debug(format!(
        "policy: {}",
        args.policy.as_deref().map_or_else(
            || "embedded in the manifest".into(),
            |p| p.display().to_string()
        )
    ));
    if let Some(date) = as_of {
        ctx.debug(format!("dispositions evaluated as of {date}"));
    }
    let (checked, exit) = manifest::check(&m, policy, as_of)?;
    args.output.render(ctx, &checked, Format::Table)?;
    if exit == Exit::Incomplete {
        super::warn_incomplete(ctx, &checked.events, super::RECOLLECTED);
    }
    Ok(exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_of_is_a_calendar_date() {
        assert_eq!(
            as_of("2026-09-30").unwrap(),
            NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()
        );
        for bad in ["yesterday", "2026-02-30", "2026-09-30T00:00:00Z", ""] {
            assert_eq!(
                crate::exit_for(&as_of(bad).unwrap_err()),
                Exit::Usage,
                "{bad}"
            );
        }
    }
}
