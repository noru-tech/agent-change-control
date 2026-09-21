# Agent reviewers: independence without a human in the loop

Status: design for review, no code. Phase 4 of the in-toto plan
([in-toto-integration.md](in-toto-integration.md)), building on the ROADMAP's "AI reviewer
classification" sketch and closing the review-evidence question left open by Phase 3
(in-toto-integration.md §11). Everything here is opt-in: the default policy is unchanged, and an
agent approval never qualifies unless a policy says so.

## 1. What changes and what does not

Today a qualifying approval must come from a human other than the effective human. Teams already
let an AI reviewer approve some changes, and `acc` can only say "not a human": ACC001 or ACC003
fails, and the manifest cannot express that the approval was an agent's, whether that agent was
independent of the author, or on what evidence.

The four-eyes principle does not say the second pair of eyes must be human. It says the second
judgment must be **independent of the first**. For two humans, independence is a property of the
people. For an agent and an agent, it has to be decomposed into things that can be observed or
attested: who directed each, who built each, which key signed each, and who wrote the reviewer's
instructions. That decomposition is the design; the rules are small once it holds.

What does not change: `acc` reads no diff, calls no model, and infers nothing from account
names. An agent reviewer is an agent only because a caller mapped its account
(`--agent-account`), exactly as for authors. Unknown remains an answer. Incomplete input cannot
pass.

## 2. Model

### 2.1 Vendor on agent actors

An agent actor gains an optional `vendor`: the organization that builds and operates the model
behind the tool. A built-in registry maps tool names to vendors and callers extend it:

| Agent | Vendor |
| --- | --- |
| `claude-code`, `claude-code-review` | `anthropic` |
| `copilot`, `copilot-review` | `github` |
| `codex` | `openai` |
| `gemini-cli`, `gemini-code-assist` | `google` |
| `cursor` | `cursor` |

`--agent-vendor AGENT=VENDOR` extends it; an unregistered agent has `vendor: null`, which makes
the provider dimension `unknown`, never `independent`. Vendor is the granularity, not model family:
a tool's model changes under the same name and the forge never records it, so a finer claim would
be a guess. If a review attestation (§2.3) names a model, it is recorded as `model` on the review
but is not an independence input in this version.

### 2.2 Review-level agent facts

An agent's independence from the author can differ per review, so the facts live on the review,
not the actor. A review gains an optional `agent` object, present only when the reviewer is an
agent and only from a review attestation:

```json
{
  "operator": "github:carol",
  "identity": "https://github.com/acme/review-bot/.github/workflows/review.yml@refs/heads/main",
  "instructions_owner": "github:security-team",
  "model": "claude-opus-5"
}
```

Every field is nullable. `operator` is the human (or team) who directed the reviewing agent for
this review; `identity` is the signing identity the caller verified; `instructions_owner` is the
actor that controls what the reviewer was told to check. None of these can be observed from the
forge. They come only from a signed review attestation, or they are null.

### 2.3 The review attestation

Phase 3 found that the review predicates in circulation (`human-review`, gittuf's reference
authorization) carry the reviewer in the signature, which pre-verified input never sees. The
resolution is a predicate of our own that names the reviewer in the predicate, signed by the
reviewer's harness (or the human's tooling), and verified by the caller exactly like the
provenance attestation:

```json
{
  "_type": "https://in-toto.io/Statement/v1",
  "subject": [{"name": "github:acme/api:pr:421", "digest": {"gitCommit": "<head sha>"}}],
  "predicateType": "https://noru.tech/spec/ai-change-provenance/review/v0.1",
  "predicate": {
    "spec_version": "0.1",
    "reviewer": {"kind": "agent", "id": "github:acme-review[bot]", "agent": "claude-code-review"},
    "decision": "approved",
    "change": {"head_commit": "<head sha>"},
    "submitted_at": "2026-08-14T08:59:10Z",
    "operator": {"id": "github:carol"},
    "instructions": {"owner": "github:security-team", "digest": "sha256:<hex>"},
    "session": {"id": "rev_9a1c", "started_at": "2026-08-14T08:41:00Z"}
  }
}
```

