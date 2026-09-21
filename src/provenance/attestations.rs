//! Attestations as evidence: in-toto Statements the caller hands over from disk, bare or inside
//! a DSSE envelope or a Sigstore bundle.
//!
//! `acc` does not verify signatures. Verification needs a trust root (which identities may sign
//! which claims), and that configuration does not belong in an offline evaluator. The caller
//! verifies with the signer's tooling (`gh attestation verify`, `cosign verify-blob-attestation`)
//! and states so with `--verified-by`; the manifest records that statement next to every piece of
//! evidence the attestation produced. Without it, an attestation is read for its claims but they
//! count as `declared`, exactly like a provenance document found in the repository. With it, and
//! only when the container actually carried a signature, they count as `signed`.
//!
//! What can be read today is the authorship claim: a Statement of predicate type
//! [`PROVENANCE_PREDICATE_TYPE`] whose predicate is the provenance document of the specification
//! (§3.2) and whose subject is the change's head commit. Review claims are not read, because the
//! predicates that exist for them (`human-review`, gittuf's reference authorization) carry the
//! reviewer in the signature, which is exactly what pre-verified input does not expose.

use crate::model::{ActorKind, Attestation, Evidence, EvidenceKind, ReviewState, Signer};
use anyhow::{Context, Result, ensure};
use base64::Engine;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAX_FILE: u64 = 32 * 1024 * 1024;
const DSSE_PAYLOAD_TYPE: &str = "application/vnd.in-toto+json";

/// The predicate type of a signed provenance document: the specification's §3.2 document as an
/// in-toto predicate, with the change's head commit as the subject.
pub const PROVENANCE_PREDICATE_TYPE: &str =
    "https://noru.tech/spec/ai-change-provenance/provenance/v0.1";

/// The predicate type of a signed review document: `schemas/review.schema.json` as an in-toto
/// predicate, with the head commit as the subject. It names the reviewer in the predicate so
/// that pre-verified input can use it; it upgrades a forge-observed review and never creates one.
pub const REVIEW_PREDICATE_TYPE: &str = "https://noru.tech/spec/ai-change-provenance/review/v0.1";

/// The evidence source name used for every claim read from an attestation.
pub const SOURCE: &str = "attestation";

/// One loaded Statement with its registry record.
#[derive(Debug, Clone)]
struct Loaded {
    id: String,
    record: Attestation,
    statement: Value,
}

/// The authorship an attestation set establishes for one head commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorship {
    pub agent: String,
    pub operator: Option<String>,
    pub evidence: Vec<Evidence>,
    /// The registry entries to record with the export.
    pub records: BTreeMap<String, Attestation>,
}

/// One review attestation bound to a head commit, ready to be matched to a forge review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewClaim {
    /// The reviewer's actor identifier, lowercased (`github:acme-review[bot]`).
    pub reviewer_id: String,
    pub reviewer_kind: ActorKind,
    /// The agent name the predicate gives for an agent reviewer.
    pub agent: Option<String>,
    pub decision: ReviewState,
    pub operator: Option<String>,
    pub instructions_owner: Option<String>,
    pub model: Option<String>,
    /// The verified signer's identity, when the attestation came from a verifier's output.
    pub identity: Option<String>,
    pub evidence: Evidence,
    pub id: String,
    pub record: Attestation,
}

/// Attestations read from disk, indexed for lookup by subject commit.
#[derive(Debug, Default)]
pub struct Attestations {
    loaded: Vec<Loaded>,
    verified_by: Option<String>,
}

/// A parsed container: the payload bytes that were (or would be) signed, the Statement, and
/// whether the container carried a signature.
struct Container {
    payload: Vec<u8>,
    statement: Value,
    signed: bool,
}

fn container(value: Value) -> Result<Container> {
    let envelope = if value.get("dsseEnvelope").is_some() {
        Some(value["dsseEnvelope"].clone())
    } else if value.get("payloadType").is_some() && value.get("payload").is_some() {
        Some(value.clone())
    } else {
        None
    };
    match envelope {
        Some(env) => {
            ensure!(
                env["payloadType"].as_str() == Some(DSSE_PAYLOAD_TYPE),
                "DSSE payload type is not an in-toto Statement"
            );
            let encoded = env["payload"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("DSSE payload is not a string"))?;
            let payload = base64::engine::general_purpose::STANDARD
                .decode(encoded)
                .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(encoded))
                .context("DSSE payload is not base64")?;
            let statement: Value =
                serde_json::from_slice(&payload).context("DSSE payload is not JSON")?;
            let signed = env["signatures"].as_array().is_some_and(|s| !s.is_empty());
            Ok(Container {
                payload,
                statement,
                signed,
            })
        }
        None => {
            ensure!(
                value.get("_type").is_some(),
                "not an in-toto Statement, a DSSE envelope or a Sigstore bundle"
            );
            let payload = crate::normalize::canonical(&value)?.into_bytes();
            Ok(Container {
                payload,
                statement: value,
                signed: false,
            })
        }
    }
}

