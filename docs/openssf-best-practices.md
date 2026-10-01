# OpenSSF Best Practices: passing-level answers

Prepared answers for the [OpenSSF Best Practices badge](https://www.bestpractices.dev/) "passing"
level, one row per criterion, to paste into the form. Nothing here is submitted. The criteria
list is the 67 passing criteria in `criteria/criteria.yml` of
[coreinfrastructure/best-practices-badge](https://github.com/coreinfrastructure/best-practices-badge)
(main branch, September 2026), in the same order and with the same identifiers.

Answers are honest as of 2026-10-01: **Unmet** where the project does not meet a criterion today,
and *maintainer to confirm* where only a maintainer can attest (knowledge of secure design, the
response record). The project was first published on 2026-09-18, so several "in the last 2 to 12
months" criteria have no history to measure yet. Links to `docs/rules/`, `docs/exit-codes.md` and
`llms.txt` resolve once the branch that adds them is merged.

Summary: of 67 criteria, 56 Met, 2 Unmet (both SUGGESTED), 9 N/A. Every MUST is Met or N/A.

| Answer | Criteria |
| --- | --- |
| Unmet | `test_most`, `dynamic_analysis` (both SUGGESTED) |
| N/A | `release_notes_vulns`, `vulnerability_report_response`, `crypto_keylength`, `crypto_pfs`, `crypto_password_storage`, `crypto_random`, `dynamic_analysis_unsafe`, `dynamic_analysis_fixed`, `static_analysis_fixed` |
| Maintainer to confirm | `know_secure_design`, `know_common_errors`, `report_responses`, `static_analysis_fixed` |

Repository: <https://github.com/noru-tech/agent-change-control> (below, `REPO` stands for this URL).

## Basics

| Criterion | Level | Answer | Justification and evidence |
| --- | --- | --- | --- |
| `description_good` | MUST | Met | The README's first sentence says what it does: "Deterministic change control for code written by AI coding agents. Checks each change for independent human approval and emits SARIF and in-toto statements." REPO#readme |
| `interact` | MUST | Met | Obtain: README "Install" (Homebrew, crates.io, GitHub Releases). Feedback: issue forms at REPO/issues/new/choose. Contribute: REPO/blob/main/CONTRIBUTING.md |
| `contribution` | MUST | Met | Pull requests on GitHub; CONTRIBUTING describes the checks to run before opening one, how goldens and snapshots are updated, and the release process. REPO/blob/main/CONTRIBUTING.md |
| `contribution_requirements` | SHOULD | Met | CONTRIBUTING "Ground rules" (offline, deterministic, truthful unknowns, no LLM dependencies) and "Before opening a PR" (`cargo fmt`, `clippy -D warnings`, `cargo test`, `cargo deny`, `typos`); the PR template repeats the checklist. REPO/blob/main/CONTRIBUTING.md#ground-rules |
| `floss_license` | MUST | Met | MIT. REPO/blob/main/LICENSE |
| `floss_license_osi` | SUGGESTED | Met | MIT is OSI-approved. |
| `license_location` | MUST | Met | `LICENSE` at the repository root. REPO/blob/main/LICENSE |
| `documentation_basics` | MUST | Met | README (install, quick start, commands, outputs), `docs/` and the specification. REPO#readme |
| `documentation_interface` | MUST | Met | Inputs and outputs are specified in AI Change Provenance (REPO/blob/main/spec/ai-change-provenance.md) with JSON Schemas (REPO/tree/main/schemas); every rule, validation code and exit code has a page (REPO/blob/main/docs/rules/README.md, REPO/blob/main/docs/exit-codes.md); `acc --help`, `acc manpage`. |
| `sites_https` | MUST | Met | Repository, releases and crates.io are HTTPS only: REPO, https://crates.io/crates/agent-change-control |
| `discussion` | MUST | Met | GitHub issues and pull requests: searchable, URL-addressable, open to new participants, no proprietary client needed. REPO/issues |
| `english` | SHOULD | Met | All documentation is in English; reports are accepted in English. |
| `maintained` | MUST | Met | Regular releases (0.2.0 to 0.6.0 between 2026-09-19 and 2026-10-01) and weekly Dependabot updates. REPO/releases |

## Change Control

| Criterion | Level | Answer | Justification and evidence |
| --- | --- | --- | --- |
| `repo_public` | MUST | Met | REPO |
| `repo_track` | MUST | Met | Git history with author, committer and date; changes land through pull requests. REPO/commits/main |
| `repo_interim` | MUST | Met | Every change between releases is a public pull request merged to `main`. REPO/pulls?q=is%3Apr |
| `repo_distributed` | SUGGESTED | Met | Git. |
| `version_unique` | MUST | Met | Each release has a unique `X.Y.Z` version in `Cargo.toml` and a tag. REPO/tags |
| `version_semver` | SUGGESTED | Met | Semantic Versioning, stated in the changelog header. REPO/blob/main/CHANGELOG.md |
| `version_tags` | SUGGESTED | Met | Releases are tagged `vX.Y.Z`, and release binaries are built from the tag. REPO/tags |
| `release_notes` | MUST | Met | Keep a Changelog file with a section per release, plus the notes on each GitHub Release. REPO/blob/main/CHANGELOG.md |
| `release_notes_vulns` | MUST | N/A | No publicly known vulnerability has been fixed in any release (none has been reported). The changelog would name the identifier if one were. |

## Reporting

| Criterion | Level | Answer | Justification and evidence |
| --- | --- | --- | --- |
| `report_process` | MUST | Met | GitHub issues with bug report, feature request, rule, collector and specification-change forms. REPO/issues/new/choose |
| `report_tracker` | SHOULD | Met | GitHub issues. REPO/issues |
| `report_responses` | MUST | Met (maintainer to confirm) | No bug report has been filed since the first release on 2026-09-18 (0 issues on 2026-10-01), so there is nothing unacknowledged. Re-check before submitting. REPO/issues?q=is%3Aissue |
| `enhancement_responses` | SHOULD | Met | No enhancement request has been filed yet; see above. |
| `report_archive` | MUST | Met | GitHub issues and pull requests are public and searchable. REPO/issues?q=is%3Aissue |
| `vulnerability_report_process` | MUST | Met | SECURITY.md: email `security@noru.tech` or GitHub private vulnerability reporting. REPO/blob/main/SECURITY.md |
| `vulnerability_report_private` | MUST | Met | Private reporting through GitHub Security Advisories (REPO/security/advisories/new) or email; SECURITY.md asks reporters not to open public issues. REPO/blob/main/SECURITY.md#reporting-a-vulnerability |
| `vulnerability_report_response` | MUST | N/A | No vulnerability report received in the last 6 months (maintainer to confirm, including the security@ mailbox). SECURITY.md commits to acknowledgement within 5 business days. |

## Quality

| Criterion | Level | Answer | Justification and evidence |
| --- | --- | --- | --- |
| `build` | MUST | Met | `cargo build`; release builds through cargo-dist in GitHub Actions. REPO/blob/main/CONTRIBUTING.md#development |
| `build_common_tools` | SUGGESTED | Met | Cargo, the standard Rust build tool. |
| `build_floss_tools` | SHOULD | Met | Rust, Cargo and cargo-dist are FLOSS. |
| `test` | MUST | Met | Unit tests, `assert_cmd` CLI tests, insta snapshots, byte-exact fixture goldens, a loopback GitHub API replay and a published conformance corpus, all MIT. REPO/tree/main/tests, REPO/tree/main/conformance |
| `test_invocation` | SHOULD | Met | `cargo test`. |
| `test_most` | SUGGESTED | Unmet | Every rule has passing and failing fixtures and every validation code a reject vector, but branch coverage is not measured or published. To meet it: run `cargo llvm-cov` in CI and publish the figure. |
| `test_continuous_integration` | SUGGESTED | Met | `ci.yml` runs fmt, clippy, tests on Linux and macOS, MSRV, cargo-deny, typos, an independent RFC 8785 cross-check and the conformance corpus on every pull request. REPO/blob/main/.github/workflows/ci.yml |
| `test_policy` | MUST | Met | CONTRIBUTING: a new rule needs at least one passing and one failing fixture, a new rule a failing and a non-failing conformance vector, a new `ACV` code a reject vector; output changes need reviewed snapshots and goldens. REPO/blob/main/CONTRIBUTING.md#adding-a-rule |
| `tests_are_added` | MUST | Met | Recent major features added tests with the code: the conformance corpus (REPO/pull/24), ACP 0.3 with I-JSON input checks and their fixtures (REPO/pull/23), agent reviewer rules with `agent-review-*` fixtures (REPO/pull/18). |
| `tests_documented_added` | SUGGESTED | Met | CONTRIBUTING "Adding a rule" and "Conformance corpus", and the PR template checklist. REPO/blob/main/.github/PULL_REQUEST_TEMPLATE.md |
| `warnings` | MUST | Met | `RUSTFLAGS=-D warnings` and `cargo clippy --all-targets --all-features -- -D warnings` in CI. REPO/blob/main/.github/workflows/ci.yml |
| `warnings_fixed` | MUST | Met | CI fails on any compiler or clippy warning, so none can be merged. |
| `warnings_strict` | SUGGESTED | Met | All rustc and default clippy warnings are errors; `typos` and `cargo deny` also gate merges. (Clippy's `pedantic` group is not enabled.) |

## Security

| Criterion | Level | Answer | Justification and evidence |
| --- | --- | --- | --- |
| `know_secure_design` | MUST | Met (maintainer to confirm) | Maintainer attestation. The design shows the principles: fail-safe defaults (incomplete collection never passes, unknowns are never rounded), economy of mechanism (an offline, pure evaluator), least privilege (GET-only, fixed origin, no redirects, read-only token, per-job workflow permissions), input validation of every document. REPO/blob/main/SECURITY.md#scope-and-threat-model |
| `know_common_errors` | MUST | Met (maintainer to confirm) | Maintainer attestation. Mitigations in place for the relevant classes: untrusted input (size caps, I-JSON and schema validation, no remote schema resolution), terminal injection (control characters stripped), credential leakage (tokens never logged or serialized, fixed transport error messages), SSRF (fixed API origin), memory safety (Rust, no `unsafe` in `src/`). REPO/blob/main/SECURITY.md |
| `crypto_published` | MUST | Met | Uses only SHA-256 (digests over RFC 8785 bytes) and TLS through rustls. |
| `crypto_call` | SHOULD | Met | Cryptography comes from the `sha2` crate and rustls (through `ureq`); `acc` implements none itself. REPO/blob/main/Cargo.toml |
| `crypto_floss` | MUST | Met | `sha2` and rustls are FLOSS. |
| `crypto_keylength` | MUST | N/A | `acc` creates and configures no keys. Its only key exchange is TLS to `api.github.com`, negotiated by rustls, which supports no key sizes below the NIST minimums. |
| `crypto_working` | MUST | Met | No MD4, MD5, single DES, RC4 or Dual_EC_DRBG. Git commit SHA-1 identifiers are recorded as data from the forge, not used as a security mechanism by `acc`. |
| `crypto_weaknesses` | SHOULD | Met | SHA-256 for every digest; rustls offers only TLS 1.2 and 1.3 AEAD suites. |
| `crypto_pfs` | SHOULD | N/A | `acc` implements no key agreement protocol. (Its TLS client, rustls, only negotiates forward-secret ECDHE suites.) |
| `crypto_password_storage` | MUST | N/A | `acc` stores no passwords. |
| `crypto_random` | MUST | N/A | `acc` generates no keys or nonces; TLS randomness is rustls's. |
| `delivery_mitm` | MUST | Met | Releases are delivered over HTTPS (GitHub Releases, crates.io, the Homebrew tap), each archive with a SHA-256 checksum and a GitHub artifact attestation. REPO#verify-before-you-run |
| `delivery_unsigned` | MUST | Met | Checksums are fetched over HTTPS from the same release, and the README has users verify the Sigstore-signed artifact attestation with `gh attestation verify` before unpacking. REPO#verify-before-you-run |
| `vulnerabilities_fixed_60_days` | MUST | Met | No known unpatched vulnerability; `cargo deny check` (RustSec advisories) runs on every pull request and Dependabot updates dependencies weekly. REPO/blob/main/deny.toml |
| `vulnerabilities_critical_fixed` | SHOULD | Met | None reported; SECURITY.md sets the response process. |
| `no_leaked_credentials` | MUST | Met | No credentials in the repository: workflows use the job token and OIDC (crates.io Trusted Publishing, Sigstore signing); no stored publishing secret. REPO/blob/main/.github/workflows/publish-crate.yml |

## Analysis

| Criterion | Level | Answer | Justification and evidence |
| --- | --- | --- | --- |
| `static_analysis` | MUST | Met | CodeQL analyzes the Rust, Python and workflow code on every pull request, on `main` and weekly; clippy runs too. REPO/blob/main/.github/workflows/codeql.yml |
| `static_analysis_common_vulnerabilities` | SUGGESTED | Met | CodeQL's default security queries target common vulnerability classes (CWE). |
| `static_analysis_fixed` | MUST | N/A (maintainer to confirm) | No medium or higher exploitable finding has been reported by CodeQL or Scorecard so far (CodeQL was added on 2026-10-01). Check REPO/security/code-scanning before submitting; if alerts exist, answer Met once they are fixed. |
| `static_analysis_often` | SUGGESTED | Met | On every pull request and push to `main`, plus weekly. REPO/blob/main/.github/workflows/codeql.yml |
| `dynamic_analysis` | SUGGESTED | Unmet | No fuzzer runs, and branch coverage of the test suite is not measured, so it cannot count as the 80% alternative. To meet it: fuzz the I-JSON parser and the document loaders with `cargo fuzz`. |
| `dynamic_analysis_unsafe` | SUGGESTED | N/A | Written in Rust with no `unsafe` code in `src/`. |
| `dynamic_analysis_enable_assertions` | SUGGESTED | Met | Tests run as debug builds, with `debug_assert!`, overflow checks and clap's `debug_assert` of the CLI definition enabled. |
| `dynamic_analysis_fixed` | MUST | N/A | No dynamic analysis tool (in the criterion's sense) is run yet, so there are no findings to fix. |
