//! `acc validate MANIFEST`

use super::Ctx;
use super::io;
use crate::model::Manifest;
use crate::{Exit, manifest};
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct Args {
    /// A manifest (JSON or YAML) written by `scan` or `evaluate`.
    pub input: PathBuf,
}

pub fn run(ctx: &Ctx, args: Args) -> Result<Exit> {
    let m: Manifest = io::read(&args.input, "manifest")?;
    manifest::validate(&m)?;
    ctx.note("Valid manifest");
    Ok(Exit::Ok)
}
