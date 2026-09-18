//! Command-line surface. Each subcommand lives in its own module and gets a [`Ctx`].

pub mod check;
pub mod completions;
pub mod evaluate;
pub mod export;
pub mod forge;
pub mod io;
pub mod pr;
pub mod scan;
pub mod validate;

use crate::Exit;
use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::process::ExitCode;

const ABOUT: &str = "acc — change control for software written with coding agents";
const LONG_ABOUT: &str = "\
acc — change control for software written with coding agents.

Records the agent, human operator, reviewers and merger of each pull request, then evaluates
explicit separation-of-duty rules (ACC001, ACC002, ACC003, ACC006) deterministically and offline.
Only `scan`, `export` and `pr` contact GitHub; `evaluate`, `validate` and `check` never touch the
network or the clock. No LLM is involved, and agent authorship alone is never a finding.

Exit codes: 0 ok · 1 policy threshold exceeded (check, pr) · 2 usage · 3 invalid input or
manifest · 4 collection incomplete (beats 1) · 5 authentication rejected · 6 API, permission,
rate-limit or transport failure · 7 unsupported API data.";

#[derive(Debug, Parser)]
#[command(
    name = "acc",
    version,
    about = ABOUT,
    long_about = LONG_ABOUT,
    propagate_version = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Args, Default)]
pub struct Global {
    /// Suppress status lines on stderr.
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Collect facts from a forge and write an evaluated manifest.
    Scan(scan::Args),
    /// Export normalized events as JSON for offline evaluation.
    Export(export::Args),
    /// Evaluate a normalized event export offline.
    Evaluate(evaluate::Args),
    /// Validate a manifest: schema, timeline, references and recomputed contents.
    Validate(validate::Args),
    /// Enforce policy against a manifest, honoring recorded dispositions.
    Check(check::Args),
    /// Collect and evaluate one pull request.
    Pr(pr::Args),
    /// Generate shell completions.
    Completions(completions::Args),
    /// Generate man pages.
    Manpage(completions::ManArgs),
}

/// Everything a subcommand needs.
pub struct Ctx {
    pub global: Global,
}

impl Ctx {
    /// Print a status line to stderr unless `--quiet`.
    pub fn note(&self, msg: impl AsRef<str>) {
        if !self.global.quiet {
            eprintln!("{}", msg.as_ref());
        }
    }
}

/// Run a parsed command line.
pub fn run(cli: Cli) -> Result<Exit> {
    let ctx = Ctx { global: cli.global };
    match cli.command {
        Command::Scan(args) => scan::run(&ctx, args),
        Command::Export(args) => export::run(&ctx, args),
        Command::Evaluate(args) => evaluate::run(&ctx, args),
        Command::Validate(args) => validate::run(&ctx, args),
        Command::Check(args) => check::run(&ctx, args),
        Command::Pr(args) => pr::run(&ctx, args),
        Command::Completions(args) => completions::run(&ctx, args),
        Command::Manpage(args) => completions::run_man(&ctx, args),
    }
}

/// Parse the process arguments, run, and map the outcome to an exit code.
pub fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(exit) => exit.into(),
        Err(err) => {
            eprintln!("error: {err:#}");
            crate::exit_for(&err).into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_definition_is_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