impl Attestations {
    /// Load every `.json` and `.jsonl` file under `paths` (files or directories, recursively, in
    /// sorted order). A `.json` file holds one container or an array of them; a `.jsonl` file
    /// holds one per line. `verified_by` is the caller's statement of who verified the
    /// signatures, recorded verbatim; without it nothing loaded here yields `signed` evidence.
    pub fn load(paths: &[PathBuf], verified_by: Option<String>) -> Result<Self> {
        let mut out = Self {
            loaded: Vec::new(),
            verified_by,
        };
        for path in paths {
            out.load_path(path)?;
        }
        Ok(out)
    }

    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }

    /// Load a verifier's JSON output (`gh attestation verify --format json`): each result
    /// carries the verified Sigstore bundle and the certificate identity the verifier
    /// established. The bundles are loaded as signed attestations with their signer recorded.
    pub fn load_verification(&mut self, path: &Path) -> Result<()> {
        let meta = std::fs::metadata(path)
            .with_context(|| format!("unable to read {}", path.display()))?;
        ensure!(
            meta.len() <= MAX_FILE,
            "verification file exceeds 32 MiB limit"
        );
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("unable to read {}", path.display()))?;
        let value: Value = serde_json::from_str(&data)
            .with_context(|| format!("{} is not valid JSON", path.display()))?;
        let results = match value {
            Value::Array(items) => items,
            v => vec![v],
        };
        ensure!(!results.is_empty(), "verification file holds no results");
        for (i, result) in results.into_iter().enumerate() {
            let bundle = result
                .get("attestation")
                .and_then(|a| a.get("bundle"))
                .cloned()
                .ok_or_else(|| {
                    anyhow::anyhow!("{}#{}: no attestation.bundle", path.display(), i + 1)
                })?;
            let certificate = &result["verificationResult"]["signature"]["certificate"];
            let identity = certificate["subjectAlternativeName"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| {
                    anyhow::anyhow!("{}#{}: no verified signer identity", path.display(), i + 1)
                })?;
            let signer = Signer {
                identity: identity.to_string(),
                issuer: certificate["issuer"]
                    .as_str()
                    .or_else(|| certificate["certificateIssuer"].as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(String::from),
            };
            let c = container(bundle).with_context(|| format!("{}#{}", path.display(), i + 1))?;
            ensure!(
                c.signed,
                "{}#{}: verified bundle carries no signature",
                path.display(),
                i + 1
            );
            self.push(c, path, i, Some(signer));
        }
        Ok(())
    }

    fn push(&mut self, c: Container, path: &Path, i: usize, signer: Option<Signer>) {
        let digest = format!("sha256:{:x}", Sha256::digest(&c.payload));
        let id = format!("attestation:{}", &digest[7..23]);
        if let Some(existing) = self.loaded.iter_mut().find(|l| l.id == id) {
            // The same payload seen again: keep the record that knows more.
            if existing.record.signer.is_none() {
                existing.record.signer = signer;
                existing.record.signed |= c.signed;
            }
            return;
        }
        let predicate_type = c.statement["predicateType"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        self.loaded.push(Loaded {
            id,
            record: Attestation {
                file: format!("{}#{}", path.display(), i + 1),
                predicate_type,
                payload_digest: digest,
                signed: c.signed,
                verified_by: self.verified_by.clone(),
                signer,
                matched: true,
            },
            statement: c.statement,
        });
    }

    fn load_path(&mut self, path: &Path) -> Result<()> {
        let meta = std::fs::metadata(path)
            .with_context(|| format!("unable to read {}", path.display()))?;
        if meta.is_dir() {
            let mut entries: Vec<PathBuf> = std::fs::read_dir(path)?
                .map(|e| e.map(|e| e.path()))
                .collect::<std::io::Result<_>>()?;
            entries.sort();
            for entry in entries {
                let ext = entry.extension().and_then(|x| x.to_str());
                if entry.is_dir() || matches!(ext, Some("json" | "jsonl")) {
                    self.load_path(&entry)?;
                }
            }
            return Ok(());
        }
        ensure!(
            meta.len() <= MAX_FILE,
            "attestation file exceeds 32 MiB limit"
        );
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("unable to read {}", path.display()))?;
        let documents: Vec<Value> = if path.extension().is_some_and(|x| x == "jsonl") {
            data.lines()
                .enumerate()
                .filter(|(_, l)| !l.trim().is_empty())
                .map(|(i, l)| {
                    serde_json::from_str(l)
                        .with_context(|| format!("{}:{} is not valid JSON", path.display(), i + 1))
                })
                .collect::<Result<_>>()?
        } else {
            match serde_json::from_str(&data)
                .with_context(|| format!("{} is not valid JSON", path.display()))?
            {
                Value::Array(items) => items,
                v => vec![v],
            }
        };
        for (i, document) in documents.into_iter().enumerate() {
            let c = container(document).with_context(|| format!("{}#{}", path.display(), i + 1))?;
            self.push(c, path, i, None);
        }
        Ok(())
    }

    /// Whether `statement` has `head` among its `gitCommit` subjects.
    fn covers(statement: &Value, head: &str) -> bool {
        statement["subject"].as_array().is_some_and(|subjects| {
            subjects.iter().any(|s| {
                s["digest"]["gitCommit"]
                    .as_str()
                    .is_some_and(|sha| sha.eq_ignore_ascii_case(head))
            })
        })
    }

    /// Every review attestation bound to `head`, validated against the review schema. Matching
    /// to forge reviews, and the actor kind and agent checks, are the collector's.
    pub fn reviews(&self, head: &str) -> Result<Vec<ReviewClaim>> {
        let mut out = Vec::new();
        for l in &self.loaded {
            if l.record.predicate_type != REVIEW_PREDICATE_TYPE || !Self::covers(&l.statement, head)
            {
                continue;
            }
            let p = &l.statement["predicate"];
            crate::normalize::schema(p, "review")
                .with_context(|| format!("attestation {}", l.record.file))?;
            ensure!(
                p["change"]["head_commit"].as_str() == Some(head),
                "attestation {}: review head does not match change",
                l.record.file
            );
            let reviewer_kind = match p["reviewer"]["kind"].as_str() {
                Some("agent") => ActorKind::Agent,
                _ => ActorKind::Human,
            };
            let decision = match p["decision"].as_str() {
                Some("approved") => ReviewState::Approved,
                Some("changes_requested") => ReviewState::ChangesRequested,
                _ => ReviewState::Commented,
            };
            let text = |v: &Value| v.as_str().map(|s| s.to_ascii_lowercase());
            let kind = if l.record.signed && l.record.verified_by.is_some() {
                EvidenceKind::Signed
            } else {
                EvidenceKind::Declared
            };
            out.push(ReviewClaim {
                reviewer_id: p["reviewer"]["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_ascii_lowercase(),
                reviewer_kind,
                agent: text(&p["reviewer"]["agent"]),
                decision,
                operator: text(&p["operator"]["id"]),
                instructions_owner: text(&p["instructions"]["owner"]),
                model: p["model"].as_str().map(String::from),
                identity: l.record.signer.as_ref().map(|s| s.identity.clone()),
                evidence: Evidence {
                    source: SOURCE.into(),
                    r#ref: l.id.clone(),
                    kind,
                },
                id: l.id.clone(),
                record: l.record.clone(),
            });
        }
        Ok(out)
    }

    /// The authorship claim for the change whose head is `head`, from every loaded provenance
    /// attestation bound to it. Several attestations must agree; a bound attestation whose
    /// predicate is invalid or names another head is an error, never ignored.
    pub fn authorship(&self, head: &str) -> Result<Option<Authorship>> {
        let mut claim: Option<(String, Option<String>)> = None;
        let mut evidence = Vec::new();
        let mut records = BTreeMap::new();
        for l in &self.loaded {
            if l.record.predicate_type != PROVENANCE_PREDICATE_TYPE
                || !Self::covers(&l.statement, head)
            {
                continue;
            }
            let (agent, operator) = crate::provenance::convention(&l.statement["predicate"], head)
                .with_context(|| format!("attestation {}", l.record.file))?;
            let operator = operator.map(|o| o.to_ascii_lowercase());
            match &claim {
                Some(existing) => ensure!(
                    *existing == (agent.clone(), operator.clone()),
                    "conflicting authorship attestations for one change"
                ),
                None => claim = Some((agent, operator)),
            }
            let kind = if l.record.signed && l.record.verified_by.is_some() {
                EvidenceKind::Signed
            } else {
                EvidenceKind::Declared
            };
            evidence.push(Evidence {
                source: SOURCE.into(),
                r#ref: l.id.clone(),
                kind,
            });
            records.insert(l.id.clone(), l.record.clone());
        }
        let Some((agent, operator)) = claim else {
            return Ok(None);
        };
        // The spec's provenance document always carries an operator; keep the type honest anyway.
        Ok(Some(Authorship {
            agent,
            operator,
            evidence,
            records,
        }))
    }
}

