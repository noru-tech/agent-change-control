//! `acc validate INPUT`
//!
//! The input is a manifest (JSON or YAML), one in-toto Statement (`--format in-toto`), or JSON
//! Lines of Statements (`--format in-toto-jsonl`). A Statement is recognized by its `_type`
//! key; JSON Lines by several lines that are each a JSON object.
//!
//! ACP 0.2 documents (written by acc 0.4) are still accepted: their digests are recomputed with
//! the legacy canonicalization they were written with, and the status line says so.

use super::Ctx;
use super::io;
use crate::canonical::Canonicalization;
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

/// The status line suffix for a document's canonicalization.
fn legacy_note(canon: Canonicalization) -> &'static str {
    match canon {
        Canonicalization::Jcs => "",
        Canonicalization::Legacy => {
            " (ACP 0.2, legacy canonicalization; acc now writes 0.3 with RFC 8785 digests)"
        }
    }
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let text = io::read_text(&args.input)?;
    if looks_like_jsonl(&text) {
        let statements = intoto::validate_jsonl(&text)?;
        let canon = manifest::canonicalization(&statements[0].version)?;
        ctx.note(format!(
            "Valid attestations: {} statements{}",
            statements.len(),
            legacy_note(canon)
        ));
        return Ok(Exit::Ok);
    }
    let value = io::parse(&text)?;
    if value.get("_type").is_some() {
        let m = intoto::validate_statement(&value).context("invalid attestation")?;
        let canon = manifest::canonicalization(&m.version)?;
        ctx.note(format!("Valid attestation{}", legacy_note(canon)));
        return Ok(Exit::Ok);
    }
    crate::normalize::schema(&value, "manifest")?;
    let m: Manifest = serde_json::from_value(value)?;
    let canon = manifest::validate(&m)?;
    ctx.note(format!("Valid manifest{}", legacy_note(canon)));
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
