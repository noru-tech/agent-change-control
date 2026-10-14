//! Manifest generation, tamper-detecting validation and disposition-aware policy checks.

use crate::canonical::Canonicalization;
use crate::model::*;
use crate::{Exit, failure_with_hint};
use anyhow::{Result, anyhow, bail, ensure};
use chrono::NaiveDate;

/// The manifest version `acc` writes: ACP 0.4, digests over RFC 8785 bytes.
pub const VERSION: &str = "0.4";
/// The last manifest version with the legacy canonicalization, read for validation only.
pub const LEGACY_VERSION: &str = "0.2";

/// An ACP version `acc` reads. Each is evaluated exactly as it was specified, so a manifest
/// written under it re-validates byte for byte: its rule catalogue, whether findings carry
/// `legacy_ids`, whether the policy has a `checks` block, and how digests are serialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Revision {
    /// ACP 0.2: the legacy canonicalization.
    V02,
    /// ACP 0.3: RFC 8785, findings carry the 0.2 identifier as `legacy_ids`.
    V03,
    /// ACP 0.4: ACC004 and the `checks` policy; `legacy_ids` removed.
    V04,
}

impl Revision {
    /// The revision `acc` writes.
    pub const CURRENT: Revision = Revision::V04;

    pub fn parse(version: &str) -> Result<Self> {
        match version {
            "0.2" => Ok(Self::V02),
            "0.3" => Ok(Self::V03),
            "0.4" => Ok(Self::V04),
            _ => bail!("unsupported manifest version"),
        }
    }

    pub const fn version(self) -> &'static str {
        match self {
            Self::V02 => "0.2",
            Self::V03 => "0.3",
            Self::V04 => "0.4",
        }
    }

    pub const fn canonicalization(self) -> Canonicalization {
        match self {
            Self::V02 => Canonicalization::Legacy,
            Self::V03 | Self::V04 => Canonicalization::Jcs,
        }
    }

    /// The rules this revision specifies, in identifier order.
    pub fn rules(self) -> &'static [RuleId] {
        match self {
            Self::V02 | Self::V03 => &crate::policy::RULES_0_3,
            Self::V04 => &crate::policy::RULES,
        }
    }

    /// Whether resolved policies carry the `checks` block.
    pub const fn has_checks(self) -> bool {
        matches!(self, Self::V04)
    }
}

/// Normalize `e`, resolve `p` and evaluate every rule into a manifest.
pub fn evaluate(e: Events, p: Policy) -> Result<Manifest> {
    evaluate_as(e, p, Revision::CURRENT, false)
}

/// [`evaluate`] as the given revision specified it. Rule semantics shared between revisions are
/// identical; what differs is the catalogue, the version marks and the bytes that are hashed.
/// `keep_legacy` keeps each finding's 0.2 identifier in `legacy_ids` whatever the revision, for
/// [`check`] to match dispositions by before it drops them.
fn evaluate_as(e: Events, p: Policy, revision: Revision, keep_legacy: bool) -> Result<Manifest> {
    let canon = revision.canonicalization();
    let e = crate::normalize::events(e)?;
    let p = crate::policy::resolve_for(p, revision)?;
    let (mut findings, assessments) = crate::rules::evaluate(&e, &p)?;
    for f in &mut findings {
        match revision {
            Revision::V02 => f.id = std::mem::take(&mut f.legacy_ids).remove(0),
            Revision::V03 => {}
            Revision::V04 if keep_legacy => {}
            Revision::V04 => f.legacy_ids.clear(),
        }
    }
    let mut summary = Summary {
        changes: e.changes.len(),
        findings: findings.len(),
        ..Default::default()
    };
    for c in &e.changes {
        let kind = e.actors[&c.author.actor_id].kind;
        summary.human_authored += usize::from(kind == ActorKind::Human);
        summary.agent_authored += usize::from(kind == ActorKind::Agent);
        summary.agent_operator_unknown += usize::from(
            kind == ActorKind::Agent
                && c.agent_operator
                    .as_ref()
                    .and_then(|o| o.actor_id.as_ref())
                    .is_none(),
        );
        if findings.iter().any(|f| f.change_id == c.id) {
            summary.with_findings += 1;
        } else if !e.window.complete
            || !c.reviews_complete
            || assessments
                .iter()
                .any(|a| a.change_id == c.id && a.status == Status::Unknown)
        {
            summary.indeterminate += 1;
        } else {
            summary.clean += 1;
        }
    }
    let generated = Generated {
        tool: "agent-change-control".into(),
        version: crate::version().into(),
        source_digest: canon.digest(&e)?,
    };
    let applied_tiers = crate::rules::applied_tiers(&e, &p);
    Ok(Manifest {
        version: revision.version().into(),
        events: e,
        policy: p,
        applied_tiers,
        summary,
        findings,
        assessments,
        generated,
    })
}

