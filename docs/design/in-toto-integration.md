# in-toto integration: Phase 0 findings

Status: orientation complete, nothing implemented. This document records what the repository
already does, where the `acc x in-toto` implementation plan is out of date, and the decisions that
need Bip's answer before Phase 1 starts. Each later phase appends its own section.

## 1. The plan is one release behind the repository

The plan describes the crate as `0.1.0` with an unsigned manifest and no attestation output. The
repository is at **0.3.1**, and `--format in-toto` shipped in 0.2.0 (commit f0fd619, CHANGELOG
0.2.0). What exists today:

| Plan item | State in 0.3.1 |
| --- | --- |
| `--format intoto` on every evaluated command | Exists as `--format in-toto` on `scan`, `evaluate`, `check` and `pr` (`src/output/mod.rs`, `src/output/intoto/mod.rs`). The GitHub Action accepts `format: in-toto`. |
| Statement v1 envelope | Emitted, unsigned, in canonical JSON. `_type` is `https://in-toto.io/Statement/v1`. |
| Predicate type | `https://noru.tech/spec/ai-change-provenance/v0.1`, specified in `spec/ai-change-provenance.md` §7.3. CONTRIBUTING ties the predicate type version to the spec and schema version. |
| Predicate shape | The whole manifest (events, policy, summary, findings, assessments, generation metadata). |
| Subject | One entry per change. `name` is the change ID (`github:acme/api:pr:421`), `digest` is `{"gitCommit": <head sha>}`. |
| One Statement per change, JSONL | Not implemented. One Statement per manifest with N subjects. |
| `.intoto.json` extension inference | Not implemented. `Format::from_extension` sees only the last extension (`json`). |
| `validate` accepts a Statement | Not implemented. `validate` reads manifests only. |
| Predicate JSON Schema | None. The predicate validates as a manifest through `schemas/manifest.schema.json`. |
| Signing docs and workflow | Not implemented. README and `docs/github-action.md` say "sign it with the DSSE signer you already use". |
| Tests | `tests/cli.rs` round-trips the predicate through `manifest::validate`, and insta snapshots exist for three fixtures (`tests/snapshots/cli__intoto_*.snap`). |

Consequence: Phase 1 as written would add a second, incompatible in-toto format next to a
published one. Phase 1 should instead evolve the shipped format, and every change to it is a
specification change under CONTRIBUTING ("Changing the specification").

## 2. Source map

| Concern | Where |
| --- | --- |
| Format selection and extension inference | `src/output/mod.rs` (`Format`, `from_extension`), `src/cli/io.rs` (`OutputArgs::format`, `render`) |
| Renderers | `src/output/{json,yaml,table,sarif,intoto}/mod.rs` |
| Public model | `src/model/mod.rs`, mirrored by `schemas/*.schema.json`; every struct is `deny_unknown_fields`, every closed vocabulary an enum |
| Evidence | `model::Evidence { source, ref, kind }`, `kind` in `declared`, `derived`, `observed`; `Operator.confidence` in `explicit`, `derived`, `unknown`. Sources in use: `github_api`, `pr_metadata`, `known_agent_account`, `commit_trailer`, `agent_trace` |
| Rules | `src/rules/mod.rs` (`Facts`, `assess`), catalogue and defaults in `src/policy/mod.rs` |
| Canonical JSON, digests, timeline validation | `src/normalize/mod.rs` (`canonical`, `digest`, `events`, `schema`) |
| Manifest generation, tamper check, policy check | `src/manifest/mod.rs` (`evaluate`, `validate`, `check`) |
| Source digest | `Generated.source_digest` is `sha256:` over the canonical normalized events |
| Finding IDs | `acc-` plus 16 hex of SHA-256 over `[repository, change_id, rule_id, sorted_actor_ids]` |
| Subcommands | `src/cli/{scan,export,evaluate,validate,check,pr}.rs`; `evaluate` defaults to JSON, `scan` to YAML, `check` and `pr` to table |
| Goldens | `tests/fixtures/*/expected-manifest.json` and `expected-findings.json`, refreshed only with `UPDATE_GOLDENS=1`; rendered formats under insta snapshots |

