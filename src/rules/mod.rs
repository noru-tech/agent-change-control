//! Pure evaluation of normalized facts. No forge APIs, clock, environment or filesystem.
//!
//! The exact conditions are documented in `docs/policy.md`. Reason strings are part of the
//! manifest goldens; change them together with the fixtures.
//!
//! Agent reviewers: an agent's approval never counts as a human's. Under the opt-in
//! `agent_review` policy it can still satisfy independence when every required dimension
//! (operator, provider, identity, instructions) is `independent` and its evidence reaches the
//! policy minimum. `unknown` on a required dimension never yields a clean result.

use crate::model::*;
use crate::policy::RULES;
use anyhow::Result;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

const NOT_AGENT: &str = "Effective author is not an agent.";
const OPERATOR_UNKNOWN: &str = "Agent authorship is recorded but its human operator is unknown.";
const OPERATOR_BELOW_MINIMUM: &str =
    "A human agent operator is recorded, but its evidence is below the policy minimum.";
const HUMAN_BELOW_MINIMUM: &str = "Effective human author's evidence is below the policy minimum; independence cannot be established.";
const OPERATOR_KNOWN: &str = "A human agent operator is recorded.";
const NOT_MERGED: &str = "Change has not been merged.";
const HUMAN_UNKNOWN: &str =
    "Effective human author is unknown; independence cannot be established.";
const SELF_APPROVED: &str = "The effective human author or agent operator approved this change.";
const REVIEWS_INCOMPLETE: &str = "Review collection is incomplete.";
const NO_SELF_APPROVAL: &str = "No self-approval was observed in complete review data.";
const REVIEWS_INCOMPLETE_FOR_INDEPENDENCE: &str =
    "Review collection is incomplete; missing independent approval cannot be established.";
const INDEPENDENT: &str =
    "A different human approved the current head before merge, with no later withdrawal recorded.";
const INDEPENDENT_AGENT: &str = "An agent independent of the effective author on every required dimension approved the current head before merge, as permitted by policy.";
const AGENT_INDEPENDENCE_UNKNOWN: &str = "An agent approved the current head, but its independence from the effective author is unknown on a required dimension.";
const NOT_INDEPENDENT: &str =
    "No qualifying independent human approval of the current head exists before merge.";
// ACC007
const AGENT_APPROVED: &str = "An agent approved the current head.";
const AGENT_REVIEWED_NO_APPROVAL: &str = "An agent reviewed the current head without approving it.";
const NO_AGENT_REVIEW: &str = "No agent reviewed the current head.";
// ACC008
const HUMAN_APPROVED: &str = "A human approved the current head.";
const NO_APPROVAL: &str = "No approval of the current head exists.";
const VENDOR_UNKNOWN: &str = "The author agent's or an approving agent's vendor is unknown; same-vendor review cannot be established.";
const SAME_VENDOR: &str =
    "Every approval of the current head is by an agent of the author agent's vendor.";
const OTHER_VENDOR: &str = "An agent of another vendor approved the current head.";
// ACC009 and ACC010
const MODE_OFF: &str = "Agent approvals do not satisfy independence under this policy.";
const OUT_OF_SCOPE: &str = "Change carries none of the labels the policy scopes agent review to.";
const HUMAN_INDEPENDENT: &str =
    "A human independent of the effective author approved the current head.";
const NO_AGENT_CANDIDATE: &str =
    "No agent approval of the current head carries the evidence the policy requires.";
const AGENT_INDEPENDENT: &str =
    "An agent approval of the current head is independent on every required dimension.";
const AGENT_DIMENSION_UNKNOWN: &str = "An agent approval of the current head is unknown on a required dimension and dependent on none.";
const AGENT_DEPENDENT: &str =
    "Every agent approval of the current head is dependent on a required dimension.";
const NO_AGENT_APPROVAL: &str = "No agent approved the current head.";
const AGENT_UNSIGNED: &str =
    "An agent approval of the current head lacks the signed evidence the policy requires.";
const AGENT_SIGNED: &str = "Every agent approval of the current head carries signed evidence.";
// ACC004
const CHECKS_NOT_COLLECTED: &str = "Check data was not collected for this change.";
const CHECKS_INCOMPLETE: &str = "Check collection is incomplete.";
const REQUIRED_INCOMPLETE: &str =
    "The forge's required checks could not all be read; required checks cannot be established.";
const NOTHING_REQUIRED: &str = "No checks are required for this change.";
const CHECKS_PASSED: &str = "Every required check passed on the head or merge commit before merge.";

// ACC005
const BYPASS_NOT_COLLECTED: &str = "Ruleset evaluations were not collected for this change.";
const BYPASS_INCOMPLETE: &str = "Ruleset evaluations for this change could not be read, or are older than the forge's listing reaches.";
const BYPASS_RECORDED: &str = "A ruleset evaluation of the merge records a bypass: the rules failed and the push landed anyway.";
const BYPASS_FAILED: &str = "A ruleset evaluation of the merge failed but the push landed; whether active protection was bypassed cannot be established.";
const BYPASS_NONE_RECORDED: &str = "No ruleset evaluation of the merge is recorded; a push by an exempt actor or an override of classic branch protection leaves none, so a bypass cannot be ruled out.";
const BYPASS_PASSED: &str = "Every ruleset evaluation of the merge passed.";

