//! The manifest as RFC 8785 bytes: exactly the preimage of its digest, with no trailing newline.

use crate::model::Manifest;
use anyhow::Result;

pub fn render(m: &Manifest) -> Result<String> {
    crate::canonical::jcs_bytes(m)
}
