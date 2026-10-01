//! `agent-change-control` — the library half of the `acc` command-line tool.
//!
//! Everything lives here so it is testable; `src/main.rs` only parses arguments and maps the
//! result to an exit code.
//!
//! Design in one paragraph: a collector (only GitHub so far) turns forge data into the public
//! [`model::Events`] shape, which mirrors the JSON Schemas under `schemas/`. [`normalize`] validates
//! the schema and the timeline and sorts everything into canonical order. [`rules`] evaluates the
//! normalized facts into findings and assessments without touching the network, the clock or the
//! environment. [`manifest`] wraps the result with the resolved policy, a summary and a source
//! digest, and can re-evaluate a manifest to detect tampering. [`output`] renders JSON, YAML, a
//! table or SARIF.

pub mod canonical;
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

/// Base URL of the documentation tree. Every rule, validation code and exit code has a stable page
/// under it; this is the one place to change when the docs move to their own site.
pub const DOCS_BASE_URL: &str = "https://github.com/noru-tech/agent-change-control/blob/main/docs";

/// The documentation page of a rule (`ACC001`) or validation code (`ACV001`).
pub fn rule_doc_url(code: &str) -> String {
    format!("{DOCS_BASE_URL}/rules/{code}.md")
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

    /// The anchor of this code's section in `docs/exit-codes.md`.
    pub const fn anchor(self) -> &'static str {
        match self {
            Exit::Ok => "0-success",
            Exit::PolicyFailed => "1-policy-threshold-exceeded",
            Exit::Usage => "2-invalid-command-line-arguments",
            Exit::InvalidInput => "3-invalid-input-or-manifest",
            Exit::Incomplete => "4-collection-incomplete",
            Exit::Auth => "5-authentication-rejected",
            Exit::Api => "6-api-permission-rate-limit-or-transport-failure",
            Exit::Unsupported => "7-unsupported-api-data-condition",
        }
    }

    /// The documentation of this code: its section in `docs/exit-codes.md`.
    pub fn doc_url(self) -> String {
        format!("{DOCS_BASE_URL}/exit-codes.md#{}", self.anchor())
    }
}

impl From<Exit> for std::process::ExitCode {
    fn from(e: Exit) -> Self {
        std::process::ExitCode::from(e.code() as u8)
    }
}

/// An error that carries the process exit status it must produce, and optionally how to fix it.
///
/// Transport failures use fixed messages so that response headers and bodies never reach the
/// terminal or a log. The hint and the documentation link are printed on their own stderr lines
/// (`help: …`, `see: …`) and never reach a machine-readable output.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct Failure {
    pub exit: Exit,
    pub message: Cow<'static, str>,
    /// What to do about it, in one sentence.
    pub hint: Option<Cow<'static, str>>,
    /// Where to read more; defaults to the exit code's section when a hint is given.
    pub see: Option<String>,
}

impl Failure {
    pub fn new(exit: Exit, message: impl Into<Cow<'static, str>>) -> Self {
        Self {
            exit,
            message: message.into(),
            hint: None,
            see: None,
        }
    }

    /// Attach a hint.
    pub fn hint(mut self, hint: impl Into<Cow<'static, str>>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Attach a documentation link other than the exit code's section.
    pub fn see(mut self, url: impl Into<String>) -> Self {
        self.see = Some(url.into());
        self
    }

    /// The documentation link to print: the explicit one, else the exit code's section when
    /// there is a hint.
    pub fn doc_url(&self) -> Option<String> {
        self.see
            .clone()
            .or_else(|| self.hint.as_ref().map(|_| self.exit.doc_url()))
    }
}

/// Build an [`anyhow::Error`] that exits with `exit`.
pub fn failure(exit: Exit, message: impl Into<Cow<'static, str>>) -> anyhow::Error {
    Failure::new(exit, message).into()
}

/// Build an [`anyhow::Error`] that exits with `exit` and tells the user how to fix it.
pub fn failure_with_hint(
    exit: Exit,
    message: impl Into<Cow<'static, str>>,
    hint: impl Into<Cow<'static, str>>,
) -> anyhow::Error {
    Failure::new(exit, message).hint(hint).into()
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
    fn exit_codes_link_to_their_section() {
        assert_eq!(
            Exit::Auth.doc_url(),
            "https://github.com/noru-tech/agent-change-control/blob/main/docs/exit-codes.md#5-authentication-rejected"
        );
        let docs = include_str!("../docs/exit-codes.md");
        for exit in [
            Exit::Ok,
            Exit::PolicyFailed,
            Exit::Usage,
            Exit::InvalidInput,
            Exit::Incomplete,
            Exit::Auth,
            Exit::Api,
            Exit::Unsupported,
        ] {
            assert!(
                docs.contains(&format!("(#{})", exit.anchor())),
                "{}",
                exit.anchor()
            );
        }
    }

    #[test]
    fn hints_default_to_the_exit_code_section() {
        let plain = Failure::new(Exit::Usage, "bad flag");
        assert_eq!(plain.doc_url(), None);
        let hinted = Failure::new(Exit::Usage, "bad flag").hint("fix it");
        assert_eq!(hinted.doc_url(), Some(Exit::Usage.doc_url()));
        let linked = Failure::new(Exit::InvalidInput, "bad policy")
            .hint("fix it")
            .see("https://example.invalid/policy");
        assert_eq!(
            linked.doc_url().as_deref(),
            Some("https://example.invalid/policy")
        );
        let err = failure_with_hint(Exit::Auth, "nope", "set a token");
        assert_eq!(exit_for(&err), Exit::Auth);
        assert_eq!(err.to_string(), "nope");
    }

    #[test]
    fn rule_pages_live_under_the_docs_base() {
        assert_eq!(
            rule_doc_url("ACC001"),
            "https://github.com/noru-tech/agent-change-control/blob/main/docs/rules/ACC001.md"
        );
    }

    #[test]
    fn failures_carry_their_exit_and_plain_errors_are_invalid_input() {
        assert_eq!(exit_for(&failure(Exit::Auth, "nope")), Exit::Auth);
        assert_eq!(exit_for(&anyhow::anyhow!("nope")), Exit::InvalidInput);
        assert_eq!(failure(Exit::Usage, "bad flag").to_string(), "bad flag");
    }
}
