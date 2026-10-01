# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `codeql.yml`: CodeQL static analysis of the Rust, Python and workflow code on every pull request,
  on main and weekly; results go to code scanning.
- A stable documentation page for every rule (`docs/rules/ACC001.md` …), every validation code
  (`docs/rules/ACV001.md` to `ACV010.md`) and the exit codes (`docs/exit-codes.md`), with an index
  at `docs/rules/README.md`. Each rule page gives the rule in one sentence, why it matters, every
  outcome with its exact reason text, the documented control mapping, a failing and a passing
  example on a real fixture, how to fix it and how to record a disposition.
- SARIF rule descriptors carry `helpUri`, linking each rule to its page. The base URL is one
  constant (`DOCS_BASE_URL`), so the pages can move to a docs site later.
- Error messages that name a validation code end with `(see <url>)`, the code's page. Only the
  human-readable stderr line changes; JSON, YAML, SARIF results, manifests and the conformance
  result line are unchanged.
- `llms.txt` at the repository root (llms.txt convention): what `acc` is, install commands and
  links to the most important pages.
- `docs/openssf-best-practices.md`: prepared answers for the OpenSSF Best Practices passing level.
- Consistent CLI flags. `--format text` is an alias of `table` wherever `table` is accepted, with
  identical bytes. A global `-v`/`--verbose` prints extra diagnostics on stderr (resolved policy,
  format and destination, counts, whether a token is used); stdout is unchanged. A global
  `--no-color`, and a non-empty `NO_COLOR`, switch off color in clap's help and usage errors;
  `acc`'s own output was and stays uncolored.
- `acc validate --format text|json` and `--output FILE`. JSON is a new machine output: one result
  object with `valid`, `message`, and `code` and `help_uri` when a validation code applies (README,
  "Output formats and exit codes"). Text, the default, is unchanged.
- `acc completions SHELL --output FILE` and `acc manpage --output FILE` write to a file.
- Errors with a fix and a docs link. The common failures print two more stderr lines after
  `error: …`: `help: …` (what to do) and `see: <url>` (the section of `docs/exit-codes.md`, or the
  policy page): a missing or rejected token (exit 5), a rate limit, a forbidden or missing
  repository and transport failures (exit 6; a 403 with an exhausted rate limit is told apart from a
  permission problem), a missing `--repo`/`GITHUB_REPOSITORY`, a bad `--since`/`--until`/`--as-of`
  or a reversed window (exit 2), and an invalid policy file (exit 3). An incomplete collection
  (exit 4) prints `warning: collection incomplete: <reason>` with the same lines, unless `-q`.
  Messages, exit codes and every machine-readable output are unchanged. `Failure` gains optional
  `hint` and `see` fields.
- Zero-config collection: `acc scan` (and `export`) works with no arguments in a GitHub clone.
  GitHub is the default forge (`acc scan acme/api` is `acc scan github acme/api`); `OWNER/REPO`
  defaults to `GITHUB_REPOSITORY`, else the github.com `origin` remote (https, ssh and
  `git@github.com:` forms), which `pr` now also falls back to; a missing `--until` is now and a
  missing `--since` is 30 days before the end, in UTC. The resolved repository and window are
  printed on stderr and the window is recorded in the output exactly as if passed, so evaluation
  stays deterministic. Only the collecting commands read the clock (`ACC_NOW` replaces it in
  tests).
- `ACC_GITHUB_API_URL`, testing only: points the binary at the loopback replay server. Only
  `http://127.0.0.1:PORT`/`http://localhost:PORT` without a token is accepted.