// ACC011
const NO_TEAM_REQUIRED: &str = "No reviewer team is required for this change's tier.";
const TEAMS_REVIEWS_INCOMPLETE: &str =
    "Review collection is incomplete; required team approvals cannot be established.";
const TEAMS_APPROVED: &str =
    "A member of every required reviewer team approved the current head before merge.";

/// Whether an assessment's tier would lower what a change must meet below `missing`: an
/// emergency, or a lower level.
fn lowers(tier: Tier, missing: Tier) -> bool {
    tier == Tier::Emergency || tier < missing
}

/// The tier applied to `c` under `p`, and why; None when the policy has no tiers. An assessment
/// that would lower the requirements counts only on evidence at `minimum_risk_evidence`.
pub fn applied_tier(c: &Change, p: &Policy) -> Option<(Tier, TierSource)> {
    p.tiers.as_ref()?;
    let missing = p
        .missing_tier
        .unwrap_or_else(crate::policy::default_missing_tier);
    let minimum = p
        .minimum_risk_evidence
        .unwrap_or_else(crate::policy::default_risk_minimum);
    Some(match &c.risk {
        None => (missing, TierSource::Missing),
        Some(r) if lowers(r.tier, missing) && !reaches(&r.provenance, minimum) => {
            (missing, TierSource::BelowMinimum)
        }
        Some(r) => (r.tier, TierSource::Assessment),
    })
}

/// The applied tier of every change, when the policy has tiers.
pub fn applied_tiers(e: &Events, p: &Policy) -> Vec<AppliedTier> {
    e.changes
        .iter()
        .filter_map(|c| {
            applied_tier(c, p).map(|(tier, source)| AppliedTier {
                change_id: c.id.clone(),
                tier,
                source,
            })
        })
        .collect()
}

/// The effective human of `c` under `p` (spec §2), when known.
pub fn effective_human(e: &Events, c: &Change, p: &Policy) -> Option<String> {
    Facts::new(e, c, p).effective.map(String::from)
}

/// What the policy requires of `c`'s applied tier.
pub fn tier_policy<'p>(c: &Change, p: &'p Policy) -> Option<&'p TierPolicy> {
    let (tier, _) = applied_tier(c, p)?;
    p.tiers.as_ref()?.get(&tier)
}

/// The retroactive review an emergency change owes: due its window's days after the merge date.
pub fn obligation(c: &Change, p: &Policy) -> Option<Obligation> {
    let (tier, _) = applied_tier(c, p)?;
    if tier != Tier::Emergency {
        return None;
    }
    let days = p
        .tiers
        .as_ref()?
        .get(&Tier::Emergency)?
        .retroactive_review_within_days?;
    let merged = c.merged_at?.date_naive();
    Some(Obligation {
        kind: ObligationKind::RetroactiveReview,
        due: merged + chrono::Days::new(u64::from(days)),
    })
}

/// ACC005 for one change. Exact conditions are in `docs/policy.md`.
fn protection_bypass(c: &Change) -> (Status, &'static str) {
    use Status::*;
    if c.merged_at.is_none() {
        return (NotApplicable, NOT_MERGED);
    }
    let Some(b) = &c.protection else {
        return (Unknown, BYPASS_NOT_COLLECTED);
    };
    let has = |r: SuiteResult| b.suites.iter().any(|s| s.result == r);
    if has(SuiteResult::Bypass) {
        (Fail, BYPASS_RECORDED)
    } else if !b.complete {
        (Unknown, BYPASS_INCOMPLETE)
    } else if has(SuiteResult::Fail) {
        (Unknown, BYPASS_FAILED)
    } else if b.suites.is_empty() {
        (Unknown, BYPASS_NONE_RECORDED)
    } else {
        (Pass, BYPASS_PASSED)
    }
}

/// ACC004 for one change. Exact conditions are in `docs/policy.md`.
fn required_checks(c: &Change, p: &Policy) -> (Status, Cow<'static, str>) {
    use Status::*;
    let Some(merged) = c.merged_at else {
        return (NotApplicable, NOT_MERGED.into());
    };
    let Some(k) = &c.checks else {
        return (Unknown, CHECKS_NOT_COLLECTED.into());
    };
    if !k.runs_complete {
        return (Unknown, CHECKS_INCOMPLETE.into());
    }
    let policy = p.checks.clone().unwrap_or_default();
    let mut required: BTreeSet<&str> = policy.required.iter().map(String::as_str).collect();
    if policy.from_forge {
        required.extend(k.required.iter().map(String::as_str));
    }
    let commits: Vec<&str> = std::iter::once(c.head_sha.as_str())
        .chain(c.merge_commit_sha.as_deref())
        .collect();
    let mut failing = Vec::new();
    for name in &required {
        // On each commit, the latest result completed by the merge. A tie at the latest instant
        // passes only if every result at that instant passes.
        let mut seen = false;
        let passed = commits.iter().any(|sha| {
            let runs: Vec<&CheckRun> = k
                .runs
                .iter()
                .filter(|r| r.name == *name && r.commit_sha == *sha && r.completed_at <= merged)
                .collect();
            let Some(latest) = runs.iter().map(|r| r.completed_at).max() else {
                return false;
            };
            seen = true;
            runs.iter()
                .filter(|r| r.completed_at == latest)
                .all(|r| r.conclusion.passing())
        });
        if !passed {
            failing.push(format!(
                "{name} ({})",
                if seen { "did not pass" } else { "no run" }
            ));
        }
    }
    if !failing.is_empty() {
        return (
            Fail,
            format!(
                "Required checks did not pass on the head or merge commit before merge: {}.",
                failing.join(", ")
            )
            .into(),
        );
    }
    if policy.from_forge && !k.required_complete {
        return (Unknown, REQUIRED_INCOMPLETE.into());
    }
    if required.is_empty() {
        return (NotApplicable, NOTHING_REQUIRED.into());
    }
    (Pass, CHECKS_PASSED.into())
}

