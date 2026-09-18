# agent-change-control

> `acc` — deterministic change control for software written with coding agents: record the agent,
> the human operator, the reviewers and the merger of every pull request, then evaluate explicit
> separation-of-duty rules. Offline, reproducible, no LLM.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![ci](https://github.com/noru-tech/agent-change-control/actions/workflows/ci.yml/badge.svg)](https://github.com/noru-tech/agent-change-control/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/agent-change-control.svg)](https://crates.io/crates/agent-change-control)

Software change-control systems were designed around a simple assumption: the author recorded by
the development platform is the person who produced the change. Coding agents break that identity
model. A pull request opened by a human account may have been written entirely by an agent; the
human who prompted it may also be the one who approved it.

`acc` records the agent, human operator, reviewers and merger, then evaluates explicit
separation-of-duty rules against those facts. It makes no legal compliance claim and uses no LLM.
Agent authorship alone is not a finding: what matters is whether an **independent human** reviewed
the change.

Phase 1 (`0.1.0`) implements GitHub collection, offline evaluation, public JSON Schemas,
deterministic manifests, dispositions, and table / JSON / YAML / SARIF output. Deployments, bypass
detection, other forges and signed attestations are outside this release.

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

```bash
# GITHUB_TOKEN (or GH_TOKEN) should already be set through your secret manager.
acc scan github acme/api --since 2026-08-01 --until 2026-08-31 --output manifest.json
acc check manifest.json
```

The token needs read access to repository contents and pull requests (public repositories can be
read without a token). Requests are GET-only, restricted to `api.github.com`, with redirects
disabled. Tokens are never written to exports or error messages.

Historical scans select PRs **merged in the inclusive UTC window**. Dates expand to the start/end of
the day. They do not select by PR creation date. `pr` evaluates an open or merged PR directly.

```bash
acc export github acme/api --since 2026-08-01 --until 2026-08-31 > events.json
acc evaluate events.json --output manifest.json
acc validate manifest.json
acc check manifest.json --policy policy.yml
acc pr 421 --repo acme/api
acc evaluate events.json --format sarif > results.sarif
```

Try the offline workflow without credentials:

```bash
acc evaluate tests/fixtures/claude-operator-self-approved/events.json --output /tmp/manifest.json
acc check /tmp/manifest.json          # exits 1; reports ACC001, ACC002, ACC003
acc evaluate tests/fixtures/claude-clean/events.json --format table
```

## Commands

| Command | What it does |
| --- | --- |
| `acc scan github OWNER/REPO --since D --until D` | Collect merged PRs and write an evaluated manifest (default `.agent-change-control/manifest.yml`) |
| `acc export github OWNER/REPO --since D --until D` | Write normalized events as JSON for offline evaluation |
| `acc evaluate EVENTS` | Evaluate an export offline into a manifest (`--policy`, `-f`, `-o`) |
| `acc validate MANIFEST` | Check schema, timeline, references and recomputed findings, summary and digest |
| `acc check MANIFEST` | Enforce policy, honoring dispositions (`--policy`, `--as-of DATE`); exit 1 on failure |
| `acc pr NUMBER --repo OWNER/REPO` | Collect and check one pull request (`GITHUB_REPOSITORY` is honored) |
| `acc completions <shell>` / `acc manpage` | Shell completions and man pages |

Global flags: `-q` silences status lines on stderr. Every evaluated command accepts
`--format table|json|yaml|sarif` and `--output FILE`; the format is inferred from the output
extension when omitted. `export` emits normalized JSON only. `scan` without output flags writes
`.agent-change-control/manifest.yml`. Findings never prevent manifest generation.

## Explicit agent attribution

Place an exact fenced block in the PR body:

````text
```agent-change-control
author: claude-code
operator: alice
```
````

Only this delimited declaration is interpreted. The operator must resolve to a GitHub human
account; unmatched identifiers and emails remain unknown. Omit `operator` when unknown. This is
**declared evidence**, not a cryptographic assertion of identity. PR metadata is mutable.

For an account your organization has verified as an agent, pass an exact mapping:

```bash
acc scan github acme/api --since 2026-08-01 --until 2026-08-31 \
  --agent-account 'my-agent[bot]=codex' --format table
```

No bot is automatically an agent. No writing style, diff size or casual mention of AI establishes
authorship. This release accepts PR declarations and caller-maintained account mappings; it does not
yet ingest commit trailers, repository provenance files or signed identity attestations. The
independent provenance convention is published in `schemas/provenance.schema.json` for producers.

## Rules and uncertainty

| ID | Finding | Default severity |
| --- | --- | --- |
| ACC001 | Agent change without independent human approval | high |
| ACC002 | Effective human author/operator approved own change | high |
| ACC003 | Merged change without independent human approval | high |
| ACC006 | Agent operator unknown | warning |

A qualifying approval must be from a different **human**, apply to the current head SHA, precede or
equal merge time, and be that reviewer's latest non-comment decision before merge. Comments do not
withdraw approval. This conservative head requirement does not attempt to reproduce GitHub's
configurable branch-protection policy.

Unknown operators yield ACC006 and `unknown` independence assessments, not invented SOD violations.
Incomplete collection produces exit 4 and cannot yield a clean result. Review dismissal timing
unavailable through the REST review snapshot is marked incomplete. A review or merge whose GitHub
account no longer exists is recorded against the `unknown:unavailable` actor, and a missing reviewer
identity marks that change's reviews incomplete. Invalid approval-after-merge data produces ACV001,
separately from governance findings.

Every finding retains input evidence references. `validate` verifies the schema, timeline, actors,
source digest, summaries and findings by re-evaluating embedded facts. This detects inconsistency,
not forgery: a digest is not a signature.

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

## Limits

The collector uses bounded pages (`--max-pages`, default 100 per endpoint; maximum 1000) and 8 MiB
per response. Historical scans list pull requests newest-updated first and stop at the first page
that ends before the window, so cost scales with recent activity rather than repository age; hitting
a bound is never silently complete. Collection is a best-effort snapshot, not an atomic historical
archive. GitHub identity changes, deleted users and mutable declarations cannot be reconstructed
reliably from current REST data. PR opener is the default effective author unless explicit accepted
agent evidence overrides it; commit authors remain separately recorded, and mixed human authorship is
not resolved in Phase 1.

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
They require loopback networking, no GitHub credentials. Integration behavior is based on GitHub's
[pull request](https://docs.github.com/en/rest/pulls/pulls) and
[review](https://docs.github.com/en/rest/pulls/reviews) APIs.

See [CONTRIBUTING.md](./CONTRIBUTING.md) for the layout, how to add a rule, and how goldens are
updated.

## License

MIT — see [LICENSE](./LICENSE).
