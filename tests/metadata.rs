//! Release metadata that no build step reads, and so nothing else would catch drifting:
//! `.zenodo.json` (which Zenodo reads from the tagged tree when it archives a release, in place
//! of `CITATION.cff`) must parse and agree with `CITATION.cff` and `Cargo.toml`.

mod common;

use serde_json::Value;

fn read(name: &str) -> String {
    std::fs::read_to_string(common::root().join(name)).unwrap()
}

/// The value of a top-level `key: value` line in a YAML or TOML file, unquoted.
fn field(text: &str, key: &str, sep: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix(&format!("{key}{sep}")))
        .unwrap_or_else(|| panic!("no {key}"))
        .trim()
        .trim_matches('"')
        .to_string()
}

#[test]
fn zenodo_metadata_agrees_with_citation_and_cargo() {
    let zenodo: Value = serde_json::from_str(&read(".zenodo.json")).unwrap();
    let citation = read("CITATION.cff");
    let cargo = read("Cargo.toml");
    assert_eq!(zenodo["upload_type"], "software");
    assert_eq!(zenodo["title"], field(&citation, "title", ": ").as_str());
    assert_eq!(
        zenodo["license"],
        field(&citation, "license", ": ").as_str()
    );
    assert_eq!(zenodo["license"], field(&cargo, "license", " = ").as_str());
    assert!(
        zenodo.get("version").is_none(),
        "Zenodo takes the version from the release tag"
    );
    let creators = zenodo["creators"].as_array().unwrap();
    assert!(!creators.is_empty());
    for c in creators {
        assert!(
            citation.contains(&format!("name: {}", c["name"].as_str().unwrap())),
            "{c} is not an author in CITATION.cff"
        );
    }
    for r in zenodo["related_identifiers"].as_array().unwrap() {
        assert!(r["identifier"].as_str().unwrap().starts_with("https://"));
        assert!(r["relation"].is_string());
    }
}
