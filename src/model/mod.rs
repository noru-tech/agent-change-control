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

/// How a piece of evidence was obtained. The derived ordering is the string order used for
/// sorting evidence lists; [`EvidenceKind::strength`] is the trust ordering policies use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Declared,
    Derived,
    Observed,
    /// Carried by an attestation whose signature the caller verified before handing it over.
    Signed,
}

impl EvidenceKind {
    /// Trust ordering: `derived < declared < observed < signed`. A computed inference is weaker
    /// than a party's explicit claim, which is weaker than what the forge itself recorded, which
    /// is weaker than a claim under a verified signature.
    pub const fn strength(self) -> u8 {
        match self {
            EvidenceKind::Derived => 0,
            EvidenceKind::Declared => 1,
            EvidenceKind::Observed => 2,
            EvidenceKind::Signed => 3,
        }
    }
}

/// The strongest kind among `evidence`, or `None` when there is none.
pub fn strongest(evidence: &[Evidence]) -> Option<EvidenceKind> {
    evidence.iter().map(|e| e.kind).max_by_key(|k| k.strength())
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
    #[serde(rename = "ACC007")]
    Acc007,
    #[serde(rename = "ACC008")]
    Acc008,
    #[serde(rename = "ACC009")]
    Acc009,
    #[serde(rename = "ACC010")]
    Acc010,
}

/// Human-readable rule names, used as policy keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleName {
    AgentChangeWithoutIndependentHuman,
    ApproverIsAuthor,
    MergedWithoutIndependentApproval,
    UnknownAgentOperator,
    AgentApprovalRecorded,
    SameVendorWriteAndReview,
    AgentApprovalNotIndependent,
    AgentApprovalUnsigned,
}

/// The dimensions along which an agent reviewer's independence from the effective author is
/// evaluated. Each is `independent`, `dependent` or `unknown` per approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// The reviewing agent's operator differs from the effective human.
    Operator,
    /// The reviewing agent's vendor differs from the author agent's vendor.
    Provider,
    /// The review attestation's verified signer differs from the authorship attestation's.
    Identity,
    /// The reviewer's instructions owner is not the effective human.
    Instructions,
}

/// The opt-in policy under which an agent approval can satisfy independence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentReview {
    /// Off by default: agent approvals never qualify unless a policy says so.
    #[serde(default)]
    pub satisfies_independence: bool,
    /// The dimensions that must be `independent` for an agent approval to qualify.
    #[serde(default = "crate::policy::default_require")]
    pub require: Vec<Dimension>,
    /// The weakest evidence an agent approval may rest on; `signed` in this version.
    #[serde(default = "crate::policy::default_agent_minimum")]
    pub minimum_evidence: EvidenceKind,
    /// When non-empty, only changes carrying one of these forge labels are in scope.
    #[serde(default)]
    pub labels: Vec<String>,
}

