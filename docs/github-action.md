# GitHub Action

The repository root doubles as a composite GitHub Action. It installs a verified `acc` release,
evaluates the pull request that triggered the workflow, writes SARIF (or any other `acc` format),
and puts the table in the job summary. Evaluation is the same deterministic, offline check the CLI
performs; the action adds no logic of its own.

## Quick start

```yaml
name: change-control
on:
  pull_request:
    types: [opened, synchronize, reopened, ready_for_review]
  pull_request_review:
    types: [submitted, dismissed]

permissions:
  contents: read
  pull-requests: read

jobs:
  acc:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: noru-tech/agent-change-control@v0.1.0
```

Run on review events as well as pushes: the verdict for an agent-written change flips from ACC001
to clean the moment an independent human approves the current head, and flips back when a new
commit invalidates that approval.

Pin the action to a commit SHA in production, and pin the `version` input so the same
release evaluates every run. The default `version` tracks the release the action was published
with.

## Inputs

| Input | Default | Meaning |
| --- | --- | --- |
| `version` | the matching release | `acc` release to install, without the leading `v` |
| `pr` | the event's pull request | Pull request number |
| `repository` | `github.repository` | `OWNER/REPO` |
| `token` | `github.token` | Read access to contents and pull requests; GET-only, never written to outputs |
| `policy` | `.agent-change-control/policy.yml` when present | Policy file, see [policy](policy.md) |
| `agent-account` | none | Verified agent accounts, one `LOGIN=AGENT` per line |
| `format` | `sarif` | `sarif`, `json`, `yaml`, `table` or `in-toto` |
| `output` | `acc-results.sarif` | Where the rendered output is written |
| `fail-on-findings` | `true` | Fail the step on exit 1 (policy) or exit 4 (incomplete) |
| `verify-attestation` | `true` | Verify the release archive with `gh attestation verify` before running it |
| `max-pages` | `100` | Page cap per list endpoint |

## Outputs

| Output | Meaning |
| --- | --- |
| `exit-code` | `0` passed, `1` policy threshold exceeded, `4` collection incomplete |
| `output` | Path of the rendered file |
| `manifest` | Path of the evaluated JSON manifest under `RUNNER_TEMP` |

Exit codes 2, 3, 5, 6 and 7 (usage, invalid input, authentication, API and unsupported data) always
fail the step; they are errors, not verdicts.

## Blocking or advisory

With the default `fail-on-findings: true`, a required status check turns the four-eyes rule into a
merge gate: an agent-written change cannot merge until a human who is not its operator approves the
current head. This is the intended deployment.

While you roll out declarations, run in advisory mode and keep the evidence:

```yaml
      - uses: noru-tech/agent-change-control@v0.1.0
        id: acc
        with:
          fail-on-findings: false
      - uses: github/codeql-action/upload-sarif@v3
        if: always()
        with:
          sarif_file: acc-results.sarif
          category: agent-change-control
```

Uploading the SARIF file needs `security-events: write`. Findings then appear under Security →
Code scanning, with `acc`'s stable finding IDs as fingerprints, so the same finding is not
re-opened on every push.

## Telling the action who the agent was

The action reads the same evidence as the CLI: a fenced `agent-change-control` block in the pull
request body, or account mappings passed as input.

```yaml
        with:
          agent-account: |
            acme-codex[bot]=codex
            acme-claude[bot]=claude-code
```

Only these explicit declarations establish agent authorship. Writing style, diff size, commit
message wording and bot-looking names never do. See [agent authorship](agent-authorship.md) and
the [specification](../spec/ai-change-provenance.md).

## Attesting the verdict

`format: in-toto` writes an unsigned in-toto Statement whose subjects are the pull request's head
commit and whose predicate is the manifest. Sign it with the DSSE signer you already trust and
store it next to the build provenance of the release that ships the change:

```yaml
      - uses: noru-tech/agent-change-control@v0.1.0
        with:
          format: in-toto
          output: change-control.intoto.json
```

## Notes

- Pull requests from forks get a read-only token; reads are all the action needs.
- The release archive's checksum is verified, and by default its GitHub artifact attestation is
  verified with the `gh` CLI. Self-hosted runners without `gh` can set `verify-attestation: false`
  and pin `version`; the checksum check still runs.
- Linux (x86_64, aarch64) and macOS runners are supported. Windows runners are not: `acc` ships no
  Windows binary yet.
- The manifest contains reviewer identities and approval history. Prefer keeping it in
  `RUNNER_TEMP` (the default) over uploading it as an artifact from public repositories. See
  [privacy](privacy.md).