### Changed
- README: four headings are now the questions people ask ("How do I enforce separation of duties
  for AI coding agents?", "What does acc not do?", "How does acc decide whether a change was
  independently approved?", "Which rules does acc check?"); the old anchors still resolve. Each
  rule in the Rules table links to its page.
- `tests/metadata.rs` checks that every rule and validation code has a page and that the
  documented action pins name the crate version.

## [0.5.4] - 2026-10-01

### Changed
- Dependencies updated: `jsonschema` 0.29 → 0.58 (`ACV010` still names the same violation:
  the one 0.29 reported first), `sha2` 0.10 → 0.11, `base64` 0.22 → 0.23, `thiserror` 2.0.21,
  and the pinned `crate-ci/typos` action to v1.50.3. Output is unchanged; `deny.toml` now allows
  the `Zlib` license (`foldhash`, through `jsonschema`).
- Workflows grant write permissions per job instead of workflow-wide: `release.yml` gives
  `contents: write` only to the jobs that create and upload the release, and `attest.yml` and
  `conformance-release.yml` give `id-token` and `attestations` write only to their signing job.

## [0.5.3] - 2026-09-30

Distribution, documentation and CI only; no change to the tool's behaviour.

### Added
- A CycloneDX SBOM (`agent-change-control.cdx.xml`) attached to every release, generated by
  `cargo-cyclonedx` through dist (`cargo-cyclonedx = true`; `release.yml` regenerated with dist
  0.33.0 and its action pins re-applied).
- Every release publishes the crate to crates.io: `release.yml` calls `publish-crate.yml` as a
  dist custom publish job after the GitHub Release is up. It uses Trusted Publishing (a
  short-lived token from `rust-lang/crates-io-auth-action`, no stored secret), skips a version
  that is already published, and can be run by hand with a tag as a fallback.
- `scorecard.yml`: OpenSSF Scorecard analysis, published to scorecard.dev, with a README badge.
- `KNOWN-LIMITATIONS.md`, collecting what `acc` does not do and cannot know.
- A feature request issue form, a VHS script for the README recording (`docs/demo.tape`), and
  weekly grouped Dependabot updates for Cargo dependencies.

### Changed
- The crate is published on crates.io as `agent-change-control`: README shows its badge and the
  `cargo binstall` / `cargo install` lines again. Also in the README: a "Verify
  before you run" section with the exact attestation and checksum commands; a Trust section;
  sections reordered (install first, then quick start, what it does and what it is not).
- `Cargo.toml`: a one-sentence description, and `in-toto` and `provenance` keywords in place of
  `github` and `cli`.
- `SECURITY.md` links GitHub private vulnerability reporting directly and gives the full
  verification commands.

## [0.5.2] - 2026-09-29

Conformance suite revision 2 in a signed release. No change to the rules or the output beyond
the recorded tool version.

### Added
- Conformance suite revision 2: three vectors for the opt-in `instructions` independence
  dimension (independent, dependent, unknown); 35 accept, 18 reject and 4 incomplete vectors.
- Tests that a review attestation naming another agent than a verified account mapping, and a
  human reviewer's review attestation naming an operator or instructions, are errors.
- The Zenodo concept DOI (10.5281/zenodo.23042500) in `CITATION.cff`, a DOI badge and a Citing
  section in the README.

### Changed
- The self-check and attest workflows run the published 0.5.1.
- The README's spec badge and prose name AI Change Provenance 0.3; `tests/metadata.rs` now
  requires every "AI Change Provenance X.Y" in the README and docs to name the current version,
  and the DOI to be the same everywhere.
- Specification §9 no longer names a single conformance suite revision (0.3 revision 3):
  revision 1 was published at v0.5.0, and later revisions only add vectors or correct expected
  results.

## [0.5.1] - 2026-09-29

The first release archived on Zenodo. No change to the specification, the rules or the output
beyond the recorded tool version.

### Added
- `.zenodo.json`: Zenodo archives each published release and mints a DOI, reading the deposit's
  metadata from this file; `tests/metadata.rs` keeps it in agreement with `CITATION.cff` and
  `Cargo.toml`.

### Changed
- The self-check and attest workflows run the published 0.5.0, and the attestations they make
  carry the `v0.3` predicate type.
- Version references across the documentation are current: supported versions in `SECURITY.md`,
  policy examples at `0.3`, the schema and model descriptions, and editorial wording in the
  specification that still said 0.1 (0.3 revision 2, no normative change).

## [0.5.0] - 2026-09-29

AI Change Provenance 0.3: RFC 8785 serialization and I-JSON input. Rules and verdicts are
unchanged; digests and finding identifiers are not.

### Changed
- Every byte sequence acc hashes or signs is the RFC 8785 (JCS) serialization of the normalized
  value (spec §8.2), through one module (`src/canonical`). The trailing newline that was part of
  every preimage is gone, so finding identifiers, `generated.source_digest` and the `sha256`
  subject digest change. Every finding, assessment and verdict across the fixture set is the same.
- JSON outputs (`--format json`, `in-toto`, `sarif`, and `export`) are written as exactly the JCS
  bytes, with no trailing newline, so the SHA-256 of a manifest file is its digest. JSON Lines
  keep one newline after each Statement as a separator.
- Manifests are version `0.3`, the predicate type is
  `https://noru.tech/spec/ai-change-provenance/v0.3`, and exports and resolved policies are
  written as `0.3`. Exports and policies of `0.1` and `0.2` remain valid input.

### Added
- `legacy_ids` on findings: the identifier the same finding had under 0.2, kept for one minor
  version. `acc check` carries a disposition over by either identifier.
- I-JSON input constraints (spec §8.2), enforced by `evaluate`, `validate`, `check`, policy
  loading and attestation loading: ACV005 non-integer number, ACV006 integer outside
  ±(2^53 − 1), ACV007 unpaired surrogate, ACV008 duplicate member name, ACV009 nesting deeper
  than 128 (rejected without parsing further).
- `acc validate` still accepts ACP 0.2 manifests and `v0.2` Statements, including attestations
  made by acc 0.4.0: it recomputes their digests with the legacy canonicalization and says so.
- `acc evaluate --conformance-json VECTOR` prints the single-line result object of the
  evaluator conformance contract (verdict, failing codes, assessments, manifest digest) and exits
  0 evaluated, 3 invalid, 4 incomplete.
- The evaluator conformance corpus (`conformance/`, spec §9): 32 accept, 18 reject and 4
  incomplete vectors under an external-verifier contract, a standard-library Python harness
  (`run.py`) that writes `conformance-report.json`, a GitHub Action (`conformance/action.yml`),
  and a generated `CORPUS-DIGESTS.txt` that `conformance-release.yml` signs at each tag. Every
  reject vector is one mutation of an accept vector. CI runs the corpus against `acc` and against
  builds with one rule, or the serialization, deliberately broken, and requires those to fail.
- ACV010 for input that does not conform to its schema; spec §6.6 lists every validation code.
- CI recomputes the goldens' digests with an independent RFC 8785 implementation in Python
  (`rfc8785`), without acc's code.

## [0.4.0] - 2026-09-21

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
- The review document (`schemas/review.schema.json`, predicate type
  `https://noru.tech/spec/ai-change-provenance/review/v0.1`): a signed statement of who
  reviewed what and decided what, naming the reviewer in the predicate. Read by
  `--attestations`, it upgrades the forge's matching review (evidence becomes `signed`) and
  never creates one; unmatched documents are recorded with `matched: false`. For an agent
  reviewer the review records `agent` (operator, identity, instructions owner, model).
- `--verification PATH` (and the `verification` action input): the JSON that
  `gh attestation verify --format json` writes, loaded as signed attestations with the `signer`
  the verifier established (identity and issuer) recorded on the attestation.
- Agent vendors: agent actors carry `vendor` from a built-in registry (`claude-code` →
  `anthropic`, `copilot` → `github`, `codex` → `openai`, …) extended by
  `--agent-vendor AGENT=VENDOR` and the `agent-vendor` action input.
- Pull request labels are collected as `labels` on changes.
- `examples/review.intoto.json`.
- Rules for agent reviewers: ACC007 agent approval recorded (`info`, an observation), ACC008
  same-vendor write and review (`high`), ACC009 agent approval lacks required independence
  (`high`) and ACC010 agent approval without signed identity (`warning`). ACC007 and ACC008
  evaluate under every policy; ACC009 and ACC010 only under `agent_review`.
- The opt-in `agent_review` policy block (`satisfies_independence`, `require`,
  `minimum_evidence`, `labels`). When on, a signed agent approval independent of the effective
  author on every required dimension (operator, provider, identity; `instructions` opt-in)
  satisfies ACC001 and ACC003 with a reason that names the policy. Unknown on a required
  dimension never yields a clean result. Default policy behaviour is unchanged.
- Fixtures `agent-review-*` for every case of the agent-reviewer design's worked table; fixture
  directories may carry a `policy.yml`.

### Changed
- **Specification 0.2.** The second form of qualifying approval under `agent_review` (§6.1),
  the independence dimensions (§6.5), ACC007 to ACC010 (§6.2), version compatibility (§12).
  Manifests and the resolved policy are version `0.2`; exports and policies written as `0.1`
  remain valid input. The attestation predicate type is
  `https://noru.tech/spec/ai-change-provenance/v0.2`; the provenance and review documents stay
  at 0.1. Earlier in this release: specification 0.1 revision 3 (§4 merge commit, §7.3
  attestation forms and the verifier's subject check), revision 4 (§3.2 provenance
  attestations, §3.6 evidence strength and pre-verified input, §6.3 evidence minimums) and
  revision 5 (§3.7 review document, signer records, vendors, labels).

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

[Unreleased]: https://github.com/noru-tech/agent-change-control/compare/v0.5.4...HEAD
[0.5.4]: https://github.com/noru-tech/agent-change-control/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/noru-tech/agent-change-control/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/noru-tech/agent-change-control/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/noru-tech/agent-change-control/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/noru-tech/agent-change-control/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/noru-tech/agent-change-control/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/noru-tech/agent-change-control/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/noru-tech/agent-change-control/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/noru-tech/agent-change-control/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/noru-tech/agent-change-control/releases/tag/v0.1.0