/// One dimension's verdict for one agent approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Independent,
    Dependent,
    Unknown,
}

/// An agent's latest decision that approves the current head, with its independence verdicts.
struct AgentApproval<'a> {
    reviewer: &'a str,
    /// The approval's evidence reaches the policy's `agent_review.minimum_evidence`.
    evidence_ok: bool,
    verdicts: BTreeMap<Dimension, Verdict>,
}

impl AgentApproval<'_> {
    fn on(&self, required: &[Dimension], verdict: Verdict) -> bool {
        required.iter().any(|d| self.verdicts[d] == verdict)
    }

    fn independent_on_all(&self, required: &[Dimension]) -> bool {
        required
            .iter()
            .all(|d| self.verdicts[d] == Verdict::Independent)
    }
}

/// The facts about one change that every rule reads.
struct Facts<'a> {
    kind: ActorKind,
    /// The known effective human: the author when human, the operator when the author is an agent.
    effective: Option<&'a str>,
    /// A different human's latest decision, at or before merge, approves the current head.
    human_independent: bool,
    /// The effective human approved the change at some point.
    self_approved: bool,
    /// An operator is recorded but its evidence is below `minimum_authorship_evidence`, so it
    /// does not name the effective human.
    weak_operator: bool,
    /// The `agent_review` mode applies to this change: enabled and, when scoped by labels, the
    /// change carries one.
    agent_mode: bool,
    /// Whether the mode is off because of the label scope rather than the switch.
    out_of_scope: bool,
    /// Latest decisions that approve the current head, by reviewer kind.
    human_approvals: usize,
    agent_approvals: Vec<AgentApproval<'a>>,
    /// An agent's latest decision on the head exists but does not approve it.
    agent_reviewed_only: bool,
    /// The author agent's vendor, when the author is an agent.
    author_vendor: Option<&'a str>,
    /// Vendors of the approving agents, `None` for an unregistered one.
    approving_vendors: Vec<Option<&'a str>>,
    /// Distinct humans other than the effective human whose latest decision approves the current
    /// head, with evidence at the review minimum.
    independent_humans: usize,
    /// Qualifying independent approvals ACC003 requires: the tier's, and never fewer than one.
    required_approvals: u32,
    /// Per required reviewer team: whether a qualifying member approved, and whether its
    /// membership is known in full.
    teams: Vec<(&'a str, bool, bool)>,
}

/// Whether `evidence` reaches `minimum` on the trust ordering.
fn reaches(evidence: &[Evidence], minimum: EvidenceKind) -> bool {
    strongest(evidence).is_some_and(|k| k.strength() >= minimum.strength())
}

