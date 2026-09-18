//! `acc evaluate EVENTS`

use super::Ctx;
use super::io::{self, OutputArgs};
use crate::output::Format;
use crate::{Exit, manifest};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Normalized events (JSON or YAML), as written by `export`.
    pub input: PathBuf,
    /// Policy file (defaults to .agent-change-control/policy.yml when present).
    #[arg(long, value_name = "FILE")]
    pub policy: Option<PathBuf>,
    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn run(_ctx: &Ctx, args: Args) -> Result<Exit> {
    let events = io::read(&args.input, "events")?;
    let m = manifest::evaluate(events, io::load_policy(args.policy.as_deref())?)?;
    args.output.render(&m, Format::Json)?;
    Ok(if io::incomplete(&m.events) {
        Exit::Incomplete
    } else {
        Exit::Ok
    })
}
