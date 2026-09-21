mod common;

use agent_change_control::{Exit, manifest, model::*, normalize, output::intoto};
use serde_json::Value;

/// The policy a fixture is evaluated under: its `policy.yml` when present, else the default.
fn fixture_policy(dir: &std::path::Path) -> Policy {
    let path = dir.join("policy.yml");
    if !path.exists() {
        return Policy::default();
    }
    let p: Policy = serde_saphyr::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    agent_change_control::policy::resolve(p).unwrap()
}

/// Every fixture directory with an `events.json` is evaluated under its policy (the default
/// unless the directory has a `policy.yml`) and compared with its reviewed expectations and
/// byte-exact goldens.
#[test]
fn fixtures_and_golden_outputs() {
    let mut seen = 0;
    for entry in std::fs::read_dir(common::fixtures()).unwrap() {
        let dir = entry.unwrap().path();
        if !dir.join("events.json").exists() {
            continue;
        }
        seen += 1;
        let policy = fixture_policy(&dir);
        let input = std::fs::read_to_string(dir.join("events.json")).unwrap();
        let e: Events = serde_json::from_str(&input).unwrap();
        let spec: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("expected-rules.json")).unwrap(),
        )
        .unwrap();
        let result = manifest::evaluate(e.clone(), policy.clone());
        if let Some(code) = spec["validation_error"].as_str() {
            let err = result.unwrap_err();
            assert!(err.to_string().contains(code), "{}: {err}", dir.display());
            continue;
        }
        let m = result.unwrap();
        let codes: Vec<_> = m.findings.iter().map(|f| f.rule_id).collect();
        assert_eq!(
            serde_json::to_value(codes).unwrap(),
            spec["rules"],
            "{}",
            dir.display()
        );
        normalize::schema(&serde_json::to_value(&m).unwrap(), "manifest").unwrap();
        manifest::validate(&m).unwrap();
        // Determinism: input order must not matter.
        let mut shuffled = e;
        shuffled.changes.reverse();
        for c in &mut shuffled.changes {
            c.reviews.reverse();
            c.provenance.reverse();
            c.commits.reverse();
        }
        assert_eq!(
            normalize::canonical(&m).unwrap(),
            normalize::canonical(&manifest::evaluate(shuffled, policy).unwrap()).unwrap()
        );
        let canonical = normalize::canonical(&m).unwrap();
        if std::env::var_os("UPDATE_GOLDENS").is_some() {
            std::fs::write(dir.join("expected-manifest.json"), &canonical).unwrap();
            std::fs::write(
                dir.join("expected-findings.json"),
                normalize::canonical(&m.findings).unwrap(),
            )
            .unwrap();
        }
        assert_eq!(
            canonical,
            std::fs::read_to_string(dir.join("expected-manifest.json")).unwrap(),
            "{}",
            dir.display()
        );
        assert_eq!(
            normalize::canonical(&m.findings).unwrap(),
            std::fs::read_to_string(dir.join("expected-findings.json")).unwrap(),
            "{}",
            dir.display()
        );
        // Every emitted Statement validates against the statement schema and round-trips
        // through statement validation; the JSON Lines form does too, line by line.
        let statement: Value = serde_json::from_str(&intoto::render(&m).unwrap()).unwrap();
        normalize::schema(&statement, "statement").unwrap();
        intoto::validate_statement(&statement).unwrap();
        let lines = intoto::render_jsonl(&m).unwrap();
        let parts = intoto::validate_jsonl(&lines).unwrap();
        assert_eq!(parts.len(), m.events.changes.len());
        for part in &parts {
            manifest::validate(part).unwrap();
        }
        assert_eq!(
            parts
                .iter()
                .flat_map(|p| p.findings.iter().map(|f| &f.id))
                .collect::<Vec<_>>(),
            m.findings.iter().map(|f| &f.id).collect::<Vec<_>>(),
            "{}: finding IDs survive the split",
            dir.display()
        );
    }
    assert!(seen >= 20, "expected the fixture set, saw {seen}");
}

/// Byte-exact attestation goldens for a failing and a clean agent change.
#[test]
fn attestation_goldens() {
    for name in ["claude-operator-self-approved", "claude-clean"] {
        let dir = common::fixtures().join(name);
        let e: Events =
            serde_json::from_str(&std::fs::read_to_string(dir.join("events.json")).unwrap())
                .unwrap();
        let m = manifest::evaluate(e, Policy::default()).unwrap();
        for (file, rendered) in [
            ("expected.intoto.json", intoto::render(&m).unwrap()),
            ("expected.intoto.jsonl", intoto::render_jsonl(&m).unwrap()),
        ] {
            if std::env::var_os("UPDATE_GOLDENS").is_some() {
                std::fs::write(dir.join(file), &rendered).unwrap();
            }
            assert_eq!(
                rendered,
                std::fs::read_to_string(dir.join(file)).unwrap(),
                "{name}/{file}"
            );
        }
    }
}

