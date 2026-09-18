//! `acc scan github OWNER/REPO --since DATE --until DATE`

use super::Ctx;
use super::forge::{self, Forge};
use super::io::{self, DEFAULT_MANIFEST};
use crate::output::Format;
use crate::{Exit, manifest};
use anyhow::Result;

#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub forge: Forge,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let Forge::Github(mut c) = args.forge;
    let policy = io::load_policy(c.policy.as_deref())?;
    let events = forge::collect(&c)?;
    let exit = if io::incomplete(&events) {
        Exit::Incomplete
    } else {
        Exit::Ok
    };
    let m = manifest::evaluate(events, policy)?;
    if c.output.output.is_none() && c.output.format.is_none() {
        c.output.output = Some(DEFAULT_MANIFEST.into());
    }
    c.output.render(&m, Format::Yaml)?;
    if let Some(path) = &c.output.output {
        ctx.note(format!("wrote {}", path.display()));
    }
    Ok(exit)
}
