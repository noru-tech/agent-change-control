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

/// Every "AI Change Provenance X.Y" in the README and the docs names the specification's current
/// version, including the URL-encoded form in the README's badge. Version history lives in the
/// changelogs, which are not checked.
#[test]
fn documentation_names_the_current_specification_version() {
    let spec = read("spec/ai-change-provenance.md");
    let current = spec
        .lines()
        .find_map(|l| l.strip_prefix("**Version "))
        .and_then(|l| l.split("**").next())
        .expect("the specification states its version")
        .to_string();
    let mut files = vec![
        "README.md".to_string(),
        "conformance/README.md".to_string(),
        "CITATION.cff".to_string(),
    ];
    for entry in std::fs::read_dir(common::root().join("docs")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "md") {
            files.push(format!(
                "docs/{}",
                path.file_name().unwrap().to_string_lossy()
            ));
        }
    }
    let mut seen = 0;
    for file in &files {
        let text = read(file).replace("%20", " ");
        for (i, _) in text.match_indices("AI Change Provenance ") {
            let rest = &text[i + "AI Change Provenance ".len()..];
            let version: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            let version = version.trim_end_matches('.');
            if version.is_empty() {
                continue;
            }
            seen += 1;
            assert_eq!(
                version, current,
                "{file} names AI Change Provenance {version}; the specification is {current}"
            );
        }
    }
    assert!(seen >= 3, "expected the README badge and prose, saw {seen}");
}

/// The concept DOI in `CITATION.cff` is the one the README badge and citation text point at.
#[test]
fn the_doi_is_the_same_everywhere() {
    let citation = read("CITATION.cff");
    let doi = field(&citation, "doi", ": ");
    assert!(doi.starts_with("10.5281/zenodo."), "{doi}");
    assert!(citation.contains(&format!("value: {doi}")));
    for file in ["README.md", "conformance/README.md"] {
        let text = read(file);
        assert!(
            text.contains(&format!("https://doi.org/{doi}")),
            "{file} does not link {doi}"
        );
        for (i, _) in text.match_indices("10.5281/zenodo.") {
            let id: String = text[i..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '/')
                .collect();
            let id = id.trim_end_matches(".svg").trim_end_matches('.');
            assert_eq!(id, doi, "{file} names another DOI");
        }
    }
}
