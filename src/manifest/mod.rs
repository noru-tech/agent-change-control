//! Manifest generation, tamper-detecting validation and disposition-aware policy checks.

use crate::model::*;
use crate::{Exit, failure};
use anyhow::{Result, anyhow, ensure};
use chrono::NaiveDate;

/// Normalize `e`, resolve `p` and evaluate every rule into a manifest.
pub fn evaluate(e: Events, p: Policy) -> Result<Manifest> {
    let e = crate::normalize::events(e)?;
    let p = crate::policy::resolve(p)?;
    let (findings, assessments) = crate::rules::evaluate(&e, &p)?;
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
        source_digest: crate::normalize::digest(&e)?,
    };
    Ok(Manifest {
        version: "0.1".into(),
        events: e,
        policy: p,
        summary,
        findings,
        assessments,
        generated,
    })
}

/// Check the schema, then re-evaluate the embedded facts and policy and require byte-identical
/// canonical output. Only structurally valid disposition edits are allowed to differ.
pub fn validate(m: &Manifest) -> Result<()> {
    crate::normalize::schema(&serde_json::to_value(m)?, "manifest")?;
    let mut expected = evaluate(m.events.clone(), m.policy.clone())?;
    ensure!(
        expected.findings.len() == m.findings.len(),
        "manifest findings do not match recorded facts"
    );
    for (actual, computed) in m.findings.iter().zip(expected.findings.iter_mut()) {
        validate_disposition(&actual.disposition)?;
        computed.disposition = actual.disposition.clone();
    }
    ensure!(
        crate::normalize::canonical(m)? == crate::normalize::canonical(&expected)?,
        "manifest differs from deterministic evaluation (facts, digest, summary or findings)"
    );
    Ok(())
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
/// finding ID, and decide the process exit. Non-open dispositions require an explicit `as_of`
/// date; the machine clock is never consulted.
pub fn check(
    m: &Manifest,
    p: Option<Policy>,
    as_of: Option<NaiveDate>,
) -> Result<(Manifest, Exit)> {
    validate(m)?;
    let mut checked = evaluate(m.events.clone(), p.unwrap_or_else(|| m.policy.clone()))?;
    for f in &mut checked.findings {
        if let Some(old) = m.findings.iter().find(|old| old.id == f.id) {
            f.disposition = old.disposition.clone();
        }
    }
    let mut failed = false;
    for f in &checked.findings {
        let d = &f.disposition;
        let suppressed = if d.status == DispositionStatus::Open {
            false
        } else {
            let date = as_of.ok_or_else(|| {
                failure(
                    Exit::Usage,
                    "--as-of is required when evaluating dispositions",
                )
            })?;
            suppressed(d, date)?
        };
        failed |= !suppressed && f.severity >= checked.policy.fail_on;
    }
    let incomplete = !checked.events.window.complete
        || checked.events.changes.iter().any(|c| !c.reviews_complete);
    let exit = if incomplete {
        Exit::Incomplete
    } else if failed {
        Exit::PolicyFailed
    } else {
        Exit::Ok
    };
    Ok((checked, exit))
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
