# Exit codes

`acc` exits with one of eight stable codes. They are part of the public interface: a code's meaning
never changes, and a new condition gets a new code rather than reusing one. The codes are defined
once, in `Exit` in [`src/lib.rs`](../src/lib.rs), and a unit test pins each value.

| Code | Meaning | Returned by |
| --- | --- | --- |
| [0](#0-success) | Success, or the policy threshold passed | every command |
| [1](#1-policy-threshold-exceeded) | Policy threshold exceeded | `check`, `pr` |
| [2](#2-invalid-command-line-arguments) | Invalid command-line arguments | every command |
| [3](#3-invalid-input-or-manifest) | Invalid input or manifest | every command that reads a file |
| [4](#4-collection-incomplete) | Collection incomplete (takes precedence over 1) | `scan`, `export`, `pr`, `evaluate`, `check` |
| [5](#5-authentication-rejected) | Authentication rejected | `scan`, `export`, `pr` |
| [6](#6-api-permission-rate-limit-or-transport-failure) | API, permission, rate-limit or transport failure | `scan`, `export`, `pr` |
| [7](#7-unsupported-api-data-condition) | Unsupported API data condition | `scan`, `export`, `pr` |

Errors go to stderr as `error: …`. For the common failures (a missing or rejected token, a rate
limit, a missing repository, a bad date, an invalid policy file) two more lines follow: `help: …`
says what to do, and `see: <url>` links to the section below or to the page that explains it. An
incomplete collection is not an error; it prints `warning: collection incomplete: <reason>` with the
same two lines (unless `-q`). None of these lines ever reaches stdout or a machine-readable output.

Findings never change the exit code of `scan` or `evaluate`: they write the manifest and exit 0
(or 4). Only `check` and `pr` enforce the policy threshold. The [GitHub Action](github-action.md)
treats 0, 1 and 4 as verdicts (it still renders SARIF and the job summary) and any other code as a
failure of the run itself.

## 0: Success

The command did what it was asked: a manifest or export was written, a document validated, or
`check`/`pr` found no unsuppressed finding at or above the policy's `fail_on` severity (default
`medium`). Advisory findings (ACC006 and ACC010 are `warning`, ACC007 is `info`) do not fail the
default policy.

```console
$ acc evaluate tests/fixtures/claude-clean/events.json --format table > /dev/null; echo $?
0
```

## 1: Policy threshold exceeded

`check` or `pr` found at least one finding whose severity is at or above `fail_on` and that no
disposition suppresses on the `--as-of` date. Each finding names its rule; see the
[rule pages](rules/README.md) for what it means and how to fix it or record a disposition.

```console
$ acc -q check tests/fixtures/human-self-approved/expected-manifest.json > /dev/null; echo $?
1
```

## 2: Invalid command-line arguments

The command line could not be used: an unknown subcommand or flag, a bad value (`--format nope`),
a malformed date or window (`--since` after `--until`), a malformed `LOGIN=AGENT` mapping, a
missing `--repo` for `pr`, `--verified-by` without attestations, or `check` on a manifest with
non-open dispositions but no `--as-of` date. The message says which.

```console
$ acc scan github acme/api --since 2026-08-31 --until 2026-08-01; echo $?
error: window is reversed
help: --since must not be later than --until
see: https://github.com/noru-tech/agent-change-control/blob/main/docs/exit-codes.md#2-invalid-command-line-arguments
2
```

## 3: Invalid input or manifest

An input file could not be read or is not a valid document: it is missing or unreadable, larger
than 32 MiB, not JSON or YAML, violates a [validation code](rules/README.md#validation-codes)
(ACV001 to ACV010), or is a manifest or attestation that does not re-validate (its findings,
summary or digest do not follow from its embedded facts). No manifest is produced for invalid
input. When the error names a validation code, the message ends with a link to that code's page.

```console
$ acc evaluate tests/fixtures/approval-after-merge/events.json > /dev/null; echo $?
error: ACV001 approval_after_merge (see https://github.com/noru-tech/agent-change-control/blob/main/docs/rules/ACV001.md)
3
```

## 4: Collection incomplete

Part of the population or a review history could not be retrieved (page limit reached, a
dismissed review whose time GitHub does not expose, an export marked incomplete). The output is
still written, with the gap recorded, but a clean result cannot be claimed. Exit 4 takes precedence
over exit 1: an incomplete window is never reported as a plain policy failure or pass.

```console
$ acc evaluate tests/fixtures/incomplete-window/events.json --format table > /dev/null; echo $?
warning: collection incomplete: Review collection unavailable
help: the input records an incomplete collection; collect again (raise --max-pages or narrow the window) before relying on the result
see: https://github.com/noru-tech/agent-change-control/blob/main/docs/exit-codes.md#4-collection-incomplete
4
```

Re-collect (raise `--max-pages`, narrow the window), or document the gap. See
[known limitations](../KNOWN-LIMITATIONS.md#collection).

## 5: Authentication rejected

GitHub answered 401 to a collecting command. The token in `GITHUB_TOKEN` (or `GH_TOKEN`) is
missing, expired or revoked. Public repositories can be read without a token; private ones need
read access to contents and pull requests. Tokens never appear in messages or exports.

```console
$ acc scan github acme/private --since 2026-08-01 --until 2026-08-31; echo $?
error: GitHub authentication failed
help: the token in GITHUB_TOKEN or GH_TOKEN was rejected: it is expired, revoked or mistyped; replace it with one that has read access to contents and pull requests
see: https://github.com/noru-tech/agent-change-control/blob/main/docs/exit-codes.md#5-authentication-rejected
5
```

## 6: API, permission, rate-limit or transport failure

A request to `api.github.com` failed: the connection failed, GitHub answered 403 or 429 (forbidden
or rate-limited), a resource was not found, the status was unexpected, the response exceeded
8 MiB, or the body was not valid JSON. Messages are fixed strings so that response headers and
bodies never reach a log. Check the token's permissions and the rate limit, then retry. The
`help:` line says which it was: without a token GitHub allows 60 requests an hour, a 404 on a
private repository usually means the token is missing or cannot read it, and a 403 with an
exhausted rate limit is reported as a rate limit, not a permission problem.

## 7: Unsupported API data condition

GitHub returned data that `acc` does not know how to interpret, such as an unknown review state
or a required field that is missing. `acc` stops instead of guessing. Please
[open an issue](https://github.com/noru-tech/agent-change-control/issues/new/choose) with the
command (not the token) and the message.

---

[Rules and validation codes](rules/README.md) · [Policy](policy.md) · [README](../README.md#output-formats-and-exit-codes)