/// Check the schema, then re-evaluate the embedded facts and policy and require byte-identical
/// canonical output. Only structurally valid disposition edits, and `generated.tool` and
/// `generated.version`, are allowed to differ: those name the implementation that wrote the
/// manifest, not the result, so a manifest from any conforming evaluator of its ACP version
/// validates. Each version is re-evaluated as it was specified; the result says which applied.
pub fn validate(m: &Manifest) -> Result<Revision> {
    let revision = Revision::parse(&m.version)?;
    crate::normalize::schema(&serde_json::to_value(m)?, "manifest")?;
    let mut expected = evaluate_as(m.events.clone(), m.policy.clone(), revision, false)?;
    expected.generated.tool = m.generated.tool.clone();
    expected.generated.version = m.generated.version.clone();
    ensure!(
        expected.findings.len() == m.findings.len(),
        "manifest findings do not match recorded facts"
    );
    for (actual, computed) in m.findings.iter().zip(expected.findings.iter_mut()) {
        validate_disposition(&actual.disposition)?;
        computed.disposition = actual.disposition.clone();
    }
    ensure!(
        crate::canonical::jcs_bytes(m)? == crate::canonical::jcs_bytes(&expected)?,
        "manifest differs from deterministic evaluation (facts, digest, summary or findings)"
    );
    Ok(revision)
}

/// The canonicalization a manifest version was written with.
pub fn canonicalization(version: &str) -> Result<Canonicalization> {
    Ok(Revision::parse(version)?.canonicalization())
}

fn enabled(p: &Policy, rule: RuleName) -> bool {
    p.rules.get(&rule).is_some_and(|r| r.enabled)
}

/// Whether the policy enables ACC004, which makes missing check data a collection gap.
pub fn checks_required(p: &Policy) -> bool {
    enabled(p, RuleName::MergedWithoutRequiredChecks)
}

/// Whether the policy enables ACC005, which makes missing ruleset evaluations a collection gap.
pub fn bypass_required(p: &Policy) -> bool {
    enabled(p, RuleName::MergedWithProtectionBypass)
}

/// The opt-in rules whose facts are missing or partial for one change, under `p`. Only a merged
/// change has such facts.
pub fn gaps(c: &Change, p: &Policy, e_teams: &[Team]) -> Vec<RuleId> {
    let mut out = Vec::new();
    if c.merged_at.is_none() {
        return out;
    }
    let from_forge = p.checks.as_ref().is_none_or(|k| k.from_forge);
    if checks_required(p)
        && c.checks
            .as_ref()
            .is_none_or(|k| !k.runs_complete || (from_forge && !k.required_complete))
    {
        out.push(RuleId::Acc004);
    }
    if bypass_required(p) && c.protection.as_ref().is_none_or(|b| !b.complete) {
        out.push(RuleId::Acc005);
    }
    if teams_required(p, c, e_teams) {
        out.push(RuleId::Acc011);
    }
    out
}

