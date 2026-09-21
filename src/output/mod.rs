//! Renderers for an evaluated manifest.

pub mod intoto;
pub mod json;
pub mod sarif;
pub mod table;
pub mod yaml;

use crate::model::Manifest;
use anyhow::Result;
use std::path::Path;

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
    /// JSON Lines of unsigned in-toto Statements, one per change.
    InTotoJsonl,
}

impl Format {
    pub const fn as_str(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Yaml => "yaml",
            Format::Table => "table",
            Format::Sarif => "sarif",
            Format::InToto => "in-toto",
            Format::InTotoJsonl => "in-toto-jsonl",
        }
    }

    /// The format implied by an output file extension, if any.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "json" => Some(Format::Json),
            "yaml" | "yml" => Some(Format::Yaml),
            "sarif" => Some(Format::Sarif),
            "jsonl" => Some(Format::InTotoJsonl),
            _ => None,
        }
    }

    /// The format implied by an output path: the `.intoto.json` and `.intoto.jsonl` suffixes
    /// first, then the extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?;
        if name.ends_with(".intoto.jsonl") {
            return Some(Format::InTotoJsonl);
        }
        if name.ends_with(".intoto.json") {
            return Some(Format::InToto);
        }
        path.extension()
            .and_then(|s| s.to_str())
            .and_then(Format::from_extension)
    }
}

pub fn render(m: &Manifest, format: Format) -> Result<String> {
    match format {
        Format::Json => json::render(m),
        Format::Yaml => yaml::render(m),
        Format::Sarif => sarif::render(m),
        Format::InToto => intoto::render(m),
        Format::InTotoJsonl => intoto::render_jsonl(m),
        Format::Table => Ok(table::render(m)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attestation_suffixes_win_over_the_extension() {
        assert_eq!(
            Format::from_path(Path::new("out/x.intoto.json")),
            Some(Format::InToto)
        );
        assert_eq!(
            Format::from_path(Path::new("x.intoto.jsonl")),
            Some(Format::InTotoJsonl)
        );
        assert_eq!(
            Format::from_path(Path::new("x.jsonl")),
            Some(Format::InTotoJsonl)
        );
        assert_eq!(Format::from_path(Path::new("x.json")), Some(Format::Json));
        assert_eq!(Format::from_path(Path::new("x.txt")), None);
        assert_eq!(Format::from_path(Path::new("x")), None);
    }
}