No code path reads a clock: `grep now()` finds nothing under `src/`. Determinism holds for the
Statement today, and Phase 1 must not introduce a render-time timestamp.

## 3. Rule identifiers

| ID | State |
| --- | --- |
| ACC001, ACC002, ACC003, ACC006 | Implemented (`RuleId` enum, `RULES`, both schemas) |
| ACC004, ACC005 | Reserved for deployment and bypass rules (spec §6.2, ROADMAP "Planned") |
| ACC007, ACC008 | Proposed in ROADMAP "Next" for AI reviewer classification: ACC007 agent approval recorded (`info`), ACC008 same-vendor write and review (`high`) |
| ACV001, ACV003, ACV004 | Implemented integrity errors (`src/normalize/mod.rs`) |
| ACV002 | Reserved for deployment validation (`docs/model.md`) |

The plan's Phase 4 proposes new IDs for "agent approval lacks required independence" and "agent
approval without signed identity". Those overlap with the ROADMAP's ACC007 and ACC008 design, which
also names the policy key (`agent_review.satisfies_independence`) and a vendor registry. Phase 4
should start from the ROADMAP sketch and, if two more rules are needed, take **ACC009 and ACC010**.
Identifiers are never recycled (CONTRIBUTING).

## 4. Rust bindings

Checked on crates.io on 2026-09-21:

| Crate | Version | Licence | Notes |
| --- | --- | --- | --- |
| `in_toto_attestation` (in-toto/attestation `rust/`) | 0.1.0 published 2025-08-28; 0.2.0 in the repository, unpublished | Apache-2.0, allowed by `deny.toml` | Protobuf-generated types: depends on `protobuf`, `protobuf-json-mapping`, build-time `protobuf-codegen` and `regex`. Serialization goes through protobuf JSON mapping, not serde, so it cannot feed `normalize::canonical` directly. |
| `in-toto` (in-toto-rs) | 0.4.0, 2024-12 | MIT | The layout and link library (`ring`, `pem`, `walkdir`). Not the attestation framework. Only relevant if Phase 4's layout export is built. |

Decision: **keep plain serde structs**, as the shipped renderer already does. The Statement layer
is four fields, the predicate is our own type, and the bindings would add a protobuf toolchain to
a crate that is otherwise pure Rust with static musl builds. Revisit only if in-toto ships a
serde-based crate.

The upstream repository publishes the Statement layer as protobuf and prose, not as a JSON Schema.
Phase 1 therefore needs a small `statement` schema of our own for `validate`, and the acceptance
criterion "validates against the in-toto Statement v1 schema" means that schema.

Upstream also has `spec/predicates/template` for new predicates, which `docs/in-toto.md` should
follow, and no `human-review` predicate: the URI `https://in-toto.io/attestation/human-review/v0.1`
in Phase 3 comes from a 2023 SLSA blog example and is not a registered predicate type.

## 5. Deviations from the plan, with recommendations

### 5.1 Predicate type URI

The plan proposes `https://noru.tech/attestation/change-control/v0.1`. The repository already
publishes `https://noru.tech/spec/ai-change-provenance/v0.1`, versioned with the specification,
and 0.2.0 and 0.3.x attestations carry it. Changing the URI orphans them and breaks the "spec,
schemas and predicate type share a version" rule.

Recommendation: keep the `noru.tech/spec/ai-change-provenance/` family. Bump the version segment
only when the predicate shape changes (see 5.2). Bip still needs to decide where the URI resolves;
today it resolves nowhere.

### 5.2 Predicate shape: manifest, not a slim record

The plan's draft predicate (`change`, `actors`, `approvals`, `findings`, `collection`,
`evaluator`, `sourceDigest`) is a projection of the manifest. It cannot satisfy the plan's own
round-trip requirement ("`validate` recomputes findings from embedded facts"): recomputation needs
the full review history, the actor registry and the resolved policy, which the draft omits.
Anything that can be recomputed is the manifest.

