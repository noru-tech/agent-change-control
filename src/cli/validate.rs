//! `acc validate INPUT`
//!
//! The input is a manifest (JSON or YAML), one in-toto Statement (`--format in-toto`), or JSON
//! Lines of Statements (`--format in-toto-jsonl`). A Statement is recognized by its `_type`
//! key; JSON Lines by several lines that are each a JSON object.

use super::Ctx;
use super::io;
use crate::model::Manifest;
use crate::output::intoto;
use crate::{Exit, manifest};
use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// A manifest written by `scan` or `evaluate`, an in-toto Statement, or JSON Lines of
    /// Statements.
    pub input: PathBuf,
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

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let text = io::read_text(&args.input)?;
    if looks_like_jsonl(&text) {
        let statements = intoto::validate_jsonl(&text)?;
        ctx.note(format!(
            "Valid attestations: {} statements",
            statements.len()
        ));
        return Ok(Exit::Ok);
    }
    let value = io::parse(&text)?;
    if value.get("_type").is_some() {
        intoto::validate_statement(&value).context("invalid attestation")?;
        ctx.note("Valid attestation");
        return Ok(Exit::Ok);
    }
    crate::normalize::schema(&value, "manifest")?;
    let m: Manifest = serde_json::from_value(value)?;
    manifest::validate(&m)?;
    ctx.note("Valid manifest");
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
}
