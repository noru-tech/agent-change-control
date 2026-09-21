//! The rule catalogue, the default policy and policy resolution.

use crate::model::*;
use anyhow::Result;
use std::collections::BTreeMap;

/// Every rule this release evaluates, in identifier order.
pub const RULES: [RuleId; 8] = [
    RuleId::Acc001,
    RuleId::Acc002,
    RuleId::Acc003,
    RuleId::Acc006,
    RuleId::Acc007,
    RuleId::Acc008,
    RuleId::Acc009,
    RuleId::Acc010,
];

/// The default `fail_on` threshold.
pub fn default_threshold() -> Severity {
    Severity::Medium
}

/// The default `minimum_authorship_evidence`: every tier counts, including derived records.
pub fn default_authorship_minimum() -> EvidenceKind {
    EvidenceKind::Derived
}

/// The default `minimum_review_evidence`: what the forge recorded.
pub fn default_review_minimum() -> EvidenceKind {
    EvidenceKind::Observed
}

/// The dimensions an agent approval must be independent on by default: the three a signed
/// review document plus the vendor registry can establish. `instructions` is opt-in, because
/// few producers can declare it yet.
pub fn default_require() -> Vec<Dimension> {
    vec![
        Dimension::Operator,
        Dimension::Provider,
        Dimension::Identity,
    ]
}

/// The weakest evidence an agent approval may rest on: `signed`, and not lower in this version.
pub fn default_agent_minimum() -> EvidenceKind {
    EvidenceKind::Signed
}

impl Default for AgentReview {
    fn default() -> Self {
        Self {
            satisfies_independence: false,
            require: default_require(),
            minimum_evidence: default_agent_minimum(),
            labels: Vec::new(),
        }
    }
}

impl RuleId {
    /// The severity a rule carries unless a policy overrides it. ACC006 and ACC010 are
    /// advisory; ACC007 is an observation.
    pub const fn default_severity(self) -> Severity {
        match self {
            RuleId::Acc006 | RuleId::Acc010 => Severity::Warning,
            RuleId::Acc007 => Severity::Info,
            _ => Severity::High,
        }
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            version: "0.2".into(),
            fail_on: default_threshold(),
            minimum_authorship_evidence: default_authorship_minimum(),
            minimum_review_evidence: default_review_minimum(),
            agent_review: AgentReview::default(),
            rules: RULES
                .iter()
                .map(|rule| {
                    (
                        rule.name(),
                        RulePolicy {
                            enabled: true,
                            severity: rule.default_severity(),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        }
    }
}

/// Validate a policy against its schema and fill unspecified rules from the defaults.
pub fn resolve(p: Policy) -> Result<Policy> {
    crate::normalize::schema(&serde_json::to_value(&p)?, "policy")?;
    let mut agent_review = p.agent_review;
    agent_review.require.sort();
    agent_review.require.dedup();
    agent_review.labels.sort();
    agent_review.labels.dedup();
    let mut full = Policy {
        fail_on: p.fail_on,
        minimum_authorship_evidence: p.minimum_authorship_evidence,
        minimum_review_evidence: p.minimum_review_evidence,
        agent_review,
        ..Policy::default()
    };
    full.rules.extend(p.rules);
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_covers_every_rule() {
        let p = Policy::default();
        assert_eq!(p.rules.len(), RULES.len());
        assert_eq!(p.fail_on, Severity::Medium);
        assert_eq!(p.minimum_authorship_evidence, EvidenceKind::Derived);
        assert_eq!(p.minimum_review_evidence, EvidenceKind::Observed);
        assert!(!p.agent_review.satisfies_independence);
        assert_eq!(p.agent_review.require, default_require());
        assert_eq!(p.agent_review.minimum_evidence, EvidenceKind::Signed);
        assert_eq!(
            p.rules[&RuleName::UnknownAgentOperator].severity,
            Severity::Warning
        );
        assert_eq!(
            p.rules[&RuleName::AgentApprovalRecorded].severity,
            Severity::Info
        );
        assert_eq!(
            p.rules[&RuleName::AgentApprovalUnsigned].severity,
            Severity::Warning
        );
        assert_eq!(
            p.rules[&RuleName::SameVendorWriteAndReview].severity,
            Severity::High
        );
        assert_eq!(
            p.rules[&RuleName::ApproverIsAuthor].severity,
            Severity::High
        );
    }

    #[test]
    fn partial_policies_inherit_defaults() {
        let partial: Policy = serde_json::from_value(serde_json::json!({
            "version": "0.1",
            "fail_on": "high",
            "minimum_authorship_evidence": "signed",
            "rules": {"approver_is_author": {"enabled": false, "severity": "warning"}}
        }))
        .unwrap();
        let p = resolve(partial).unwrap();
        assert_eq!(p.rules.len(), RULES.len());
        assert_eq!(p.fail_on, Severity::High);
        assert_eq!(p.minimum_authorship_evidence, EvidenceKind::Signed);
        assert_eq!(p.minimum_review_evidence, EvidenceKind::Observed);
        assert!(!p.rules[&RuleName::ApproverIsAuthor].enabled);
        assert!(p.rules[&RuleName::MergedWithoutIndependentApproval].enabled);
    }

    #[test]
    fn agent_review_is_validated_and_normalized() {
        let p: Policy = serde_json::from_value(serde_json::json!({
            "version": "0.2",
            "agent_review": {"satisfies_independence": true, "require": ["provider", "operator"], "labels": ["b", "a", "a"]},
            "rules": {}
        }))
        .unwrap();
        let p = resolve(p).unwrap();
        assert!(p.agent_review.satisfies_independence);
        assert_eq!(
            p.agent_review.require,
            vec![Dimension::Operator, Dimension::Provider]
        );
        assert_eq!(p.agent_review.labels, vec!["a", "b"]);
        assert_eq!(p.agent_review.minimum_evidence, EvidenceKind::Signed);
        for bad in [
            serde_json::json!({"version": "0.2", "agent_review": {"require": []}, "rules": {}}),
            serde_json::json!({"version": "0.2", "agent_review": {"minimum_evidence": "declared"}, "rules": {}}),
            serde_json::json!({"version": "0.2", "agent_review": {"require": ["vibes"]}, "rules": {}}),
            serde_json::json!({"version": "0.2", "agent_review": {"require": ["provider", "provider"]}, "rules": {}}),
        ] {
            assert!(crate::normalize::schema(&bad, "policy").is_err(), "{bad}");
        }
    }

    #[test]
    fn unknown_rules_are_rejected() {
        let v = serde_json::json!({"version": "0.1", "rules": {"typo": {"enabled": true, "severity": "high"}}});
        assert!(serde_json::from_value::<Policy>(v.clone()).is_err());
        assert!(crate::normalize::schema(&v, "policy").is_err());
    }
}
