# AI Change Provenance

**Version 0.1** · Status: draft for public comment · Editors: [Noru](https://noru.tech) ·
Reference implementation: [`acc`](../README.md)

AI Change Provenance (ACP) is a small, machine-readable convention for answering one question
about a software change: **did a human who is independent of the change's effective author
approve it?** It exists because the account-based four-eyes principle stops answering that
question once coding agents write the code.

The specification defines (1) how agent authorship and the human operator are declared, (2) what
a collector must record from a development platform, (3) how an evaluator decides independence,
and (4) the interchange formats: a manifest, SARIF, and an in-toto attestation. Every step is
deterministic. No part of it uses a language model, style heuristics or diff statistics.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are to be interpreted as in
[RFC 2119](https://www.rfc-editor.org/rfc/rfc2119).

## 1. Problem statement

Change-control frameworks (SOC 2 CC8.1, ISO/IEC 27001:2022 A.8.32, PCI DSS 6.5.1, NIST SP 800-53
CM-3 and AC-5, among others) require that a change be reviewed and approved by someone other than
the person who made it. Development platforms implement this with accounts: the pull request
author and the approving reviewer must be different users.

That implementation rests on an assumption that is no longer true: *the account that opened the
change is the party that produced it, and the only party whose judgment is embodied in it.*

Coding agents break the assumption in two directions.

- **Same account, two judgments collapsed into one.** An engineer prompts an agent, reads the
  output, and opens the pull request from their own account. A colleague approves. Two accounts,
  two humans: the platform is satisfied, and correctly so. Nothing in this case is wrong; it is
  listed to show that the control still works when the operator is honest about being the author.
- **Two accounts, one judgment.** The agent runs under its own account (a bot, an app, a service
  user, a teammate's automation) and opens the pull request. The engineer who prompted it, chose
  its plan and steered its edits then approves it. Two accounts; one human judgment. The platform
  reports "author ≠ approver" and the control is silently gone.

There is a third, subtler failure: **nobody knows.** An agent opened the change, the platform
records a bot as author, and no record says which human directed it. An evaluator that guesses
("the merger must be the operator") invents a separation that may not exist. An evaluator that
ignores the case ("bots are exempt") waives the control.

ACP replaces "author ≠ approver" with **"the effective human ≠ an approving human, on the current
head, before merge, with the evidence recorded, or an explicit `unknown`."**

## 2. Terminology

| Term | Definition |
| --- | --- |
| **Change** | A unit of proposed modification with an identity, a head commit, a review history and an optional merge: a pull request, merge request or equivalent. |
| **Forge** | The platform hosting the change (GitHub, GitLab, …). |
| **Actor** | A principal that can appear in a change's history. Kinds: `human`, `agent`, `bot`, `service`, `unknown`. |
| **Agent** | A coding agent: software that produces code from natural-language direction. Named by tool, e.g. `claude-code`, `codex`, `cursor`; never by the account it used. |
| **Operator** | The human who directed the agent for this change. |
| **Forge author** | The account the forge records as having opened the change. |
| **Effective author** | The actor that produced the change: a human, or an agent when agent authorship is established. |
| **Effective human** | The human whose judgment the change embodies: the effective author when human, the operator when the effective author is an agent. May be unknown. |
| **Qualifying approval** | An approval by a human other than the effective human, on the change's current head commit, at or before merge, not later withdrawn by that reviewer (§6.1). |
| **Evidence** | A reference to where a fact came from, with a kind: `observed` (read from the forge API), `declared` (asserted by a party), `derived` (computed). |
| **Complete** | A collection whose window and per-change review history were fully retrieved. Anything short of that is *incomplete* and can never yield a clean result. |

Actor identifiers are `namespace:name`, lowercase where the namespace is case-insensitive:
`github:alice`, `agent:claude-code`. A role whose account cannot be resolved is recorded as
`unknown:unavailable`, never guessed.

## 3. Establishing agent authorship

Agent authorship is **declared evidence**. It is accepted only from explicit, delimited sources.
Nothing in this specification permits inferring agent authorship from writing style, diff size,
commit-message phrasing, account naming, or the presence of the word "AI".

### 3.1 Inline declaration

A producer (the agent, its integration, or the operator) MAY place exactly one fenced block with
the info string `agent-change-control` in the change description. Its body is YAML (JSON is
valid YAML) with these keys and no others:

````text
```agent-change-control
author: claude-code
operator: alice
```
````

- `author` (required, non-empty): the agent name. Characters: `[A-Za-z0-9._-]`.
- `operator` (optional): the operator's identity, either bare (`alice`) or namespaced
  (`github:alice`). Bare names resolve in the collecting forge's namespace.

A collector MUST treat more than one block, an unterminated block, unknown keys or an empty
`author` as an error for that change, not as "no declaration".

### 3.2 Provenance document

A producer MAY emit a standalone provenance document conforming to
[`schemas/provenance.schema.json`](../schemas/provenance.schema.json):

```json
{
  "spec_version": "0.1",
  "agent": {"name": "codex", "version": "2026.9.1"},
  "operator": {"id": "github:alice"},
  "session": {"id": "sess_7f3a", "started_at": "2026-09-18T09:12:00Z"},
  "change": {"base_commit": "a1b2…", "head_commit": "c3d4…"}
}
```

A consumer MUST reject a provenance document whose `change.head_commit` differs from the head of
the change it is applied to. Binding to the head commit is what prevents a document from being
replayed against a later, different change. How the document is transported (a file in the
repository, an attestation, a check-run payload) is out of scope for 0.1.

### 3.3 Verified agent accounts

A consumer MAY be configured with an exact mapping from forge accounts to agents
(`my-agent[bot]` → `codex`) for accounts its organization has verified. The mapping is a
consumer-side trust decision, recorded as `declared` evidence. A mapping and an inline
declaration that disagree about the agent MUST be an error.

### 3.4 Reserved

Commit trailers (`Agent-Author:`, `Agent-Operator:`) and signed identity assertions are reserved
for a later version. Consumers MUST NOT interpret them under 0.1.

## 4. Collection

A collector turns forge data into the normalized event model
([`schemas/change-events.schema.json`](../schemas/change-events.schema.json)). For each change in
scope it MUST record:

- the change identity, repository, title, URL, opening time, merge time (if merged) and current
  head commit;
- the forge author, every commit author the forge exposes, and the merger (if merged);
- the complete review history: for each review decision its stable identifier, actor, state
  (`approved`, `changes_requested`, `commented`, `dismissed`), time, and the commit it was
  submitted against;
- the effective author and, when the effective author is an agent, the operator with its
  confidence (`explicit`, `derived`, `unknown`);
- evidence references for each of the above.

Collectors MUST:

1. **Never resolve an identity by guessing.** A declared operator resolves only to an account the
   forge confirms is a human. Emails, display names and "probably the merger" do not resolve.
2. **Record incompleteness rather than absence.** A truncated listing, a deleted account in a
   review, a dismissed review whose dismissal time the API does not expose, or a change that moved
   during collection MUST mark the affected scope incomplete. An empty review list for a change
   whose reviews were not retrieved is a lie; `reviews_complete: false` is the truth.
3. **Bind reviews to commits.** A review that does not carry the commit it applied to cannot
   qualify as an approval of the head.
4. **Use read-only access** against a fixed origin. Collection MUST NOT execute repository
   contents.

## 5. Actor classification

An actor is `human` only when the forge asserts a user account. Apps, installations and accounts
the forge marks as bots are `bot`. Agents (`agent:*`) never appear as forge accounts; they are
introduced by §3. A bot is not an agent unless §3.3 says so, and a bot's approval is never a human
approval.

## 6. Evaluation

An evaluator reads the normalized model and a policy and produces, for every enabled rule and
every change, an assessment with status `pass`, `fail`, `unknown` or `not_applicable`, plus a
finding for every `fail`. It MUST NOT consult the network, the clock or the environment.

### 6.1 Independence

Let *H* be the effective human of a change (§2), or unknown.

For each reviewer, take their **latest non-comment decision** at or before the merge time (all
decisions, for an open change). A `commented` review never withdraws an earlier decision; a later
`changes_requested` or `dismissed` decision does.

A change has a **qualifying independent approval** when some reviewer's latest decision is
`approved`, was submitted against the current head commit, the reviewer is a `human`, and the
reviewer is not *H*.

If *H* is unknown, independence is `unknown`, never `pass`. If review collection for the change is
incomplete, independence is `unknown`: an approval that was later withdrawn could be missing.

### 6.2 Rules

| Rule | Fails when | Not applicable when | Unknown when |
| --- | --- | --- | --- |
| **ACC001** Agent change without independent human | Effective author is an agent, *H* is known, reviews complete, and no qualifying approval exists. | Effective author is not an agent. | *H* unknown, or reviews incomplete. |
| **ACC002** Approver is author | Any `approved` review, at any point, is by *H*. A stale self-approval still counts: the act, not its current effect, is the finding. | never | *H* unknown; or reviews incomplete with no self-approval seen. |
| **ACC003** Merged without independent approval | The change is merged, *H* is known, reviews complete, and no qualifying approval exists. | The change is not merged. | *H* unknown, or reviews incomplete. |
| **ACC006** Unknown agent operator | Effective author is an agent and no human operator is recorded. | Effective author is not an agent. | never |

ACC004 and ACC005 are reserved (deployment and bypass rules). Retired identifiers are never
reused.

Note the design choices these encode:

- **Agent authorship alone is not a finding.** A declared agent with an independent approval is a
  clean change. The control is about human independence, not about tools.
- **Unknown is an answer.** An unknown operator yields ACC006 (advisory by default) and `unknown`
  independence, not a fabricated violation and not a pass.
- **Incomplete inputs cannot pass.** An evaluator MUST signal incompleteness with precedence over
  a policy verdict.

### 6.3 Policy

A policy ([`schemas/policy.schema.json`](../schemas/policy.schema.json)) enables or disables rules,
assigns severities from `info < warning < medium < high`, and sets a failure threshold. Disabling a
rule removes its finding and assessment; it never alters recorded facts.

### 6.4 Dispositions

A finding MAY carry a human decision: `accepted`, `remediated` or `false_positive`, with owner,
decision date, rationale and optional expiry. Dispositions suppress a finding from the threshold
decision on a stated date; they never delete it, and the date MUST be supplied explicitly rather
than read from a clock.

## 7. Outputs

### 7.1 Manifest

The manifest ([`schemas/manifest.schema.json`](../schemas/manifest.schema.json)) embeds the
normalized events, the resolved policy, a summary, the findings, the assessments and generation
metadata including a `source_digest` over the canonical events. A consumer MUST be able to
re-evaluate the embedded events under the embedded policy and obtain byte-identical findings,
assessments and summary; a manifest that does not survive this is invalid. This detects
inconsistency and tampering with the derived parts; it is not a signature and does not authenticate
the events.

Finding identifiers are `acc-` followed by the first 16 hexadecimal characters of SHA-256 over the
canonical JSON array `[repository, change_id, rule_id, sorted_unique_actor_ids]`. They are stable
across severities, policies and dispositions.

### 7.2 SARIF

A SARIF 2.1.0 log with one `result` per finding, `ruleId` set to the rule identifier, the change
URL as the location, the finding identifier as a partial fingerprint, and the assessments under run
properties. `invocations[].executionSuccessful` is `false` when collection was incomplete.

### 7.3 in-toto attestation

An [in-toto Statement v1](https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md)
with:

- `subject`: one entry per evaluated change, `name` set to the change identifier and `digest`
  `{"gitCommit": <head commit>}`;
- `predicateType`: `https://noru.tech/spec/ai-change-provenance/v0.1`;
- `predicate`: the manifest of §7.1.

The statement is emitted unsigned and in canonical form. Signing is done by wrapping the bytes in
a [DSSE](https://github.com/secure-systems-lab/dsse) envelope with a key or identity the
organization trusts. A verifier MUST check that the subject digests cover the commits it is asking
about and SHOULD validate the predicate as in §7.1 before trusting its findings. An attestation
over an empty change set is not produced.

## 8. Determinism

Identical normalized input, resolved policy and tool version MUST produce identical output bytes.
Canonical JSON in this specification means: sorted object keys, compact separators, UTF-8, a single
trailing newline; changes ordered by identifier, commits by hash, reviews by UTC instant then
identifier; evidence lists sorted and de-duplicated; timestamps normalized to UTC. This is a
project canonicalization, not an RFC 8785 claim. Input order MUST NOT affect output.

## 9. Conformance

- A **producer** conforms when every declaration it emits satisfies §3.1 or §3.2 and it never
  emits a declaration for a change it did not author.
- A **collector** conforms when its output validates against the event schema and it satisfies
  §4 and §5.
- An **evaluator** conforms when, for the fixtures published with the reference implementation, it
  produces the same assessments and findings as §6, and it satisfies §8.

## 10. Security and privacy considerations

- A declaration is a claim by the declaring party. It can be forged or omitted, and change
  descriptions are mutable. ACP makes the claim explicit and auditable; it does not authenticate
  it. Signed provenance is future work (§3.4).
- Collection is a snapshot of a mutable system. Deleted accounts, renamed users and edited
  descriptions are recorded as what they are at collection time, with the gaps marked incomplete.
- Manifests contain names, approval histories and work activity. They are personal data. Keep them
  out of public repositories, restrict access, and pseudonymize before sharing.
- A clean result is a statement about the recorded scope and trusted inputs, not a compliance
  certification.

## 11. Versioning

The specification, the schemas and the predicate type share a version. Additive changes (new
optional fields, new rules with new identifiers) increment the minor version. Changes to the
meaning of an existing rule, identifier or format increment the major version. Rule identifiers
are never reused.

## Changelog

- **0.1 (2026-09)** — initial draft: inline declaration, provenance document, verified accounts,
  collection and evaluation requirements, ACC001/ACC002/ACC003/ACC006, manifest, SARIF, in-toto.
