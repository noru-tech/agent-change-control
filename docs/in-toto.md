# Predicate type: AI Change Provenance

Type URI: `https://noru.tech/spec/ai-change-provenance/v0.1`

Version: 0.1 (revision 3)

This page describes the predicate `acc` emits inside an
[in-toto Statement v1](https://github.com/in-toto/attestation/blob/main/spec/v1/statement.md),
following the structure in-toto asks of a new predicate type, so that the same text can be
proposed upstream once the predicate has run in production outside this repository. The normative
definitions are in the [specification](../spec/ai-change-provenance.md); this page is the
attestation view of them.

## Purpose

Record, for a software change, who its effective author was (a human, or a coding agent and the
human who operated it), who reviewed and merged it, and whether a human independent of the
effective author approved the current head before merge. The predicate is the evaluated evidence
of a separation-of-duties control, with every finding traceable to the platform records it was
derived from, and it can be re-evaluated offline to confirm the findings follow from the facts.

Existing predicates cover adjacent ground: SLSA provenance describes how an artifact was built,
the SLSA source track and the proposed `source-review-coverage` predicate describe review of a
source revision, and agent-decision and AI-authorship proposals describe what an agent did. None
of them model the agent, its operator and an independent reviewer as separate actors with
independence rules between them, which is the question a change-control auditor asks.

## Use Cases

- **Merge gate.** A CI job produces the Statement for the pull request's head; a policy engine
  refuses to merge or deploy a head without a Statement whose findings are clean.
- **Release evidence.** The Statement for each change in a release is signed and stored next to
  the release's build provenance, so the change-control evidence and the build provenance share a
  subject (the merge commit) and travel together.
- **Audit population.** The JSON Lines form over a reporting window is the population an auditor
  samples from, with the independence question answered per item and gaps marked as incomplete.

## Prerequisites

- Familiarity with the in-toto Statement v1 layer and DSSE.
- The AI Change Provenance [model](../spec/ai-change-provenance.md#2-terminology): change,
  actor, effective author, effective human, operator, qualifying approval, evidence, complete.

## Model

The subject is the change's commit or commits. The predicate is a manifest: the normalized facts
about one or more changes, the policy they were evaluated under, the findings and per-rule
assessments, a summary, and generation metadata with a digest over the facts.

```text
Statement
├── subject: for each change
│   ├── <change id>        gitCommit = head commit (what the approvals are bound to)
│   └── <change id>:merge  gitCommit = merge commit (when merged, known and distinct)
├── predicateType: https://noru.tech/spec/ai-change-provenance/v0.1
└── predicate (manifest)
    ├── events      repository, window, actors, changes (facts with evidence references)
    ├── policy      the resolved rules, severities and threshold
    ├── findings    one per failed rule per change, stable identifier, disposition
    ├── assessments pass | fail | unknown | not_applicable per rule per change
    ├── summary     counts: clean, with findings, indeterminate
    └── generated   tool, version, source digest over the canonical events
```

Two shapes exist. A single Statement covers every change in the manifest. The JSON Lines shape
has one Statement per change, each with a manifest covering that change alone (and only the
actors it refers to) as its predicate; finding identifiers are the same in both shapes.

The producer is the evaluator, `acc`. It observes the forge through read-only API calls, reads
explicit agent declarations, evaluates the rules offline, and emits the Statement unsigned. The
signer is whoever runs it, typically a CI workflow identity or an organization key.

## Schema

The Statement shape is [`schemas/statement.schema.json`](../schemas/statement.schema.json), a
strict subset of Statement v1 (subjects carry a name and a digest set only). The predicate is
[`schemas/manifest.schema.json`](../schemas/manifest.schema.json). Both are JSON Schema 2020-12
and are embedded in the `acc` binary.

```json
{
  "_type": "https://in-toto.io/Statement/v1",
  "subject": [
    {"name": "github:acme/api:pr:421", "digest": {"gitCommit": "<head sha>"}},
    {"name": "github:acme/api:pr:421:merge", "digest": {"gitCommit": "<merge sha>"}}
  ],
  "predicateType": "https://noru.tech/spec/ai-change-provenance/v0.1",
  "predicate": {
    "version": "0.1",
    "events": { "...": "repository, window, actors, changes" },
    "policy": { "...": "resolved policy" },
    "summary": { "...": "counts" },
    "findings": [ "..." ],
    "assessments": [ "..." ],
    "generated": {
      "tool": "agent-change-control",
      "version": "<acc version>",
      "source_digest": "sha256:<hex>"
    }
  }
}
```

### Parsing rules

- The Statement is canonical JSON: sorted keys, compact separators, UTF-8, one trailing newline.
  The bytes are what gets signed; do not re-serialize before signing.
- A consumer MUST check that the subjects are exactly those the predicate's changes produce (head
  commit, then merge commit when present and distinct), or that the Statement has a single subject
  whose `sha256` digest is over the canonical bytes of the predicate (the form SHA-2-only signers
  such as GitHub artifact attestations produce; see [signing](signing.md)). It SHOULD re-evaluate
  the predicate: the embedded events under the embedded policy must reproduce the findings,
  assessments and summary byte for byte. `acc validate` performs both checks.
- A consumer MUST treat `events.window.complete: false` or any change with
  `reviews_complete: false` as an incomplete record that cannot be read as clean, whatever the
  findings say.
- Timestamps are UTC (RFC 3339 with `Z`). No timestamp in the predicate is the time of rendering;
  all of them are source facts.
- Unknown keys are rejected on input, with one exception: `merge_commit_sha` may be absent in
  exports written before it existed and then means null.

### Fields

Predicate fields, by section. Exact types are in the manifest schema; the meaning of each fact is
in the specification section cited.

| Field | Meaning |
| --- | --- |
| `version` | Predicate and schema version, `0.1`. |
| `events.repository` | The forge repository, `OWNER/REPO`. |
| `events.window` | Inclusive UTC window of merge times the changes were selected from, whether collection was complete, and why not. Spec §4. |
| `events.actors` | Actor registry keyed by `namespace:name`: `kind` (`human`, `agent`, `bot`, `service`, `unknown`) and display name. Spec §2, §5. |
| `events.changes[].id`, `.repository`, `.forge`, `.title`, `.url` | Change identity. |
| `events.changes[].opened_at`, `.merged_at` | Source timestamps. |
| `events.changes[].head_sha` | The current head commit; the subject. |
| `events.changes[].merge_commit_sha` | The commit the merge produced, when merged and reported; the second subject. |
| `events.changes[].commits[]` | Commits with their authors, when the forge exposes them. |
| `events.changes[].forge_author` | The account that opened the change. |
| `events.changes[].author` | The effective author: a human, or an agent when agent authorship is established. Spec §3. |
| `events.changes[].agent_operator` | The human who operated the agent, with confidence `explicit`, `derived` or `unknown`. Spec §3. |
| `events.changes[].reviews[]` | The complete review history: actor, state, time, the commit reviewed, evidence. Spec §4. |
| `events.changes[].reviews_complete` | Whether the review history was fully retrieved. |
| `events.changes[].merger` | Who merged, when merged. |
| `*.provenance[]` | Evidence references: `source`, `ref`, and `kind` (`observed`, `derived`, `declared`). Spec §2. |
| `policy` | The resolved policy: `fail_on` threshold and, per rule, `enabled` and `severity`. Spec §6.3. |
| `findings[]` | One per failing rule per change: stable `id`, `rule_id`, `rule`, `severity`, `change_id`, `actor_ids`, `explanation`, `provenance`, `disposition`. Spec §6.2, §6.4, §7.1. |
| `assessments[]` | Per rule per change: `pass`, `fail`, `unknown` or `not_applicable`, with a reason. Spec §6. |
| `summary` | Counts of changes, human- and agent-authored changes, unknown operators, clean, with findings, indeterminate, and findings. |
| `generated` | `tool`, `version`, and `source_digest`, SHA-256 over the canonical events. |

## Example

The repository's fixtures are attested in tests. `tests/fixtures/claude-operator-self-approved/expected.intoto.json`
is a merged agent change whose operator approved it: two subjects, three findings (ACC001,
ACC002, ACC003). `tests/fixtures/claude-clean/expected.intoto.json` is the same shape with an
independent approval and no findings. The JSON Lines form of each is next to it as
`expected.intoto.jsonl`. Produce your own with:

```bash
acc evaluate tests/fixtures/claude-clean/events.json -o clean.intoto.json
acc validate clean.intoto.json
```

## Changelog and Migrations

- **0.1 revision 3** — second subject for the merge commit; JSON Lines form; verifier subject
  check; Statement schema. Statements produced by earlier revisions (head subject only) remain
  valid under this revision when the predicate has no merge commit.
- **0.1** — initial: head-commit subjects, manifest as predicate.
