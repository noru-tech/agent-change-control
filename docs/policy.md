# Policy and dispositions

The evaluator uses defaults unless a policy is passed or `.agent-change-control/policy.yml` exists. Partial rule maps inherit defaults. `check` uses the policy embedded in the manifest unless `--policy` explicitly overrides it, preserving reproducibility across machines. Disabling a rule removes its finding and assessment but does not alter facts. Unknown rules and severities fail validation.

```yaml
version: "0.1"
fail_on: medium
minimum_authorship_evidence: derived   # derived | declared | observed | signed
minimum_review_evidence: observed      # derived | declared | observed | signed
agent_review:
  satisfies_independence: false        # opt in: agent approvals may satisfy independence
  require: [operator, provider, identity]   # dimensions that must be independent
  minimum_evidence: signed             # not lowerable
  labels: []                           # scope the mode to changes carrying one of these
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
  agent_approval_recorded:
    enabled: true
    severity: info
  same_vendor_write_and_review:
    enabled: true
    severity: high
  agent_approval_not_independent:
    enabled: true
    severity: high
  agent_approval_unsigned:
    enabled: true
    severity: warning
```

Severity order is info < warning < medium < high; a finding at or above `fail_on` fails `check`. Incomplete collection takes exit-code precedence. ACC006 is advisory by default; unknown independence does not prove a violation.

Evidence kinds are ordered derived < declared < observed < signed. `minimum_authorship_evidence` is the weakest kind an agent operator claim may rest on: below it the operator does not name the effective human, ACC006 fails with the reason "below the policy minimum", and ACC001, ACC002 and ACC003 are unknown. `minimum_review_evidence` is the weakest kind an approval may rest on: below it the approval does not qualify as independent. Forge-observed approvals are `observed`, so `signed` review evidence cannot be satisfied until signed review attestations can be read (see [agent authorship](agent-authorship.md#signed-tier)); the key exists so a policy can say what it requires. Both keys only move a verdict away from pass.

## Agent reviewers

By default an agent's approval never qualifies as independent. With `agent_review.satisfies_independence: true` it does when its evidence is `signed` (a review document verified before the run, see [signing](signing.md)) and every dimension in `require` is independent: `operator` (the reviewing agent's operator is not the effective human), `provider` (the reviewing agent's vendor differs from the author agent's), `identity` (the review document's verified signer differs from the signer behind the authorship claim) and `instructions` (the reviewer's instructions owner is not the effective human; opt-in, because few producers can declare it yet). ACC001 and ACC003 then pass with a reason that names the policy. If no approval qualifies but an agent approval is unknown on a required dimension and dependent on none, ACC001, ACC003 and ACC009 are unknown, never pass. `labels` restricts the mode to changes carrying one of the listed forge labels; the fixtures `agent-review-*` cover each case of the design's worked table.

## Exact deterministic conditions

A known effective human is the effective author when actor.kind is human, or the recorded operator when actor.kind is agent. Other cases remain unknown. A qualifying independent approval is the latest non-comment decision per reviewer at or before merge (all decisions for an open PR), has state approved and the current head SHA, belongs to a human and differs from the known effective human. Any later changes-requested/dismissed decision withdraws that reviewer's approval. Approval at exactly merge time qualifies.

| ID | Condition | Pass fixture | Fail fixture |
| --- | --- | --- | --- |
| ACC001 | Effective author is agent, operator known, reviews complete and no qualifying independent human approval. Partial collection cannot establish whether an approval was later withdrawn. Unknown operator or missing reviews yields unknown. | claude-clean | claude-operator-self-approved |
| ACC002 | Any recorded approved review is by the known effective human. A self-approval still counts if stale; latest effective review state determines independence separately. Without self-approval, complete reviews pass; partial reviews or unknown human yield unknown. | human-clean | human-self-approved |
| ACC003 | Change is merged, effective human known, reviews complete and no qualifying independent human approval. Open changes are not applicable; missing evidence yields unknown. | human-clean | dismissed-approval |
| ACC006 | Effective author is agent and no known human operator exists. Non-agent authors are not applicable. | codex-clean | agent-operator-unknown |
| ACC007 | An agent's latest decision at or before merge approves the current head. An observation (info), never a violation. Not applicable when no agent reviewed the head. | human-clean (n/a) | agent-review-declared |
| ACC008 | Effective author is an agent, at least one approval of the head exists, no human approved it, and every approving agent has the author agent's vendor. Unknown when a vendor is null. | agent-review-independent | agent-review-same-vendor |
| ACC009 | `agent_review` applies, no human independent approval exists, and every agent approval carrying the required evidence is dependent on a required dimension. Unknown when one is unknown on a required dimension and dependent on none. Not applicable when the mode is off, out of label scope, a human independent approval exists, or no agent approval carries the evidence. | agent-review-independent | agent-review-dependent-operator |
| ACC010 | `agent_review` applies and an agent approval of the head has evidence below `agent_review.minimum_evidence`. Not applicable when the mode is off or no agent approved the head. | agent-review-independent | agent-review-unsigned |

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
