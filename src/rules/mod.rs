//! Pure evaluation of normalized facts. No forge APIs, clock, environment or filesystem.
//!
//! The exact conditions are documented in `docs/policy.md`. Reason strings are part of the
//! manifest goldens; change them together with the fixtures.

use crate::model::*;
use crate::policy::RULES;
use anyhow::Result;
use std::collections::BTreeMap;

const NOT_AGENT: &str = "Effective author is not an agent.";
const OPERATOR_UNKNOWN: &str = "Agent authorship is recorded but its human operator is unknown.";
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
const NOT_INDEPENDENT: &str =
    "No qualifying independent human approval of the current head exists before merge.";

/// The facts about one change that every rule reads.
struct Facts<'a> {
    kind: ActorKind,
    /// The known effective human: the author when human, the operator when the author is an agent.
    effective: Option<&'a str>,
    /// A different human's latest decision, at or before merge, approves the current head.
    independent: bool,
    /// The effective human approved the change at some point.
    self_approved: bool,
}

impl<'a> Facts<'a> {
    fn new(e: &'a Events, c: &'a Change) -> Self {
        let kind = e.actors[&c.author.actor_id].kind;
        let effective = match kind {
            ActorKind::Human => Some(c.author.actor_id.as_str()),
            ActorKind::Agent => c
                .agent_operator
                .as_ref()
                .and_then(|o| o.actor_id.as_deref()),
            _ => None,
        };
        // Latest non-comment decision per reviewer, ignoring anything after the merge.
        let mut latest: BTreeMap<&str, &Review> = BTreeMap::new();
        for r in &c.reviews {
            if r.state != ReviewState::Commented && c.merged_at.is_none_or(|m| r.at <= m) {
                latest.insert(&r.actor_id, r);
            }
        }
        let independent = effective.is_some_and(|author| {
            latest.values().any(|r| {
                r.state == ReviewState::Approved
                    && r.commit_sha == c.head_sha
                    && r.actor_id != author
                    && e.actors[&r.actor_id].kind == ActorKind::Human
            })
        });
        let self_approved = c
            .reviews
            .iter()
            .any(|r| r.state == ReviewState::Approved && Some(r.actor_id.as_str()) == effective);
        Self {
            kind,
            effective,
            independent,
            self_approved,
        }
    }

    fn assess(&self, rule: RuleId, c: &Change) -> (Status, &'static str) {
        use Status::*;
        let agent = self.kind == ActorKind::Agent;
        match rule {
            RuleId::Acc006 if !agent => (NotApplicable, NOT_AGENT),
            RuleId::Acc006 if self.effective.is_none() => (Fail, OPERATOR_UNKNOWN),
            RuleId::Acc006 => (Pass, OPERATOR_KNOWN),
            RuleId::Acc001 if !agent => (NotApplicable, NOT_AGENT),
            RuleId::Acc003 if c.merged_at.is_none() => (NotApplicable, NOT_MERGED),
            _ if self.effective.is_none() => (Unknown, HUMAN_UNKNOWN),
            RuleId::Acc002 if self.self_approved => (Fail, SELF_APPROVED),
            RuleId::Acc002 if !c.reviews_complete => (Unknown, REVIEWS_INCOMPLETE),
            RuleId::Acc002 => (Pass, NO_SELF_APPROVAL),
            _ if !c.reviews_complete => (Unknown, REVIEWS_INCOMPLETE_FOR_INDEPENDENCE),
            _ if self.independent => (Pass, INDEPENDENT),
            _ => (Fail, NOT_INDEPENDENT),
        }
    }
}

/// Evaluate every enabled rule against every change.
pub(crate) fn evaluate(e: &Events, p: &Policy) -> Result<(Vec<Finding>, Vec<Assessment>)> {
    let mut findings = Vec::new();
    let mut assessments = Vec::new();
    for c in &e.changes {
        let facts = Facts::new(e, c);
        for rule in RULES {
            let Some(policy) = p.rules.get(&rule.name()) else {
                continue;
            };
            if !policy.enabled {
                continue;
            }
            let (status, reason) = facts.assess(rule, c);
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
