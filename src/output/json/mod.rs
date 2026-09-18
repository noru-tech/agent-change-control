//! Canonical JSON: the byte-stable format that digests and goldens are computed over.

use crate::model::Manifest;
use anyhow::Result;

pub fn render(m: &Manifest) -> Result<String> {
    crate::normalize::canonical(m)
}
