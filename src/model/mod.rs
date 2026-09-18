//! The public data model. Every type here mirrors a definition in `schemas/`; every closed
//! vocabulary is an enum whose serialized form is the schema's string.
//!
//! Enum variants that participate in sorting are declared in the order of their serialized
//! strings, so canonical output is identical to a string comparison.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// A UTC instant. Serialized as RFC 3339 with a `Z` suffix; any offset is accepted on input.
pub type Timestamp = DateTime<Utc>;

/// How a piece of evidence was obtained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Declared,
    Derived,
    Observed,
}

/// What kind of principal an actor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    Human,
    Agent,
    Bot,
    Service,
    Unknown,
}

/// How an agent operator was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Explicit,
    Derived,
    Unknown,
}

/// The state of a review decision, as recorded by the forge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
}

/// Source forges. Phase 1 collects from GitHub only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Forge {
    Github,
}

/// Finding severity. Variants are declared from least to most severe so the derived ordering is
/// the policy ranking: `info < warning < medium < high`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Medium,
    High,
}

/// The outcome of one rule for one change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    Unknown,
    NotApplicable,
}

/// A recorded human decision about a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DispositionStatus {
    #[default]
    Open,
    Accepted,
    Remediated,
    FalsePositive,
}

/// Public rule identifiers. Retired identifiers are never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RuleId {
    #[serde(rename = "ACC001")]
    Acc001,
    #[serde(rename = "ACC002")]
    Acc002,
    #[serde(rename = "ACC003")]
    Acc003,
    #[serde(rename = "ACC006")]
    Acc006,
}

/// Human-readable rule names, used as policy keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleName {
    AgentChangeWithoutIndependentHuman,
    ApproverIsAuthor,
    MergedWithoutIndependentApproval,
    UnknownAgentOperator,
}

impl RuleId {
    /// The public identifier, e.g. `ACC001`.
    pub const fn code(self) -> &'static str {
        match self {
            RuleId::Acc001 => "ACC001",
            RuleId::Acc002 => "ACC002",
            RuleId::Acc003 => "ACC003",
            RuleId::Acc006 => "ACC006",
        }
    }

    /// The rule's policy name.
    pub const fn name(self) -> RuleName {
        match self {
            RuleId::Acc001 => RuleName::AgentChangeWithoutIndependentHuman,
            RuleId::Acc002 => RuleName::ApproverIsAuthor,
            RuleId::Acc003 => RuleName::MergedWithoutIndependentApproval,
            RuleId::Acc006 => RuleName::UnknownAgentOperator,
        }
    }
}

impl RuleName {
    pub const fn as_str(self) -> &'static str {
        match self {
            RuleName::AgentChangeWithoutIndependentHuman => {
                "agent_change_without_independent_human"
            }
            RuleName::ApproverIsAuthor => "approver_is_author",
            RuleName::MergedWithoutIndependentApproval => "merged_without_independent_approval",
            RuleName::UnknownAgentOperator => "unknown_agent_operator",
        }
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl fmt::Display for RuleName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A reference to where a fact came from.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: String,
    pub r#ref: String,
    pub kind: EvidenceKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Actor {
    pub kind: ActorKind,
    pub display_name: Option<String>,
}

/// The inclusive UTC collection window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub from: Timestamp,
    pub to: Timestamp,
    pub complete: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub actor_id: String,
    pub provenance: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operator {
    pub actor_id: Option<String>,
    pub confidence: Confidence,
    pub provenance: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub id: String,
    pub actor_id: String,
    pub state: ReviewState,
    pub at: Timestamp,
    pub commit_sha: String,
    pub provenance: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commit {
    pub sha: String,
    pub author: Option<Identity>,
}

/// One pull request (or equivalent) with everything the rules need.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Change {
    pub id: String,
    pub repository: String,
    pub forge: Forge,
    pub title: String,
    pub url: String,
    pub opened_at: Timestamp,
    pub merged_at: Option<Timestamp>,
    pub head_sha: String,
    pub commits: Vec<Commit>,
    pub forge_author: Identity,
    pub author: Identity,
    pub agent_operator: Option<Operator>,
    pub reviews: Vec<Review>,
    pub reviews_complete: bool,
    pub merger: Option<Identity>,
    pub provenance: Vec<Evidence>,
}

/// A normalized export: the input to evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Events {
    pub version: String,
    pub repository: String,
    pub window: Window,
    pub actors: BTreeMap<String, Actor>,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RulePolicy {
    pub enabled: bool,
    pub severity: Severity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub version: String,
    #[serde(default = "crate::policy::default_threshold")]
    pub fail_on: Severity,
    pub rules: BTreeMap<RuleName, RulePolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Disposition {
    pub status: DispositionStatus,
    pub owner: Option<String>,
    pub decided_at: Option<NaiveDate>,
    pub expires_at: Option<NaiveDate>,
    pub rationale: Option<String>,
    pub remediated_at: Option<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    pub id: String,
    pub rule_id: RuleId,
    pub rule: RuleName,
    pub severity: Severity,
    pub change_id: String,
    pub actor_ids: Vec<String>,
    pub explanation: String,
    pub provenance: Vec<Evidence>,
    pub disposition: Disposition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assessment {
    pub change_id: String,
    pub rule_id: RuleId,
    pub status: Status,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub changes: usize,
    pub human_authored: usize,
    pub agent_authored: usize,
    pub agent_operator_unknown: usize,
    pub clean: usize,
    pub with_findings: usize,
    pub indeterminate: usize,
    pub findings: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generated {
    pub tool: String,
    pub version: String,
    pub source_digest: String,
}

/// The evaluated output: embedded facts, resolved policy, findings and assessments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: String,
    pub events: Events,
    pub policy: Policy,
    pub summary: Summary,
    pub findings: Vec<Finding>,
    pub assessments: Vec<Assessment>,
    pub generated: Generated,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_serialize_to_schema_strings() {
        assert_eq!(
            serde_json::to_string(&RuleId::Acc006).unwrap(),
            "\"ACC006\""
        );
        assert_eq!(
            serde_json::to_string(&RuleName::ApproverIsAuthor).unwrap(),
            "\"approver_is_author\""
        );
        assert_eq!(
            serde_json::to_string(&ReviewState::ChangesRequested).unwrap(),
            "\"changes_requested\""
        );
        assert_eq!(
            serde_json::to_string(&Status::NotApplicable).unwrap(),
            "\"not_applicable\""
        );
        assert_eq!(
            serde_json::to_string(&DispositionStatus::FalsePositive).unwrap(),
            "\"false_positive\""
        );
    }

    #[test]
    fn sorted_enums_follow_string_order() {
        let kinds = [
            EvidenceKind::Declared,
            EvidenceKind::Derived,
            EvidenceKind::Observed,
        ];
        let strings: Vec<String> = kinds
            .iter()
            .map(|k| serde_json::to_string(k).unwrap())
            .collect();
        assert!(strings.windows(2).all(|w| w[0] < w[1]));
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Medium);
        assert!(Severity::Medium < Severity::High);
    }

    #[test]
    fn timestamps_normalize_to_utc_on_input() {
        let t: Timestamp = serde_json::from_str("\"2026-08-01T14:00:00+02:00\"").unwrap();
        assert_eq!(
            serde_json::to_string(&t).unwrap(),
            "\"2026-08-01T12:00:00Z\""
        );
    }
}
