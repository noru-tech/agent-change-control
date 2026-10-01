//! Manifest generation, tamper-detecting validation and disposition-aware policy checks.

use crate::canonical::Canonicalization;
use crate::model::*;
use crate::{Exit, failure_with_hint};
use anyhow::{Result, anyhow, bail, ensure};
use chrono::NaiveDate;

/// The manifest version `acc` writes: ACP 0.3, digests over RFC 8785 bytes.
pub const VERSION: &str = "0.3";
/// The last manifest version with the legacy canonicalization, read for validation only.
pub const LEGACY_VERSION: &str = "0.2";
/// The only release that wrote [`LEGACY_VERSION`] manifests.
pub const LEGACY_GENERATOR: &str = "0.4.0";

/// Normalize `e`, resolve `p` and evaluate every rule into a manifest.
pub fn evaluate(e: Events, p: Policy) -> Result<Manifest> {
    evaluate_as(e, p, Canonicalization::Jcs)
}

/// [`evaluate`], with the digests, identifiers and version marks of the given canonicalization.
/// Rule semantics are the same for both; only the bytes that are hashed differ.
fn evaluate_as(e: Events, p: Policy, canon: Canonicalization) -> Result<Manifest> {
    let e = crate::normalize::events(e)?;
    let mut p = crate::policy::resolve(p)?;
    let (mut findings, assessments) = crate::rules::evaluate(&e, &p)?;
    if canon == Canonicalization::Legacy {
        p.version = LEGACY_VERSION.into();
        for f in &mut findings {
            f.id = std::mem::take(&mut f.legacy_ids).remove(0);
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
    let (version, generator) = match canon {
        Canonicalization::Jcs => (VERSION, crate::version()),
        Canonicalization::Legacy => (LEGACY_VERSION, LEGACY_GENERATOR),
    };
    let generated = Generated {
        tool: "agent-change-control".into(),
        version: generator.into(),
        source_digest: canon.digest(&e)?,
    };
    Ok(Manifest {
        version: version.into(),
        events: e,
        policy: p,
        summary,
        findings,
        assessments,
        generated,
    })
}

/// Check the schema, then re-evaluate the embedded facts and policy and require byte-identical
/// canonical output. Only structurally valid disposition edits are allowed to differ. A 0.2
/// manifest is re-evaluated with the legacy canonicalization it was written with; the result
/// says which one applied.
pub fn validate(m: &Manifest) -> Result<Canonicalization> {
    let canon = canonicalization(&m.version)?;
    crate::normalize::schema(&serde_json::to_value(m)?, "manifest")?;
    let mut expected = evaluate_as(m.events.clone(), m.policy.clone(), canon)?;
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
    Ok(canon)
}

/// The canonicalization a manifest version was written with.
pub fn canonicalization(version: &str) -> Result<Canonicalization> {
    match version {
        VERSION => Ok(Canonicalization::Jcs),
        LEGACY_VERSION => Ok(Canonicalization::Legacy),
        _ => bail!("unsupported manifest version"),
    }
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
    validate(m)?;
    let mut checked = evaluate(m.events.clone(), p.unwrap_or_else(|| m.policy.clone()))?;
    for f in &mut checked.findings {
        if let Some(old) = m
            .findings
            .iter()
            .find(|old| old.id == f.id || f.legacy_ids.contains(&old.id))
        {
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
                failure_with_hint(
                    Exit::Usage,
                    "--as-of is required when evaluating dispositions",
                    "pass --as-of YYYY-MM-DD, the date the dispositions are evaluated on; acc never reads the clock",
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