/// Whether two operator identities name the same account in `namespace`, allowing one to be
/// bare (`alice`) and the other namespaced (`github:alice`).
pub fn same_operator(a: &str, b: &str, namespace: &str) -> bool {
    let prefix = format!("{namespace}:");
    let norm = |s: &str| {
        s.trim()
            .to_ascii_lowercase()
            .trim_start_matches(&prefix)
            .to_string()
    };
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn statement(agent: &str, operator: &str, head: &str) -> Value {
        json!({
            "_type": "https://in-toto.io/Statement/v1",
            "subject": [{"name": "github:acme/api:pr:421", "digest": {"gitCommit": head}}],
            "predicateType": PROVENANCE_PREDICATE_TYPE,
            "predicate": {
                "spec_version": "0.1",
                "agent": {"name": agent, "version": null},
                "operator": {"id": operator},
                "session": {"id": "s", "started_at": "2026-08-01T00:00:00Z"},
                "change": {"base_commit": "base", "head_commit": head}
            }
        })
    }

    fn envelope(statement: &Value, signatures: usize) -> Value {
        let payload = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(statement).unwrap());
        json!({
            "payloadType": DSSE_PAYLOAD_TYPE,
            "payload": payload,
            "signatures": (0..signatures).map(|i| json!({"keyid": format!("k{i}"), "sig": "AA=="})).collect::<Vec<_>>()
        })
    }

    fn load(files: &[(&str, &Value)], verified_by: Option<&str>) -> Result<Attestations> {
        let dir = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for (name, value) in files {
            let path = dir.path().join(name);
            let text = if name.ends_with(".jsonl") {
                value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| serde_json::to_string(v).unwrap() + "\n")
                    .collect::<String>()
            } else {
                serde_json::to_string_pretty(value).unwrap()
            };
            std::fs::write(&path, text).unwrap();
            paths.push(path);
        }
        Attestations::load(&paths, verified_by.map(String::from))
    }

    #[test]
    fn bare_statements_are_declared_evidence_even_when_a_verifier_is_named() {
        let a = load(
            &[("p.json", &statement("codex", "github:alice", "head"))],
            Some("me"),
        )
        .unwrap();
        let auth = a.authorship("head").unwrap().unwrap();
        assert_eq!(auth.agent, "codex");
        assert_eq!(auth.operator.as_deref(), Some("github:alice"));
        assert_eq!(auth.evidence.len(), 1);
        assert_eq!(auth.evidence[0].kind, EvidenceKind::Declared);
        assert_eq!(auth.evidence[0].source, "attestation");
        let record = &auth.records[&auth.evidence[0].r#ref];
        assert!(!record.signed);
        assert_eq!(record.verified_by.as_deref(), Some("me"));
        assert!(record.file.ends_with("p.json#1"));
        assert!(a.authorship("other").unwrap().is_none());
    }

    #[test]
    fn signed_needs_a_signature_and_a_stated_verifier() {
        let s = statement("codex", "alice", "head");
        let signed = load(
            &[("e.json", &envelope(&s, 1))],
            Some("gh attestation verify"),
        )
        .unwrap();
        let auth = signed.authorship("head").unwrap().unwrap();
        assert_eq!(auth.evidence[0].kind, EvidenceKind::Signed);
        assert_eq!(auth.operator.as_deref(), Some("alice"));
        let unverified = load(&[("e.json", &envelope(&s, 1))], None).unwrap();
        let auth = unverified.authorship("head").unwrap().unwrap();
        assert_eq!(auth.evidence[0].kind, EvidenceKind::Declared);
        assert!(auth.records.values().next().unwrap().signed);
        let unsigned = load(&[("e.json", &envelope(&s, 0))], Some("x")).unwrap();
        assert_eq!(
            unsigned.authorship("head").unwrap().unwrap().evidence[0].kind,
            EvidenceKind::Declared
        );
    }

    #[test]
    fn sigstore_bundles_and_json_lines_are_read_and_duplicates_collapse() {
        let s = statement("codex", "alice", "head");
        let bundle = json!({"mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
            "verificationMaterial": {}, "dsseEnvelope": envelope(&s, 1)});
        let a = load(
            &[
                ("a.json", &bundle),
                (
                    "b.jsonl",
                    &json!([envelope(&s, 1), statement("codex", "alice", "elsewhere")]),
                ),
            ],
            Some("v"),
        )
        .unwrap();
        let auth = a.authorship("head").unwrap().unwrap();
        // The bundle and the first line carry the same payload: one attestation, one record.
        assert_eq!(auth.evidence.len(), 1);
        assert_eq!(auth.records.len(), 1);
        assert_eq!(auth.evidence[0].kind, EvidenceKind::Signed);
        assert!(a.authorship("elsewhere").unwrap().is_some());
    }

    #[test]
    fn conflicts_and_bad_bindings_are_errors_not_silence() {
        let a = load(
            &[
                ("a.json", &statement("codex", "alice", "head")),
                ("b.json", &statement("claude-code", "alice", "head")),
            ],
            None,
        )
        .unwrap();
        assert!(
            a.authorship("head")
                .unwrap_err()
                .to_string()
                .contains("conflicting")
        );
        // Subject says head, predicate says another commit: bound but inconsistent.
        let mut s = statement("codex", "alice", "head");
        s["predicate"]["change"]["head_commit"] = "other".into();
        let a = load(&[("a.json", &s)], None).unwrap();
        assert!(a.authorship("head").is_err());
        // Another predicate type on the same subject is not an authorship claim.
        let mut s = statement("codex", "alice", "head");
        s["predicateType"] = "https://example.com/other".into();
        let a = load(&[("a.json", &s)], None).unwrap();
        assert!(a.authorship("head").unwrap().is_none());
    }

    #[test]
    fn malformed_containers_are_rejected() {
        assert!(load(&[("x.json", &json!({"hello": 1}))], None).is_err());
        let mut e = envelope(&statement("codex", "alice", "head"), 1);
        e["payloadType"] = "text/plain".into();
        assert!(load(&[("x.json", &e)], None).is_err());
        let mut e = envelope(&statement("codex", "alice", "head"), 1);
        e["payload"] = "not base64!".into();
        assert!(load(&[("x.json", &e)], None).is_err());
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("x.json"), "{").unwrap();
        assert!(Attestations::load(&[dir.path().to_path_buf()], None).is_err());
        assert!(Attestations::load(&[dir.path().join("missing.json")], None).is_err());
        assert!(Attestations::load(&[], None).unwrap().is_empty());
    }

    fn review(kind: &str, id: &str, decision: &str, head: &str) -> Value {
        json!({
            "_type": "https://in-toto.io/Statement/v1",
            "subject": [{"name": "github:acme/api:pr:421", "digest": {"gitCommit": head}}],
            "predicateType": REVIEW_PREDICATE_TYPE,
            "predicate": {
                "spec_version": "0.1",
                "reviewer": {"kind": kind, "id": id, "agent": if kind == "agent" { json!("claude-code-review") } else { Value::Null }},
                "decision": decision,
                "change": {"head_commit": head},
                "submitted_at": "2026-08-14T08:59:10Z",
                "operator": if kind == "agent" { json!({"id": "github:Carol"}) } else { Value::Null },
                "instructions": if kind == "agent" { json!({"owner": "github:security", "digest": null}) } else { Value::Null },
                "model": if kind == "agent" { json!("claude-opus-5") } else { Value::Null }
            }
        })
    }

    fn verification(bundles: Vec<Value>, san: &str, issuer: &str) -> Value {
        Value::Array(
            bundles
                .into_iter()
                .map(|b| {
                    json!({
                        "attestation": {"bundle": {"mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json", "verificationMaterial": {}, "dsseEnvelope": b}},
                        "verificationResult": {"signature": {"certificate": {"subjectAlternativeName": san, "issuer": issuer}}}
                    })
                })
                .collect(),
        )
    }

    #[test]
    fn review_claims_are_read_and_bound() {
        let a = load(
            &[
                (
                    "agent.json",
                    &envelope(
                        &review("agent", "github:Acme-Review[bot]", "approved", "head"),
                        1,
                    ),
                ),
                (
                    "human.json",
                    &review("human", "github:bob", "changes_requested", "head"),
                ),
                (
                    "other.json",
                    &review("human", "github:bob", "approved", "elsewhere"),
                ),
            ],
            Some("v"),
        )
        .unwrap();
        let claims = a.reviews("head").unwrap();
        assert_eq!(claims.len(), 2);
        let agent = claims
            .iter()
            .find(|c| c.reviewer_kind == ActorKind::Agent)
            .unwrap();
        assert_eq!(agent.reviewer_id, "github:acme-review[bot]");
        assert_eq!(agent.agent.as_deref(), Some("claude-code-review"));
        assert_eq!(agent.decision, ReviewState::Approved);
        assert_eq!(agent.operator.as_deref(), Some("github:carol"));
        assert_eq!(agent.instructions_owner.as_deref(), Some("github:security"));
        assert_eq!(agent.model.as_deref(), Some("claude-opus-5"));
        assert_eq!(agent.evidence.kind, EvidenceKind::Signed);
        assert!(agent.identity.is_none(), "no verifier output, no identity");
        let human = claims
            .iter()
            .find(|c| c.reviewer_kind == ActorKind::Human)
            .unwrap();
        assert_eq!(human.decision, ReviewState::ChangesRequested);
        assert_eq!(human.evidence.kind, EvidenceKind::Declared);
        assert!(human.operator.is_none());
        assert!(
            a.authorship("head").unwrap().is_none(),
            "review claims are not authorship"
        );
        // Bound but inconsistent, or invalid against the schema: errors.
        let mut bad = review("human", "github:bob", "approved", "head");
        bad["predicate"]["change"]["head_commit"] = "other".into();
        assert!(
            load(&[("x.json", &bad)], None)
                .unwrap()
                .reviews("head")
                .is_err()
        );
        let mut bad = review("human", "github:bob", "approved", "head");
        bad["predicate"]["decision"] = "dismissed".into();
        assert!(
            load(&[("x.json", &bad)], None)
                .unwrap()
                .reviews("head")
                .is_err()
        );
    }

    #[test]
    fn verifier_output_supplies_signed_bundles_with_their_signer() {
        let s = review("agent", "github:acme-review[bot]", "approved", "head");
        let v = verification(
            vec![envelope(&s, 1)],
            "https://github.com/acme/review-bot/.github/workflows/review.yml@refs/heads/main",
            "https://token.actions.githubusercontent.com",
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("verify.json");
        std::fs::write(&path, serde_json::to_string(&v).unwrap()).unwrap();
        let mut a = Attestations::load(&[], Some("gh attestation verify".into())).unwrap();
        a.load_verification(&path).unwrap();
        let claims = a.reviews("head").unwrap();
        assert_eq!(claims.len(), 1);
        assert!(
            claims[0]
                .identity
                .as_deref()
                .unwrap()
                .starts_with("https://github.com/acme/review-bot/")
        );
        assert_eq!(claims[0].evidence.kind, EvidenceKind::Signed);
        let signer = claims[0].record.signer.as_ref().unwrap();
        assert_eq!(
            signer.issuer.as_deref(),
            Some("https://token.actions.githubusercontent.com")
        );
        assert!(claims[0].record.matched);
        // The same payload loaded again from a plain envelope keeps the signer.
        let plain = dir.path().join("plain.json");
        std::fs::write(&plain, serde_json::to_string(&envelope(&s, 1)).unwrap()).unwrap();
        a.load_path(&plain).unwrap();
        assert_eq!(a.reviews("head").unwrap().len(), 1);
        assert!(a.reviews("head").unwrap()[0].identity.is_some());
        // Missing identity or unsigned bundle: errors.
        let bad = verification(vec![envelope(&s, 1)], "", "x");
        std::fs::write(&path, serde_json::to_string(&bad).unwrap()).unwrap();
        assert!(
            Attestations::load(&[], None)
                .unwrap()
                .load_verification(&path)
                .is_err()
        );
        let bad = verification(vec![envelope(&s, 0)], "someone", "x");
        std::fs::write(&path, serde_json::to_string(&bad).unwrap()).unwrap();
        assert!(
            Attestations::load(&[], None)
                .unwrap()
                .load_verification(&path)
                .is_err()
        );
        std::fs::write(&path, "[]").unwrap();
        assert!(
            Attestations::load(&[], None)
                .unwrap()
                .load_verification(&path)
                .is_err()
        );
    }

    #[test]
    fn operators_compare_across_namespacing() {
        assert!(same_operator("github:Alice", "alice", "github"));
        assert!(same_operator("alice", "alice", "github"));
        assert!(!same_operator("github:alice", "bob", "github"));
    }
}