impl RuleId {
    /// The public identifier, e.g. `ACC001`.
    pub const fn code(self) -> &'static str {
        match self {
            RuleId::Acc001 => "ACC001",
            RuleId::Acc002 => "ACC002",
            RuleId::Acc003 => "ACC003",
            RuleId::Acc006 => "ACC006",
            RuleId::Acc007 => "ACC007",
            RuleId::Acc008 => "ACC008",
            RuleId::Acc009 => "ACC009",
            RuleId::Acc010 => "ACC010",
        }
    }

    /// The rule's policy name.
    pub const fn name(self) -> RuleName {
        match self {
            RuleId::Acc001 => RuleName::AgentChangeWithoutIndependentHuman,
            RuleId::Acc002 => RuleName::ApproverIsAuthor,
            RuleId::Acc003 => RuleName::MergedWithoutIndependentApproval,
            RuleId::Acc006 => RuleName::UnknownAgentOperator,
            RuleId::Acc007 => RuleName::AgentApprovalRecorded,
            RuleId::Acc008 => RuleName::SameVendorWriteAndReview,
            RuleId::Acc009 => RuleName::AgentApprovalNotIndependent,
            RuleId::Acc010 => RuleName::AgentApprovalUnsigned,
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
            RuleName::AgentApprovalRecorded => "agent_approval_recorded",
            RuleName::SameVendorWriteAndReview => "same_vendor_write_and_review",
            RuleName::AgentApprovalNotIndependent => "agent_approval_not_independent",
            RuleName::AgentApprovalUnsigned => "agent_approval_unsigned",
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
    /// For agents: the organization that builds and operates the model behind the tool, from
    /// the vendor registry. Null for other kinds and for unregistered agents. May be absent in
    /// exports written before it existed.
    #[serde(default)]
    pub vendor: Option<String>,
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

/// What a signed review attestation says about the agent that produced a review. Every field
/// may be null; none can be observed from the forge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ReviewAgent {
    /// The human who directed the reviewing agent for this review.
    pub operator: Option<String>,
    /// The signing identity the caller's verifier established for the review attestation.
    pub identity: Option<String>,
    /// The actor that controls what the reviewer was instructed to check.
    pub instructions_owner: Option<String>,
    /// The model the attestation names; recorded, not an independence input.
    pub model: Option<String>,
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
    /// Present only for an agent reviewer with a matched review attestation. May be absent in
    /// exports written before it existed.
    #[serde(default)]
    pub agent: Option<ReviewAgent>,
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
    /// The commit the merge produced, when merged and known. Null for an open change, and for a
    /// merged change whose merge commit the forge did not report. The one key that may be absent
    /// on input, so that exports written before it existed remain valid.
    #[serde(default)]
    pub merge_commit_sha: Option<String>,
    pub commits: Vec<Commit>,
    pub forge_author: Identity,
    pub author: Identity,
    pub agent_operator: Option<Operator>,
    pub reviews: Vec<Review>,
    pub reviews_complete: bool,
    pub merger: Option<Identity>,
    pub provenance: Vec<Evidence>,
    /// The forge labels on the change, sorted and de-duplicated. May be absent in exports
    /// written before it existed.
    #[serde(default)]
    pub labels: Vec<String>,
}

/// An attestation the collector consulted, as handed over by the caller. `acc` does not verify
/// signatures: `signed` records that the container carried one, and `verified_by` records the
/// caller's statement of who verified it. Evidence is `signed` only when both hold.
/// The identity a verifier established for an attestation's signature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Signer {
    /// The certificate's subject alternative name, or the key identifier.
    pub identity: String,
    /// The OIDC issuer that vouched for the identity, or null for a raw key.
    pub issuer: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Attestation {
    /// `<file>#<n>`: the file and the 1-based document or line within it.
    pub file: String,
    pub predicate_type: String,
    /// `sha256:<hex>` over the DSSE payload bytes, or over the canonical bytes of a bare
    /// Statement.
    pub payload_digest: String,
    pub signed: bool,
    pub verified_by: Option<String>,
    /// The signer, when the attestation came from a verifier's output (`--verification`).
    #[serde(default)]
    pub signer: Option<Signer>,
    /// Whether the attestation matched a fact of the change it is bound to. False for a review
    /// attestation with no corresponding forge review.
    #[serde(default = "default_true")]
    pub matched: bool,
}

/// A normalized export: the input to evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Events {
    pub version: String,
    pub repository: String,
    pub window: Window,
    pub actors: BTreeMap<String, Actor>,
    /// Attestations that produced evidence, keyed by `attestation:<16 hex>` of their payload
    /// digest. May be absent in exports written before it existed.
    #[serde(default)]
    pub attestations: BTreeMap<String, Attestation>,
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
    /// The weakest evidence an agent operator claim may rest on and still name the effective
    /// human. Below it the operator is treated as unknown.
    #[serde(default = "crate::policy::default_authorship_minimum")]
    pub minimum_authorship_evidence: EvidenceKind,
    /// The weakest evidence an approval may rest on and still qualify as independent approval.
    #[serde(default = "crate::policy::default_review_minimum")]
    pub minimum_review_evidence: EvidenceKind,
    /// Whether, and on what conditions, an agent approval satisfies independence.
    #[serde(default)]
    pub agent_review: AgentReview,
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
            EvidenceKind::Signed,
        ];
        let strings: Vec<String> = kinds
            .iter()
            .map(|k| serde_json::to_string(k).unwrap())
            .collect();
        assert!(strings.windows(2).all(|w| w[0] < w[1]));
        assert!(EvidenceKind::Derived.strength() < EvidenceKind::Declared.strength());
        assert!(EvidenceKind::Declared.strength() < EvidenceKind::Observed.strength());
        assert!(EvidenceKind::Observed.strength() < EvidenceKind::Signed.strength());
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