fn example() -> Manifest {
    let e = serde_json::from_str(include_str!(
        "fixtures/claude-operator-self-approved/events.json"
    ))
    .unwrap();
    manifest::evaluate(e, Policy::default()).unwrap()
}

#[test]
fn tampering_is_rejected() {
    let mut m = example();
    m.findings.clear();
    assert!(manifest::validate(&m).is_err());
    let mut m = example();
    m.summary.clean = 1;
    assert!(manifest::validate(&m).is_err());
    let mut m = example();
    m.findings[0].severity = Severity::Info;
    assert!(manifest::validate(&m).is_err());
}

#[test]
fn warnings_do_not_fail() {
    let e =
        serde_json::from_str(include_str!("fixtures/agent-operator-unknown/events.json")).unwrap();
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    assert_eq!(manifest::check(&m, None, None).unwrap().1, Exit::Ok);
    assert_eq!(m.summary.clean, 0);
}

#[test]
fn disposition_preserves_history_and_expires() {
    let mut m = example();
    for f in &mut m.findings {
        f.disposition = Disposition {
            status: DispositionStatus::Accepted,
            owner: Some("security".into()),
            decided_at: "2026-09-01".parse().ok(),
            expires_at: "2026-09-30".parse().ok(),
            rationale: Some("Emergency fix".into()),
            remediated_at: None,
        };
    }
    let err = manifest::check(&m, None, None).unwrap_err();
    assert_eq!(agent_change_control::exit_for(&err), Exit::Usage);
    let date = |s: &str| s.parse().ok();
    let (checked, exit) = manifest::check(&m, None, date("2026-09-18")).unwrap();
    assert_eq!(exit, Exit::Ok);
    assert_eq!(checked.findings.len(), 3);
    assert_eq!(
        manifest::check(&m, None, date("2026-10-01")).unwrap().1,
        Exit::PolicyFailed
    );
}

#[test]
fn policy_override_changes_the_verdict_not_the_facts() {
    let m = example();
    let mut lenient = Policy {
        fail_on: Severity::High,
        ..Policy::default()
    };
    for rule in lenient.rules.values_mut() {
        rule.severity = Severity::Warning;
    }
    let (checked, exit) = manifest::check(&m, Some(lenient), None).unwrap();
    assert_eq!(exit, Exit::Ok);
    assert_eq!(checked.findings.len(), 3);
    assert_eq!(checked.events.changes.len(), m.events.changes.len());
    assert_eq!(checked.generated.source_digest, m.generated.source_digest);
}

#[test]
fn evidence_minimums_turn_weak_facts_into_unknowns_not_passes() {
    // claude-clean: declared operator, observed independent approval. Requiring signed
    // authorship evidence makes the operator unknown for evaluation (ACC006 fails, ACC001 and
    // ACC003 unknown); requiring signed review evidence disqualifies the approval (ACC001 and
    // ACC003 fail). Neither can produce a pass.
    let e: Events =
        serde_json::from_str(include_str!("fixtures/claude-clean/events.json")).unwrap();
    let signed_authorship = Policy {
        minimum_authorship_evidence: EvidenceKind::Signed,
        ..Policy::default()
    };
    let m = manifest::evaluate(e.clone(), signed_authorship).unwrap();
    let codes: Vec<_> = m.findings.iter().map(|f| f.rule_id).collect();
    assert_eq!(codes, vec![RuleId::Acc006]);
    assert!(
        m.findings[0]
            .explanation
            .contains("below the policy minimum")
    );
    let status = |id: RuleId| {
        m.assessments
            .iter()
            .find(|a| a.rule_id == id)
            .unwrap()
            .status
    };
    assert_eq!(status(RuleId::Acc001), Status::Unknown);
    assert_eq!(status(RuleId::Acc003), Status::Unknown);
    assert_eq!(m.summary.clean, 0);
    let signed_review = Policy {
        minimum_review_evidence: EvidenceKind::Signed,
        ..Policy::default()
    };
    let m = manifest::evaluate(e.clone(), signed_review).unwrap();
    let codes: Vec<_> = m.findings.iter().map(|f| f.rule_id).collect();
    assert_eq!(codes, vec![RuleId::Acc001, RuleId::Acc003]);
    // The defaults accept everything the fixture carries.
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    assert!(m.findings.is_empty());
    manifest::validate(&m).unwrap();
}