Recommendation: the predicate stays the manifest. The design rule "reuse the manifest's actor,
approval and finding structures, do not create a second model" is then satisfied trivially. Two
consequences the plan did not anticipate:

- Field names stay `snake_case`. in-toto predicates may choose either; SLSA uses camelCase, but a
  camelCase mirror of the manifest would be the second model the plan forbids.
- The predicate carries everything the manifest carries: display names, titles, evidence URLs,
  the full review history. See 5.7 on privacy.

If Bip prefers a slim predicate anyway, it must embed the events and policy, which makes it the
manifest with extra top-level keys, and it is a format change that bumps the predicate version.

### 5.3 Subject: head commit today, merge commit needs a model change

Today the subject digest is the head SHA, which is the commit the approvals are bound to and the
commit the rules evaluate. The model has no merge commit SHA. GitHub's pull request object exposes
`merge_commit_sha`, but the collector does not record it, the schemas reject unknown fields, and
adding it touches `change-events.schema.json`, `manifest.schema.json`, the model, the collector,
every fixture and every golden.

The plan's argument for the merge commit is real: after a squash or rebase merge the head SHA never
appears in the target branch's history, so a verifier walking `main` cannot find the attestation
by digest. The argument for the head commit is equally real: it is what was reviewed.

Recommendation: add `merge_commit_sha` (nullable) to the change model as an observed fact, and for
a merged change emit **two subject entries**, one for the head commit and one for the merge commit,
each with a `gitCommit` digest. A `DigestSet` holds several digests of one artifact, so two
different commits cannot share one entry. Open changes keep a single head subject. If the merge
commit is unavailable, only the head subject is emitted and the predicate shows `merge_commit_sha:
null`; nothing is invented. The subject `name` stays the change ID rather than
`git+https://github.com/OWNER/REPO`: the change ID is unique per subject entry, and the resource
URI form fits a per-repository attestation (SLSA source track) rather than a per-change one. Bip to
confirm.

### 5.4 One Statement per change and JSONL

The shipped format is one Statement per manifest with N subjects, which matches a window scan and
the per-window summary. The plan wants one Statement per change so that a signer and a verifier
handle one change at a time. Both are useful, and a per-change Statement is a per-change manifest:
evaluating the events with one change yields a valid manifest whose finding IDs are unchanged,
because IDs depend only on repository, change, rule and actors.

Recommendation: keep `--format in-toto` as it is (no change to existing users) and add
`--format in-toto-jsonl`, one unsigned Statement per line, one per change, inferred from a
`.intoto.jsonl` file name; infer `--format in-toto` from `.intoto.json`. Two notes against the
plan text: JSONL lines sort by change ID like everything else in the manifest, which is
lexicographic (`pr:10` before `pr:9`), not by PR number, so that the ordering rule stays the one the
spec already has; and the in-toto "bundle" convention is a JSONL of DSSE envelopes, so an unsigned
JSONL of Statements is bundle-shaped but not a bundle until signed. The window summary is lost per
line and stays in the manifest.

### 5.5 `validate` on Statements

Feasible without new dependencies: detect `_type` and `predicateType`, validate against the new
`statement` schema, check that every subject digest is the head or merge commit of a change in the
predicate, then run `manifest::validate` on the predicate. JSONL input validates each line and
requires the change IDs to be unique and sorted. `check` on a Statement is out of scope for Phase 1.

### 5.6 Evidence strength

