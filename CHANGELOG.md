# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `merge_commit_sha` on changes: the commit a merge produced, recorded by the GitHub collector
  for merged pull requests and null otherwise. The key is optional on input, so exports written
  by earlier releases remain valid.
- A merged change's merge commit is a second attestation subject (`<change id>:merge`), so an
  attestation is found by the commit reachable from the target branch after a squash or rebase
  merge as well as by the reviewed head. Open changes, and merged changes whose merge commit the
  forge did not report, keep a head subject only.
- `--format in-toto-jsonl`: JSON Lines, one unsigned in-toto Statement per change in change ID
  order, each with a manifest covering that change alone as its predicate and unchanged finding
  IDs. Output names ending in `.intoto.json`, `.intoto.jsonl` or `.jsonl` select the attestation
  formats.
- `acc validate` accepts an in-toto Statement or JSON Lines of Statements: it checks the Statement
  schema (`schemas/statement.schema.json`), the predicate type, the predicate as a manifest, and
  that the subjects are exactly those the predicate's changes produce.
- `docs/in-toto.md` documents the predicate in the in-toto predicate template;
  `docs/design/in-toto-integration.md` records the design.
- `acc validate` also accepts a Statement whose single subject is the `sha256` of the canonical
  predicate, the form GitHub artifact attestations and `cosign attest-blob` produce, and
  recomputes that digest from the predicate.
- `docs/signing.md`: signing the verdict with `actions/attest` (dogfooded on this repository for
  every merged pull request by `.github/workflows/attest.yml`) or with cosign, and verifying it
  with `gh attestation verify` plus `acc validate`.

- `--attestations PATH` (repeatable) on `scan`, `export` and `pr`, and matching action inputs:
  in-toto Statements, DSSE envelopes or Sigstore bundles (`.json`, `.jsonl`, files or
  directories) bound to changes by head commit. A Statement of predicate type
  `https://noru.tech/spec/ai-change-provenance/provenance/v0.1` establishes agent authorship at
  the explicit tier and must agree with any inline declaration.
- `--verified-by TEXT`: the caller's statement of who verified the attestations' signatures,
  recorded verbatim. `acc` does not verify signatures. Evidence is `signed` only from a container
  that carried a signature and with a recorded verifier; otherwise the claims are `declared`.
- Evidence kind `signed`, the trust ordering `derived < declared < observed < signed`, and an
  `attestations` record in exports (optional on input) that every `signed` evidence entry must
  resolve to; validation rejects `signed` evidence without a signed, verified record (ACV003).
- Policy keys `minimum_authorship_evidence` (default `derived`) and `minimum_review_evidence`
  (default `observed`). An operator below the minimum does not name the effective human (ACC006
  fails, independence unknown); an approval below the minimum does not qualify.
- `examples/provenance.intoto.json`, the provenance document as a Statement to sign.

### Changed
- Specification 0.1 revision 3: §4 records the merge commit, §7.3 defines both attestation
  forms and the verifier's subject check. Revision 4: §3.2 provenance attestations, §3.6
  evidence strength and pre-verified input, §6.3 evidence minimums.

## [0.3.1] - 2026-09-19

### Fixed
- The GitHub Action's description is under the Marketplace's 125-character limit, so the action
  can be listed. No functional change.

## [0.3.0] - 2026-09-19

### Added
- A derived evidence tier below declarations: vendor `Co-Authored-By` trailers (built-in registry
  for Claude Code and Copilot, `--agent-trailer EMAIL=AGENT` to extend, `--ignore-trailers` to
  disable) and Agent Trace records (`--agent-trace PATH`) bound to a change's commits by
  `vcs.revision`. Both yield `derived` confidence; the operator is derived only when one human
  account authored every commit. Declarations and account mappings take precedence, and derived
  records naming two agents are not interpreted. Evidence sources `commit_trailer` and
  `agent_trace`; the table marks derived operators.
- Matching `agent-trailer`, `ignore-trailers` and `agent-trace` inputs on the GitHub Action.
- Specification 0.1 revision 2: §3.4 derived evidence, §11 relationship to Agent Trace.

### Changed
- The repository's self-check now installs the published 0.2.0 release.

## [0.2.0] - 2026-09-19

### Added
- `--format in-toto`: an unsigned in-toto Statement v1 whose subjects are the evaluated changes'
  head commits and whose predicate is the manifest, ready for DSSE signing.
- A composite GitHub Action (`action.yml`) that installs a checksum- and attestation-verified
  release, runs `acc pr` on the current pull request, writes SARIF and a job summary, and exposes
  the exit code. This repository runs it on its own pull requests.
- The AI Change Provenance 0.1 specification (`spec/ai-change-provenance.md`), a narrative on why
  account-based four-eyes fails for coding agents (`docs/four-eyes.md`), a control mapping to
  SOC 2, ISO/IEC 27001, PCI DSS, NIST SP 800-53 and SSDF (`docs/control-mapping.md`), a roadmap,
  examples, issue and pull request templates, CODEOWNERS and a citation file.

## [0.1.0] - 2026-09-18

### Added
- Initial `acc` command-line tool: `scan`, `export`, `evaluate`, `validate`, `check` and `pr`, plus
  `completions` and `manpage`.
- Phase 1 public JSON Schemas (`change-events`, `manifest`, `policy`, `provenance`), a forge-neutral
  model and the agent provenance convention.
- Deterministic offline evaluation of ACC001, ACC002, ACC003 and ACC006 with `pass`, `fail`,
  `unknown` and `not_applicable` assessments per change.
- GitHub collection of pull requests, reviews, merges, commit identities and explicit agent
  declarations, over a fixed origin with no redirects and bounded pages and bodies.
- Table, JSON, YAML and SARIF 2.1.0 output; stable finding IDs and a source digest.
- Completeness and unknown assessments, integrity validation and dated dispositions.
- Fixture goldens, CLI tests, loopback GitHub tests, snapshot tests and CI.

[Unreleased]: https://github.com/noru-tech/agent-change-control/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/noru-tech/agent-change-control/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/noru-tech/agent-change-control/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/noru-tech/agent-change-control/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/noru-tech/agent-change-control/releases/tag/v0.1.0
