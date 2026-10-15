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

/// Source forges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Forge {
    Github,
    Gitlab,
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
    #[serde(rename = "ACC004")]
    Acc004,
    #[serde(rename = "ACC005")]
    Acc005,
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
    #[serde(rename = "ACC011")]
    Acc011,
    #[serde(rename = "ACC012")]
    Acc012,
    #[serde(rename = "ACC013")]
    Acc013,
}

/// Human-readable rule names, used as policy keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleName {
    AgentChangeWithoutIndependentHuman,
    ApproverIsAuthor,
    MergedWithoutIndependentApproval,
    MergedWithoutRequiredChecks,
    MergedWithProtectionBypass,
    UnknownAgentOperator,
    AgentApprovalRecorded,
    SameVendorWriteAndReview,
    AgentApprovalNotIndependent,
    AgentApprovalUnsigned,
    MissingRequiredReviewerTeam,
    UnaccountedBranchUpdate,
    DeployedWithoutPassingChange,
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
            RuleId::Acc004 => "ACC004",
            RuleId::Acc005 => "ACC005",
            RuleId::Acc006 => "ACC006",
            RuleId::Acc007 => "ACC007",
            RuleId::Acc008 => "ACC008",
            RuleId::Acc009 => "ACC009",
            RuleId::Acc010 => "ACC010",
            RuleId::Acc011 => "ACC011",
            RuleId::Acc012 => "ACC012",
            RuleId::Acc013 => "ACC013",
        }
    }

    /// The rule's policy name.
    pub const fn name(self) -> RuleName {
        match self {
            RuleId::Acc001 => RuleName::AgentChangeWithoutIndependentHuman,
            RuleId::Acc002 => RuleName::ApproverIsAuthor,
            RuleId::Acc003 => RuleName::MergedWithoutIndependentApproval,
            RuleId::Acc004 => RuleName::MergedWithoutRequiredChecks,
            RuleId::Acc005 => RuleName::MergedWithProtectionBypass,
            RuleId::Acc006 => RuleName::UnknownAgentOperator,
            RuleId::Acc007 => RuleName::AgentApprovalRecorded,
            RuleId::Acc008 => RuleName::SameVendorWriteAndReview,
            RuleId::Acc009 => RuleName::AgentApprovalNotIndependent,
            RuleId::Acc010 => RuleName::AgentApprovalUnsigned,
            RuleId::Acc011 => RuleName::MissingRequiredReviewerTeam,
            RuleId::Acc012 => RuleName::UnaccountedBranchUpdate,
            RuleId::Acc013 => RuleName::DeployedWithoutPassingChange,
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
            RuleName::MergedWithoutRequiredChecks => "merged_without_required_checks",
            RuleName::MergedWithProtectionBypass => "merged_with_protection_bypass",
            RuleName::UnknownAgentOperator => "unknown_agent_operator",
            RuleName::AgentApprovalRecorded => "agent_approval_recorded",
            RuleName::SameVendorWriteAndReview => "same_vendor_write_and_review",
            RuleName::AgentApprovalNotIndependent => "agent_approval_not_independent",
            RuleName::AgentApprovalUnsigned => "agent_approval_unsigned",
            RuleName::MissingRequiredReviewerTeam => "missing_required_reviewer_team",
            RuleName::UnaccountedBranchUpdate => "unaccounted_branch_update",
            RuleName::DeployedWithoutPassingChange => "deployed_without_passing_change",
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
    /// Check and status results and the required checks, for ACC004 (ACP 0.4). Absent when the
    /// collector was not asked for them, and in every document written before 0.4; serialized
    /// only when present, so earlier documents keep their bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<Checks>,
    /// The change's risk tier, from a change-risk assessment bound to its head commit (ACP
    /// 0.4). Absent when none was handed over; serialized only when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<Risk>,
    /// Ruleset evaluations of the merge, for ACC005 (ACP 0.4). Absent when the collector was not
    /// asked for them; serialized only when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protection: Option<Protection>,
}

/// The outcome of a ruleset evaluation of a push, as the forge records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuiteResult {
    /// The rules failed and an actor with bypass permission pushed anyway.
    Bypass,
    Fail,
    Pass,
}

/// One recorded ruleset evaluation of the push that landed the merge commit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuleSuite {
    /// The forge's identifier for the evaluation.
    pub id: String,
    /// The commit the push moved the branch to; always the change's merge commit.
    pub after_sha: String,
    pub result: SuiteResult,
    /// The pushing account's login, as the forge reports it. Recorded, not resolved.
    pub actor: Option<String>,
    pub pushed_at: Timestamp,
    pub provenance: Vec<Evidence>,
}

