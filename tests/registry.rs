//! The in-toto registry submission draft (`docs/in-toto-registry/`): its worked example and the
//! preimage of every digest in it are generated here, so the document cannot drift from what
//! `acc` emits. `UPDATE_REGISTRY=1 cargo test --test registry` rewrites the generated blocks
//! between `<!-- generated:NAME -->` and `<!-- /generated:NAME -->`.

mod common;

use agent_change_control::canonical::{jcs_bytes, sha256};
use agent_change_control::model::{Events, Manifest, Policy};
use agent_change_control::{manifest, normalize, output::intoto};
use serde_json::Value;

const DOC: &str = "docs/in-toto-registry/ai-change-provenance.md";
const FIXTURE: &str = "claude-operator-self-approved";

/// Commit hashes for the example, in place of the fixture's `head` and `merge` placeholders.
const HEAD: &str = "3f9c2e4b7a1d8c6e5f0a2b4c6d8e0f1a3b5c7d9e";
const MERGE: &str = "8b1e0c7d6f5a4e3b2c1d0e9f8a7b6c5d4e3f2a1b";

fn example() -> (Manifest, Value) {
    let text = std::fs::read_to_string(common::fixture(FIXTURE, "events.json"))
        .unwrap()
        .replace("\"head\"", &format!("\"{HEAD}\""))
        .replace("\"merge\"", &format!("\"{MERGE}\""));
    let e: Events = serde_json::from_str(&text).unwrap();
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    let statement: Value = serde_json::from_str(&intoto::render(&m).unwrap()).unwrap();
    (m, statement)
}

fn block(lang: &str, body: &str) -> String {
    format!("```{lang}\n{body}\n```\n")
}

fn generated() -> Vec<(&'static str, String)> {
    let (m, statement) = example();
    // Shown in the conventional member order for reading; the signed bytes are the JCS
    // serialization, whose order is sorted.
    let indent = |v: &Value| {
        serde_json::to_string_pretty(v)
            .unwrap()
            .replace('\n', "\n  ")
    };
    let shown = format!(
        "{{\n  \"_type\": {},\n  \"subject\": {},\n  \"predicateType\": {},\n  \"predicate\": {}\n}}",
        indent(&statement["_type"]),
        indent(&statement["subject"]),
        indent(&statement["predicateType"]),
        indent(&statement["predicate"]),
    );
    assert_eq!(serde_json::from_str::<Value>(&shown).unwrap(), statement);
    let example = block("json", &shown);

    let mut digests = String::new();
    digests.push_str(
        "Each preimage below is shown exactly: the bytes inside the code block, without the \
         newline that ends the block. To recompute one, save the block's content without that final \
         newline and run `sha256sum`; or serialize the corresponding part of the Statement above \
         with any RFC 8785 implementation.\n\n",
    );
    for f in &m.findings {
        let preimage =
            jcs_bytes(&(&m.events.repository, &f.change_id, f.rule_id, &f.actor_ids)).unwrap();
        let digest = sha256(preimage.as_bytes());
        assert_eq!(f.id, format!("acc-{}", &digest[7..23]));
        digests.push_str(&format!(
            "**Finding `{}`** ({}): `acc-` and the first 16 hexadecimal characters of the \
             SHA-256 of the JCS array `[repository, change_id, rule_id, actor_ids]`.\n\n",
            f.id,
            f.rule_id.code()
        ));
        digests.push_str(&block("json", &preimage));
        digests.push_str(&format!("\nSHA-256: `{}`\n\n", &digest[7..]));
    }
    let events = jcs_bytes(&m.events).unwrap();
    assert_eq!(m.generated.source_digest, sha256(events.as_bytes()));
    digests.push_str(
        "**`generated.source_digest`**: the SHA-256 of the JCS serialization of \
         `predicate.events`.\n\n<details><summary>Preimage</summary>\n\n",
    );
    digests.push_str(&block("json", &events));
    digests.push_str(&format!(
        "\n</details>\n\nSHA-256: `{}`\n\n",
        &m.generated.source_digest[7..]
    ));
    let predicate = jcs_bytes(&m).unwrap();
    let predicate_digest = sha256(predicate.as_bytes());
    digests.push_str(
        "**The `sha256` subject form**: a signer that only accepts SHA-2 subjects uses a single \
         subject whose `sha256` is the digest of the JCS serialization of `predicate`, which is \
         also the exact content of the manifest file `acc --format json` writes.\n\n\
         <details><summary>Preimage</summary>\n\n",
    );
    digests.push_str(&block("json", &predicate));
    digests.push_str(&format!(
        "\n</details>\n\nSHA-256: `{}`\n\n",
        &predicate_digest[7..]
    ));
    let bytes = jcs_bytes(&statement).unwrap();
    digests.push_str(&format!(
        "**The Statement itself**: the bytes a DSSE signer wraps are the JCS serialization of \
         the Statement above, {} bytes with SHA-256 `{}`.\n",
        bytes.len(),
        &sha256(bytes.as_bytes())[7..]
    ));
    vec![("example", example), ("digests", digests)]
}

