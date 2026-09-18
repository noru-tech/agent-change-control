//! `acc completions SHELL` and `acc manpage [--out-dir DIR]`

use super::{Cli, Ctx};
use crate::Exit;
use anyhow::Result;
use clap::CommandFactory;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
}

pub fn run(_ctx: &Ctx, args: Args) -> Result<Exit> {
    let mut cmd = Cli::command();
    clap_complete::generate(args.shell, &mut cmd, "acc", &mut std::io::stdout());
    Ok(Exit::Ok)
}

#[derive(Debug, clap::Args)]
pub struct ManArgs {
    /// Write one page per command into DIR instead of printing acc.1 to stdout.
    #[arg(long, value_name = "DIR")]
    pub out_dir: Option<PathBuf>,
}

pub fn run_man(ctx: &Ctx, args: ManArgs) -> Result<Exit> {
    let cmd = Cli::command();
    if let Some(dir) = args.out_dir {
        std::fs::create_dir_all(&dir)?;
        clap_mangen::generate_to(cmd, &dir)?;
        ctx.note(format!("wrote man pages to {}", dir.display()));
    } else {
        let mut buf = Vec::new();
        clap_mangen::Man::new(cmd).render(&mut buf)?;
        std::io::stdout().lock().write_all(&buf)?;
    }
    Ok(Exit::Ok)
}