/// The verified signer identities behind `evidence`.
fn identities<'a>(evidence: &'a [Evidence], e: &'a Events) -> BTreeSet<&'a str> {
    evidence
        .iter()
        .filter(|ev| ev.kind == EvidenceKind::Signed)
        .filter_map(|ev| e.attestations.get(&ev.r#ref))
        .filter_map(|a| a.signer.as_ref())
        .map(|s| s.identity.as_str())
        .collect()
}

fn compare(a: Option<&str>, b: Option<&str>) -> Verdict {
    match (a, b) {
        (Some(a), Some(b)) if a == b => Verdict::Dependent,
        (Some(_), Some(_)) => Verdict::Independent,
        _ => Verdict::Unknown,
    }
}

impl<'a> Facts<'a> {
    fn new(e: &'a Events, c: &'a Change, p: &'a Policy) -> Self {
        let kind = e.actors[&c.author.actor_id].kind;
        let operator = c.agent_operator.as_ref().filter(|o| o.actor_id.is_some());
        let weak_operator = kind == ActorKind::Agent
            && operator.is_some_and(|o| !reaches(&o.provenance, p.minimum_authorship_evidence));
        let effective = match kind {
            ActorKind::Human => Some(c.author.actor_id.as_str()),
            ActorKind::Agent if weak_operator => None,
            ActorKind::Agent => operator.and_then(|o| o.actor_id.as_deref()),
            _ => None,
        };
        let author_vendor = (kind == ActorKind::Agent)
            .then(|| e.actors[&c.author.actor_id].vendor.as_deref())
            .flatten();
        let mut author_identities = identities(&c.author.provenance, e);
        if let Some(o) = &c.agent_operator {
            author_identities.extend(identities(&o.provenance, e));
        }
        // Latest non-comment decision per reviewer, ignoring anything after the merge.
        let mut latest: BTreeMap<&str, &Review> = BTreeMap::new();
        for r in &c.reviews {
            if r.state != ReviewState::Commented && c.merged_at.is_none_or(|m| r.at <= m) {
                latest.insert(&r.actor_id, r);
            }
        }
        let approvals = || {
            latest
                .values()
                .filter(|r| r.state == ReviewState::Approved && r.commit_sha == c.head_sha)
        };
        let human_independent = effective.is_some_and(|author| {
            approvals().any(|r| {
                r.actor_id != author
                    && e.actors[&r.actor_id].kind == ActorKind::Human
                    && reaches(&r.provenance, p.minimum_review_evidence)
            })
        });
        let self_approved = c
            .reviews
            .iter()
            .any(|r| r.state == ReviewState::Approved && Some(r.actor_id.as_str()) == effective);
        let human_approvals = approvals()
            .filter(|r| e.actors[&r.actor_id].kind == ActorKind::Human)
            .count();
        let agent_approvals: Vec<AgentApproval<'a>> = approvals()
            .filter(|r| e.actors[&r.actor_id].kind == ActorKind::Agent)
            .map(|r| {
                let facts = r.agent.as_ref();
                let reviewer_vendor = e.actors[&r.actor_id].vendor.as_deref();
                let provider = if kind != ActorKind::Agent {
                    Verdict::Independent
                } else {
                    compare(author_vendor, reviewer_vendor)
                };
                let identity = match facts.and_then(|a| a.identity.as_deref()) {
                    Some(id) if !author_identities.is_empty() => {
                        if author_identities.contains(id) {
                            Verdict::Dependent
                        } else {
                            Verdict::Independent
                        }
                    }
                    _ => Verdict::Unknown,
                };
                AgentApproval {
                    reviewer: &r.actor_id,
                    evidence_ok: reaches(&r.provenance, p.agent_review.minimum_evidence),
                    verdicts: BTreeMap::from([
                        (
                            Dimension::Operator,
                            compare(facts.and_then(|a| a.operator.as_deref()), effective),
                        ),
                        (Dimension::Provider, provider),
                        (Dimension::Identity, identity),
                        (
                            Dimension::Instructions,
                            compare(
                                facts.and_then(|a| a.instructions_owner.as_deref()),
                                effective,
                            ),
                        ),
                    ]),
                }
            })
            .collect();
        let approving_vendors = approvals()
            .filter(|r| e.actors[&r.actor_id].kind == ActorKind::Agent)
            .map(|r| e.actors[&r.actor_id].vendor.as_deref())
            .collect();
        let agent_reviewed_only = agent_approvals.is_empty()
            && latest.values().any(|r| {
                r.commit_sha == c.head_sha && e.actors[&r.actor_id].kind == ActorKind::Agent
            });
        let in_scope = p.agent_review.labels.is_empty()
            || c.labels.iter().any(|l| p.agent_review.labels.contains(l));
        let tier = tier_policy(c, p);
        let switched = tier
            .and_then(|t| t.agent_review)
            .map_or(p.agent_review.satisfies_independence, |a| {
                a == TierAgentReview::Allowed
            });
        let qualifying_human = |r: &&&Review| {
            effective.is_some_and(|author| r.actor_id != author)
                && e.actors[&r.actor_id].kind == ActorKind::Human
                && reaches(&r.provenance, p.minimum_review_evidence)
        };
        let independent_humans = approvals().filter(qualifying_human).count();
        let teams = tier
            .map(|t| {
                t.required_reviewer_teams
                    .iter()
                    .map(|id| {
                        let fact = e.teams.iter().find(|f| f.id == *id);
                        let approved = fact.is_some_and(|f| {
                            approvals()
                                .filter(qualifying_human)
                                .any(|r| f.members.binary_search(&r.actor_id).is_ok())
                        });
                        (id.as_str(), approved, fact.is_some_and(|f| f.complete))
                    })
                    .collect()
            })
            .unwrap_or_default();
        Self {
            kind,
            effective,
            human_independent,
            self_approved,
            weak_operator,
            agent_mode: switched && in_scope,
            out_of_scope: switched && !in_scope,
            human_approvals,
            agent_approvals,
            agent_reviewed_only,
            author_vendor,
            approving_vendors,
            independent_humans,
            required_approvals: tier.and_then(|t| t.min_human_approvals).unwrap_or(1).max(1),
            teams,
        }
    }

    /// Agent approvals that carry the evidence the mode requires.
    fn candidates(&self) -> impl Iterator<Item = &AgentApproval<'a>> {
        self.agent_approvals.iter().filter(|a| a.evidence_ok)
    }

    /// An agent approval satisfies independence under the mode.
    fn agent_qualifying(&self, p: &Policy) -> bool {
        self.agent_mode
            && self
                .candidates()
                .any(|a| a.independent_on_all(&p.agent_review.require))
    }

    /// No approval qualifies, but an agent approval might: it is unknown on a required
    /// dimension and dependent on none.
    fn agent_unknown(&self, p: &Policy) -> bool {
        self.agent_mode
            && !self.human_independent
            && !self.agent_qualifying(p)
            && self.candidates().any(|a| {
                !a.on(&p.agent_review.require, Verdict::Dependent)
                    && a.on(&p.agent_review.require, Verdict::Unknown)
            })
    }

    /// An agent approval might make up a missing approval: it is unknown on a required dimension
    /// and dependent on none, whatever the human approvals.
    fn agent_maybe(&self, p: &Policy) -> bool {
        self.agent_mode
            && !self.agent_qualifying(p)
            && self.candidates().any(|a| {
                !a.on(&p.agent_review.require, Verdict::Dependent)
                    && a.on(&p.agent_review.require, Verdict::Unknown)
            })
    }

    /// ACC003 when the tier requires more than one approval.
    fn assess_count(&self, c: &Change, p: &Policy) -> (Status, Cow<'static, str>) {
        use Status::*;
        let (status, reason) = self.assess_independence(RuleId::Acc003, c, p);
        if status == NotApplicable
            || [
                HUMAN_BELOW_MINIMUM,
                HUMAN_UNKNOWN,
                REVIEWS_INCOMPLETE_FOR_INDEPENDENCE,
            ]
            .contains(&reason)
        {
            return (status, reason.into());
        }
        let required = self.required_approvals as usize;
        let have = self.independent_humans + usize::from(self.agent_qualifying(p));
        let line = format!(
            "The tier requires {required} qualifying independent approvals of the current head before merge; {have} exist."
        );
        if have >= required {
            (Pass, line.into())
        } else if have + 1 >= required && self.agent_maybe(p) {
            (Unknown, AGENT_INDEPENDENCE_UNKNOWN.into())
        } else {
            (Fail, line.into())
        }
    }

    /// ACC011: every reviewer team the tier requires approved the head.
    fn assess_teams(&self, c: &Change) -> (Status, Cow<'static, str>) {
        use Status::*;
        if c.merged_at.is_none() {
            return (NotApplicable, NOT_MERGED.into());
        }
        if self.teams.is_empty() {
            return (NotApplicable, NO_TEAM_REQUIRED.into());
        }
        if self.weak_operator {
            return (Unknown, HUMAN_BELOW_MINIMUM.into());
        }
        if self.effective.is_none() {
            return (Unknown, HUMAN_UNKNOWN.into());
        }
        if !c.reviews_complete {
            return (Unknown, TEAMS_REVIEWS_INCOMPLETE.into());
        }
        let missing: Vec<&str> = self
            .teams
            .iter()
            .filter(|(_, approved, _)| !approved)
            .map(|(id, _, _)| *id)
            .collect();
        if missing.is_empty() {
            return (Pass, TEAMS_APPROVED.into());
        }
        let unknown: Vec<&str> = self
            .teams
            .iter()
            .filter(|(_, approved, known)| !approved && !known)
            .map(|(id, _, _)| *id)
            .collect();
        if !unknown.is_empty() {
            return (
                Unknown,
                format!(
                    "The membership of {} could not be read in full; whether a member approved cannot be established.",
                    unknown.join(", ")
                )
                .into(),
            );
        }
        (
            Fail,
            format!(
                "No qualifying approval of the current head before merge by a member of {}.",
                missing.join(", ")
            )
            .into(),
        )
    }

    fn assess(&self, rule: RuleId, c: &Change, p: &Policy) -> (Status, Cow<'static, str>) {
        if rule == RuleId::Acc004 {
            return required_checks(c, p);
        }
        if rule == RuleId::Acc011 {
            return self.assess_teams(c);
        }
        if rule == RuleId::Acc003 && self.required_approvals > 1 {
            return self.assess_count(c, p);
        }
        if rule == RuleId::Acc005 {
            let (status, reason) = protection_bypass(c);
            return (status, reason.into());
        }
        let (status, reason) = self.assess_independence(rule, c, p);
        (status, reason.into())
    }

    fn assess_independence(&self, rule: RuleId, c: &Change, p: &Policy) -> (Status, &'static str) {
        use Status::*;
        let agent = self.kind == ActorKind::Agent;
        match rule {
            RuleId::Acc006 if !agent => (NotApplicable, NOT_AGENT),
            RuleId::Acc006 if self.weak_operator => (Fail, OPERATOR_BELOW_MINIMUM),
            RuleId::Acc006 if self.effective.is_none() => (Fail, OPERATOR_UNKNOWN),
            RuleId::Acc006 => (Pass, OPERATOR_KNOWN),
            RuleId::Acc007 if !self.agent_approvals.is_empty() => (Fail, AGENT_APPROVED),
            RuleId::Acc007 if self.agent_reviewed_only => (Pass, AGENT_REVIEWED_NO_APPROVAL),
            RuleId::Acc007 => (NotApplicable, NO_AGENT_REVIEW),
            RuleId::Acc008 if !agent => (NotApplicable, NOT_AGENT),
            RuleId::Acc008 if self.human_approvals > 0 => (NotApplicable, HUMAN_APPROVED),
            RuleId::Acc008 if self.agent_approvals.is_empty() => (NotApplicable, NO_APPROVAL),
            RuleId::Acc008
                if self.author_vendor.is_none()
                    || self.approving_vendors.iter().any(Option::is_none) =>
            {
                (Unknown, VENDOR_UNKNOWN)
            }
            RuleId::Acc008
                if self
                    .approving_vendors
                    .iter()
                    .all(|v| *v == self.author_vendor) =>
            {
                (Fail, SAME_VENDOR)
            }
            RuleId::Acc008 => (Pass, OTHER_VENDOR),
            RuleId::Acc009 | RuleId::Acc010 if self.out_of_scope => (NotApplicable, OUT_OF_SCOPE),
            RuleId::Acc009 | RuleId::Acc010 if !self.agent_mode => (NotApplicable, MODE_OFF),
            RuleId::Acc010 if self.agent_approvals.is_empty() => (NotApplicable, NO_AGENT_APPROVAL),
            RuleId::Acc010 if self.agent_approvals.iter().any(|a| !a.evidence_ok) => {
                (Fail, AGENT_UNSIGNED)
            }
            RuleId::Acc010 => (Pass, AGENT_SIGNED),
            RuleId::Acc009 if self.human_independent => (NotApplicable, HUMAN_INDEPENDENT),
            RuleId::Acc009 if self.candidates().next().is_none() => {
                (NotApplicable, NO_AGENT_CANDIDATE)
            }
            RuleId::Acc009 if self.agent_qualifying(p) => (Pass, AGENT_INDEPENDENT),
            RuleId::Acc009 if self.agent_unknown(p) => (Unknown, AGENT_DIMENSION_UNKNOWN),
            RuleId::Acc009 => (Fail, AGENT_DEPENDENT),
            RuleId::Acc001 if !agent => (NotApplicable, NOT_AGENT),
            RuleId::Acc003 if c.merged_at.is_none() => (NotApplicable, NOT_MERGED),
            _ if self.weak_operator => (Unknown, HUMAN_BELOW_MINIMUM),
            _ if self.effective.is_none() => (Unknown, HUMAN_UNKNOWN),
            RuleId::Acc002 if self.self_approved => (Fail, SELF_APPROVED),
            RuleId::Acc002 if !c.reviews_complete => (Unknown, REVIEWS_INCOMPLETE),
            RuleId::Acc002 => (Pass, NO_SELF_APPROVAL),
            _ if !c.reviews_complete => (Unknown, REVIEWS_INCOMPLETE_FOR_INDEPENDENCE),
            _ if self.human_independent => (Pass, INDEPENDENT),
            _ if self.agent_qualifying(p) => (Pass, INDEPENDENT_AGENT),
            _ if self.agent_unknown(p) => (Unknown, AGENT_INDEPENDENCE_UNKNOWN),
            _ => (Fail, NOT_INDEPENDENT),
        }
    }
}

/// `acc-` and the first 16 hexadecimal characters of a `sha256:` digest.
fn finding_id(digest: &str) -> String {
    format!("acc-{}", &digest[7..23])
}

const BRANCH_INCOMPLETE: &str = "The branch's updates in the window could not be read in full.";
const BRANCH_ACCOUNTED: &str = "Every recorded update of the branch landed a change.";
const DEPLOY_COVERED: &str =
    "Every commit the deployment shipped was landed by a change that passes the policy.";

/// What ACC012 makes of one branch update: why it is not accounted for, or `None` when it is.
fn unaccounted(ev: &BranchEvent, merges: &BTreeSet<String>) -> Option<&'static str> {
    match ev.kind {
        BranchEventKind::ForcePush => Some("force push"),
        BranchEventKind::BranchDeletion => Some("deletion"),
        BranchEventKind::Push if !merges.contains(&ev.after.to_ascii_lowercase()) => {
            Some("direct push")
        }
        _ => None,
    }
}