/// What the collector recorded about ruleset evaluations of a merged change's merge commit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Protection {
    /// The branch the change merged into.
    pub branch: String,
    /// The evaluations whose push landed the merge commit. Empty when none is recorded: no
    /// ruleset applied, or the actor was exempt, which the forge does not record.
    pub suites: Vec<RuleSuite>,
    /// False when the evaluations could not be read, or the merge is older than the forge lets
    /// a listing reach.
    pub complete: bool,
    pub provenance: Vec<Evidence>,
}

/// What kind of result a check is: a GitHub check run or commit status, or a GitLab pipeline
/// or one of its jobs. A required check is matched by name against all of them, as the forge
/// does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckKind {
    CheckRun,
    Job,
    Pipeline,
    Status,
}

/// How a check run or commit status ended. Only `success`, `neutral` and `skipped` satisfy a
/// required check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conclusion {
    ActionRequired,
    Cancelled,
    Error,
    Failure,
    Neutral,
    Skipped,
    Stale,
    Success,
    TimedOut,
}

impl Conclusion {
    /// Whether this conclusion satisfies a required check.
    pub const fn passing(self) -> bool {
        matches!(
            self,
            Conclusion::Success | Conclusion::Neutral | Conclusion::Skipped
        )
    }
}

/// Where the forge's required checks for the target branch were read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredSource {
    /// Classic branch protection.
    BranchProtection,
    /// GitLab's project merge settings: "Pipelines must succeed" requires the `pipeline` check.
    ProjectSettings,
    /// Rulesets, repository and organization, in active enforcement.
    Rules,
}

/// One completed check run or commit status on the head or the merge commit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CheckRun {
    /// The check run's name or the status's context.
    pub name: String,
    pub kind: CheckKind,
    pub commit_sha: String,
    pub conclusion: Conclusion,
    pub completed_at: Timestamp,
    pub provenance: Vec<Evidence>,
}

/// What the collector recorded about checks for a merged change.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checks {
    /// The branch the change merged into.
    pub branch: String,
    /// Completed runs and statuses on the head and the merge commit. Runs completed after the
    /// merge are recorded too; the evaluator ignores them.
    pub runs: Vec<CheckRun>,
    /// False when a listing was truncated or could not be read.
    pub runs_complete: bool,
    /// The check names the forge's rules for `branch` require, as read at collection time. The
    /// forge exposes no record of the rules in force at merge time.
    pub required: Vec<String>,
    /// The sources `required` was read from.
    pub required_sources: Vec<RequiredSource>,
    /// False when a source of required checks could not be read.
    pub required_complete: bool,
    pub provenance: Vec<Evidence>,
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
    /// Reviewer teams the policy requires, with their members as observed at collection (ACP
    /// 0.4). Serialized only when present.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub teams: Vec<Team>,
    /// Updates to protected branches in the window, for ACC012 (ACP 0.4). Serialized only when
    /// present.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branches: Vec<Branch>,
    /// Deployments to production environments, for ACC013 (ACP 0.4). Absent when not collected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployments: Option<Deployments>,
    pub changes: Vec<Change>,
}

/// What moved a protected branch, as the forge's activity record names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BranchEventKind {
    Push,
    ForcePush,
    BranchCreation,
    BranchDeletion,
    PrMerge,
    MergeQueueMerge,
}

/// One update of a protected branch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BranchEvent {
    /// The forge's identifier for the update.
    pub id: String,
    pub kind: BranchEventKind,
    /// The commit the branch pointed at before and after the update.
    pub before: String,
    pub after: String,
    pub at: Timestamp,
    /// Who updated the branch, when the forge names an account.
    pub actor_id: Option<String>,
    pub provenance: Vec<Evidence>,
}

/// A protected branch and every update of it in the window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    /// `FORGE:REPOSITORY:branch:NAME`, the subject ACC012 assesses.
    pub id: String,
    pub name: String,
    pub events: Vec<BranchEvent>,
    /// False when the forge's record of the window could not be read in full.
    pub complete: bool,
    pub provenance: Vec<Evidence>,
}

/// The production deployments in a window, and the environments that count as production.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Deployments {
    /// The environments read; empty when the forge's own production flag decided.
    pub environments: Vec<String>,
    pub from: Timestamp,
    pub to: Timestamp,
    pub deployments: Vec<Deployment>,
    /// False when the listing could not be read in full.
    pub complete: bool,
    pub provenance: Vec<Evidence>,
}

/// One successful deployment to a production environment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Deployment {
    /// `FORGE:REPOSITORY:deployment:ID`, the subject ACC013 assesses.
    pub id: String,
    pub environment: String,
    /// The commit deployed.
    pub sha: String,
    /// When the deployment succeeded.
    pub at: Timestamp,
    pub creator: Option<String>,
    /// The commit of the environment's previous successful deployment, when one was found.
    pub previous_sha: Option<String>,
    /// The first-parent commits after `previous_sha` up to and including `sha`, newest first;
    /// only `sha` when no previous deployment bounds the range.
    pub commits: Vec<DeployedCommit>,
    /// False when the range could not be read in full or has no previous deployment to bound it.
    pub commits_complete: bool,
    pub provenance: Vec<Evidence>,
}