fn splice(doc: &str, name: &str, body: &str) -> String {
    let open = format!("<!-- generated:{name} -->\n");
    let close = format!("<!-- /generated:{name} -->");
    let start = doc.find(&open).unwrap_or_else(|| panic!("no {open}")) + open.len();
    let end = doc[start..]
        .find(&close)
        .unwrap_or_else(|| panic!("no {close}"))
        + start;
    format!("{}\n{}\n{}", &doc[..start], body.trim_end(), &doc[end..])
}

#[test]
fn the_worked_example_is_what_acc_emits() {
    let path = common::root().join(DOC);
    let doc = std::fs::read_to_string(&path).unwrap();
    let mut want = doc.clone();
    for (name, body) in generated() {
        want = splice(&want, name, &body);
    }
    if std::env::var_os("UPDATE_REGISTRY").is_some() {
        std::fs::write(&path, &want).unwrap();
    }
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        want,
        "{DOC} is stale; regenerate with UPDATE_REGISTRY=1"
    );
}

/// Every `SHA-256:` line in the document is the digest of the code block just before it, read
/// back from the document text itself.
#[test]
fn every_digest_in_the_document_recomputes_from_its_preimage() {
    if std::env::var_os("UPDATE_REGISTRY").is_some() {
        return; // the document is being rewritten concurrently
    }
    let doc = std::fs::read_to_string(common::root().join(DOC)).unwrap();
    let section = &doc[doc.find("<!-- generated:digests -->").unwrap()..];
    let mut last_block: Option<String> = None;
    let mut lines = section.lines();
    let mut checked = 0;
    while let Some(line) = lines.next() {
        if line.starts_with("```json") {
            let body: Vec<&str> = lines.by_ref().take_while(|l| *l != "```").collect();
            last_block = Some(body.join("\n"));
        } else if let Some(rest) = line.strip_prefix("SHA-256: `") {
            let hex = rest.trim_end_matches('`');
            let preimage = last_block
                .take()
                .expect("a digest without a preimage block");
            assert_eq!(sha256(preimage.as_bytes()), format!("sha256:{hex}"));
            checked += 1;
        }
    }
    assert_eq!(
        checked, 5,
        "three finding IDs, the source digest and the subject form"
    );
}

/// Acceptance 6.6: the predicate schema validates the worked example and the manifest of every
/// accept vector in the conformance corpus, and the example re-validates as a Statement.
#[test]
fn the_schema_validates_the_example_and_every_accept_manifest() {
    let (_, statement) = example();
    normalize::schema(&statement, "statement").unwrap();
    normalize::schema(&statement["predicate"], "manifest").unwrap();
    intoto::validate_statement(&statement).unwrap();
    let corpus = common::root().join("conformance");
    let index: Value =
        serde_json::from_str(&std::fs::read_to_string(corpus.join("MANIFEST.json")).unwrap())
            .unwrap();
    let mut n = 0;
    for v in index["vectors"].as_array().unwrap() {
        if v["kind"] != "accept" {
            continue;
        }
        let vector: Value = serde_json::from_str(
            &std::fs::read_to_string(corpus.join(v["path"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let m = manifest::evaluate(
            serde_json::from_value(vector["events"].clone()).unwrap(),
            serde_json::from_value(vector["policy"].clone()).unwrap(),
        )
        .unwrap();
        normalize::schema(&serde_json::to_value(&m).unwrap(), "manifest").unwrap();
        n += 1;
    }
    assert_eq!(n, index["counts"]["accept"].as_u64().unwrap() as usize);
}
