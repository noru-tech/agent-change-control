<!--
Thanks for contributing. This repository runs its own change control on every pull request
(.github/workflows/change-control.yml).

If a coding agent wrote this change, declare it: add a fenced code block whose info string is
agent-change-control, with an `author:` line naming the agent (for example claude-code or codex)
and an `operator:` line with your GitHub login. See examples/pull-request-body.md for the exact
shape. If a human wrote it, add nothing; only that exact block is read.
-->

## What

<!-- One or two sentences on the change. -->

## Why

<!-- The problem, the rule condition, or the specification section it relates to. -->

## Checks

- [ ] `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings && cargo test`
- [ ] Rule or schema changes: schema, model, `docs/policy.md`, README table and the specification updated together
- [ ] Output changes: snapshots reviewed with `cargo insta review`; goldens updated deliberately
- [ ] No real exports, manifests or tokens in the diff
