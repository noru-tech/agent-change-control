//! Renderers for an evaluated manifest.

pub mod intoto;
pub mod json;
pub mod sarif;
pub mod table;
pub mod yaml;

use crate::model::Manifest;
use anyhow::Result;

/// Output formats. JSON is canonical and byte-stable; YAML is a presentation format. SARIF and
/// in-toto are canonical JSON too, so they can be digested and signed as they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Json,
    Yaml,
    Table,
    Sarif,
    /// An unsigned in-toto Statement v1 whose predicate is the manifest.
    InToto,
}

impl Format {
    pub const fn as_str(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Yaml => "yaml",
            Format::Table => "table",
            Format::Sarif => "sarif",
            Format::InToto => "in-toto",
        }
    }

    /// The format implied by an output file extension, if any.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "json" => Some(Format::Json),
            "yaml" | "yml" => Some(Format::Yaml),
            "sarif" => Some(Format::Sarif),
            _ => None,
        }
    }
}

pub fn render(m: &Manifest, format: Format) -> Result<String> {
    match format {
        Format::Json => json::render(m),
        Format::Yaml => yaml::render(m),
        Format::Sarif => sarif::render(m),
        Format::InToto => intoto::render(m),
        Format::Table => Ok(table::render(m)),
    }
}
