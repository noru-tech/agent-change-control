# Agent authorship

Technical identity and independent human judgment are different relationships. The forge opener, commit authors, effective author and human operator are stored separately.

Phase 1 accepts an exact `agent-change-control` fenced YAML/JSON declaration in the PR description, or explicit `--agent-account LOGIN=AGENT` mappings supplied by a caller who verified the account. Account mappings are a caller trust boundary, not an automatically verified account directory. Their provenance is labeled declared. An API reference to the observed opener is also retained. Conflicting declarations/mappings and malformed structured declarations are errors, never silently ignored.

Example PR body block:

````text
```agent-change-control
author: codex
operator: github:alice
```
````

Only matching GitHub human identities resolve an operator. The exporter can GET `/users/alice` to resolve a declared operator not otherwise present in the PR. An email alone does not resolve to a GitHub account. Unknown operators stay unknown. The merger, PR creator or first reviewer is never implicitly substituted as agent operator.

Evidence records `source`, `ref`, and `kind` (observed, derived, declared, signed). API observations and user declarations are distinguishable even though both have source links. Empty evidence arrays are invalid for important observations. Known-account recognition retains both the mapping and observed PR evidence.

The separately published provenance schema defines tool-neutral agent/version, operator ID, session and change base/head fields. The library can validate its binding to an explicit head SHA. File discovery, signed assertions, app installation verification and email identity maps are deferred. No LLM, wording/style classifier or blanket bot-to-agent conversion is used.

## Signed tier

The provenance document can be carried as the predicate of an in-toto Statement
(`https://noru.tech/spec/ai-change-provenance/provenance/v0.1`, subject: the head commit) inside
a DSSE envelope or a Sigstore bundle, signed by the agent's integration or the operator. `acc`
reads such files with `--attestations PATH` (files or directories, `.json` or `.jsonl`), binds
each to the change whose head it names, and requires the subject and the predicate's
`change.head_commit` to agree. It sits at the explicit tier with the inline declaration: both
may be present, they must name the same agent and operator, and their evidence is merged.

`acc` does not verify signatures. Verify with the signer's tooling first, then say so with
`--verified-by TEXT`; the text is recorded verbatim in the manifest's `attestations` record next
to the file, the predicate type and the payload digest. Only an attestation that carried a
signature *and* has a recorded verifier yields `signed` evidence; without `--verified-by` its
claims are `declared`, the same as a provenance document found on disk. A policy can require
`signed` authorship evidence with `minimum_authorship_evidence` ([policy](policy.md)).

Reviews have their own document ([spec §3.7](../spec/ai-change-provenance.md#37-review-document),
predicate type `https://noru.tech/spec/ai-change-provenance/review/v0.1`), which names the
reviewer in the predicate because the predicates in circulation put the reviewer in the
signature. `--attestations` reads it too, and `--verification PATH` reads the JSON that
`gh attestation verify --format json` writes, loading its bundles as signed attestations with the
signer the verifier established. A review document upgrades the forge's review (its evidence
becomes `signed`, which is what `minimum_review_evidence: signed` needs) and never creates one.
For an agent reviewer it also records the operator, instructions owner, model and signing
identity on the review; no rule reads those facts yet. See [signing](signing.md).

## Agent reviewers

A reviewer account mapped with `--agent-account` is an agent and its approval never counts as a
human's. Agent actors carry a vendor from a built-in registry extended by
`--agent-vendor AGENT=VENDOR`; unregistered agents have none. The facts a review document states
about an agent reviewer are recorded on the review as `agent` (operator, identity,
instructions owner, model). Under the opt-in `agent_review` policy an agent approval can satisfy
independence when it is signed and independent of the effective author on the required
dimensions; see [policy](policy.md#agent-reviewers) and spec §6.5. Without it, ACC007 records
the approval and ACC008 catches a model checking its own vendor's work.

## Derived tier

When no declaration or account mapping applies, two kinds of record written at authoring time establish agent authorship with `derived` confidence and `derived` evidence:

- **Vendor `Co-Authored-By` trailers** in git's trailer block (the final paragraph, every line a `Token: value`). The built-in registry maps `noreply@anthropic.com` to `claude-code` and `copilot@users.noreply.github.com` to `copilot`; `--agent-trailer EMAIL=AGENT` extends it and `--ignore-trailers` disables the tier. Display names are never matched. Evidence source: `commit_trailer`, referencing the commit.
- **Agent Trace records** (`--agent-trace PATH`, files or directories) whose `vcs.revision` is one of the change's commits and whose contributors include `ai` or `mixed` ranges; the agent is the record's `tool.name`. Records without a revision or a tool name are ignored. Evidence source: `agent_trace`, referencing `file#id`.

Precedence is strict: an attestation, a declaration or a mapping wins and derived records are then not consulted; derived records naming two different agents for one change are not interpreted. The operator is derived only when a single human account authored every commit of the change; otherwise it stays unknown (ACC006), as does anything authored by a bot account. The table output marks such operators `(derived)`.
