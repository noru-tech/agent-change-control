//! `acc evaluate EVENTS`

use super::Ctx;
use super::io::{self, OutputArgs};
use crate::output::Format;
use crate::{Exit, manifest};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Normalized events (JSON or YAML), as written by `export`; with --conformance-json, a
    /// conformance vector.
    pub input: PathBuf,
    /// Policy file (defaults to .agent-change-control/policy.yml when present).
    #[arg(long, value_name = "FILE", conflicts_with = "conformance_json")]
    pub policy: Option<PathBuf>,
    /// Read INPUT as an ACP conformance vector (an object with `events` and `policy`) and print
    /// the corpus contract's single-line JSON result: verdict, codes, assessments and manifest
    /// digest. Exit 0 evaluated, 3 invalid, 4 incomplete. See conformance/README.md.
    #[arg(long, conflicts_with_all = ["format", "output"])]
    pub conformance_json: bool,
    #[command(flatten)]
    pub output: OutputArgs,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    if args.conformance_json {
        return super::conformance::run(&args.input);
    }
    let events = io::read(&args.input, "events")?;
    ctx.debug(format!(
        "policy: {}",
        io::policy_source(args.policy.as_deref())
    ));
    let m = manifest::evaluate(events, io::load_policy(args.policy.as_deref())?)?;
    args.output.render(ctx, &m, Format::Json)?;
    if io::incomplete(&m.events) {
        super::warn_incomplete(ctx, &m.events, super::RECOLLECTED);
        return Ok(Exit::Incomplete);
    }
    Ok(Exit::Ok)
}
