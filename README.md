<p align="center">
  <img src="./docs/assets/agent-change-control-logo.png" alt="agent-change-control logo" width="480">
</p>

# agent-change-control

Deterministic change control for code written by AI coding agents. Checks each change for independent human approval and emits SARIF and in-toto statements.

> **Enforcing the four-eyes principle for coding agents.**
>
> Different accounts do not necessarily represent independent humans. `acc` records the human
> behind an agent's change and checks for independent human approval. Offline, reproducible, no LLM.

[![release](https://img.shields.io/github/v/release/noru-tech/agent-change-control)](https://github.com/noru-tech/agent-change-control/releases/latest)
[![ci](https://github.com/noru-tech/agent-change-control/actions/workflows/ci.yml/badge.svg)](https://github.com/noru-tech/agent-change-control/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/noru-tech/agent-change-control/badge)](https://scorecard.dev/viewer/?uri=github.com/noru-tech/agent-change-control)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![crates.io](https://img.shields.io/crates/v/agent-change-control.svg)](https://crates.io/crates/agent-change-control)

## Install

The binary is called `acc`; the package is `agent-change-control` everywhere.

### Homebrew

```bash
brew install noru-tech/tap/acc            # macOS and Linux
```

The formula installs bash, zsh and fish completions and the man pages (`man acc`) along with the
binary.

### crates.io

The crate is `agent-change-control` (the short name `acc` on crates.io belongs to an unrelated
tool). The binary it installs is `acc`.

```bash
cargo binstall agent-change-control       # prebuilt binary from GitHub Releases
cargo install agent-change-control --locked
```

### Prebuilt binaries

Download an archive for your platform from
[GitHub Releases](https://github.com/noru-tech/agent-change-control/releases): fully static Linux
builds (`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`) and macOS builds
(`aarch64-apple-darwin`, `x86_64-apple-darwin`), each named
`agent-change-control-<target>.tar.xz` and holding `acc`, the README, the changelog, the license,
shell completions (`completions/acc.bash`, `completions/_acc`, `completions/acc.fish`) and man
pages (`man/*.1`). Homebrew installs the completions and man pages for you; from an archive, copy
them where your shell and `man` look, or generate them with `acc completions <shell>` and
`acc manpage --out-dir DIR`.

### Verify before you run

Every archive carries a GitHub artifact attestation from the release workflow and a SHA-256
checksum file next to it (`<archive>.sha256`; `sha256.sum` lists all of them). Check both before
you unpack:

```bash
VERSION=v0.5.4
ARCHIVE=agent-change-control-aarch64-apple-darwin.tar.xz
gh release download "$VERSION" --repo noru-tech/agent-change-control \
  --pattern "$ARCHIVE" --pattern "$ARCHIVE.sha256"

# Built by this repository's release workflow, from a tagged commit
gh attestation verify "$ARCHIVE" --repo noru-tech/agent-change-control \
  --signer-workflow noru-tech/agent-change-control/.github/workflows/release.yml

# The bytes match the published checksum (on Linux: sha256sum -c "$ARCHIVE.sha256")
shasum -a 256 -c "$ARCHIVE.sha256"

tar -xJf "$ARCHIVE"
```

There is no `curl | sh` installer in these instructions, and there will not be one.

## Quick start

Try it offline, without credentials, on the synthetic fixtures:

```bash
acc evaluate tests/fixtures/claude-operator-self-approved/events.json --format table
# github:acme/api:pr:421  agent:claude-code  github:alice  FAIL
# (the per-rule counts under the table show ACC001, ACC002 and ACC003)
acc evaluate tests/fixtures/claude-clean/events.json --format table
# github:acme/api:pr:421  agent:claude-code  github:alice  PASS
```

In a clone of a GitHub repository, `acc scan` works with zero arguments: it finds the repository
(from `GITHUB_REPOSITORY`, else the `origin` remote), scans the last 30 days in UTC ending now,
prints the resolved repository and window on stderr, and writes
`.agent-change-control/manifest.yml`:

```bash
# GITHUB_TOKEN (or GH_TOKEN) should already be set through your secret manager.
acc scan
acc check .agent-change-control/manifest.yml
```

Or scan a named repository for a reporting period:

```bash
acc scan github acme/api --since 2026-08-01 --until 2026-08-31 --output manifest.json
acc check manifest.json
```

Or gate every pull request with the [GitHub Action](docs/github-action.md):

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
      - uses: noru-tech/agent-change-control@v0.5.4
```

<a id="what-it-does"></a>
## How do I enforce separation of duties for AI coding agents?

Record the human behind each agent's change and require a different human to approve the commit
that is merged: `acc` checks exactly that on every pull request, offline and deterministically,
and fails the change when the approval is missing or came from the agent's own operator.

### Why do two accounts not mean two humans?

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
The convention it implements is published as [AI Change Provenance 0.3](spec/ai-change-provenance.md).

### What is in this repository

| Piece | Where |
| --- | --- |
| **Specification** — AI Change Provenance 0.3: declarations, collection, evaluation, formats | [`spec/`](spec/ai-change-provenance.md) |
| **Schemas** — events, manifest, policy, provenance, review, in-toto statement (JSON Schema 2020-12) | [`schemas/`](schemas/) |
| **CLI** — `acc`: GitHub collector, offline evaluator, validator, policy check | [`src/`](src/) |
| **GitHub Action** — evaluate the current pull request, SARIF and job summary | [`action.yml`](action.yml), [docs](docs/github-action.md) |
| **Outputs** — table, JSON, YAML, SARIF 2.1.0, in-toto Statement v1 (one, or JSON Lines per change) | [`src/output/`](src/output/), [docs](docs/in-toto.md) |
| **Control mapping** — where the evidence lands in SOC 2, ISO 27001, PCI DSS, NIST | [docs](docs/control-mapping.md) |
| **Rule pages** — one per rule, validation code and exit code: controls, examples, fixes | [`docs/rules/`](docs/rules/README.md), [exit codes](docs/exit-codes.md) |

<a id="what-it-is-not-and-known-limitations"></a>
## What does acc not do?

`acc` does not detect AI-written code, does not verify signatures and does not certify
compliance; it checks recorded approvals against recorded authorship, and says so when the record
is incomplete.

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

The full list, with where each limit is described, is in [KNOWN-LIMITATIONS.md](KNOWN-LIMITATIONS.md).

<a id="how-it-works"></a>
## How does acc decide whether a change was independently approved?

It asks whether a human other than the change's effective human (the author, or the agent's
operator) approved the current head before merge, with no later withdrawal, in four steps:

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

<a id="rules"></a>
## Which rules does acc check?

Eight rules, each with a page that explains it, maps it to controls, shows a failing and a passing
example and says how to fix it or record a disposition ([all rules and validation
codes](docs/rules/README.md)):

| ID | Finding | Default severity |
| --- | --- | --- |
| [ACC001](docs/rules/ACC001.md) | Agent change without independent human approval | high |
| [ACC002](docs/rules/ACC002.md) | Effective human author/operator approved own change | high |
| [ACC003](docs/rules/ACC003.md) | Merged change without independent human approval | high |
| [ACC006](docs/rules/ACC006.md) | Agent operator unknown | warning |
| [ACC007](docs/rules/ACC007.md) | Agent approval recorded (observation) | info |
| [ACC008](docs/rules/ACC008.md) | Same-vendor write and review | high |
| [ACC009](docs/rules/ACC009.md) | Agent approval lacks required independence (under `agent_review`) | high |
| [ACC010](docs/rules/ACC010.md) | Agent approval without signed identity (under `agent_review`) | warning |

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
approval-after-merge data produces [ACV001](docs/rules/ACV001.md), separately from governance
findings; every validation code has a page in the [rules index](docs/rules/README.md#validation-codes).

## Commands

| Command | What it does |
| --- | --- |
| `acc scan [github] [OWNER/REPO] [--since D] [--until D]` | Collect merged PRs and write an evaluated manifest (default `.agent-change-control/manifest.yml`) |
| `acc export [github] [OWNER/REPO] [--since D] [--until D]` | Write normalized events as JSON for offline evaluation |
| `acc evaluate EVENTS` | Evaluate an export offline into a manifest (`--policy`, `-f`, `-o`) |
| `acc validate INPUT` | Check a manifest (schema, timeline, references, recomputed findings, summary and digest), or an in-toto Statement or JSON Lines of Statements (subjects and predicate); `--format json` prints a [result object](#validate-result-object) |
| `acc check MANIFEST` | Enforce policy, honoring dispositions (`--policy`, `--as-of DATE`); exit 1 on failure |
| `acc pr NUMBER [--repo OWNER/REPO]` | Collect and check one pull request (the repository is found like `scan`'s) |
| `acc doctor [--online]` | Check the environment: version, token set (never printed), default policy file, detected repository; `--online` adds the token, the rate limit and whether a newer `acc` exists (GET only; the only update check, never automatic). Exit 0 healthy, 2 broken. `--format json` prints a [report object](#doctor-report-object) |
| `acc completions <shell>` / `acc manpage` | Shell completions and man pages (`-o FILE` writes them to a file) |

Global flags: `-q` silences status lines on stderr; `-v` (`--verbose`) adds diagnostics there
(resolved policy, format, destination, counts, whether a token is used, never its value);
`--no-color` and a non-empty `NO_COLOR` keep help and usage errors uncolored (`acc`'s own output is
never colored). None of them changes stdout. Output formats and exit codes are
[below](#output-formats-and-exit-codes).

The collecting commands (`scan`, `export`, `pr`) take the evidence flags: `--agent-account
LOGIN=AGENT` and `--agent-vendor AGENT=VENDOR` for accounts and vendors your organization has
verified, `--agent-trailer`, `--ignore-trailers` and `--agent-trace PATH` for the derived tier,
and `--attestations PATH`, `--verification PATH` (the JSON that `gh attestation verify --format
json` writes, with the verified signer) and `--verified-by TEXT` for signed authorship and review
documents ([signing](docs/signing.md)).

Historical scans select PRs **merged in the inclusive UTC window**. Dates expand to the start/end of
the day. GitHub is the default forge, so `github` may be left out. Without `OWNER/REPO`, the
collecting commands use `GITHUB_REPOSITORY`, else the github.com `origin` remote of the working
directory (`https`, `ssh` and `git@github.com:` forms). Without `--until` the window ends now;
without `--since` it starts 30 days before its end. A defaulted window is printed on stderr and
recorded in the output exactly as if it had been passed, so evaluating that output later is as
deterministic as ever; only the collecting commands read the clock. `pr` evaluates an open or merged PR directly. The token needs read access to repository
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
embedded facts. The predicate type is `https://noru.tech/spec/ai-change-provenance/v0.3`; the
predicate is documented in [docs/in-toto.md](docs/in-toto.md). Every digest is SHA-256 over
[RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) bytes, and `acc` writes JSON as exactly those
bytes, so any JCS implementation reproduces them.

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

## Output formats and exit codes

Every evaluated command accepts
`--format table|json|yaml|sarif|in-toto|in-toto-jsonl` and `--output FILE`; the format is inferred
from the output name when omitted (`.intoto.json` and `.intoto.jsonl` select the attestation
forms). `text` is an alias of `table` wherever `table` is accepted, with identical bytes. `export`
emits normalized JSON only. `scan` without output flags
writes `.agent-change-control/manifest.yml`. Findings never prevent manifest generation.

<a id="validate-result-object"></a>
`acc validate` takes `--format text|json` and `--output FILE`. Text is the status line on stderr
(stdout stays empty, as it always was); with `--output` the line goes to the file. JSON is one
object on stdout (or in the file; a `.json` output name selects it), as RFC 8785 bytes and a
newline, whether or not the input is valid:

```json
{"code":"ACV008","help_uri":"https://github.com/noru-tech/agent-change-control/blob/main/docs/rules/ACV008.md","message":"ACV008 duplicate member name at byte 347","valid":false}
```

`valid` and `message` are always present; `code` (the validation code) and `help_uri` (its page)
only when an `ACV` code applies. The exit status is the same as in text.

<a id="doctor-report-object"></a>
`acc doctor --format json` prints one object (RFC 8785 bytes and a newline): `version`, `healthy`
(false when any check failed; the exit status is then 2), and `checks`, each with `name`
(`version`, `token`, `policy`, `repository`, and with `--online` `github` and `update`), `status`
(`ok`, `warn` or `fail`), `detail` and, when there is something to do, `hint`:

```json
{"checks":[{"detail":"acc 0.5.4","name":"version","status":"ok"},{"detail":"GITHUB_TOKEN is set","name":"token","status":"ok"}],"healthy":true,"version":"0.5.4"}
```

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

What each code means and what to do about it: [exit codes](docs/exit-codes.md). Common failures
(missing or rejected token, rate limit, missing repository, bad date, invalid policy file, incomplete
collection) print a `help:` line with the fix and a `see:` line with that page's section on stderr.

Warnings do not fail the default policy. `scan` and `evaluate` write findings without returning
policy exit 1; use `check` to enforce policy. See [policy semantics](docs/policy.md).

## Conformance

An ACP evaluator conforms when it passes the published corpus in [`conformance/`](conformance/README.md):
accept, reject and incomplete vectors run through a small external-verifier contract (`<cmd>
<vector-file>`, verdict in the exit status, one JSON result line on stdout). `acc` passes it with
`acc evaluate --conformance-json`, and another implementation can run it in one step:

```bash
python3 conformance/run.py --verifier "acc evaluate --conformance-json"
```

The corpus is an answer key, not a proof; independent implementations that disagree with it are
the most useful reports.

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

## How to cite

Every release is archived on Zenodo. Cite the concept DOI
[10.5281/zenodo.23042500](https://doi.org/10.5281/zenodo.23042500), which resolves to the latest
release, or the version DOI of the release you used, listed on that record, when the exact bytes
matter (an audit report, or a conformance claim). `CITATION.cff` has the citation metadata.

The specification is [AI Change Provenance 0.3](spec/ai-change-provenance.md); its predicate type
URIs are listed in [Attestations](#attestations).

## Trust

How a release gets from this repository to your machine, and how to check it:

- **Built in CI, from a tag.** Releases are built by [cargo-dist](https://github.com/axodotdev/cargo-dist)
  in GitHub Actions ([release.yml](.github/workflows/release.yml)) from the tagged commit; no
  release artifact is built on a workstation.
- **Attested.** Every archive has a GitHub artifact attestation (SLSA build provenance, signed
  through Sigstore) naming the workflow and commit that built it. Verify it as shown in
  [Verify before you run](#verify-before-you-run).
- **Checksums.** Each archive has a `.sha256` file, and `sha256.sum` lists every archive in the
  release.
- **SBOM.** Releases after 0.5.2 attach a CycloneDX SBOM (`agent-change-control.cdx.xml`)
  generated by `cargo-cyclonedx`.
- **Signed conformance corpus and verdicts.** The conformance corpus digest list is attested at
  every tag ([workflow](.github/workflows/conformance-release.yml)), and the change-control
  verdict of every merged pull request is attested ([workflow](.github/workflows/attest.yml)).
- **Pinned and scored.** Every GitHub Action is pinned to a commit SHA and kept current by
  Dependabot. The repository's supply-chain posture is measured by
  [OpenSSF Scorecard](https://scorecard.dev/viewer/?uri=github.com/noru-tech/agent-change-control).
- **Vulnerabilities.** Report privately through
  [GitHub private vulnerability reporting](https://github.com/noru-tech/agent-change-control/security/advisories/new);
  see [SECURITY.md](SECURITY.md) for scope and response times.

## License

MIT, see [LICENSE](./LICENSE). The tool, the schemas and the specification are MIT-licensed and
independent of the Noru platform; Noru consumes the same manifests everyone else does.

Maintained by [Noru](https://noru.tech), a continuous compliance platform.