/// A commit a deployment shipped, with its committer date.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeployedCommit {
    pub sha: String,
    pub committed_at: Timestamp,
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
    /// Which checks ACC004 requires (ACP 0.4). Always present in a resolved 0.4 policy; absent
    /// from earlier documents, and serialized only when present so they keep their bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<ChecksPolicy>,
    /// What each risk tier requires (ACP 0.4). Without it no tier applies to any change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tiers: Option<BTreeMap<Tier, TierPolicy>>,
    /// The tier of a change with no usable risk assessment. Defaults to `standard` when `tiers`
    /// is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing_tier: Option<Tier>,
    /// The weakest evidence a risk assessment may rest on and still lower what a change must
    /// meet. Defaults to `signed` when `tiers` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_risk_evidence: Option<EvidenceKind>,
    /// The protected branches ACC012 watches (ACP 0.4). Serialized only when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branches: Option<BranchesPolicy>,
    /// The production environments ACC013 watches (ACP 0.4). Serialized only when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployments: Option<DeploymentsPolicy>,
    pub rules: BTreeMap<RuleName, RulePolicy>,
}

/// A change's risk tier (ACP 0.4), as a change-risk assessment states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Low,
    Standard,
    High,
    Emergency,
}

impl Tier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Tier::Low => "low",
            Tier::Standard => "standard",
            Tier::High => "high",
            Tier::Emergency => "emergency",
        }
    }
}

/// What one tier requires.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TierPolicy {
    /// Qualifying independent approvals ACC003 requires; at least one, whatever this says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_human_approvals: Option<u32>,
    /// Whether an agent approval may satisfy independence in this tier, overriding
    /// `agent_review.satisfies_independence`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_review: Option<TierAgentReview>,
    /// `ORG/SLUG` teams each of which must approve (ACC011).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_reviewer_teams: Vec<String>,
    /// Days after the merge within which an emergency change must be reviewed retroactively.
    /// Emergency tier only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retroactive_review_within_days: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierAgentReview {
    Allowed,
    Forbidden,
}

/// The risk tier a change-risk assessment gave a change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Risk {
    pub tier: Tier,
    /// Who declared the emergency, for an emergency tier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emergency: Option<RiskEmergency>,
    pub provenance: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RiskEmergency {
    pub declared_by: String,
    pub declared_at: Timestamp,
}

/// A reviewer team and its members as observed at collection, not at review time.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Team {
    /// `ORG/SLUG`.
    pub id: String,
    /// Actor identifiers (`github:login`), sorted.
    pub members: Vec<String>,
    pub complete: bool,
    pub observed_at: Timestamp,
    pub provenance: Vec<Evidence>,
}

/// The tier applied to one change, and why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppliedTier {
    pub change_id: String,
    pub tier: Tier,
    pub source: TierSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TierSource {
    /// The change's risk assessment.
    Assessment,
    /// No risk assessment: the policy's `missing_tier`.
    Missing,
    /// An assessment that would lower the requirements, on evidence below
    /// `minimum_risk_evidence`: the policy's `missing_tier`.
    BelowMinimum,
}

/// A dated obligation a finding carries: for an emergency change, the retroactive review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Obligation {
    pub kind: ObligationKind,
    pub due: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObligationKind {
    RetroactiveReview,
}

/// The protected branches ACC012 watches (ACP 0.4).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BranchesPolicy {
    /// Branch names; empty means the repository's default branch.
    #[serde(default)]
    pub protected: Vec<String>,
}

/// The environments ACC013 treats as production (ACP 0.4).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DeploymentsPolicy {
    /// Environment names; empty means those the forge marks as production.
    #[serde(default)]
    pub environments: Vec<String>,
}

/// The checks ACC004 requires on the approved head of a merged change.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChecksPolicy {
    /// Check names always required, whatever the forge's rules say.
    #[serde(default)]
    pub required: Vec<String>,
    /// Also require the checks the forge's rules required, as read at collection time.
    #[serde(default = "crate::policy::default_from_forge")]
    pub from_forge: bool,
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
    /// The identifier the same finding had under ACP 0.2, whose preimage ended in a newline.
    /// Carried for one minor version so dispositions keyed by the old identifier still apply.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legacy_ids: Vec<String>,
    /// The retroactive review an emergency change owes (ACP 0.4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub obligation: Option<Obligation>,
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
    /// The tier applied to each change, when the policy has tiers (ACP 0.4).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied_tiers: Vec<AppliedTier>,
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
