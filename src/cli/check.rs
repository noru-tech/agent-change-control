//! `acc check MANIFEST [--policy FILE] [--as-of DATE]`

use super::Ctx;
use super::io::{self, OutputArgs};
use crate::output::Format;
use crate::{Exit, manifest};
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
    pub as_of: Option<NaiveDate>,
    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn run(_ctx: &Ctx, args: Args) -> Result<Exit> {
    let m = io::read(&args.input, "manifest")?;
    let policy = args
        .policy
        .as_deref()
        .map(|p| io::load_policy(Some(p)))
        .transpose()?;
    let (checked, exit) = manifest::check(&m, policy, args.as_of)?;
    args.output.render(&checked, Format::Table)?;
    Ok(exit)
}