#[test]
fn signed_evidence_must_resolve_to_a_verified_attestation() {
    let base: Events =
        serde_json::from_str(include_str!("fixtures/claude-clean/events.json")).unwrap();
    let signed = Evidence {
        source: "attestation".into(),
        r#ref: "attestation:0123456789abcdef".into(),
        kind: EvidenceKind::Signed,
    };
    let record = |signed: bool, verified: Option<&str>| Attestation {
        file: "prov.json#1".into(),
        predicate_type: "https://noru.tech/spec/ai-change-provenance/provenance/v0.1".into(),
        payload_digest: format!("sha256:0123456789abcdef{}", "0".repeat(48)),
        signed,
        verified_by: verified.map(String::from),
        signer: None,
        matched: true,
    };
    // Unresolved reference.
    let mut e = base.clone();
    e.changes[0].author.provenance.push(signed.clone());
    let err = normalize::events(e).unwrap_err().to_string();
    assert!(err.contains("ACV003"), "{err}");
    // Resolved, but the attestation carried no signature.
    let mut e = base.clone();
    e.changes[0].author.provenance.push(signed.clone());
    e.attestations
        .insert(signed.r#ref.clone(), record(false, Some("me")));
    assert!(normalize::events(e).is_err());
    // Resolved and signed, but nobody stated they verified it.
    let mut e = base.clone();
    e.changes[0].author.provenance.push(signed.clone());
    e.attestations
        .insert(signed.r#ref.clone(), record(true, None));
    assert!(normalize::events(e).is_err());
    // Signed evidence from any other source is rejected.
    let mut e = base.clone();
    e.changes[0].author.provenance.push(Evidence {
        source: "pr_metadata".into(),
        ..signed.clone()
    });
    assert!(normalize::events(e).is_err());
    // A malformed or mismatched registry entry is rejected.
    let mut e = base.clone();
    e.attestations
        .insert("attestation:nothex".into(), record(true, Some("me")));
    assert!(normalize::events(e).is_err());
    let mut e = base.clone();
    e.attestations.insert(
        "attestation:fedcba9876543210".into(),
        record(true, Some("me")),
    );
    assert!(normalize::events(e).is_err());
    // The consistent case validates, round-trips the schema and stays byte-stable.
    let mut e = base;
    e.changes[0].author.provenance.push(signed.clone());
    e.attestations
        .insert(signed.r#ref.clone(), record(true, Some("me")));
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    normalize::schema(&serde_json::to_value(&m).unwrap(), "manifest").unwrap();
    manifest::validate(&m).unwrap();
    assert_eq!(m.events.attestations.len(), 1);
}

#[test]
fn agent_review_facts_need_an_agent_reviewer_and_human_references() {
    let base: Events =
        serde_json::from_str(include_str!("fixtures/claude-clean/events.json")).unwrap();
    // bob, a human, cannot carry agent facts.
    let mut e = base.clone();
    e.changes[0].reviews[0].agent = Some(ReviewAgent::default());
    let err = normalize::events(e).unwrap_err().to_string();
    assert!(err.contains("ACV003"), "{err}");
    // An agent reviewer may; its operator must resolve to a human.
    let mut e = base.clone();
    e.actors.get_mut("github:bob").unwrap().kind = ActorKind::Agent;
    e.changes[0].reviews[0].agent = Some(ReviewAgent {
        operator: Some("github:nobody".into()),
        ..ReviewAgent::default()
    });
    assert!(normalize::events(e).is_err());
    let mut e = base.clone();
    e.actors.get_mut("github:bob").unwrap().kind = ActorKind::Agent;
    e.changes[0].reviews[0].agent = Some(ReviewAgent {
        operator: Some("github:alice".into()),
        identity: Some("key:abc".into()),
        ..ReviewAgent::default()
    });
    e.changes[0].labels = vec!["Zeta".into(), "alpha".into(), "alpha".into()];
    let normalized = normalize::events(e).unwrap();
    assert_eq!(normalized.changes[0].labels, vec!["Zeta", "alpha"]);
    let m = manifest::evaluate(normalized, Policy::default()).unwrap();
    normalize::schema(&serde_json::to_value(&m).unwrap(), "manifest").unwrap();
    manifest::validate(&m).unwrap();
}

#[test]
fn exports_written_under_0_1_still_evaluate() {
    let e: Events =
        serde_json::from_str(include_str!("fixtures/human-clean/events-0.1.json")).unwrap();
    assert_eq!(e.version, "0.1");
    let m = manifest::evaluate(e, Policy::default()).unwrap();
    assert_eq!(m.version, "0.2");
    assert_eq!(m.events.version, "0.1");
    manifest::validate(&m).unwrap();
    let mut v: Value =
        serde_json::from_str(include_str!("fixtures/human-clean/events-0.1.json")).unwrap();
    v["version"] = "0.3".into();
    assert!(normalize::schema(&v, "events").is_err());
}

#[test]
fn schema_rejects_extra_fields() {
    let mut v: Value =
        serde_json::from_str(include_str!("fixtures/human-clean/events.json")).unwrap();
    v["surprise"] = true.into();
    assert!(normalize::schema(&v, "events").is_err());
}
