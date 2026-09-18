//! The rule catalogue, the default policy and policy resolution.

use crate::model::*;
use anyhow::Result;
use std::collections::BTreeMap;

/// Every rule this release evaluates, in identifier order.
pub const RULES: [RuleId; 4] = [
    RuleId::Acc001,
    RuleId::Acc002,
    RuleId::Acc003,
    RuleId::Acc006,
];

/// The default `fail_on` threshold.
pub fn default_threshold() -> Severity {
    Severity::Medium
}

impl RuleId {
    /// The severity a rule carries unless a policy overrides it. ACC006 is advisory.
    pub const fn default_severity(self) -> Severity {
        match self {
            RuleId::Acc006 => Severity::Warning,
            _ => Severity::High,
        }
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            version: "0.1".into(),
            fail_on: default_threshold(),
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
    let mut full = Policy {
        fail_on: p.fail_on,
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
        assert_eq!(
            p.rules[&RuleName::UnknownAgentOperator].severity,
            Severity::Warning
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
            "rules": {"approver_is_author": {"enabled": false, "severity": "warning"}}
        }))
        .unwrap();
        let p = resolve(partial).unwrap();
        assert_eq!(p.rules.len(), RULES.len());
        assert_eq!(p.fail_on, Severity::High);
        assert!(!p.rules[&RuleName::ApproverIsAuthor].enabled);
        assert!(p.rules[&RuleName::MergedWithoutIndependentApproval].enabled);
    }

    #[test]
    fn unknown_rules_are_rejected() {
        let v = serde_json::json!({"version": "0.1", "rules": {"typo": {"enabled": true, "severity": "high"}}});
        assert!(serde_json::from_value::<Policy>(v.clone()).is_err());
        assert!(crate::normalize::schema(&v, "policy").is_err());
    }
}
