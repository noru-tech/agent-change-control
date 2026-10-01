//! `acc completions SHELL [--output FILE]` and `acc manpage [--output FILE | --out-dir DIR]`

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
    /// Write the script to FILE instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

/// Write `bytes` to `path` (creating parent directories) or stdout.
fn emit(ctx: &Ctx, bytes: &[u8], path: Option<&std::path::Path>) -> Result<()> {
    match path {
        Some(path) => {
            super::io::write(&String::from_utf8_lossy(bytes), Some(path))?;
            ctx.note(format!("wrote {}", path.display()));
        }
        None => std::io::stdout().lock().write_all(bytes)?,
    }
    Ok(())
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    clap_complete::generate(args.shell, &mut cmd, "acc", &mut buf);
    emit(ctx, &buf, args.output.as_deref())?;
    Ok(Exit::Ok)
}

#[derive(Debug, clap::Args)]
pub struct ManArgs {
    /// Write one page per command into DIR instead of printing acc.1 to stdout.
    #[arg(long, value_name = "DIR", conflicts_with = "output")]
    pub out_dir: Option<PathBuf>,
    /// Write acc.1 (the top-level page) to FILE instead of stdout.
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
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
        emit(ctx, &buf, args.output.as_deref())?;
    }
    Ok(Exit::Ok)
}
