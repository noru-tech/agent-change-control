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
    fn new(e: &'a Events, c: &'a Change, p: &Policy) -> Self {
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
        Self {
            kind,
            effective,
            human_independent,
            self_approved,
            weak_operator,
            agent_mode: p.agent_review.satisfies_independence && in_scope,
            out_of_scope: p.agent_review.satisfies_independence && !in_scope,
            human_approvals,
            agent_approvals,
            agent_reviewed_only,
            author_vendor,
            approving_vendors,
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

    fn assess(&self, rule: RuleId, c: &Change, p: &Policy) -> (Status, &'static str) {
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

/// Evaluate every enabled rule against every change.
pub(crate) fn evaluate(e: &Events, p: &Policy) -> Result<(Vec<Finding>, Vec<Assessment>)> {
    let mut findings = Vec::new();
    let mut assessments = Vec::new();
    for c in &e.changes {
        let facts = Facts::new(e, c, p);
        for rule in RULES {
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
                reason: reason.into(),
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
            provenance.sort();
            provenance.dedup();
            // Structured tuple, not concatenation: IDs never change with severity or dispositions.
            let hash = crate::normalize::digest(&(&e.repository, &c.id, rule.code(), &actors))?;
            findings.push(Finding {
                id: format!("acc-{}", &hash[7..23]),
                rule_id: rule,
                rule: rule.name(),
                severity: policy.severity,
                change_id: c.id.clone(),
                actor_ids: actors,
                explanation: reason.into(),
                provenance,
                disposition: Disposition::default(),
            });
        }
    }
    findings
        .sort_by(|a, b| (&a.change_id, a.rule_id, &a.id).cmp(&(&b.change_id, b.rule_id, &b.id)));
    Ok((findings, assessments))
}
