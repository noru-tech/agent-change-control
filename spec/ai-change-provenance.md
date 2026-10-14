# AI Change Provenance

**Version 0.4** · Status: draft for public comment · Editors: [Noru](https://noru.tech) ·
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
Other transports (a check-run payload, a file in the repository) are out of scope for this version.

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
version. Consumers MUST NOT interpret them under this version.

### 3.6 Evidence strength

Evidence kinds are ordered by trust: `derived < declared < observed < signed`. A computed
inference is weaker than a party's explicit claim, which is weaker than what the forge itself
recorded, which is weaker than a claim under a verified signature. A policy (§6.3) MAY set the
weakest kind it accepts for authorship claims and for approvals.

A consumer is not required to verify signatures, and the reference implementation does not:
verification needs a trust root (which identities may sign which claims) that belongs with the
signer's tooling, not in an offline evaluator. A consumer that accepts pre-verified attestations
MUST record, for each attestation it drew evidence from, where it came from, its predicate type,
a digest of the signed payload (the DSSE payload bytes exactly as signed, or, for a Statement
handed over without an envelope, its §8.2 serialization), whether the container carried a signature, and the operator's
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

A matched document adds its evidence (`signed` or `declared` per §3.6) to the review. For an agent
reviewer it also records, on the review, the facts only the reviewer's tooling can state: the
**operator** who directed the reviewing agent, the **instructions owner** who controls what it was
told to check, the **model**, and the **identity** the verifier established for the document's
signature. Operator and instructions owner resolve only to accounts the forge confirms are human
(§4), else they are null. A human reviewer's document MUST NOT name an operator or instructions.
These facts are recorded in this version; the rules that read them (independence between an agent
reviewer and the effective author) are ACC008 to ACC010 (§6.2) and the dimensions of §6.5.

### 3.8 Risk assessment

How risky a change is, and therefore which requirements it must meet, is decided outside this
specification, by a [Change Risk](https://noru.tech/spec/ai-change-provenance/change-risk/v0.1)
assessment: an in-toto Statement with predicate type
`https://noru.tech/spec/ai-change-provenance/change-risk/v0.1` whose predicate names the change's
head commit (`change.head`), its tier (`low`, `standard`, `high` or `emergency`) and, for an
emergency, who declared it and when (`emergency.declared_by`, `emergency.declared_at`).

A consumer reads an assessment handed over like any attestation (§3.6) and records its conclusion
as the change's `risk`: the tier, the emergency's declarer and time, and evidence that is `signed`
when the caller verified the signature and `declared` otherwise. It does not re-derive the tier.
Assessments of one head that disagree are an error for the change. The policy decides how much
evidence a tier needs before it may lower what a change must meet (§6.7).

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
   qualify as an approval of the head. Where the forge records when an approval was given but
   not which commit it was given against, a collector MAY bind it to the head the forge recorded
   as current at that time, as `derived` evidence that names the record it came from. When
   that record cannot settle the question (no head recorded before the approval, or a new head
   recorded close enough to the approval that either could be the one approved) the approval is
   not recorded, and the review history is incomplete (§4.4).
4. **Use read-only access** against a fixed origin. Collection MUST NOT execute repository
   contents.

### 4.1 Checks

When asked for the facts ACC004 reads (§6.2), a collector records `checks` for each merged
change:

- the branch the change merged into;
- every completed check run and commit status on the head commit and, when distinct, the merge
  commit: its name (a status's context), kind, commit, conclusion and completion time (a status's
  creation time), with evidence. Results completed after the merge are recorded; the evaluator
  ignores them;
- the check names the forge's rules for that branch require, and which sources were read
  (`rules`, `branch_protection`). Forges expose these rules as they are now, not as they were at
  the merge; they are recorded as observed at collection, never as a fact about the merge;
- `runs_complete: false` when a listing was truncated or could not be read, or held a conclusion
  outside the vocabulary, and `required_complete: false` when a source of required checks could
  not be read.

A collector that was not asked omits `checks`. Absence never reads as a pass (§6.2).

### 4.2 Protection bypass

When asked for the facts ACC005 reads (§6.2), a collector records `protection` for each merged
change:

- the branch the change merged into;
- every ruleset evaluation the forge recorded for the push that landed the merge commit: its
  identifier, the commit the push moved the branch to (always the merge commit), its result
  (`pass`, `fail` or `bypass`), the pushing account as the forge reports it, and the push time,
  with evidence;
- `complete: false` when the evaluations could not be read, when the merge is older than the
  forge's listing reaches, when the merge commit is unknown, or when a result is outside the
  vocabulary.

Forges do not record everything a bypass can be. An actor a ruleset exempts is not evaluated at
all, and an override of protection that is not a ruleset may be visible only in an
administrative audit log. An empty `suites` list is therefore never evidence that nothing was
bypassed. A collector that was not asked omits `protection`.

### 4.3 Reviewer teams

When a policy's tiers require reviewer teams (§6.7) and ACC011 is enabled, a collector records each
required team once for the export, in `teams`:

- its identifier, lowercased: `ORG/SLUG` on GitHub, the group's full path on GitLab;
- its members as the forge lists them **at collection**, as actor identifiers, with the time they
  were observed and evidence;
- `complete: false` when the listing could not be read in full.

Forges expose a team's membership as it is now, not as it was when a review was submitted. A
member who left between the review and the collection is not counted, and one who joined is: the
observation time is recorded so that a reader can tell. A listing that could not be read is
incomplete, never empty.

### 4.4 GitLab

How a GitLab collector meets §4 to §4.3, where GitLab's records differ from GitHub's:

- **Identity.** Actors are `gitlab:username`, lowercased. The repository is the project's full
  path (`GROUP/…/PROJECT`), and a change is `gitlab:PATH:mr:IID`. The account GitLab assigns a
  deleted user's contributions to (`ghost`) is `unknown:unavailable`. An account is `human` only
  when GitLab's user record says it is not a bot; when the record cannot be read, the actor is
  `unknown`. Commit authors are names and emails without accounts, so none is recorded (rule 1).
- **Approvals.** GitLab lists the approvals in force on a merge request with the time each was
  given, and no commit. Each is bound (rule 3) to the head of the latest diff version GitLab
  created at or before the approval. Because GitLab creates a version shortly after a push, a
  version created within one second of the approval, before or after it, leaves the binding
  ambiguous. A withdrawn approval is no longer listed, so it cannot be recorded, and never needs
  to be: it would not qualify. Without the approval time (GitLab Community Edition omits it),
  nothing binds and the history is incomplete.
- **Merge commit.** The merge commit, or for a squashed fast-forward the squash commit. A plain
  fast-forward reports neither, and the merge commit is then unknown.
- **Checks (§4.1).** GitLab has no named required checks. When the project's merge settings
  require pipelines to succeed, the required check is `pipeline` (source `project_settings`).
  Every finished pipeline on the head or merge commit is recorded as a result named `pipeline`
  (kind `pipeline`), and every finished job as a result under the job's name (kind `job`). A job
  allowed to fail that failed is `neutral`; a job or pipeline that has not finished, including a
  manual job never started, has no result. A merged-results pipeline runs on a commit that is
  neither, and is not recorded.
- **Bypass (§4.2).** GitLab does not record an evaluation of each push against the target
  branch's protection on every tier, so `protection` is recorded with `complete: false`.
- **Reviewer teams (§4.3).** A team is a group, and its members are the group's direct members.
  Members inherited from an ancestor group are not counted: they would make the parent group the
  team.
- **Branch updates and deployments (§4.5, §4.6).** GitLab's push events say neither whether a
  push was forced nor whether a merge request landed it, so branches are recorded with
  `complete: false`; deployments are recorded with `complete: false` in this version.

### 4.5 Branch updates

When asked for the facts ACC012 reads (§6.2), a collector records, in `branches`, each protected
branch the policy names (`branches.protected`), or the repository's default branch when it names
none:

- its subject identifier, `FORGE:REPOSITORY:branch:NAME`, and its name;
- every update of it in the window, oldest first: the forge's identifier for the update, its kind
  (`push`, `force_push`, `branch_creation`, `branch_deletion`, `pr_merge`, `merge_queue_merge`),
  the commits the branch pointed at before and after, the time, and the updating account when
  the forge names one, with evidence;
- `complete: false` when the forge's record of the window could not be read in full, reaches
  less far back than the window, or holds a kind outside the vocabulary.

On GitHub the source is the repository activity record, which reaches a year back from
collection. Changes are a population of pull requests; a direct push or a force push is not one,
which is why branch updates are a population of their own.

### 4.6 Deployments

When asked for the facts ACC013 reads (§6.2), a collector records `deployments`: the environments
that count as production (the policy's `deployments.environments`; when it names none, those the
forge marks as production), the deployment window, and every successful deployment to them in
that window:

- its subject identifier, `FORGE:REPOSITORY:deployment:ID`, environment, the commit deployed, the
  time it succeeded and the account that created it, with evidence;
- the commit of the same environment's previous successful deployment, and the first-parent
  commits after it up to and including the deployed one, newest first, each with its committer
  date. A deployment of a commit at or behind the previous one shipped only that commit;
- `commits_complete: false` when that history could not be read in full, or when no earlier
  deployment bounds it. The deployed commit itself is then still recorded and judged.

`complete: false` on the record when a listing could not be read in full. A deployment window
that starts later than the merge window lets the changes a deployment shipped be merged inside
the merge window; a commit dated before the merge window cannot be judged.

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

Under the opt-in `agent_review` policy (§6.3) there is a second form: an **agent's** latest
decision is `approved` on the current head, its evidence reaches `agent_review.minimum_evidence`,
and every dimension the policy requires is `independent` (§6.5). The assessment reason then
states that policy, not a human, made the call. If no approval qualifies but some agent approval
with sufficient evidence is `unknown` on a required dimension and `dependent` on none,
independence is `unknown`, never `pass`.

If *H* is unknown, independence is `unknown`, never `pass`. If review collection for the change is
incomplete, independence is `unknown`: an approval that was later withdrawn could be missing.

### 6.2 Rules

| Rule | Fails when | Not applicable when | Unknown when |
| --- | --- | --- | --- |
| **ACC001** Agent change without independent human | Effective author is an agent, *H* is known, reviews complete, and no qualifying approval exists. | Effective author is not an agent. | *H* unknown, or reviews incomplete. |
| **ACC002** Approver is author | Any `approved` review, at any point, is by *H*. A stale self-approval still counts: the act, not its current effect, is the finding. | never | *H* unknown; or reviews incomplete with no self-approval seen. |
| **ACC003** Merged without independent approval | The change is merged, *H* is known, reviews complete, and fewer qualifying approvals exist than its tier requires (one, without tiers; §6.7). | The change is not merged. | *H* unknown, or reviews incomplete; or an agent approval unknown on a required dimension could make up the difference. |
| **ACC004** Merged without required checks | The change is merged, check collection is complete, and for some required check neither the head commit nor the merge commit has a passing latest result completed at or before the merge. | The change is not merged, or no check is required. | Check facts were not collected or are incomplete; or, under `from_forge`, the forge's required checks could not all be read and every known required check passed. |
| **ACC005** Merged with protection bypass | The change is merged and a recorded ruleset evaluation of its merge is `bypass`. | The change is not merged. | Protection facts were not collected or are incomplete; an evaluation of the merge is `fail` although the push landed; or no evaluation of the merge is recorded. |
| **ACC006** Unknown agent operator | Effective author is an agent and no human operator is recorded. | Effective author is not an agent. | never |
| **ACC007** Agent approval recorded (`info`) | An agent's latest decision at or before merge is `approved` on the current head. Not a violation: it makes the population of agent-approved changes visible. | No agent reviewed the current head. | never |
| **ACC008** Same-vendor write and review | Effective author is an agent, at least one approval of the current head exists, no human approved it, and every approving agent has the author agent's vendor. | Effective author is not an agent, a human approved the head, or nothing approved it. | The author's or an approving agent's vendor is null. |
| **ACC009** Agent approval lacks required independence | `agent_review` applies, no human independent approval exists, at least one agent approval carries the required evidence, and every such approval is `dependent` on a required dimension. | `agent_review` is off or out of scope, a human independent approval exists, or no agent approval carries the required evidence. | Some such approval is `unknown` on a required dimension and `dependent` on none. |
| **ACC010** Agent approval without signed identity (`warning`) | `agent_review` applies and an agent approval of the current head has evidence below `agent_review.minimum_evidence`. | `agent_review` is off or out of scope, or no agent approved the head. | never |
| **ACC011** Missing required reviewer team | The change is merged, *H* is known, reviews complete, and for some team its tier requires, no member (§4.3) approved the current head before merge independently of *H*. | The change is not merged, or its tier requires no team. | *H* unknown, or reviews incomplete; or a team that did not approve has incomplete membership. |
| **ACC012** Unaccounted branch update | A protected branch (§4.5) was force-pushed or deleted in the window, or pushed to a commit that is no merged change's merge commit. | never | The branch's record is incomplete and no such update was seen. |
| **ACC013** Deployed without passing change | A production deployment (§4.6) shipped a commit dated inside the merge window that no merged change landed, or that a change landed which fails: an assessment of it at or above `fail_on` is `fail`. | never | No commit fails, and the history is incomplete or unbounded, a commit is dated before the merge window, or a change that landed one is unknown (an unknown assessment, incomplete reviews, or missing opt-in facts). |

For ACC004, the required checks are the policy's `checks.required` and, when `checks.from_forge`
is true (the default), the forge's required checks recorded under §4.1. A required check passes
when, on the head commit or on the merge commit, its latest result completed at or before the
merge is `success`, `neutral` or `skipped`; when several results share that instant, every one
must pass. The merge commit counts because a merge queue runs checks on the commit it merges,
not on the head. A check with no result at all fails. ACC004 is disabled unless a policy enables
it, and when it is enabled, a merged change whose check facts are missing or incomplete makes
collection incomplete (§6.3), so it can never pass on data that was not read.

ACC005 passes only when every recorded evaluation of the merge passed. It never passes on an
empty record (§4.2): absence is `unknown`. A recorded `bypass` fails even when the record is
otherwise incomplete. ACC005 is disabled unless a policy enables it, and when it is enabled, a
merged change whose protection facts are missing or incomplete makes collection incomplete.

ACC012 and ACC013 judge subjects that are not changes: a branch and a deployment. Their assessments
and findings name the subject's identifier where the others name a change (§7.1). ACC012 has one
assessment per branch and a finding per offending update, named `BRANCH#UPDATE`; ACC013 an assessment and, when it
fails, a finding per deployment. ACC013 reads the change assessments of the same evaluation, so
dispositions, which `check` applies later, do not clear it: a deployment of a change whose finding
was accepted needs its own disposition. Both rules are disabled unless a policy enables them, and
when one is enabled, missing or incomplete facts for it make collection incomplete. A single
pull request evaluated alone (the reference implementation's `pr`) does not evaluate them.

Retired identifiers are never reused. ACC007 and ACC008
evaluate under every policy, as observations; ACC009 and ACC010 only under `agent_review`.

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

Two blocks name the subjects of ACC012 and ACC013: `branches.protected` lists the protected
branches (empty: the repository's default branch), and `deployments.environments` the production
environments (empty: those the forge marks as production). Both are 0.4 keys, normalized sorted
and de-duplicated, and present in a resolved policy only when given.

The `agent_review` block lets an agent approval satisfy independence:

```yaml
agent_review:
  satisfies_independence: false        # default: agent approvals never qualify
  require: [operator, provider, identity]
  minimum_evidence: signed             # not lowerable in this version
  labels: []                           # when set, only changes carrying one are in scope
```

`require` names the dimensions (§6.5) that MUST be `independent`; it MUST NOT be empty, and its
default is the three a signed review document (§3.7) plus the vendor registry (§5) can establish.
`instructions` is opt-in because few producers can declare instruction ownership yet, and a
policy author who leaves it out accepts that a clean result under the mode says nothing about who
wrote the reviewer's instructions. `minimum_evidence` MUST be `signed`: a declared agent reviewer
is never enough. `labels` scopes the mode to changes carrying one of the listed forge labels;
risk classification itself stays outside this specification.

The `checks` block names what ACC004 requires:

```yaml
checks:
  required: []                         # check names always required
  from_forge: true                     # also require what the forge's rules required (§4.1)
rules:
  merged_without_required_checks:
    enabled: false                     # default: off
    severity: high
```

A policy that enables ACC004 also widens incompleteness: a merged change without complete check
facts, or, under `from_forge`, without a complete set of the forge's required checks, makes the
collection incomplete, which takes precedence over a policy verdict (§6.2). ACC005
(`merged_with_protection_bypass`, disabled by default) does the same for protection facts.

A policy with `tiers` (§6.7) widens incompleteness the same way: a merged change whose tier
requires a team without a complete membership record (§4.3) makes collection incomplete when
ACC011 is enabled.

### 6.5 Independence dimensions

For an agent's approval of the current head, each dimension is `independent`, `dependent` or
`unknown`. `unknown` on a required dimension never yields a clean result.

| Dimension | Independent when | Dependent when | Unknown when |
| --- | --- | --- | --- |
| `operator` | The review's recorded operator differs from *H* | They are the same actor | Either is unknown |
| `provider` | The reviewing agent's vendor differs from the author agent's vendor; for a human author, always | Same vendor | Either vendor is null |
| `identity` | The review document's verified signer differs from every verified signer behind the authorship claim | It is one of them | The review has no verified signer, or the authorship claim has none |
| `instructions` | The review's recorded instructions owner differs from *H* | It is *H* | Either is unknown |

`operator` and `instructions` look alike and are not: the same human can direct a reviewer they
did not configure, and a different human can run a reviewer whose instructions the author wrote;
only `instructions` catches the second case. `identity` is the in-toto notion of distinct
functionaries with distinct keys and the one dimension that rests on a verified fact rather than
a declared one. `provider` is deliberately coarse (§5): it answers whether one model checked its
own work and nothing finer. A human approval is never evaluated on these dimensions; §6.1's first
form is its whole test.

### 6.4 Dispositions

A finding MAY carry a human decision: `accepted`, `remediated` or `false_positive`, with owner,
decision date, rationale and optional expiry. Dispositions suppress a finding from the threshold
decision on a stated date; they never delete it, and the date MUST be supplied explicitly rather
than read from a clock.

A finding with a retroactive-review obligation (§6.7) is decided by that obligation alone. On the
evaluation date it is **pending**, and does not count against the threshold, until its due date; it
is **met**, and does not count, when it carries a `remediated` disposition whose `remediated_at` is
on or before the due date and the evaluation date, and whose `owner` is a human actor of the export
other than *H* and the change's author; otherwise, after the due date, it is **overdue** and counts.
No other disposition discharges it. The owner's independence is checked against the facts, not
taken on the disposition's word.

### 6.6 Validation

An evaluator MUST reject input that is not a valid ACP document, and MUST NOT produce findings,
assessments or a manifest for it. It SHOULD report why with these codes; when several apply,
reporting any one of them is enough, and a consumer compares codes as a set.

| Code | Input is rejected when |
| --- | --- |
| ACV001 | An `approved` review is later than the change's merge. |
| ACV002 | Reserved (deployment validation). |
| ACV003 | A reference between facts does not resolve or is not allowed: a malformed team identifier or team member, a risk tier without evidence, a malformed emergency declarer; an actor, operator or attestation that is not in the registry; a check result on a commit that is neither the change's head nor its merge commit; a ruleset evaluation of a push other than the change's merge; an operator that is not a human, or on a change whose effective author is not an agent; agent review facts on a reviewer that is not an agent; `signed` evidence that does not resolve to a signed attestation with a recorded verifier (§3.6); a malformed actor or attestation identifier. |
| ACV004 | The timeline or identities are inconsistent: a reversed window, or a window whose completeness and reason disagree; a merge before opening, a merge time without a merger or the reverse, a merge commit, check facts or protection facts on an unmerged change; a review before opening; duplicate change, review or commit identifiers, or duplicate check results, ruleset evaluations or teams; a risk tier whose emergency details disagree with it (present on a non-emergency tier, or absent on an emergency); two decisions by one reviewer at one instant that disagree; a change whose repository differs from the export's; an invalid timestamp. |
| ACV005 to ACV009 | The input is not I-JSON as §8.2 constrains it. |
| ACV010 | The input does not conform to its schema: an unknown member, a missing required member, a wrong type, or a value outside a closed vocabulary. |

### 6.7 Risk tiers

A policy MAY vary what a change must meet by its risk tier (§3.8):

```yaml
tiers:
  low:       { min_human_approvals: 1, agent_review: allowed }
  standard:  { min_human_approvals: 1 }
  high:      { min_human_approvals: 2, agent_review: forbidden, required_reviewer_teams: [acme/security] }
  emergency: { retroactive_review_within_days: 3 }
missing_tier: standard          # default; never emergency
minimum_risk_evidence: signed   # default
```

Without `tiers`, no tier applies to any change and nothing in this section changes a result.

**The applied tier.** A change's applied tier is its assessment's tier, except that an assessment
that would *lower* what the change must meet (an `emergency`, or a level below `missing_tier`,
where `low < standard < high`) counts only when its evidence reaches `minimum_risk_evidence`;
otherwise, and when there is no assessment, the applied tier is `missing_tier`. The manifest records
every change's applied tier and why: `assessment`, `missing` or `below_minimum` (§7.1). A tier
that raises the requirements is never refused for its evidence.

**Approvals.** ACC003 requires `max(1, min_human_approvals)` qualifying approvals (§6.1) for the
applied tier: distinct qualifying human reviewers, plus one when an agent approval qualifies. A
`min_human_approvals` of `0` never makes an unreviewed change pass; it states what the forge's
pre-merge rules may allow, as for an emergency, and the change is held to account afterwards.

**Agent review.** A tier's `agent_review` (`allowed` or `forbidden`) replaces
`agent_review.satisfies_independence` for changes of that tier; the dimensions, evidence minimum
and label scope still come from `agent_review`.

**Reviewer teams.** ACC011 requires, for each team the applied tier names, an approval of the current
head before merge by a human member (§4.3), independent of *H*, with evidence at
`minimum_review_evidence`.

**Emergencies.** An emergency change that merged without the approvals its tier's ACC003 requires
fails ACC003 like any other, and when the policy's emergency tier sets
`retroactive_review_within_days`, its finding carries an obligation: a retroactive review due
that many days after the merge date (UTC). The obligation changes how the finding counts against
the threshold (§6.4), never whether it exists.

## 7. Outputs

### 7.1 Manifest

The manifest ([`schemas/manifest.schema.json`](../schemas/manifest.schema.json)) embeds the
normalized events, the resolved policy, a summary, the findings, the assessments and generation
metadata including a `source_digest` over the normalized events. A consumer MUST be able to
re-evaluate the embedded events under the embedded policy and obtain byte-identical findings,
assessments and summary; a manifest that does not survive this is invalid. This detects
inconsistency and tampering with the derived parts; it is not a signature and does not authenticate
the events.

Finding identifiers are `acc-` followed by the first 16 hexadecimal characters of SHA-256 over the
§8.2 serialization of the array `[repository, change_id, rule_id, sorted_unique_actor_ids]`. They
are stable across severities, policies and dispositions. For ACC012 and ACC013, `change_id` names
the subject (§6.2): an ACC013 assessment and finding name the deployment, an ACC012 assessment
names the branch, and an ACC012 finding names the update, as the branch identifier, `#`, and the
update's identifier, so each update has its own finding and identifier. The `source_digest` is SHA-256 over the
§8.2 serialization of the normalized events.

Because 0.2 hashed these preimages with a trailing newline (§8.2), every finding identifier
changed in 0.3, and for that one minor version a 0.3 finding carried `legacy_ids`, the identifier
the same finding had under 0.2. 0.4 removes `legacy_ids`; a consumer that carries dispositions
from an earlier manifest to a re-evaluation MAY still match a 0.2 identifier, and the reference
implementation's `check` does.

Under a policy with tiers (§6.7), `applied_tiers` lists every change's applied tier and its source,
and a finding with a retroactive-review obligation carries it as `obligation` (`kind`, `due`). Both
are derived, and re-evaluation reproduces them.

`generated.tool` and `generated.version` name the implementation that wrote a manifest, not its
result. Re-evaluation excludes them from the comparison, so a manifest written by any conforming
evaluator of its version validates.

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
- `predicateType`: `https://noru.tech/spec/ai-change-provenance/v0.4`;
- `predicate`: the manifest of §7.1.

Two forms are emitted, both unsigned and serialized as §8.2 requires: one Statement whose subjects cover
every change in the manifest, and JSON Lines with one Statement per change in change identifier
order, where each predicate is a manifest covering that change alone and carrying only the actors
it refers to. A branch or deployment (§4.5, §4.6) belongs to no change, so only the single
Statement carries it and its findings. Each line is one Statement's §8.2 bytes followed by an LF that separates lines and
is part of no Statement. Finding identifiers are identical in both forms. Signing is done by wrapping a
Statement's bytes in a [DSSE](https://github.com/secure-systems-lab/dsse) envelope with a key or
identity the organization trusts. A verifier MUST check that the subject digests cover the commits
it is asking about, MUST check that the subjects are exactly those the predicate's changes produce,
and SHOULD validate the predicate as in §7.1 before trusting its findings; the reference
implementation performs the last two checks with `acc validate`. An attestation over an empty
change set is not produced.

A signer that accepts only SHA-2 subject digests (GitHub artifact attestations, `cosign
attest-blob`) MAY instead produce a Statement whose single subject carries a `sha256` digest over
the §8.2 serialization of the predicate. The reference implementation's JSON output is exactly
those bytes, so the digest of the manifest file is the subject digest. The commits are then found inside the predicate (`head_sha`,
`merge_commit_sha`), and a verifier MUST recompute that digest from the predicate before trusting
the subject. `acc validate` accepts both subject forms. A `v0.3` or `v0.2` Statement is verified
as the version that produced it (§12). The Statement shape is published as
[`schemas/statement.schema.json`](../schemas/statement.schema.json), and the predicate is
documented in the in-toto predicate template in [`docs/in-toto.md`](../docs/in-toto.md).

### 7.4 Release attestation

A release attestation joins change-level evidence to the build provenance of the same artifacts.
It is an in-toto Statement v1
([`schemas/release.schema.json`](../schemas/release.schema.json)):

- `subject`: the release artifacts, each a name and a `sha256` digest the producer supplies (at
  least one);
- `predicateType`: `https://noru.tech/spec/ai-change-provenance/release/v0.1`;
- `predicate`: the range (`base`, `head`, as full commit identifiers), and for every first-parent
  commit after `base` up to and including `head`, newest first, its coverage (`covered`,
  `incomplete`, `uncovered`, `failing`) and the changes that landed it, each with its verdict
  (`pass`, `incomplete`, `fail`), the identifiers of its failing findings, a `verdict_digest`
  (SHA-256 over the §8.2 serialization of `{change_id, merge_commit_sha, verdict, failing}`) and
  the digest of the manifest it came from; the commits no change landed (`uncovered`); the
  manifests read, each with its digest and `source_digest`; whether the policy was the manifests'
  own or one given (`policy`); the date dispositions were evaluated on (`as_of`); and the
  producer (`generated`);
- `verdict`: `pass` when every commit is covered, `incomplete` when a commit's only verdicts are
  incomplete, and `fail` otherwise.

Coverage is the deploy gate's: a commit is covered by a change whose merge commit it is and which
passes the policy as `check` judges it, with dispositions; commits a merge brought in are covered
by that merge's change. A producer writes the Statement whatever its verdict, because a release
that shipped uncovered commits is itself a fact to attest. It is unsigned, and is signed as any
other Statement is (§7.3).

## 8. Determinism

Identical normalized input, resolved policy and tool version MUST produce identical output bytes.
Two steps get there: normalization fixes the value (§8.1), and serialization fixes its bytes
(§8.2).

### 8.1 Normalization

Changes are ordered by identifier, commits by hash, reviews by UTC instant then identifier;
evidence lists and labels are sorted and de-duplicated; timestamps are normalized to UTC
(RFC 3339 with `Z`). Input order MUST NOT affect output. These rules carry ACP semantics that a
serialization cannot: RFC 8785 orders the members of an object, never the elements of an array.

### 8.2 Serialization

Every byte sequence ACP hashes or signs is the
[RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) (JSON Canonicalization Scheme, JCS)
serialization of the normalized value. That covers the finding identifiers and `source_digest`
(§7.1), the manifest and Statement bytes and the `sha256` subject form (§7.3), and the payload
digest recorded for an attestation handed over without an envelope (§3.6). The preimage of any
ACP digest is the exact JCS byte sequence and nothing else.

ACP documents (event exports, policies, manifests, Statements, provenance and review documents)
are [I-JSON](https://www.rfc-editor.org/rfc/rfc7493) with these additional constraints. A
consumer MUST reject a document that violates one, with the code given:

| Constraint | Code |
| --- | --- |
| Every number is an integer literal, with no fraction and no exponent. Non-integer numbers are forbidden anywhere in an ACP document. | ACV005 |
| Every integer is in the range ±(2^53 − 1). | ACV006 |
| Strings contain no unpaired surrogate, escaped or otherwise. | ACV007 |
| Member names are unique within each object, compared after unescaping. | ACV008 |
| Nesting depth is at most 128, counting the outermost container as depth 1 and each nested container as one more. A consumer rejects the input at the first container past the bound, without parsing further. | ACV009 |

The depth bound is the one the AI Agent Action predicate proposal and its conformance suite use.
Together the constraints make every ACP value have exactly one JCS serialization, which any
RFC 8785 implementation reproduces.

A file holding an ACP document MAY end with one LF; that LF is never part of a preimage. The
reference implementation writes its JSON outputs without it, so that the digest of a file it
wrote is the digest of the document. JSON Lines is a sequence of JCS documents, each terminated
by an LF that belongs to no document.

YAML output is a presentation of the same data model. Digests are always computed over the JCS
serialization of the JSON data model, never over YAML bytes, and a YAML input is held to the
same number and depth constraints.

**Legacy serialization.** ACP 0.2 used a project canonicalization: sorted keys, compact
separators, UTF-8 and one trailing LF that *was* part of every preimage. For the values 0.2
documents contain it differs from JCS only by that LF, but every 0.2 digest and finding
identifier depends on it. A consumer MAY validate a 0.2 document by recomputing its digests that
way (§12); it MUST NOT produce new 0.2 documents.

## 9. Conformance

- A **producer** conforms when every declaration it emits satisfies §3.1 or §3.2 and it never
  emits a declaration for a change it did not author.
- A **collector** conforms when its output validates against the event schema and it satisfies
  §4 and §5.
- An **evaluator** conforms when it passes a named `suiteRevision` of the ACP evaluator
  conformance corpus (`acp-evaluator-conformance`) at a named release, and it satisfies §8. The
  corpus is published under [`conformance/`](../conformance/README.md) with a digest list signed
  at the release tag. Each specification version has its own suite revisions: for 0.3, revision 1
  was published at `v0.5.0`; for 0.4, revision 1 is published with the release that implements
  it. Later revisions add vectors or correct expected results, never change a released one. A claim names all three, for
  example "`acp-evaluator-conformance` revision 1 at `v0.5.0`", and is backed by the report the
  corpus harness writes.

The corpus runs any evaluator as an external verifier: `<cmd> <vector-file>`, where the vector is
a JSON object holding normalized events and a policy. The exit status is `0` evaluated, `3`
invalid input, `4` collection incomplete, and the last line of stdout is one JSON object with
`verdict` (`evaluated`, `invalid` or `incomplete`), `codes` (every rule with outcome `fail`, or
the §6.6 codes of invalid input, compared as a set), `assessments` (every change, rule and
outcome, compared exactly for accept vectors) and optionally `manifestDigest`, the digest of the
manifest without `generated.tool` and `generated.version`, which must then match and so tests
§8.2 as well. Every reject vector is one mutation of an accept vector, so a
verifier that rejects everything fails. Passing shows that an implementation produced the
expected outputs for these inputs; the corpus is a public answer key, not a proof that the rules
are implemented, and runs that disagree with it are the most useful reports.

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
meaning of an existing rule, identifier or format increment the major version; before 1.0 they
increment the minor version instead, name the documents they invalidate, and define how the
previous version's documents are still validated. Rule identifiers are never reused. Exports
and policies written under earlier versions remain valid input (`version` accepts `0.1` to
`0.4`). A 0.4 consumer validates 0.3 and 0.2 manifests, and `v0.3` and `v0.2` Statements, as the
version that wrote them: its rule catalogue, its policy shape and, for 0.2, the legacy
serialization of §8.2. The provenance (§3.2) and review (§3.7) documents are versioned on their
own and stay at 0.1.

The schemas are published at version-pinned identifiers,
`https://noru.tech/spec/ai-change-provenance/v0.4/<name>.schema.json`, and the provenance and
review schemas under `provenance/v0.1/` and `review/v0.1/`. A consumer that reads manifests
across versions can rely on these keeping their name and meaning in every later version unless
the changelog names the change as breaking: a finding's `id`, `rule_id`, `change_id`, `severity`
and `disposition`; an assessment's `change_id`, `rule_id` and `status`; the `summary` counts; and
`generated.source_digest`. A later version adds members, rules and vocabulary values; a consumer
that validates against a version's schema validates against that version's, never a later one's.

## Changelog

- **0.4 (2026-10-14)** — required checks and protection bypass: the collection of check runs,
  commit statuses and the forge's required checks (§4.1) and of ruleset evaluations of the merge
  (§4.2) for merged changes; rules ACC004 and ACC005 (§6.2); the `checks` policy block (§6.3);
  and validation of both kinds of fact (§6.6). Risk tiers: the change-risk assessment as input
  (§3.8), reviewer teams as observed at collection (§4.3), the `tiers`, `missing_tier` and
  `minimum_risk_evidence` policy keys with approval counts, per-tier agent review and the
  emergency's retroactive-review obligation (§6.7, §6.4), and rule ACC011 (§6.2), enabled by
  default and inert without tiers. `applied_tiers` and finding obligations in the manifest (§7.1). Both rules are disabled by default; when one is
  enabled, its missing or incomplete facts make collection incomplete. ACC005 never passes on an
  empty record. `legacy_ids` is removed (§7.1), as 0.3 scheduled. Re-validation ignores
  `generated.tool` and `generated.version` (§7.1). Predicate type `v0.4` (§7.3). Schemas move to
  version-pinned identifiers, and §12 names the members a consumer can rely on across versions.
  Every document written under 0.3 stays valid input and validates as 0.3; 0.3 manifests and
  Statements are not rewritten. GitLab: the `gitlab` forge, approvals bound to a commit from the
  forge's head history as derived evidence (§4 rule 3), and how a GitLab collector records
  identity, approvals, checks (`pipeline` and `job` results, the `project_settings` source),
  bypass and reviewer teams (§4.4); team identifiers may be a group path of more than two
  segments. Branch updates and deployments: the `branches` and `deployments` facts (§4.5, §4.6),
  the `branches` and `deployments` policy blocks (§6.3), and rules ACC012 and ACC013 (§6.2), whose
  subjects are a branch and a deployment; both disabled by default.

- **0.3, revision 3 (2026-09-29)** — §9 no longer names a single suite revision for this
  version: revision 1 was published at `v0.5.0`, and later revisions (revision 2 adds vectors
  for the `instructions` dimension of §6.5) only add vectors or correct expected results.
- **0.3, revision 2 (2026-09-29)** — editorial: sections written under 0.1 now say "this
  version" (§3.2, §3.5), and §3.7 names the rules that read agent reviewer facts. No normative
  change.
- **0.3 (2026-09-29)** — serialization is RFC 8785 (§8.2), replacing the project canonicalization;
  I-JSON input constraints with codes ACV005 to ACV009; normalization and serialization split into
  §8.1 and §8.2. Validation codes listed in the specification (§6.6), with ACV010 for input that
  does not conform to its schema. Evaluator conformance is defined by a published, versioned corpus
  under an external-verifier contract (§9). Breaking for digests, not for rules: the trailing
  newline leaves every preimage, so every finding identifier, `source_digest` and `sha256` subject
  digest changes, while every finding, assessment and verdict stays the same. Findings carry
  `legacy_ids` for one minor version (§7.1); the predicate type is `v0.3` (§7.3); 0.2 documents stay
  valid through the legacy path (§8.2, §12). Exports and resolved policies are written as `0.3`.
- **0.2 (2026-09-21)** — agent reviewers: the second form of qualifying approval under the
  opt-in `agent_review` policy (§6.1, §6.3), the independence dimensions (§6.5), rules ACC007
  to ACC010 (§6.2), predicate type `v0.2` (§7.3). Version compatibility rules (§12). Defaults
  are unchanged: without `agent_review`, an agent approval never qualifies, and ACC007 and
  ACC008 are observations.
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