/// The reviewer teams ACC011 needs facts for: every team a tier of `p` requires, when the rule is
/// enabled.
pub fn teams_required_by(p: &Policy) -> Vec<String> {
    if !enabled(p, RuleName::MissingRequiredReviewerTeam) {
        return Vec::new();
    }
    let mut ids: Vec<String> = p
        .tiers
        .iter()
        .flat_map(|t| t.values())
        .flat_map(|t| t.required_reviewer_teams.iter().cloned())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// Whether ACC011 is enabled and a team `c`'s tier requires has no complete membership record.
fn teams_required(p: &Policy, c: &Change, teams: &[Team]) -> bool {
    enabled(p, RuleName::MissingRequiredReviewerTeam)
        && crate::rules::tier_policy(c, p).is_some_and(|t| {
            t.required_reviewer_teams
                .iter()
                .any(|id| !teams.iter().any(|f| f.id == *id && f.complete))
        })
}

/// Whether collection is incomplete for the decision `p` asks for: the window or a review history
/// is incomplete, or an enabled opt-in rule's facts are missing or partial for a merged change
/// ([`gaps`]). Incomplete collection takes exit-code precedence and can never produce a clean
/// result.
pub fn incomplete(e: &Events, p: &Policy) -> bool {
    !e.window.complete
        || e.changes.iter().any(|c| !c.reviews_complete)
        || e.changes.iter().any(|c| !gaps(c, p, &e.teams).is_empty())
}

fn validate_disposition(d: &Disposition) -> Result<()> {
    if d.status != DispositionStatus::Open {
        ensure!(
            d.owner.is_some() && d.decided_at.is_some() && d.rationale.is_some(),
            "disposition requires owner, decision date and rationale"
        );
    }
    if let Some(expires) = d.expires_at {
        ensure!(
            d.decided_at.is_some_and(|decided| expires >= decided),
            "disposition expiry precedes decision"
        );
    }
    if d.status == DispositionStatus::Remediated {
        ensure!(
            d.remediated_at
                .is_some_and(|r| d.decided_at.is_some_and(|decided| r >= decided)),
            "remediation requires a valid remediation date"
        );
    }
    Ok(())
}

/// Whether a non-open disposition suppresses its finding on `date`.
fn suppressed(d: &Disposition, date: NaiveDate) -> Result<bool> {
    let decided = d
        .decided_at
        .ok_or_else(|| anyhow!("disposition lacks a decision date"))?;
    Ok(date >= decided
        && d.expires_at.is_none_or(|x| date <= x)
        && (d.status != DispositionStatus::Remediated
            || d.remediated_at.is_some_and(|x| date >= x)))
}

/// Validate `m`, re-evaluate it under `p` (or its embedded policy), carry dispositions over by
/// finding ID (the current identifier or a legacy one, so a 0.2 manifest's dispositions survive
/// the move to 0.3), and decide the process exit. Non-open dispositions require an explicit `as_of`
/// date; the machine clock is never consulted.
pub fn check(
    m: &Manifest,
    p: Option<Policy>,
    as_of: Option<NaiveDate>,
) -> Result<(Manifest, Exit)> {
    let (checked, failing) = reevaluate(m, p, as_of)?;
    let exit = if incomplete(&checked.events, &checked.policy) {
        Exit::Incomplete
    } else if failing.iter().any(|&f| f) {
        Exit::PolicyFailed
    } else {
        Exit::Ok
    };
    Ok((checked, exit))
}

/// One change's verdict under a policy: what a deploy gate needs to know about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeVerdict {
    pub change_id: String,
    /// The commit the change landed on the target branch, when it is merged and the forge
    /// reported it.
    pub merge_commit_sha: Option<String>,
    pub verdict: Verdict,
    /// The identifiers of the findings that fail the policy for this change.
    pub failing: Vec<String>,
}

/// Whether a change passes the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// Complete facts and no unsuppressed finding at or above `fail_on`.
    Pass,
    /// Its review history, or the facts an enabled opt-in rule needs, are incomplete, and no
    /// finding fails it outright.
    Incomplete,
    /// At least one unsuppressed finding at or above `fail_on`.
    Fail,
}

impl Verdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Incomplete => "incomplete",
            Verdict::Fail => "fail",
        }
    }
}

/// Every change's verdict, exactly as [`check`] decides the manifest's: the same re-evaluation,
/// dispositions and `as_of` rule, applied one change at a time. A failing finding decides a change
/// even when its facts are incomplete; otherwise incomplete facts never pass. The window's own
/// completeness is about which changes were listed, not about any one of them, so it is not part
/// of a change's verdict.
pub fn verdicts(
    m: &Manifest,
    p: Option<Policy>,
    as_of: Option<NaiveDate>,
) -> Result<Vec<ChangeVerdict>> {
    let (checked, failing) = reevaluate(m, p, as_of)?;
    Ok(checked
        .events
        .changes
        .iter()
        .map(|c| {
            let failing: Vec<String> = checked
                .findings
                .iter()
                .zip(&failing)
                .filter(|(f, fails)| **fails && f.change_id == c.id)
                .map(|(f, _)| f.id.clone())
                .collect();
            let verdict = if !failing.is_empty() {
                Verdict::Fail
            } else if !c.reviews_complete
                || !gaps(c, &checked.policy, &checked.events.teams).is_empty()
            {
                Verdict::Incomplete
            } else {
                Verdict::Pass
            };
            ChangeVerdict {
                change_id: c.id.clone(),
                merge_commit_sha: c.merged_at.and(c.merge_commit_sha.clone()),
                verdict,
                failing,
            }
        })
        .collect())
}

