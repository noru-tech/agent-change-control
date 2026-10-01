//! `acc validate INPUT`
//!
//! The input is a manifest (JSON or YAML), one in-toto Statement (`--format in-toto`), or JSON
//! Lines of Statements (`--format in-toto-jsonl`). A Statement is recognized by its `_type`
//! key; JSON Lines by several lines that are each a JSON object.
//!
//! ACP 0.2 documents (written by acc 0.4) are still accepted: their digests are recomputed with
//! the legacy canonicalization they were written with, and the status line says so.
//!
//! `--format json` prints a small result object instead of nothing on stdout:
//!
//! ```json
//! {"code":"ACV001","help_uri":"https://…/docs/rules/ACV001.md","message":"ACV001 …","valid":false}
//! ```
//!
//! `valid` and `message` are always present; `code` and `help_uri` only when the input violates
//! a validation code. The exit status is the same in every format.

use super::Ctx;
use super::io::{self, ResultFormat};
use crate::canonical::Canonicalization;
use crate::model::Manifest;
use crate::output::intoto;
use crate::{Exit, manifest};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, clap::Args)]
pub struct Args {
    /// A manifest written by `scan` or `evaluate`, an in-toto Statement, or JSON Lines of
    /// Statements.
    pub input: PathBuf,
    /// Result format: `text` (the status line, on stderr) or `json` (a result object, on
    /// stdout); inferred from the --output extension when omitted.
    #[arg(short, long, value_enum)]
    pub format: Option<ResultFormat>,
    /// Write the result to FILE instead (the status line in text, the object in JSON).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

/// Several non-empty lines that each look like a JSON object.
fn looks_like_jsonl(text: &str) -> bool {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let objects = lines
        .by_ref()
        .take(2)
        .filter(|l| l.starts_with('{'))
        .count();
    objects == 2
}

/// The status line suffix for a document's canonicalization.
fn legacy_note(canon: Canonicalization) -> &'static str {
    match canon {
        Canonicalization::Jcs => "",
        Canonicalization::Legacy => {
            " (ACP 0.2, legacy canonicalization; acc now writes 0.3 with RFC 8785 digests)"
        }
    }
}

/// Validate the document at `path` and return its status line.
fn validate(ctx: &Ctx, path: &Path) -> Result<String> {
    let text = io::read_text(path)?;
    if looks_like_jsonl(&text) {
        ctx.debug("document: JSON Lines of in-toto Statements");
        let statements = intoto::validate_jsonl(&text)?;
        let canon = manifest::canonicalization(&statements[0].version)?;
        return Ok(format!(
            "Valid attestations: {} statements{}",
            statements.len(),
            legacy_note(canon)
        ));
    }
    let value = io::parse(&text)?;
    if value.get("_type").is_some() {
        ctx.debug("document: in-toto Statement");
        let m = intoto::validate_statement(&value).context("invalid attestation")?;
        let canon = manifest::canonicalization(&m.version)?;
        return Ok(format!("Valid attestation{}", legacy_note(canon)));
    }
    ctx.debug("document: manifest");
    crate::normalize::schema(&value, "manifest")?;
    let m: Manifest = serde_json::from_value(value)?;
    let canon = manifest::validate(&m)?;
    Ok(format!("Valid manifest{}", legacy_note(canon)))
}

/// The JSON result object for an outcome.
pub fn result_object(outcome: &Result<String>) -> Value {
    match outcome {
        Ok(message) => json!({"valid": true, "message": message}),
        Err(err) => {
            let mut object = json!({"valid": false, "message": format!("{err:#}")});
            if let Some(code) = super::conformance::validation_codes(err).first() {
                object["code"] = code.as_str().into();
                object["help_uri"] = crate::rule_doc_url(code).into();
            }
            object
        }
    }
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let outcome = validate(ctx, &args.input);
    let format = ResultFormat::resolve(args.format, args.output.as_deref());
    let rendered = match format {
        ResultFormat::Json => Some(crate::canonical::jcs_bytes(&result_object(&outcome))? + "\n"),
        ResultFormat::Text => args.output.as_ref().map(|_| match &outcome {
            Ok(message) => format!("{message}\n"),
            Err(err) => format!("Invalid: {err:#}\n"),
        }),
    };
    if let Some(rendered) = rendered {
        ctx.debug(format!(
            "writing the {} result to {}",
            match format {
                ResultFormat::Json => "json",
                ResultFormat::Text => "text",
            },
            io::destination(args.output.as_deref())
        ));
        io::write(&rendered, args.output.as_deref())?;
    }
    let message = outcome?;
    ctx.note(message);
    Ok(Exit::Ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonl_needs_two_object_lines() {
        assert!(looks_like_jsonl("{\"a\":1}\n{\"b\":2}\n"));
        assert!(looks_like_jsonl("{\"a\":1}\n\n{\"b\":2}"));
        assert!(!looks_like_jsonl("{\"a\":1}\n"));
        assert!(!looks_like_jsonl("{\n  \"a\": 1\n}\n"));
        assert!(!looks_like_jsonl("version: '0.1'\nevents:\n"));
    }

    #[test]
    fn result_objects_carry_a_code_only_when_one_applies() {
        let ok = result_object(&Ok("Valid manifest".into()));
        assert_eq!(ok, json!({"valid": true, "message": "Valid manifest"}));
        let plain = result_object(&Err(anyhow::anyhow!("input is not valid JSON or YAML")));
        assert_eq!(plain["valid"], false);
        assert!(plain.get("code").is_none() && plain.get("help_uri").is_none());
        let coded = result_object(&Err(anyhow::anyhow!("ACV001 approval_after_merge")));
        assert_eq!(coded["code"], "ACV001");
        assert_eq!(coded["help_uri"], crate::rule_doc_url("ACV001"));
    }
}
