# Security Policy

## Supported versions

This project is pre-1.0; security fixes are applied to the latest release on the `main` branch.

| Version | Supported |
| ------- | --------- |
| 0.1.x   | ✅        |

## Reporting a vulnerability

Please report security issues **privately** — do not open a public issue for an unfixed
vulnerability.

- Email: **security@noru.tech** with a subject line beginning `[SECURITY] agent-change-control`.
- Or use GitHub **Private vulnerability reporting** (Security → *Report a vulnerability*).

Please include: a description of the issue, the affected version or commit, reproduction steps or a
proof of concept, and the impact you foresee. Do not put tokens, private exports or employee data
in a report unless they are essential to demonstrate the issue.

We aim to acknowledge reports within **5 business days** and to provide a remediation timeline after
triage. We will credit reporters who wish to be named once a fix is released.

## Scope and threat model

`acc` is a local command-line tool that reads a forge's REST API and evaluates the result offline.

- Forge responses and local inputs are untrusted data. Repository contents are never executed.
- The collector uses GET-only requests to a fixed GitHub HTTPS origin, disables redirects, caps
  response bodies and pages, and reports fixed transport errors rather than headers or response
  bodies. `GITHUB_TOKEN` / `GH_TOKEN` is not serialized or logged. Test-only loopback origins reject
  tokens. A 403/429 is an API, permission or rate-limit error; 401 is an authentication failure.
- Input documents are limited to 32 MiB and parsed as data with `serde_json` or `serde-saphyr`; no
  YAML object construction or repository commands run. JSON Schema references are embedded; remote
  schema resolution is disabled. Inputs undergo schema and semantic validation before evaluation.
  Terminal control characters are removed from table identifiers. SARIF and JSON use escaped
  serialization.
- `evaluate`, `validate` and `check` perform no network calls. Only `scan`, `export` and `pr`
  contact GitHub.
- Release binaries are built by GitHub Actions from tagged commits, published with SHA-256 checksums
  and GitHub artifact attestations. Verify with
  `gh attestation verify <archive> --repo noru-tech/agent-change-control`.

An explicit agent declaration is not authenticated provenance. A source digest detects changes, not
malicious replacement of an entire export. GitHub snapshots are mutable and non-atomic, and may omit
information the token cannot see. Do not treat a clean result as assurance beyond the recorded scope
and trusted input sources.

### A note on manifests

Manifests and normalized exports contain names, usernames, approval history, merge history and work
activity. Treat them as **sensitive**; keep real exports out of public repositories. See
[docs/privacy.md](./docs/privacy.md).
