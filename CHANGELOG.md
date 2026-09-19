# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/noru-tech/agent-change-control/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/noru-tech/agent-change-control/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/noru-tech/agent-change-control/releases/tag/v0.1.0