/// The re-evaluated manifest and, for each of its findings in order, whether it fails the policy.
fn reevaluate(
    m: &Manifest,
    p: Option<Policy>,
    as_of: Option<NaiveDate>,
) -> Result<(Manifest, Vec<bool>)> {
    validate(m)?;
    let mut checked = evaluate_as(
        m.events.clone(),
        p.unwrap_or_else(|| m.policy.clone()),
        Revision::CURRENT,
        true,
    )?;
    // A disposition recorded under a 0.2 identifier still applies: `legacy_ids` left the output
    // in 0.4, but matching by it costs nothing and loses no decision on an upgrade.
    for f in &mut checked.findings {
        if let Some(old) = m.findings.iter().find(|old| {
            old.id == f.id || f.legacy_ids.contains(&old.id) || old.legacy_ids.contains(&f.id)
        }) {
            f.disposition = old.disposition.clone();
        }
        f.legacy_ids.clear();
    }
    let mut failing = Vec::with_capacity(checked.findings.len());
    for f in &checked.findings {
        let d = &f.disposition;
        let date = || {
            as_of.ok_or_else(|| {
                failure_with_hint(
                    Exit::Usage,
                    "--as-of is required when evaluating dispositions or an emergency's retroactive review",
                    "pass --as-of YYYY-MM-DD, the date they are evaluated on; acc never reads the clock",
                )
            })
        };
        let suppressed = if f.obligation.is_some() {
            // Only the retroactive review discharges an emergency's finding; until its window
            // closes, the finding is pending rather than failing.
            matches!(
                standing(&checked, f, Some(date()?)),
                Some(Standing::Pending | Standing::Met)
            )
        } else if d.status == DispositionStatus::Open {
            false
        } else {
            suppressed(d, date()?)?
        };
        failing.push(!suppressed && f.severity >= checked.policy.fail_on);
    }
    Ok((checked, failing))
}

/// Where an emergency change's retroactive review stands on a date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Not yet reviewed, and the window is still open: the finding does not fail.
    Pending,
    /// Reviewed in time by someone independent of the effective author.
    Met,
    /// The window closed with no such review.
    Overdue,
}

impl Standing {
    pub const fn as_str(self) -> &'static str {
        match self {
            Standing::Pending => "pending",
            Standing::Met => "met",
            Standing::Overdue => "overdue",
        }
    }
}

/// The standing of `f`'s obligation on `as_of`: None when it has none, or no date was given.
///
/// The review is recorded as a `remediated` disposition. It meets the obligation only when it
/// was remediated within the window, and its owner is a human actor of the
/// export who is not the change's effective author: the independence is checked, not taken on
/// the owner's word. No other disposition discharges it.
pub fn standing(m: &Manifest, f: &Finding, as_of: Option<NaiveDate>) -> Option<Standing> {
    let o = f.obligation.as_ref()?;
    let date = as_of?;
    let d = &f.disposition;
    let change = m.events.changes.iter().find(|c| c.id == f.change_id)?;
    let effective = crate::rules::effective_human(&m.events, change, &m.policy);
    let independent = d.owner.as_deref().is_some_and(|owner| {
        m.events
            .actors
            .get(owner)
            .is_some_and(|a| a.kind == ActorKind::Human)
            && Some(owner) != effective.as_deref()
            && owner != change.author.actor_id
    });
    let met = d.status == DispositionStatus::Remediated
        && independent
        && d.remediated_at.is_some_and(|r| r <= o.due && date >= r);
    Some(if met {
        Standing::Met
    } else if date <= o.due {
        Standing::Pending
    } else {
        Standing::Overdue
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn accepted() -> Disposition {
        Disposition {
            status: DispositionStatus::Accepted,
            owner: Some("security".into()),
            decided_at: Some(date("2026-09-01")),
            expires_at: Some(date("2026-09-30")),
            rationale: Some("Emergency fix".into()),
            remediated_at: None,
        }
    }

    #[test]
    fn dispositions_need_owner_date_and_rationale() {
        assert!(validate_disposition(&Disposition::default()).is_ok());
        assert!(validate_disposition(&accepted()).is_ok());
        let mut d = accepted();
        d.rationale = None;
        assert!(validate_disposition(&d).is_err());
        let mut d = accepted();
        d.expires_at = Some(date("2026-08-01"));
        assert!(validate_disposition(&d).is_err());
        let mut d = accepted();
        d.status = DispositionStatus::Remediated;
        assert!(validate_disposition(&d).is_err());
        d.remediated_at = Some(date("2026-09-02"));
        assert!(validate_disposition(&d).is_ok());
    }

    #[test]
    fn suppression_window_is_inclusive() {
        let d = accepted();
        assert!(!suppressed(&d, date("2026-08-31")).unwrap());
        assert!(suppressed(&d, date("2026-09-01")).unwrap());
        assert!(suppressed(&d, date("2026-09-30")).unwrap());
        assert!(!suppressed(&d, date("2026-10-01")).unwrap());
        let mut r = accepted();
        r.status = DispositionStatus::Remediated;
        r.remediated_at = Some(date("2026-09-10"));
        assert!(!suppressed(&r, date("2026-09-05")).unwrap());
        assert!(suppressed(&r, date("2026-09-10")).unwrap());
    }
}
