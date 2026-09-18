//! `agent-change-control` — the library half of the `acc` command-line tool.
//!
//! Everything lives here so it is testable; `src/main.rs` only parses arguments and maps the
//! result to an exit code.
//!
//! Design in one paragraph: a collector (only GitHub in Phase 1) turns forge data into the public
//! [`model::Events`] shape, which mirrors the JSON Schemas under `schemas/`. [`normalize`] validates
//! the schema and the timeline and sorts everything into canonical order. [`rules`] evaluates the
//! normalized facts into findings and assessments without touching the network, the clock or the
//! environment. [`manifest`] wraps the result with the resolved policy, a summary and a source
//! digest, and can re-evaluate a manifest to detect tampering. [`output`] renders JSON, YAML, a
//! table or SARIF.

pub mod cli;
pub mod collectors;
pub mod manifest;
pub mod model;
pub mod normalize;
pub mod output;
pub mod policy;
pub mod provenance;
pub mod rules;

use std::borrow::Cow;

/// Crate version, as compiled in.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Process exit codes used by `acc`.
///
/// * `0` — successful generation/validation, or the policy threshold passed
/// * `1` — the policy threshold was exceeded (`check` / `pr`)
/// * `2` — invalid command-line arguments
/// * `3` — invalid input or manifest
/// * `4` — collection incomplete (takes precedence over a policy failure)
/// * `5` — authentication rejected
/// * `6` — API, permission, rate-limit or transport failure
/// * `7` — unsupported API data condition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    PolicyFailed = 1,
    Usage = 2,
    InvalidInput = 3,
    Incomplete = 4,
    Auth = 5,
    Api = 6,
    Unsupported = 7,
}

impl Exit {
    pub const fn code(self) -> i32 {
        self as i32
    }
}

impl From<Exit> for std::process::ExitCode {
    fn from(e: Exit) -> Self {
        std::process::ExitCode::from(e.code() as u8)
    }
}

/// An error that carries the process exit status it must produce.
///
/// Transport failures use fixed messages so that response headers and bodies never reach the
/// terminal or a log.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Failure {
    pub exit: Exit,
    pub message: Cow<'static, str>,
}

/// Build an [`anyhow::Error`] that exits with `exit`.
pub fn failure(exit: Exit, message: impl Into<Cow<'static, str>>) -> anyhow::Error {
    Failure {
        exit,
        message: message.into(),
    }
    .into()
}

/// The exit status for any error: a [`Failure`] carries its own, anything else is invalid input.
pub fn exit_for(err: &anyhow::Error) -> Exit {
    err.downcast_ref::<Failure>()
        .map_or(Exit::InvalidInput, |f| f.exit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_stable() {
        assert_eq!(Exit::Ok.code(), 0);
        assert_eq!(Exit::PolicyFailed.code(), 1);
        assert_eq!(Exit::Usage.code(), 2);
        assert_eq!(Exit::InvalidInput.code(), 3);
        assert_eq!(Exit::Incomplete.code(), 4);
        assert_eq!(Exit::Auth.code(), 5);
        assert_eq!(Exit::Api.code(), 6);
        assert_eq!(Exit::Unsupported.code(), 7);
    }

    #[test]
    fn failures_carry_their_exit_and_plain_errors_are_invalid_input() {
        assert_eq!(exit_for(&failure(Exit::Auth, "nope")), Exit::Auth);
        assert_eq!(exit_for(&anyhow::anyhow!("nope")), Exit::InvalidInput);
        assert_eq!(failure(Exit::Usage, "bad flag").to_string(), "bad flag");
    }
}