Rules for reading it, mirroring the provenance attestation:

- Bound by subject and `change.head_commit`; they must agree.
- It **upgrades a forge review**: the collector matches it to an observed review by reviewer
  account, commit and decision. The matched review's evidence gains the attestation entry
  (`signed` under Phase 3's conditions, else `declared`) and, for an agent reviewer, the `agent`
  object of §2.2 is filled from the predicate. `identity` is the one exception: it is not a
  predicate field but comes from verification (§2.4).
- An attestation with no matching forge review is recorded in the `attestations` registry with a
  `matched: false` mark and produces no review. A review that exists only in an attestation is not
  a review the forge saw, and inventing one would let a signed file approve a change the platform
  never showed as approved.
- A human reviewer's attestation carries no `operator` or `instructions`. It makes the review's
  evidence `signed`, which is what unlocks `minimum_review_evidence: signed` for human reviews
  too. This is the Phase 3 gap closed for both kinds of reviewer at once.
- Conflicts (two attestations for the same review disagreeing on operator or instructions owner)
  are an error for the change.

### 2.4 Signing identity

`identity` is what the verifier established, not what the predicate says about itself. With
pre-verified input the caller has it and `acc` does not. Two ways to hand it over, one
recommended:

1. **Recommended:** `--verification PATH`, the JSON that `gh attestation verify --format json`
   (and `cosign verify-blob-attestation --output json`, to be confirmed) writes. It contains the
   verified bundle and the certificate's subject alternative name and issuer. `acc` reads the
   identity for the attestation whose payload digest matches and records it on the attestation
   entry as `signer`. The `--verified-by` text stays the human-readable statement.
2. `--attestations PATH=IDENTITY`, the caller typing the identity. Simple, but it is the caller
   asserting the very fact the dimension depends on, with nothing to cross-check.

The authorship side needs the same: the provenance attestation's signer becomes the author
agent's identity. Without both identities the identity dimension is `unknown`.

## 3. Independence dimensions

Each dimension is evaluated per (change, agent approval) as `independent`, `dependent` or
`unknown`. `unknown` on a required dimension never yields a clean result.

| Dimension | Independent when | Dependent when | Unknown when |
| --- | --- | --- | --- |
| `operator` | The review's `agent.operator` differs from the change's effective human (the author's operator) | They are the same actor | Either is null |
| `provider` | The reviewer agent's vendor differs from the author agent's vendor; for a human author, always independent | Same vendor | Either vendor is null |
| `identity` | The review attestation's verified signer differs from the provenance attestation's verified signer | Same signer | Either attestation, or either signer, is missing |
| `instructions` | The review's `agent.instructions_owner` is neither the effective human nor the author agent's operator | It is one of them | Null |

Notes on the choice of four:

- `operator` and `instructions` look alike but are not: the same human can direct a reviewer
  they did not configure (a platform team owns the review bot's rules), and a different human can
  run a reviewer whose instructions the author wrote. The second case is the agent-era version of
  "I asked my report to approve it", and only `instructions` catches it.
- `identity` is the in-toto notion: distinct functionaries with distinct keys. It is the one
  dimension a layout can enforce, and the only one that does not depend on a declared fact.
- `provider` is coarse by design (§2.1). It answers "did one model check its own work", which is
  ACC008's question, and nothing finer.
- A fifth candidate, "context", meaning the reviewer did not share the author's session, is
  subsumed: a shared session implies the same operator and the same instructions owner. Not
  proposed.

A dimension is only evaluated for agent approvals. For a human approval the existing rule (a
different human) is the whole test, unchanged.

## 4. Policy

```yaml
version: "0.1"
agent_review:
  satisfies_independence: false        # default: agent approvals never qualify
  require: [operator, provider, identity]
  minimum_evidence: signed              # a declared agent reviewer is never enough
  labels: []                            # optional forge labels that scope the mode
rules:
  agent_approval_recorded:        {enabled: true, severity: info}     # ACC007
  same_vendor_write_and_review:   {enabled: true, severity: high}     # ACC008
  agent_approval_not_independent: {enabled: true, severity: high}     # ACC009
  agent_approval_unsigned:        {enabled: true, severity: warning}  # ACC010
```

- `satisfies_independence: true` lets an agent approval qualify (§5) when every dimension in
  `require` is `independent` and its evidence reaches `minimum_evidence`. `minimum_evidence` may
  not be lowered below `signed` in this version; the key exists so the requirement is visible in
  the resolved policy, not so it can be relaxed.
- `require` may not be empty. The default requires the three dimensions that a signed review
  attestation plus the vendor registry can establish; `instructions` is opt-in because few
  producers can declare it today.
- `labels` scopes the mode to changes carrying one of the listed forge labels (a "low risk"
  label a team already uses). It needs a `labels` field on changes, collected from the pull
  request, which is a small additive schema change. Risk classification itself stays outside
  `acc`: the label is the team's decision, recorded as observed evidence.
- The assessment reason for ACC001 and ACC003 says when policy, not a human, made the call:
  "An agent independent of the effective author on [operator, provider, identity] approved the
  current head before merge, as permitted by policy." The manifest never reads as if a human
  approved.

## 5. Rules

Rule identifiers follow the reservation in in-toto-integration.md §3: ACC004 and ACC005 stay
reserved, ACC007 and ACC008 are the ROADMAP's, ACC009 and ACC010 are new.

| Rule | Fails when | Not applicable when | Unknown when |
| --- | --- | --- | --- |
| **ACC007** Agent approval recorded (`info`) | An agent's latest decision at or before merge is `approved` on the current head. Not a violation; makes the population visible. | No agent reviewed the head. | never |
| **ACC008** Same-vendor write and review (`high`) | The effective author is an agent, and every approval of the head is by agents of the author's vendor. | The author is human, or a human approved the head. | The author's or any approving agent's vendor is null. |
| **ACC009** Agent approval lacks required independence (`high`) | `satisfies_independence` is on, no human independent approval exists, and every agent approval of the head has some required dimension `dependent`. | The mode is off, or a human independent approval exists. | No agent approval is `dependent` on a required dimension but at least one is `unknown` on one. |
| **ACC010** Agent approval without signed identity (`warning`) | `satisfies_independence` is on and an agent approval of the head is being considered whose evidence is below `signed`. | The mode is off. | never |

How the existing rules change under the mode, and only under it:

- **Qualifying approval** (spec §6.1) gains a second form: an agent's latest decision is
  `approved` on the current head at or before merge, its evidence reaches `minimum_evidence`,
  and every dimension in `require` is `independent`. If instead some required dimension is
  `unknown` and no human independent approval exists, ACC001 and ACC003 are `unknown`, not
  `fail` and never `pass`.
- **ACC002** is unchanged: a self-approval by the effective human is a finding regardless of
  agents. A reviewing agent whose `operator` is the effective human is a `dependent` operator
  dimension, not ACC002; it is the author's tool approving the author's work, and ACC009 names it.
- With the mode off, ACC007 and ACC008 still evaluate (they are observations), and ACC009 and
  ACC010 are `not_applicable`. Existing fixtures and goldens do not change except for the four
  new assessment rows per change, which is the same shape of change ACC006 made.

Worked cases, all with an agent author operated by alice:

| Reviewers of the head | Mode | Result |
| --- | --- | --- |
| bob (human) | off | pass, as today |
| review-bot (declared, no attestation) | off | ACC001 fail, ACC007 info |
| review-bot, same vendor as author, signed | off | ACC001 fail, ACC007, ACC008 |
| review-bot, other vendor, signed, operator carol, distinct signer | on, require operator+provider+identity | pass with the policy reason; ACC007 |
| review-bot, other vendor, signed, operator alice | on | ACC009 fail (operator dependent); ACC001 fail |
| review-bot, other vendor, signed, no operator in predicate | on | ACC001 unknown (operator unknown); ACC009 unknown |
| review-bot, other vendor, declared only | on | ACC010; ACC001 fail (no qualifying approval) |
| review-bot, unregistered agent, signed | on | ACC001 unknown (provider unknown) |

## 6. Evidence and trust, restated

The mode stands or falls on the review attestation. Its claims are self-asserted by the
reviewing harness; the signature says which identity asserted them; the caller's verification
says the identity was one they trust; `acc` records all three layers separately (predicate
fields, `signer` from the verifier's output, `verified_by` from the caller) and never merges
them into a single "trusted". A policy author who reads a clean result under this mode can see
exactly which declared fact each dimension rested on.

This is weaker than a human's approval in one specific way: a human's independence is a fact
about the world that the forge account approximates; an agent's independence is a set of claims
about its deployment. The `instructions` dimension is where the weakness concentrates, because
the reviewer's prompt is the easiest thing for an author to influence and the hardest to prove
they did not. That is why it is `unknown` unless declared, and why the default does not require
it: requiring it would make every result unknown until producers can declare it, and not
requiring it means a clean result under the default says nothing about who wrote the reviewer's
instructions. The policy comment should say so. Bip to weigh (§9).

## 7. Stretch: in-toto layout export

`acc policy --format in-toto-layout` could render a layout fragment from a policy: a `write`
step whose functionaries are the author identities, a `review` step whose functionaries are the
reviewer identities, threshold 1 on `review`, and a rule that `review` materials equal `write`
products (the head commit). It maps `identity` and nothing else; the other dimensions have no
layout equivalent. Worth doing only when a user runs in-toto verification already. Not proposed
for the first implementation.

## 8. Implementation outline (after review)

1. Spec 0.2 (or 0.1 revision 5, §9): agent reviewers, the review predicate, the dimensions, the
   policy block, four rules. The review predicate schema `schemas/review.schema.json`.
2. Model and schemas: `Actor.vendor`, `Review.agent`, `Change.labels`, `Attestation.signer` and
   `Attestation.matched`; all optional on input.
3. Collector: vendor registry and `--agent-vendor`; labels; review attestation matching;
   `--verification` reader for `gh attestation verify --format json`.
4. Rules: the four rules, the second qualifying form, the reasons; `policy.agent_review`.
5. Fixtures for every row of the worked-cases table, goldens, snapshots, docs (`docs/policy.md`,
   `docs/agent-authorship.md` gains "agent reviewers", `docs/signing.md` gains "signing a
   review"), README rules table, CHANGELOG.

Two pull requests: model, collector and attestations first (no rule change, goldens gain
nothing), then rules and policy.

## 9. Decisions for Bip

1. **The four dimensions.** Keep `operator`, `provider`, `identity`, `instructions` as defined in
   §3, with `provider` at vendor granularity? Drop or add any?
2. **Default `require`.** `[operator, provider, identity]` as proposed, leaving `instructions`
   opt-in with the caveat in §6? Or require all four and accept that the mode yields `unknown`
   until producers declare instruction ownership?
3. **The review predicate.** Own predicate type
   `https://noru.tech/spec/ai-change-provenance/review/v0.1` as in §2.3, upgrading forge reviews
   only (never creating them)? This is the Phase 3 option 1 and it also gives humans signed
   review evidence.
4. **Signer identity.** Read it from the verifier's JSON output (`--verification`) as
   recommended in §2.4, or accept a typed identity?
5. **ACC008 with the mode off.** Evaluate it always, as an observation (proposed), or only under
   the mode?
6. **Labels.** Collect pull request labels and support `agent_review.labels` in the first
   version, or defer scoping?
7. **Versioning.** Spec 0.2 for this (new actor facts, new predicate, new rules, a second form of
   qualifying approval under opt-in policy), or 0.1 revision 5 on the grounds that defaults are
   unchanged? I lean 0.2: it is the first time the meaning of "qualifying approval" has a policy
   switch, and a reader of the spec should meet it as a version, not a revision.
8. **Rule identifiers.** ACC007 to ACC010 as in §5.
