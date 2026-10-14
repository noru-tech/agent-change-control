//! The rule catalogue, the default policy and policy resolution.

use crate::model::*;
use anyhow::Result;
use std::collections::BTreeMap;

/// Every rule this release evaluates, in identifier order.
pub const RULES: [RuleId; 10] = [
    RuleId::Acc001,
    RuleId::Acc002,
    RuleId::Acc003,
    RuleId::Acc004,
    RuleId::Acc005,
    RuleId::Acc006,
    RuleId::Acc007,
    RuleId::Acc008,
    RuleId::Acc009,
    RuleId::Acc010,
];

/// The rules ACP 0.2 and 0.3 specified, for re-validating manifests written under them.
pub const RULES_0_3: [RuleId; 8] = [
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

/// ACC004 requires what the forge's rules required unless a policy says otherwise.
pub fn default_from_forge() -> bool {
    true
}

impl Default for ChecksPolicy {
    fn default() -> Self {
        Self {
            required: Vec::new(),
            from_forge: default_from_forge(),
        }
    }
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

impl RuleId {
    /// Whether a rule runs unless a policy turns it on or off. ACC004 and ACC005 need data the
    /// collector gathers only when asked, under token permissions older workflows lack, so
    /// they are opt-in.
    pub const fn enabled_by_default(self) -> bool {
        !matches!(self, RuleId::Acc004 | RuleId::Acc005)
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            version: crate::manifest::VERSION.into(),
            fail_on: default_threshold(),
            minimum_authorship_evidence: default_authorship_minimum(),
            minimum_review_evidence: default_review_minimum(),
            agent_review: AgentReview::default(),
            checks: Some(ChecksPolicy::default()),
            rules: rule_defaults(&RULES),
        }
    }
}

fn rule_defaults(rules: &[RuleId]) -> BTreeMap<RuleName, RulePolicy> {
    rules
        .iter()
        .map(|rule| {
            (
                rule.name(),
                RulePolicy {
                    enabled: rule.enabled_by_default(),
                    severity: rule.default_severity(),
                },
            )
        })
        .collect()
}

/// Validate a policy against its schema and fill unspecified rules from the defaults.
pub fn resolve(p: Policy) -> Result<Policy> {
    resolve_for(p, crate::manifest::Revision::CURRENT)
}

/// [`resolve`] as the given ACP revision resolved it: its rule catalogue, its version mark, and
/// the `checks` block only from 0.4 on. Re-validating an earlier manifest needs the policy it
/// embedded, byte for byte.
pub fn resolve_for(p: Policy, revision: crate::manifest::Revision) -> Result<Policy> {
    crate::normalize::schema(&serde_json::to_value(&p)?, "policy")?;
    let rules = revision.rules();
    if let Some(name) = p
        .rules
        .keys()
        .find(|n| !rules.iter().any(|r| r.name() == **n))
    {
        anyhow::bail!("ACV010 ACP {} has no rule {name}", revision.version());
    }
    ensure_checks(&p, revision)?;
    let mut agent_review = p.agent_review;
    agent_review.require.sort();
    agent_review.require.dedup();
    agent_review.labels.sort();
    agent_review.labels.dedup();
    let checks = revision.has_checks().then(|| {
        let mut checks = p.checks.unwrap_or_default();
        checks.required.sort();
        checks.required.dedup();
        checks
    });
    let mut full = Policy {
        version: revision.version().into(),
        fail_on: p.fail_on,
        minimum_authorship_evidence: p.minimum_authorship_evidence,
        minimum_review_evidence: p.minimum_review_evidence,
        agent_review,
        checks,
        rules: rule_defaults(rules),
    };
    full.rules.extend(p.rules);
    Ok(full)
}

fn ensure_checks(p: &Policy, revision: crate::manifest::Revision) -> Result<()> {
    if p.checks.is_some() && !revision.has_checks() {
        anyhow::bail!("ACV010 ACP {} has no checks policy", revision.version());
    }
    if let Some(checks) = &p.checks {
        anyhow::ensure!(
            checks.required.iter().all(|n| !n.trim().is_empty()),
            "ACV010 checks.required names must not be empty"
        );
    }
    Ok(())
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
