//! Command-line surface. Each subcommand lives in its own module and gets a [`Ctx`].

pub mod check;
pub mod completions;
pub mod conformance;
pub mod detect;
pub mod evaluate;
pub mod export;
pub mod forge;
pub mod io;
pub mod pr;
pub mod scan;
pub mod validate;

use crate::Exit;
use anyhow::Result;
use clap::{Args, ColorChoice, CommandFactory, FromArgMatches, Parser, Subcommand};
use std::ffi::OsString;
use std::process::ExitCode;

const ABOUT: &str = "acc — change control for software written with coding agents";
const LONG_ABOUT: &str = "\
acc — change control for software written with coding agents.

Records the agent, human operator, reviewers and merger of each pull request, then evaluates
explicit separation-of-duty rules (ACC001 to ACC010) deterministically and offline.
Only `scan`, `export` and `pr` contact GitHub (and only they read the clock, for the default
window, or `git`, to find the repository); `evaluate`, `validate` and `check` never touch the
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
    /// Print extra diagnostics on stderr (resolved policy, format, destination, counts). Never
    /// changes stdout.
    #[arg(short, long, global = true, conflicts_with = "quiet")]
    pub verbose: bool,
    /// Never color diagnostics. acc's own output is never colored; this and a non-empty
    /// NO_COLOR environment variable switch off color in help and usage errors too.
    #[arg(long, global = true)]
    pub no_color: bool,
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

    /// Print a diagnostic line to stderr when `--verbose`.
    pub fn debug(&self, msg: impl AsRef<str>) {
        if self.global.verbose {
            eprintln!("acc: {}", msg.as_ref());
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

/// The lines [`report`] prints for an error. When it names a validation code, the first line
/// ends with a pointer to that code's documentation page. A [`crate::Failure`] with a hint adds
/// `help: …` and `see: <url>` lines.
pub fn report_lines(err: &anyhow::Error) -> Vec<String> {
    let mut lines = vec![match conformance::validation_codes(err).first() {
        Some(code) => format!("error: {err:#} (see {})", crate::rule_doc_url(code)),
        None => format!("error: {err:#}"),
    }];
    if let Some(f) = err.downcast_ref::<crate::Failure>() {
        if let Some(hint) = &f.hint {
            lines.push(format!("help: {hint}"));
        }
        if let Some(url) = f.doc_url() {
            lines.push(format!("see: {url}"));
        }
    }
    lines
}

/// Print an error to stderr; machine-readable outputs never carry it.
pub fn report(err: &anyhow::Error) {
    for line in report_lines(err) {
        eprintln!("{line}");
    }
}

/// What to do when an export or manifest records an incomplete collection.
pub const RECOLLECTED: &str = "the input records an incomplete collection; collect again (raise --max-pages or narrow the window) before relying on the result";

/// Warn on stderr (unless `--quiet`) that the collection behind a result is incomplete, why,
/// and what to do: the output is written, but exit 4 says a clean result cannot be claimed.
pub fn warn_incomplete(ctx: &Ctx, events: &crate::model::Events, hint: &str) {
    let reason = events.window.reason.clone().unwrap_or_else(|| {
        let n = events
            .changes
            .iter()
            .filter(|c| !c.reviews_complete)
            .count();
        format!("review history incomplete for {n} change(s)")
    });
    ctx.note(format!("warning: collection incomplete: {reason}"));
    ctx.note(format!("help: {hint}"));
    ctx.note(format!("see: {}", Exit::Incomplete.doc_url()));
}

/// Whether color is switched off: `--no-color` anywhere before a `--`, or a non-empty `NO_COLOR`
/// (<https://no-color.org>).
fn color_disabled(args: &[OsString]) -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
        || args
            .iter()
            .skip(1)
            .take_while(|a| *a != "--")
            .any(|a| a == "--no-color")
}

/// The clap command, with color switched off when asked. Help and usage errors are the only
/// colored output clap produces; acc's own output is plain text.
pub fn command(no_color: bool) -> clap::Command {
    let cmd = Cli::command();
    if no_color {
        cmd.color(ColorChoice::Never)
    } else {
        cmd
    }
}

/// Parse `args` (including the program name), exiting through clap on a usage error.
pub fn parse(args: Vec<OsString>) -> Cli {
    let matches = command(color_disabled(&args)).get_matches_from(args);
    Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit())
}

/// Parse the process arguments, run, and map the outcome to an exit code.
pub fn main() -> ExitCode {
    match run(parse(std::env::args_os().collect())) {
        Ok(exit) => exit.into(),
        Err(err) => {
            report(&err);
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

    #[test]
    fn no_color_is_read_from_the_flag_before_a_double_dash() {
        let args = |v: &[&str]| v.iter().map(OsString::from).collect::<Vec<_>>();
        if std::env::var_os("NO_COLOR").is_none() {
            assert!(!color_disabled(&args(&["acc", "validate", "x"])));
            assert!(!color_disabled(&args(&[
                "acc",
                "validate",
                "--",
                "--no-color"
            ])));
        }
        assert!(color_disabled(&args(&[
            "acc",
            "--no-color",
            "validate",
            "x"
        ])));
        assert!(color_disabled(&args(&[
            "acc",
            "validate",
            "x",
            "--no-color"
        ])));
    }
}
