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
| **Evidence** | A reference to where a fact came from, with a kind: `observed` (read from the forge API), `declared` (asserted by a party), `derived` (computed), `signed` (asserted in an attestation whose signature the consumer's operator verified, §3.6). |
| **Complete** | A collection whose window and per-change review history were fully retrieved. Anything short of that is *incomplete* and can never yield a clean result. |

Actor identifiers are `namespace:name`, lowercase where the namespace is case-insensitive:
`github:alice`, `agent:claude-code`. A role whose account cannot be resolved is recorded as
`unknown:unavailable`, never guessed.

## 3. Establishing agent authorship

Agent authorship is established only from explicit, delimited sources, in two tiers: declarations
(§3.1–§3.3), and records that agents and their tools write at authoring time (§3.4). Nothing in
this specification permits inferring agent authorship from writing style, diff size,
commit-message prose, account naming, or the presence of the word "AI".

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
replayed against a later, different change.

The document MAY be carried as the predicate of an in-toto Statement v1 with predicate type
`https://noru.tech/spec/ai-change-provenance/provenance/v0.1` and the head commit as a
`gitCommit` subject, wrapped in a DSSE envelope and signed by the agent's integration or the
operator. A consumer MUST require the subject commit and `change.head_commit` to agree. The
claims count as `signed` evidence under the conditions of §3.6, otherwise as `declared`. An
attestation and an inline declaration (§3.1) for the same change MUST name the same agent, and
the same operator when both name one; disagreement is an error for that change, never a choice.
Other transports (a check-run payload, a file in the repository) are out of scope for 0.1.

### 3.3 Verified agent accounts

A consumer MAY be configured with an exact mapping from forge accounts to agents
(`my-agent[bot]` → `codex`) for accounts its organization has verified. The mapping is a
consumer-side trust decision, recorded as `declared` evidence. A mapping and an inline
declaration that disagree about the agent MUST be an error.

### 3.4 Derived evidence: records written at authoring time

Two kinds of record that agents and their tools already write establish agent authorship at a
**lower tier** than §3.1–§3.3. A consumer reads them only when no declaration or verified account
applies to the change; they yield `derived` confidence (never `explicit`); and when the derived
records of one change name two different agents, the change is not interpreted and the forge
author remains the effective author. Consumers MUST allow the tier to be switched off.

**Vendor commit trailers.** A `Co-Authored-By` trailer whose email is a registered vendor
identity names that vendor's agent. Only git's trailer block is read: the final paragraph of the
commit message, when every line in it is a `Token: value` pair. Emails are lowercased and a
numeric GitHub `ID+` prefix is dropped before matching. The registry in this version:

| Email | Agent |
| --- | --- |
| `noreply@anthropic.com` | `claude-code` |
| `copilot@users.noreply.github.com` | `copilot` |

Consumers MAY extend the registry with their own `email → agent` mappings. They MUST NOT match on
the trailer's display name alone, and MUST NOT treat a co-author trailer for a human as agent
evidence.

**Agent Trace records.** An [Agent Trace](https://agent-trace.dev) record (0.1) is derived
evidence for a change when its `vcs.revision` is one of the change's commits and any
conversation-level or range-level `contributor.type` is `ai` or `mixed`. The agent name is
`tool.name`, normalized to `[a-z0-9._-]`. A record without `vcs.revision` cannot be bound and is
ignored; a record without `tool.name` attests AI authorship that this version cannot name and is
ignored. Agent Trace leaves storage implementation-defined, so the consumer is pointed at the
record files. See §11 for how the two specifications relate.

**Operator derivation.** For derived evidence, the operator is the single human account that
authored *every* commit of the change, with confidence `derived`. Any other case (a bot author,
several human authors, a missing author) leaves the operator unknown and yields ACC006. The
merger, opener and reviewers are never substituted, exactly as for declarations.

### 3.5 Reserved

ACP-specific commit trailers (`Agent-Author:`, `Agent-Operator:`) are reserved for a later
version. Consumers MUST NOT interpret them under 0.1.

### 3.6 Evidence strength

Evidence kinds are ordered by trust: `derived < declared < observed < signed`. A computed
inference is weaker than a party's explicit claim, which is weaker than what the forge itself
recorded, which is weaker than a claim under a verified signature. A policy (§6.3) MAY set the
weakest kind it accepts for authorship claims and for approvals.

A consumer is not required to verify signatures, and the reference implementation does not:
verification needs a trust root (which identities may sign which claims) that belongs with the
signer's tooling, not in an offline evaluator. A consumer that accepts pre-verified attestations
MUST record, for each attestation it drew evidence from, where it came from, its predicate type,
a digest of the signed payload, whether the container carried a signature, and the operator's
statement of who verified it. Evidence is `signed` only when the container carried a signature
and a verifier is recorded; an attestation handed over without that statement yields `declared`
evidence. The record travels with the export, so a manifest states the trust assumption behind
every `signed` fact and a validator rejects `signed` evidence that does not resolve to such a
record.

A consumer that reads a verifier's output (the reference implementation reads
`gh attestation verify --format json`) MAY also record the **signer** the verifier established
(the certificate's subject alternative name and issuer) on the attestation record. The signer is
what the verifier found, never what the predicate says about itself.

Authorship claims (§3.2) and review claims (§3.7) are read from attestations in this version.
Predicates in circulation for reviews (`human-review`, gittuf's reference authorization) carry
the reviewer's identity in the signature rather than in the predicate, which pre-verified input
does not expose; §3.7 defines a predicate that names the reviewer instead.

### 3.7 Review document

A reviewer's tooling MAY emit a review document conforming to
[`schemas/review.schema.json`](../schemas/review.schema.json), carried as the predicate of an
in-toto Statement with predicate type `https://noru.tech/spec/ai-change-provenance/review/v0.1`
and the head commit as a `gitCommit` subject:

```json
{
  "spec_version": "0.1",
  "reviewer": {"kind": "agent", "id": "github:acme-review[bot]", "agent": "claude-code-review"},
  "decision": "approved",
  "change": {"head_commit": "c3d4…"},
  "submitted_at": "2026-08-14T08:59:10Z",
  "operator": {"id": "github:carol"},
  "instructions": {"owner": "github:security-team", "digest": "sha256:…"},
  "model": "claude-opus-5"
}
```

A review document **upgrades a forge review and never creates one**. A consumer MUST match it to
a review the forge recorded by reviewer account, head commit and decision; a document with no
matching review is recorded as unmatched and yields no review, because a signed file must not
approve a change the platform never showed as approved. The reviewer's `kind` MUST agree with
the account's kind, and for an agent reviewer the named `agent` MUST agree with any verified
account mapping (§3.3); either disagreement is an error for the change.

A matched document adds its evidence (`signed` or `declared` per §3.6) to the review. For an
agent reviewer it also records, on the review, the facts only the reviewer's tooling can state:
the **operator** who directed the reviewing agent, the **instructions owner** who controls what
it was told to check, the **model**, and the **identity** the verifier established for the
document's signature. Operator and instructions owner resolve only to accounts the forge
confirms are human (§4), else they are null. A human reviewer's document MUST NOT name an
operator or instructions. These facts are recorded in this version; the rules that read them
(independence between an agent reviewer and the effective author) are defined in 0.2.

## 4. Collection

A collector turns forge data into the normalized event model
([`schemas/change-events.schema.json`](../schemas/change-events.schema.json)). For each change in
scope it MUST record:

- the change identity, repository, title, URL, opening time, merge time (if merged), current
  head commit, and the merge commit when the change is merged and the forge reports one;
- the forge author, every commit author the forge exposes, and the merger (if merged);
- the complete review history: for each review decision its stable identifier, actor, state
  (`approved`, `changes_requested`, `commented`, `dismissed`), time, and the commit it was
  submitted against;
- the effective author and, when the effective author is an agent, the operator with its
  confidence (`explicit`, `derived`, `unknown`);
- the forge labels on the change;
- evidence references for each of the above, and the record of every attestation consulted
  (§3.6).

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

An agent actor carries its **vendor**, the organization that builds and operates the model
behind the tool, from a registry keyed by agent name (`claude-code` → `anthropic`, `copilot` →
`github`, `codex` → `openai`, …) that a consumer MAY extend. An unregistered agent has a null
vendor. Vendor is the granularity: a tool's model changes under the same name and the forge never
records it, so nothing finer is asserted.

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
`approved`, was submitted against the current head commit, the reviewer is a `human`, the
reviewer is not *H*, and the decision's evidence reaches the policy's minimum review evidence
(§6.3).

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

Two keys set the weakest evidence kind (§3.6) a fact may rest on. `minimum_authorship_evidence`
(default `derived`) applies to the operator claim: an operator whose strongest evidence is below
it does not name the effective human, so ACC006 fails with that reason and independence is
`unknown`, never `pass`. `minimum_review_evidence` (default `observed`) applies to approvals: a
decision whose strongest evidence is below it does not qualify (§6.1). Neither key changes the
facts; both can only move a verdict away from `pass`.

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

- `subject`: for each evaluated change, an entry with `name` set to the change identifier and
  `digest` `{"gitCommit": <head commit>}`, followed, when the change is merged and its merge
  commit is known and differs from the head, by an entry with `name` set to the change identifier
  plus `:merge` and `digest` `{"gitCommit": <merge commit>}`. The head commit is the commit the
  approvals are bound to; the merge commit is the one reachable from the target branch after a
  squash or rebase merge. A merge commit the forge did not report is not invented;
- `predicateType`: `https://noru.tech/spec/ai-change-provenance/v0.1`;
- `predicate`: the manifest of §7.1.

Two forms are emitted, both unsigned and in canonical form: one Statement whose subjects cover
every change in the manifest, and JSON Lines with one Statement per change in change identifier
order, where each predicate is a manifest covering that change alone and carrying only the actors
it refers to. Finding identifiers are identical in both forms. Signing is done by wrapping a
Statement's bytes in a [DSSE](https://github.com/secure-systems-lab/dsse) envelope with a key or
identity the organization trusts. A verifier MUST check that the subject digests cover the commits
it is asking about, MUST check that the subjects are exactly those the predicate's changes produce,
and SHOULD validate the predicate as in §7.1 before trusting its findings; the reference
implementation performs the last two checks with `acc validate`. An attestation over an empty
change set is not produced.

A signer that accepts only SHA-2 subject digests (GitHub artifact attestations, `cosign
attest-blob`) MAY instead produce a Statement whose single subject carries a `sha256` digest over
the canonical bytes of the predicate, which is the manifest as the reference implementation's
JSON output writes it. The commits are then found inside the predicate (`head_sha`,
`merge_commit_sha`), and a verifier MUST recompute that digest from the predicate before trusting
the subject. `acc validate` accepts both subject forms. The Statement shape is published as
[`schemas/statement.schema.json`](../schemas/statement.schema.json), and the predicate is
documented in the in-toto predicate template in [`docs/in-toto.md`](../docs/in-toto.md).

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
  it. A signed provenance attestation (§3.2) authenticates the claim only to the extent that
  whoever verified the signature was right to trust the signer; the consumer records that
  statement rather than making it (§3.6).
- Collection is a snapshot of a mutable system. Deleted accounts, renamed users and edited
  descriptions are recorded as what they are at collection time, with the gaps marked incomplete.
- Manifests contain names, approval histories and work activity. They are personal data. Keep them
  out of public repositories, restrict access, and pseudonymize before sharing.
- A clean result is a statement about the recorded scope and trusted inputs, not a compliance
  certification.

## 11. Relationship to Agent Trace and line-level attribution

[Agent Trace](https://agent-trace.dev) (Cursor and partners, 0.1 RFC, CC BY 4.0) records which
model produced which line ranges of which files at a given revision, with links to the
conversation that produced them. AI Change Provenance records who is the effective human behind a
change and whether someone independent of them approved it. The two are complementary and are
meant to be used together:

| | Agent Trace | AI Change Provenance |
| --- | --- | --- |
| Unit | Line ranges within files at a revision | A change (pull request) and its head commit |
| Who wrote it | `contributor.type` (`human`, `ai`, `mixed`, `unknown`) and `model_id`; the `tool` | The agent, by tool name, and the **human operator** |
| Who approved it | Not recorded | The complete review history, bound to commits and time |
| Question answered | What in this file came from a model, and from which conversation? | Was the four-eyes control effective for this change? |
| Storage | Implementation-defined (files, git notes, a database) | Forge data plus a self-validating manifest |

An Agent Trace record is derived evidence to ACP (§3.4): it establishes that a named tool wrote
part of a change, and ACP adds the operator and the independence assessment that Agent Trace does
not model. ACP does not duplicate line attribution and does not need the conversation links; a
manifest cites the record it used by file and `id`. Other tools that write attribution at
authoring time (git-ai, which is an Agent Trace partner; AgentDiff) can be read the same way once
their formats are reviewed; the requirement is the same for all of them: a record bound to a
revision, naming the tool, written when the code was produced.

## 12. Versioning

The specification, the schemas and the predicate type share a version. Additive changes (new
optional fields, new rules with new identifiers) increment the minor version. Changes to the
meaning of an existing rule, identifier or format increment the major version. Rule identifiers
are never reused.

## Changelog

- **0.1, revision 5 (2026-09-21)** — the review document (§3.7) and its predicate type; the
  signer recorded from a verifier's output (§3.6); agent vendors (§5); labels (§4); review
  facts for agent reviewers (`agent` on reviews) and `matched` on attestation records. All
  optional on input; no rule changes. Additive.
- **0.1, revision 4 (2026-09-21)** — the provenance document as a signed in-toto predicate
  (§3.2); evidence strength and pre-verified attestations with a recorded verifier (§3.6,
  `signed` evidence kind, `attestations` record in the export, optional on input); the
  `minimum_authorship_evidence` and `minimum_review_evidence` policy keys (§6.3, §6.1).
  Additive.
- **0.1, revision 3 (2026-09-21)** — the merge commit is recorded for merged changes
  (`merge_commit_sha`, optional on input so earlier exports stay valid) and becomes a second
  attestation subject; JSON Lines attestations with one Statement per change; verifier
  requirements for subjects, including the `sha256`-of-predicate subject form for SHA-2-only
  signers; the Statement schema. Additive.
- **0.1, revision 2 (2026-09-19)** — the derived evidence tier (vendor `Co-Authored-By`
  trailers, Agent Trace records) with operator derivation from commit authorship; relationship to
  Agent Trace (§11). Additive; schemas unchanged.
- **0.1 (2026-09-18)** — initial draft: inline declaration, provenance document, verified
  accounts, collection and evaluation requirements, ACC001/ACC002/ACC003/ACC006, manifest, SARIF,
  in-toto.
