# Contributing to agent-change-control

Thanks for your interest in improving `acc`! Bug reports, new collectors, rules, output formats and
docs are all welcome. Noru's organization-wide [contributing guidelines](https://github.com/noru-tech/.github/blob/main/CONTRIBUTING.md)
apply here too.

## Ground rules

- Be respectful — see [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).
- **The evaluator is offline and deterministic.** `evaluate`, `validate` and `check` never touch the
  network, the clock or the environment. Identical normalized input, resolved policy and tool version
  must produce identical bytes. Keep it that way.
- **Truthful unknowns.** When evidence is missing, record `unknown` or mark collection incomplete.
  Never invent an operator, an approval or a timeline.
- **Keep forge logic out of the evaluator.** Collectors normalize into the public model under
  `src/model/`; rules only read the model.
- **No LLM dependencies, no proprietary APIs**, no writing-style or diff-size heuristics for
  authorship. Only explicit declarations and caller-maintained account mappings establish agent
  authorship.
- **Never recycle a public rule identifier.** Retired `ACC`/`ACV` codes stay retired.
- Keep the dependency tree pure Rust where possible so static musl builds keep working.

## Project layout

```
src/cli/          clap definitions, one file per subcommand, plus shared I/O helpers
src/model/        the public data model mirrored by schemas/ (enums for every closed vocabulary)
src/normalize/    schema validation, timeline checks, canonical ordering and canonical JSON
src/rules/        pure evaluation of normalized facts into findings and assessments
src/manifest/     manifest generation, validation (re-evaluation) and policy checks
src/policy/       rule catalogue, default policy, severity ranking
src/provenance/   agent declarations (PR body block) and the provenance-file convention
src/collectors/   forge collectors; github/ is the only one in Phase 1
src/output/       json, yaml, table and sarif renderers
schemas/          public JSON Schemas (draft 2020-12), embedded into the binary
docs/             model, policy, authorship and privacy notes
tests/            integration tests (assert_cmd), insta snapshots, fixtures under tests/fixtures
```

## Development

```bash
cargo build
cargo test
cargo run -- evaluate tests/fixtures/claude-clean/events.json --format table
```

Before opening a PR:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo deny check
typos
```

The GitHub collector tests use a local loopback HTTP server; no token is needed. Real authenticated
API testing is optional and should use a test repository you own.

Rendered output (table, SARIF) is covered by [insta](https://insta.rs) snapshots. If you change output
on purpose, review and accept the new snapshots with `cargo insta review` (install with
`cargo install cargo-insta`) and commit the `.snap` files.

## Fixtures and goldens

Every directory under `tests/fixtures/` with an `events.json` is evaluated with the default policy.
`expected-rules.json` lists the rule IDs that must fire (or a `validation_error` code for invalid
input). `expected-manifest.json` and `expected-findings.json` are byte-exact goldens of the canonical
output.

Golden updates are explicit: `UPDATE_GOLDENS=1 cargo test --test fixtures`. First review
`expected-rules.json` and the intended semantic change, then review the generated manifest and
findings diff. Never refresh goldens merely to hide a failure.

## Adding a rule

Change the public schema and the model together. Add the rule to `RULES` in `src/policy/`, add the
`rule_id` and rule name to the enums in `schemas/manifest.schema.json` and `schemas/policy.schema.json`,
implement the condition in `src/rules/`, add at least one passing and one failing fixture, document the
exact condition and evidence in `docs/policy.md`, and add the row to the README table.

## Releases

Releases are cut by tagging `vX.Y.Z` on `main`. **Always tag the current head of `main`**: GitHub
refuses to let the workflow token create a release whose target commit is behind a later change to
any `.github/workflows/*.yml` file, so a tag that trails a workflow edit fails in the `host` job with
`HTTP 403: Resource not accessible by integration`. If that happens, delete the tag and re-tag the
head. [cargo-dist](https://opensource.axo.dev/cargo-dist/) builds the binaries, installer, Homebrew
formula and GitHub Release. Run `dist plan` locally after changing `dist-workspace.toml`, and
regenerate `.github/workflows/release.yml` with `dist generate` rather than editing its steps. The
only hand edit is pinning the `uses:` actions to commit SHAs, as in `ci.yml`; `allow-dirty = ["ci"]`
in `dist-workspace.toml` lets dist tolerate that, and Dependabot keeps the pins current.

The manifest schema pins `generated.version` to the crate version. Bump both together.