/// ACC012 for one protected branch. Exact conditions are in `docs/policy.md`.
fn branch_updates<'b>(
    b: &'b Branch,
    merges: &BTreeSet<String>,
) -> (
    Status,
    Cow<'static, str>,
    Vec<(&'b BranchEvent, &'static str)>,
) {
    let offending: Vec<(&BranchEvent, &'static str)> = b
        .events
        .iter()
        .filter_map(|ev| unaccounted(ev, merges).map(|why| (ev, why)))
        .collect();
    if !offending.is_empty() {
        let mut kinds: Vec<&str> = offending.iter().map(|(_, why)| *why).collect();
        kinds.sort_unstable();
        kinds.dedup();
        let reason = format!(
            "{} update(s) of {} no change accounts for: {}.",
            offending.len(),
            b.name,
            kinds.join(", ")
        );
        return (Status::Fail, reason.into(), offending);
    }
    if !b.complete {
        return (Status::Unknown, BRANCH_INCOMPLETE.into(), offending);
    }
    (Status::Pass, BRANCH_ACCOUNTED.into(), offending)
}

/// Whether a change passes the policy as evaluated so far: a failing assessment at or above
/// `fail_on` fails it; otherwise an unknown one, or facts an enabled rule needs and lacks, leave
/// it unknown. Dispositions are applied later, by `check`, and do not enter here.
fn change_standing(c: &Change, assessments: &[Assessment], p: &Policy, teams: &[Team]) -> Status {
    let mine = || assessments.iter().filter(|a| a.change_id == c.id);
    let fails = mine().any(|a| {
        a.status == Status::Fail
            && p.rules
                .get(&a.rule_id.name())
                .is_some_and(|r| r.severity >= p.fail_on)
    });
    if fails {
        Status::Fail
    } else if !c.reviews_complete
        || mine().any(|a| a.status == Status::Unknown)
        || !crate::manifest::gaps(c, p, teams).is_empty()
    {
        Status::Unknown
    } else {
        Status::Pass
    }
}

/// ACC013 for one deployment. Exact conditions are in `docs/policy.md`.
fn deployment_cover(
    d: &Deployment,
    e: &Events,
    assessments: &[Assessment],
    p: &Policy,
) -> (Status, Cow<'static, str>) {
    let mut uncovered = Vec::new();
    let mut failing = Vec::new();
    let mut unknown = Vec::new();
    let mut before_window = Vec::new();
    for commit in &d.commits {
        let short = &commit.sha[..commit.sha.len().min(12)];
        let change = e.changes.iter().find(|c| {
            c.merged_at.is_some()
                && c.merge_commit_sha
                    .as_deref()
                    .is_some_and(|m| m.eq_ignore_ascii_case(&commit.sha))
        });
        match change {
            None if commit.committed_at < e.window.from => before_window.push(short.to_string()),
            None => uncovered.push(short.to_string()),
            Some(c) => match change_standing(c, assessments, p, &e.teams) {
                Status::Fail => failing.push(format!("{short} ({})", c.id)),
                Status::Pass => {}
                _ => unknown.push(format!("{short} ({})", c.id)),
            },
        }
    }
    if !uncovered.is_empty() || !failing.is_empty() {
        let mut parts = Vec::new();
        if !uncovered.is_empty() {
            parts.push(format!("landed by no change: {}", uncovered.join(", ")));
        }
        if !failing.is_empty() {
            parts.push(format!(
                "landed by a change that fails: {}",
                failing.join(", ")
            ));
        }
        return (
            Status::Fail,
            format!(
                "The deployment to {} shipped commits not covered by a passing change; {}.",
                d.environment,
                parts.join("; ")
            )
            .into(),
        );
    }
    if !d.commits_complete {
        return (
            Status::Unknown,
            "The commits the deployment shipped could not all be read, or no earlier deployment bounds them.".into(),
        );
    }
    if !before_window.is_empty() {
        return (
            Status::Unknown,
            format!(
                "Commits merged before the collection window cannot be judged: {}; collect changes from earlier.",
                before_window.join(", ")
            )
            .into(),
        );
    }
    if !unknown.is_empty() {
        return (
            Status::Unknown,
            format!(
                "Commits landed by a change whose verdict is unknown: {}.",
                unknown.join(", ")
            )
            .into(),
        );
    }
    (Status::Pass, DEPLOY_COVERED.into())
}

/// A finding on a subject that is not a change: an update of a branch (ACC012), named
/// `BRANCH#UPDATE`, or a deployment (ACC013). The subject is the finding's `change_id`, so its
/// identifier is computed exactly as every other finding's is.
fn subject_finding(
    e: &Events,
    rule: RuleId,
    policy: &RulePolicy,
    subject: &str,
    actor: Option<&str>,
    explanation: String,
    mut provenance: Vec<Evidence>,
) -> Result<Finding> {
    let actors: Vec<String> = actor.map(String::from).into_iter().collect();
    provenance.sort();
    provenance.dedup();
    let preimage = (&e.repository, subject, rule.code(), &actors);
    let id = finding_id(&crate::canonical::digest(&preimage)?);
    let legacy = finding_id(&crate::canonical::legacy::digest(&preimage)?);
    Ok(Finding {
        id,
        legacy_ids: vec![legacy],
        rule_id: rule,
        rule: rule.name(),
        severity: policy.severity,
        change_id: subject.to_string(),
        actor_ids: actors,
        explanation,
        provenance,
        disposition: Disposition::default(),
        obligation: None,
    })
}

/// ACC012 and ACC013, whose subjects are branches and deployments rather than changes. They run
/// after every change rule, because a deployment is judged by the changes it shipped.
fn evaluate_subjects(
    e: &Events,
    p: &Policy,
    findings: &mut Vec<Finding>,
    assessments: &mut Vec<Assessment>,
) -> Result<()> {
    if let Some(policy) = p.rules.get(&RuleId::Acc012.name()).filter(|r| r.enabled) {
        let merges: BTreeSet<String> = e
            .changes
            .iter()
            .filter(|c| c.merged_at.is_some())
            .filter_map(|c| c.merge_commit_sha.as_deref())
            .map(str::to_ascii_lowercase)
            .collect();
        for b in &e.branches {
            let (status, reason, offending) = branch_updates(b, &merges);
            assessments.push(Assessment {
                change_id: b.id.clone(),
                rule_id: RuleId::Acc012,
                status,
                reason: reason.to_string(),
            });
            for (ev, why) in offending {
                let by = ev.actor_id.as_deref().map_or_else(
                    || "an account the forge does not name".to_string(),
                    String::from,
                );
                let at = ev.at.to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true);
                let explanation = match ev.kind {
                    BranchEventKind::BranchDeletion => {
                        format!("Deletion of {} by {by} at {at}.", b.name)
                    }
                    BranchEventKind::Push => format!(
                        "Direct push to {} by {by} at {at}, from {} to {}, which no change landed.",
                        b.name,
                        short(&ev.before),
                        short(&ev.after)
                    ),
                    _ => format!(
                        "{} to {} by {by} at {at}, from {} to {}.",
                        capitalized(why),
                        b.name,
                        short(&ev.before),
                        short(&ev.after)
                    ),
                };
                let mut provenance = b.provenance.clone();
                provenance.extend(ev.provenance.clone());
                findings.push(subject_finding(
                    e,
                    RuleId::Acc012,
                    policy,
                    &format!("{}#{}", b.id, ev.id),
                    ev.actor_id.as_deref(),
                    explanation,
                    provenance,
                )?);
            }
        }
    }
    if let Some(policy) = p.rules.get(&RuleId::Acc013.name()).filter(|r| r.enabled) {
        let deployments = e.deployments.iter().flat_map(|d| &d.deployments);
        let mut judged = Vec::new();
        for d in deployments {
            let (status, reason) = deployment_cover(d, e, assessments, p);
            judged.push((d, status, reason));
        }
        for (d, status, reason) in judged {
            assessments.push(Assessment {
                change_id: d.id.clone(),
                rule_id: RuleId::Acc013,
                status,
                reason: reason.to_string(),
            });
            if status == Status::Fail {
                findings.push(subject_finding(
                    e,
                    RuleId::Acc013,
                    policy,
                    &d.id,
                    d.creator.as_deref(),
                    reason.to_string(),
                    d.provenance.clone(),
                )?);
            }
        }
    }
    Ok(())
}

fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

/// Evaluate every enabled rule against every change, then ACC012 and ACC013 against their
/// subjects.
pub(crate) fn evaluate(e: &Events, p: &Policy) -> Result<(Vec<Finding>, Vec<Assessment>)> {
    let mut findings = Vec::new();
    let mut assessments = Vec::new();
    for c in &e.changes {
        let facts = Facts::new(e, c, p);
        for rule in RULES {
            if matches!(rule, RuleId::Acc012 | RuleId::Acc013) {
                continue;
            }
            let Some(policy) = p.rules.get(&rule.name()) else {
                continue;
            };
            if !policy.enabled {
                continue;
            }
            let (status, reason) = facts.assess(rule, c, p);
            assessments.push(Assessment {
                change_id: c.id.clone(),
                rule_id: rule,
                status,
                reason: reason.to_string(),
            });
            if status != Status::Fail {
                continue;
            }
            let mut actors = vec![c.author.actor_id.clone()];
            if let Some(id) = facts.effective {
                actors.push(id.into());
            }
            if matches!(
                rule,
                RuleId::Acc007 | RuleId::Acc008 | RuleId::Acc009 | RuleId::Acc010
            ) {
                actors.extend(facts.agent_approvals.iter().map(|a| a.reviewer.to_string()));
            }
            if matches!(rule, RuleId::Acc004 | RuleId::Acc005) {
                actors.extend(c.merger.as_ref().map(|m| m.actor_id.clone()));
            }
            actors.sort();
            actors.dedup();
            let mut provenance = c.provenance.clone();
            provenance.extend(c.author.provenance.clone());
            if let Some(op) = &c.agent_operator {
                provenance.extend(op.provenance.clone());
            }
            if let Some(m) = &c.merger {
                provenance.extend(m.provenance.clone());
            }
            for r in &c.reviews {
                provenance.extend(r.provenance.clone());
            }
            if rule == RuleId::Acc004 {
                provenance.extend(c.checks.iter().flat_map(|k| k.provenance.clone()));
            }
            if rule == RuleId::Acc011 {
                provenance.extend(
                    e.teams
                        .iter()
                        .filter(|t| facts.teams.iter().any(|(id, _, _)| *id == t.id))
                        .flat_map(|t| t.provenance.clone()),
                );
            }
            if rule == RuleId::Acc005 {
                provenance.extend(c.protection.iter().flat_map(|b| {
                    b.suites
                        .iter()
                        .filter(|s| s.result == SuiteResult::Bypass)
                        .flat_map(|s| s.provenance.clone())
                }));
            }
            provenance.sort();
            provenance.dedup();
            // Structured tuple, not concatenation: IDs never change with severity or dispositions.
            let preimage = (&e.repository, &c.id, rule.code(), &actors);
            let id = finding_id(&crate::canonical::digest(&preimage)?);
            let legacy = finding_id(&crate::canonical::legacy::digest(&preimage)?);
            findings.push(Finding {
                id,
                legacy_ids: vec![legacy],
                rule_id: rule,
                rule: rule.name(),
                severity: policy.severity,
                change_id: c.id.clone(),
                actor_ids: actors,
                explanation: reason.to_string(),
                provenance,
                disposition: Disposition::default(),
                obligation: (rule == RuleId::Acc003).then(|| obligation(c, p)).flatten(),
            });
        }
    }
    evaluate_subjects(e, p, &mut findings, &mut assessments)?;
    findings
        .sort_by(|a, b| (&a.change_id, a.rule_id, &a.id).cmp(&(&b.change_id, b.rule_id, &b.id)));
    Ok((findings, assessments))
}
