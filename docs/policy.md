# Policy and dispositions

The evaluator uses defaults unless a policy is passed or `.agent-change-control/policy.yml` exists. Partial rule maps inherit defaults. `check` uses the policy embedded in the manifest unless `--policy` explicitly overrides it, preserving reproducibility across machines. Disabling a rule removes its finding and assessment but does not alter facts. Unknown rules and severities fail validation.

```yaml
version: "0.1"
fail_on: medium
rules:
  agent_change_without_independent_human:
    enabled: true
    severity: high
  approver_is_author:
    enabled: true
    severity: high
  merged_without_independent_approval:
    enabled: true
    severity: high
  unknown_agent_operator:
    enabled: true
    severity: warning
```

Severity order is info < warning < medium < high; a finding at or above `fail_on` fails `check`. Incomplete collection takes exit-code precedence. ACC006 is advisory by default; unknown independence does not prove a violation.

## Exact deterministic conditions

A known effective human is the effective author when actor.kind is human, or the recorded operator when actor.kind is agent. Other cases remain unknown. A qualifying independent approval is the latest non-comment decision per reviewer at or before merge (all decisions for an open PR), has state approved and the current head SHA, belongs to a human and differs from the known effective human. Any later changes-requested/dismissed decision withdraws that reviewer's approval. Approval at exactly merge time qualifies.

| ID | Condition | Pass fixture | Fail fixture |
| --- | --- | --- | --- |
| ACC001 | Effective author is agent, operator known, reviews complete and no qualifying independent human approval. Partial collection cannot establish whether an approval was later withdrawn. Unknown operator or missing reviews yields unknown. | claude-clean | claude-operator-self-approved |
| ACC002 | Any recorded approved review is by the known effective human. A self-approval still counts if stale; latest effective review state determines independence separately. Without self-approval, complete reviews pass; partial reviews or unknown human yield unknown. | human-clean | human-self-approved |
| ACC003 | Change is merged, effective human known, reviews complete and no qualifying independent human approval. Open changes are not applicable; missing evidence yields unknown. | human-clean | dismissed-approval |
| ACC006 | Effective author is agent and no known human operator exists. Non-agent authors are not applicable. | codex-clean | agent-operator-unknown |

## Recording exceptions

Edit only a finding's `disposition`; do not delete findings or alter their computed fields. Allowed statuses are open, accepted, remediated and false_positive. All non-open dispositions require owner, decided_at and rationale. Remediated also requires remediated_at at or after the decision. Dates are ISO calendar dates. An optional expires_at must not precede the decision date.

```json
{
  "status": "accepted",
  "owner": "security@example.com",
  "decided_at": "2026-09-01",
  "expires_at": "2026-09-30",
  "rationale": "Emergency patch; retrospective review completed.",
  "remediated_at": null
}
```

`check --as-of 2026-09-18` suppresses an accepted/false-positive finding from threshold failure on or after decided_at through expires_at inclusive. Remediated findings are suppressed only once remediated_at is reached. Non-open dispositions require explicit `--as-of`; the machine clock is never consulted. Findings, summaries and source history remain intact even when suppressed. Table FAIL/WARN labels describe recorded facts, while process exit status reflects the disposition-aware policy decision.
