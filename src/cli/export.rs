//! `acc export github OWNER/REPO --since DATE --until DATE`

use super::Ctx;
use super::forge::{self, Forge};
use super::io;
use crate::output::Format;
use crate::{Exit, canonical, failure};
use anyhow::Result;

#[derive(Debug, clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub forge: Forge,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let Forge::Github(c) = args.forge;
    if c.output.format.is_some_and(|f| f != Format::Json) {
        return Err(failure(Exit::Usage, "export supports JSON only"));
    }
    if c.policy.is_some() {
        return Err(failure(
            Exit::Usage,
            "export does not accept a policy; use evaluate",
        ));
    }
    let events = forge::collect(ctx, &c)?;
    let exit = if io::incomplete(&events) {
        Exit::Incomplete
    } else {
        Exit::Ok
    };
    ctx.debug(format!(
        "{} changes; writing json to {}",
        events.changes.len(),
        io::destination(c.output.output.as_deref())
    ));
    io::write(&canonical::jcs_bytes(&events)?, c.output.output.as_deref())?;
    if let Some(path) = &c.output.output {
        ctx.note(format!("wrote {}", path.display()));
    }
    Ok(exit)
}
