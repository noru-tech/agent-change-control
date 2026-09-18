mod common;

use agent_change_control::{Exit, manifest, model::*, normalize};
use serde_json::Value;

/// Every fixture directory with an `events.json` is evaluated under the default policy and
/// compared with its reviewed expectations and byte-exact goldens.
#[test]
fn fixtures_and_golden_outputs() {
    let mut seen = 0;
    for entry in std::fs::read_dir(common::fixtures()).unwrap() {
        let dir = entry.unwrap().path();
        if !dir.join("events.json").exists() {
            continue;
        }
        seen += 1;
        let input = std::fs::read_to_string(dir.join("events.json")).unwrap();
        let e: Events = serde_json::from_str(&input).unwrap();
        let spec: Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("expected-rules.json")).unwrap(),
        )
        .unwrap();
        let result = manifest::evaluate(e.clone(), Policy::default());
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
            normalize::canonical(&manifest::evaluate(shuffled, Policy::default()).unwrap())
                .unwrap()
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
    }
    assert!(seen >= 20, "expected the fixture set, saw {seen}");
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
fn schema_rejects_extra_fields() {
    let mut v: Value =
        serde_json::from_str(include_str!("fixtures/human-clean/events.json")).unwrap();
    v["surprise"] = true.into();
    assert!(normalize::schema(&v, "events").is_err());
}
