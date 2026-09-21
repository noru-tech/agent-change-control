<p align="center">
  <img src="./docs/assets/agent-change-control-logo.png" alt="agent-change-control logo" width="480">
</p>

# agent-change-control

> **Enforcing the four-eyes principle for coding agents.**
>
> Different accounts do not necessarily represent independent humans. `acc` records the human
> behind an agent's change and checks for independent human approval. Offline, reproducible, no LLM.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![ci](https://github.com/noru-tech/agent-change-control/actions/workflows/ci.yml/badge.svg)](https://github.com/noru-tech/agent-change-control/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/agent-change-control.svg)](https://crates.io/crates/agent-change-control)
[![spec](https://img.shields.io/badge/spec-AI%20Change%20Provenance%200.2-informational)](./spec/ai-change-provenance.md)

```yaml
# .github/workflows/change-control.yml — gate every pull request on an independent human review
on:
  pull_request:
    types: [opened, synchronize, reopened, ready_for_review]
  pull_request_review:
    types: [submitted, dismissed]
jobs:
  acc:
    runs-on: ubuntu-latest
    steps:
      - uses: noru-tech/agent-change-control@v0.4.0
```

Change control assumes that the account which opened a change is the party that produced it.
Coding agents break that assumption. When an agent opens the pull request under its own account
and the engineer who directed it approves, the platform reports two different actors and the
control is silently gone: one human judgment, two logins. When nobody recorded who directed the
agent, no tool can tell whether the reviewer was independent at all.

`acc` enforces the four-eyes principle by checking more than whether the author and approver
accounts differ: **did a human
who is independent of the change's effective author approve its current head before merge, and
what is the evidence?** Agent authorship alone is never a finding. An unknown operator is reported
as unknown, never rounded to pass or fail. Incomplete collection can never produce a clean result.

Read the argument in [Enforcing the four-eyes principle for coding agents](docs/four-eyes.md).
The convention it implements is published as [AI Change Provenance 0.2](spec/ai-change-provenance.md).

## What is in this repository

| Piece | Where |
| --- | --- |
| **Specification** — AI Change Provenance 0.2: declarations, collection, evaluation, formats | [`spec/`](spec/ai-change-provenance.md) |
| **Schemas** — events, manifest, policy, provenance, review, in-toto statement (JSON Schema 2020-12) | [`schemas/`](schemas/) |
| **CLI** — `acc`: GitHub collector, offline evaluator, validator, policy check | [`src/`](src/) |
| **GitHub Action** — evaluate the current pull request, SARIF and job summary | [`action.yml`](action.yml), [docs](docs/github-action.md) |
| **Outputs** — table, JSON, YAML, SARIF 2.1.0, in-toto Statement v1 (one, or JSON Lines per change) | [`src/output/`](src/output/), [docs](docs/in-toto.md) |
| **Control mapping** — where the evidence lands in SOC 2, ISO 27001, PCI DSS, NIST | [docs](docs/control-mapping.md) |

## Install

```bash
brew install noru-tech/tap/acc            # macOS and Linux
cargo binstall agent-change-control       # prebuilt binary from GitHub Releases
cargo install agent-change-control --locked
```

Or download an archive from [GitHub Releases](https://github.com/noru-tech/agent-change-control/releases).
Release binaries carry GitHub artifact attestations:
`gh attestation verify <archive> --repo noru-tech/agent-change-control`.

## Quick start

Try it offline, without credentials, on the synthetic fixtures:

```bash
acc evaluate tests/fixtures/claude-operator-self-approved/events.json --format table
# github:acme/api:pr:421  agent:claude-code  github:alice  FAIL
# (the per-rule counts under the table show ACC001, ACC002 and ACC003)
acc evaluate tests/fixtures/claude-clean/events.json --format table
# github:acme/api:pr:421  agent:claude-code  github:alice  PASS
```

Scan a repository for a reporting period, then check it:

```bash
# GITHUB_TOKEN (or GH_TOKEN) should already be set through your secret manager.
acc scan github acme/api --since 2026-08-01 --until 2026-08-31 --output manifest.json
acc check manifest.json
```

Or gate every pull request with the [GitHub Action](docs/github-action.md):

```yaml
- uses: noru-tech/agent-change-control@v0.4.0
```

## How it works

1. **Declare.** The agent, or the engineer operating it, puts one fenced block in the pull request
   description. Only this exact block is read; prose, style and bot names never establish
   authorship.

   ````text
   ```agent-change-control
   author: claude-code
   operator: alice
   ```
   ````

   Accounts your organization has verified as agents can be mapped instead:
   `--agent-account 'my-agent[bot]=codex'`. A signed provenance document, handed over as an
   attestation after you verified it (`--attestations PATH --verified-by TEXT`), makes the same
   claim with `signed` evidence, which a policy can require. Reviewers can sign what they decided
   the same way (a review document, matched to the forge's review, never replacing it).

   Below that sits a **derived tier**, read only when no declaration applies: the
   `Co-Authored-By` trailers that Claude Code and Copilot already write into commits, and
   [Agent Trace](https://agent-trace.dev) records bound to the change's commits
   (`--agent-trace PATH`). They yield `derived` confidence, the operator is derived only when one
   human authored every commit, and `--ignore-trailers` switches trailers off.

2. **Collect.** `acc` reads the pull request, its commits, its full review history (with the
   commit each review applied to) and its merger through GET-only calls to `api.github.com`.
   Anything it cannot retrieve is marked incomplete, never assumed.

3. **Evaluate.** Offline, deterministically, `acc` asks whether a human other than the effective
   human (the author, or the agent's operator) approved the current head before merge, with no
   later withdrawal. Each rule yields `pass`, `fail`, `unknown` or `not_applicable` per change,
   and every finding cites the API records it came from.

4. **Report.** A manifest that re-validates byte for byte, a SARIF log for code scanning, or an
   in-toto statement to sign and file next to the release's build provenance.

## Rules

| ID | Finding | Default severity |
| --- | --- | --- |
| ACC001 | Agent change without independent human approval | high |
| ACC002 | Effective human author/operator approved own change | high |
| ACC003 | Merged change without independent human approval | high |
| ACC006 | Agent operator unknown | warning |
| ACC007 | Agent approval recorded (observation) | info |
| ACC008 | Same-vendor write and review | high |
| ACC009 | Agent approval lacks required independence (under `agent_review`) | high |
| ACC010 | Agent approval without signed identity (under `agent_review`) | warning |

A qualifying approval must be from a different **human**, apply to the current head SHA, precede or
equal merge time, and be that reviewer's latest non-comment decision before merge. Comments do not
withdraw approval. An agent's approval never counts as a human's; under the opt-in `agent_review`
policy it can satisfy independence instead, when it is signed and independent of the author on
operator, provider and verified identity ([policy](docs/policy.md#agent-reviewers)). A policy can
also set the weakest evidence an operator claim or an approval may rest on
(`minimum_authorship_evidence`, `minimum_review_evidence`; kinds order as
`derived < declared < observed < signed`), which only ever moves a verdict away from pass. Exact
conditions, with the fixture that exercises each, are in [docs/policy.md](docs/policy.md) and
[the specification](spec/ai-change-provenance.md#62-rules).

Unknown operators yield ACC006 and `unknown` independence assessments, not invented violations.
Incomplete collection produces exit 4 and cannot yield a clean result. A review or merge whose
GitHub account no longer exists is recorded against the `unknown:unavailable` actor. Invalid
approval-after-merge data produces ACV001, separately from governance findings.

## Commands

| Command | What it does |
| --- | --- |
| `acc scan github OWNER/REPO --since D --until D` | Collect merged PRs and write an evaluated manifest (default `.agent-change-control/manifest.yml`) |
| `acc export github OWNER/REPO --since D --until D` | Write normalized events as JSON for offline evaluation |
| `acc evaluate EVENTS` | Evaluate an export offline into a manifest (`--policy`, `-f`, `-o`) |
| `acc validate INPUT` | Check a manifest (schema, timeline, references, recomputed findings, summary and digest), or an in-toto Statement or JSON Lines of Statements (subjects and predicate) |
| `acc check MANIFEST` | Enforce policy, honoring dispositions (`--policy`, `--as-of DATE`); exit 1 on failure |
| `acc pr NUMBER --repo OWNER/REPO` | Collect and check one pull request (`GITHUB_REPOSITORY` is honored) |
| `acc completions <shell>` / `acc manpage` | Shell completions and man pages |

Global flags: `-q` silences status lines on stderr. Every evaluated command accepts
`--format table|json|yaml|sarif|in-toto|in-toto-jsonl` and `--output FILE`; the format is inferred
from the output name when omitted (`.intoto.json` and `.intoto.jsonl` select the attestation
forms). `export` emits normalized JSON only. `scan` without output flags
writes `.agent-change-control/manifest.yml`. Findings never prevent manifest generation.

The collecting commands (`scan`, `export`, `pr`) take the evidence flags: `--agent-account
LOGIN=AGENT` and `--agent-vendor AGENT=VENDOR` for accounts and vendors your organization has
verified, `--agent-trailer`, `--ignore-trailers` and `--agent-trace PATH` for the derived tier,
and `--attestations PATH`, `--verification PATH` (the JSON that `gh attestation verify --format
json` writes, with the verified signer) and `--verified-by TEXT` for signed authorship and review
documents ([signing](docs/signing.md)).

Historical scans select PRs **merged in the inclusive UTC window**. Dates expand to the start/end of
the day. `pr` evaluates an open or merged PR directly. The token needs read access to repository
contents and pull requests (public repositories can be read without a token). Requests are
GET-only, restricted to `api.github.com`, with redirects disabled. Tokens are never written to
exports or error messages.

## Attestations

`--format in-toto` emits an unsigned [in-toto Statement v1](https://github.com/in-toto/attestation)
whose subjects are the evaluated changes and whose predicate is the manifest. Each change
contributes its head commit (the commit the approvals are bound to) and, once merged with a known
merge commit, that commit too, so the attestation is found by the commit that lands on the target
branch after a squash or rebase merge. `--format in-toto-jsonl` writes one Statement per change,
one per line, for signers and verifiers that handle a change at a time:

```bash
acc evaluate events.json -o change-control.intoto.json     # one Statement, every change
acc scan github acme/api --since 2026-08-01 --until 2026-08-31 -o august.intoto.jsonl
acc validate august.intoto.jsonl                            # subjects match, findings recompute
```

Sign them with the DSSE signer you already use for build provenance. A verifier checks that the
subjects cover the commits it cares about and runs `acc validate` on the Statement, which confirms
that the subjects are exactly the predicate's changes and that the findings follow from the
embedded facts. The predicate type is `https://noru.tech/spec/ai-change-provenance/v0.2`; the
predicate is documented in [docs/in-toto.md](docs/in-toto.md).

[docs/signing.md](docs/signing.md) shows the two signing paths: GitHub artifact attestations
through `actions/attest`, where the subject is the sha256 of the manifest file and
`gh attestation verify` finds it, and cosign over the commit-subject Statement. This repository
signs the verdict of every merged pull request that way
([workflow](.github/workflows/attest.yml), [attestations](https://github.com/noru-tech/agent-change-control/attestations)).
Attestations carry the same personal data as manifests; read the privacy note there before
publishing one to a transparency log.

Attestations also flow the other way. Two predicates of `acc`'s own carry claims it reads back as
evidence, each bound to a change by its head commit and handed over after you verified it:
`https://noru.tech/spec/ai-change-provenance/provenance/v0.1` (an agent's integration or its
operator states which agent wrote the change and who directed it) and
`https://noru.tech/spec/ai-change-provenance/review/v0.1` (a reviewer's tooling states who
reviewed what and decided what, naming the reviewer in the predicate; it upgrades the forge's
matching review and never creates one). `acc` does not verify signatures: with `--verification`
or `--verified-by` the claims count as `signed` and the manifest records who verified them;
without, they count as `declared`. See [authorship](docs/agent-authorship.md#signed-tier) and
the examples under [`examples/`](examples/).

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Successful generation/validation, or policy threshold passed |
| 1 | Policy threshold exceeded (`check` / `pr`) |
| 2 | Invalid CLI arguments |
| 3 | Invalid input or manifest |
| 4 | Collection incomplete (takes precedence over policy failure) |
| 5 | Authentication rejected |
| 6 | API, permission, rate-limit or transport failure |
| 7 | Unsupported API data condition |

Warnings do not fail the default policy. `scan` and `evaluate` write findings without returning
policy exit 1; use `check` to enforce policy. See [policy semantics](docs/policy.md).

## Limits and non-goals

The collector uses bounded pages (`--max-pages`, default 100 per endpoint; maximum 1000) and 8 MiB
per response. Collection is a best-effort snapshot, not an atomic historical archive. GitHub
identity changes, deleted users and mutable declarations cannot be reconstructed reliably from
current REST data. The PR opener is the default effective author unless explicit agent evidence
overrides it; mixed human authorship is not resolved in this release.

`acc` does not detect AI-written code, does not treat bots as agents, does not guess that the
merger operated the agent, and does not call a model. A declaration is declared evidence, not
authenticated identity; a digest detects inconsistency, not forgery. `acc` does not verify
signatures: attestations are read as pre-verified input and the manifest records who said they
verified them. A clean result is a statement
about the recorded scope, not a compliance certification. See the
[roadmap](ROADMAP.md) for what comes next, including GitLab collection and signature verification
inside `acc`.

Manifests contain employee activity data; keep real exports out of public Git repositories. Read
[the model](docs/model.md), [authorship](docs/agent-authorship.md), [privacy](docs/privacy.md) and
[security](SECURITY.md).

## Development

```bash
cargo build
cargo test            # unit + assert_cmd integration tests + insta snapshots + fixture goldens
cargo clippy --all-targets -- -D warnings
```

Tests include reviewed fixture expectations, golden manifests/findings, offline CLI workflows and a
loopback HTTP server that replays GitHub pagination, errors, deleted accounts and changing snapshots.
They require loopback networking, no GitHub credentials. This repository also runs its own check
on every pull request ([workflow](.github/workflows/change-control.yml)).

See [CONTRIBUTING.md](./CONTRIBUTING.md) for the layout, how to add a rule, and how goldens are
updated. Proposals for rules, collectors and specification changes have
[issue templates](https://github.com/noru-tech/agent-change-control/issues/new/choose).

## About

Built and maintained by [Noru](https://noru.tech), a compliance platform. The tool, the schemas and
the specification are MIT-licensed and independent of the platform; Noru consumes the same
manifests everyone else does.

## License

MIT — see [LICENSE](./LICENSE).
