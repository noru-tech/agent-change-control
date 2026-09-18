//! YAML presentation output. Not the hash format; use JSON for digests.

use crate::model::Manifest;
use anyhow::Result;

pub fn render(m: &Manifest) -> Result<String> {
    Ok(serde_saphyr::to_string(m)?)
}