The plan's `declared < mapped < signed` does not match the model. Account mappings are recorded as
`declared` evidence with source `known_agent_account` (spec §3.3), and the derived tier below
declarations (trailers, Agent Trace) already exists as `derived`. The ordering that fits the model
is `derived < declared < signed`, adding `signed` to `EvidenceKind` (it sorts after `observed`, so
the enum's string order is preserved) and keeping `mapped` as a `source`, not a kind. That is a
Phase 3 schema change. Phase 1 adds no evidence kinds.

### 5.7 Privacy

Because the predicate is the manifest, an attestation contains actor IDs, display names, PR titles
and the review history. A DSSE envelope published through Sigstore lands in Rekor, which is public
and permanent. This must be stated in `docs/privacy.md` and in `docs/signing.md` before Phase 2
recommends a public transparency log. The ROADMAP already plans a pseudonymization mode; the plan's
`--redact-actors` is that mode. Two constraints for whoever builds it: finding IDs hash actor IDs,
so a pseudonymized manifest has different finding IDs (docs/privacy.md already says to re-evaluate
after renaming rather than edit hashes), and display names, titles and evidence refs that contain
account names must be redacted with the IDs.

Recommendation: keep redaction out of Phase 1, but make the Phase 2 workflow default to a private
store (the GitHub attestations API, which is scoped to the repository) and treat Rekor publication
as an explicit opt-in.

### 5.8 Phase 2 blockers found early

- `actions/attest` accepts subjects only as SHA-2 digests (`sha224`, `sha256`, `sha384`, `sha512`,
  `sha512_224`, `sha512_256` in `src/subject.ts`) and `gh attestation verify` looks an artifact up
  by its sha256. A `gitCommit` subject cannot go through them at all. The plan's "fallback" (sha256
  of the canonical change record, commit recorded in the predicate) is the only path for
  `actions/attest`. Phase 2 should define that artifact precisely: the canonical bytes of the
  per-change Statement's predicate are the natural candidate, since they are already deterministic.
- sigstore/sigstore-python#1018 was closed on 2024-05-21 in favour of #982; whether non-SHA-2
  subject digests are accepted now was not checked and belongs in Phase 2.
- The action already renders `in-toto` via `acc check MANIFEST --format in-toto`, so a
  `--predicate-only` flag is unnecessary: the predicate is the manifest, and `acc check --format
  json` writes it.

### 5.9 Versioning

Phase 1 cannot be `0.2.0`. With 0.3.1 released, Phase 1 and 2 are **0.4.0**, Phase 3 **0.5.0**,
Phase 4 **0.6.0**, unless Bip wants Phase 3 to be the 1.0 candidate. Every release changes
`generated.version` in every golden (the manifest schema pins it to the crate version), so
"goldens stay byte-stable" holds within a version, not across releases; that is the existing
convention.

## 6. Invariants confirmed on `main` (26b5104)

`cargo clippy --all-targets --all-features -- -D warnings`, `cargo test` (36 unit, 27
integration and snapshot tests), `cargo deny check` and `typos` all pass. `cargo-insta` is not
installed on this machine; snapshot updates go through `INSTA_UPDATE` or an install.

## 7. Decisions needed before Phase 1

1. Predicate type URI: keep `https://noru.tech/spec/ai-change-provenance/v0.1` (recommended) or
   move to `https://noru.tech/attestation/change-control/`. Either way, where does it resolve?
2. Predicate shape: manifest (recommended, no format change) or slim record (format change,
   predicate `v0.2`).
3. Subject: add `merge_commit_sha` and emit head and merge subjects for merged changes
   (recommended), or head only as today.
4. JSONL: a separate `in-toto-jsonl` format (recommended) or replace the multi-subject Statement.
5. Subject ordering in JSONL: change ID order (recommended) or numeric PR order.
6. Whether Phase 1 also lands the pseudonymization mode, or Phase 2 documents the Rekor exposure
   and defaults to private storage.

## 8. Proposed Phase 1 scope, given the above

- `merge_commit_sha` in the model, schemas, collector, fixtures and goldens (spec 0.1 revision 3,
  additive).
- Head and merge subjects in the Statement; `in-toto-jsonl` format; `.intoto.json` and
  `.intoto.jsonl` inference.
- `schemas/statement.schema.json` (four fields) and `acc validate` on Statements and JSONL.
- `docs/in-toto.md` in the upstream predicate template structure, describing the manifest as a
  predicate.
- Goldens `expected.intoto.json` for `claude-operator-self-approved` and `claude-clean`, and tests
  for schema validation, ordering, round trip and byte stability.
- CHANGELOG, README, `docs/privacy.md` note on attestation contents.

Not in Phase 1: signing, evidence kinds, redaction, any rule change.
