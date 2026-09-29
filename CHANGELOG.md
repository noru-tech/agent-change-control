# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Conformance suite revision 2: three vectors for the opt-in `instructions` independence
  dimension (independent, dependent, unknown); 35 accept, 18 reject and 4 incomplete vectors.
- Tests that a review attestation naming another agent than a verified account mapping, and a
  human reviewer's review attestation naming an operator or instructions, are errors.

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

[Unreleased]: https://github.com/noru-tech/agent-change-control/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/noru-tech/agent-change-control/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/noru-tech/agent-change-control/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/noru-tech/agent-change-control/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/noru-tech/agent-change-control/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/noru-tech/agent-change-control/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/noru-tech/agent-change-control/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/noru-tech/agent-change-control/releases/tag/v0.1.0
