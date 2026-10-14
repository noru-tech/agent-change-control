# Known limitations

What `acc` does not do, what it cannot know, and where its evidence stops. Each item links to the
document that describes it in detail. Planned work is on the [roadmap](ROADMAP.md).

## What `acc` deliberately does not do

- **It does not detect AI-written code.** Agent authorship comes only from explicit, delimited
  sources: the `agent-change-control` block in the pull request description, verified account
  mappings, signed provenance documents, and (as a lower, `derived` tier) `Co-Authored-By`
  trailers and Agent Trace records. Prose, style and bot names never establish authorship
  ([authorship](docs/agent-authorship.md)).
- **It does not treat bots as agents**, and does not guess that the merger, the pull request
  creator or the first reviewer operated the agent. An unknown operator stays unknown (ACC006),
  never rounded to pass or fail.
- **It does not call a model**, read the diff, or score the prose.
- **It does not certify compliance.** A clean result is a statement about the recorded scope, with
  its gaps marked, not a certification ([control mapping](docs/control-mapping.md)).

## Evidence and trust

- **A declaration is declared evidence, not authenticated identity.** Pull request descriptions
  are mutable, and a declaration can be forged or omitted. ACP makes the claim explicit and
  auditable; it does not authenticate it
  ([spec §10](spec/ai-change-provenance.md#10-security-and-privacy-considerations)).
- **`acc` does not verify signatures.** Signed authorship and review documents are read as
  pre-verified input; the manifest records who said they verified them (`--verification`,
  `--verified-by`). `acc verify` likewise takes verified verdicts, and checks only that the
  verifier's signer identity is one the gate names. Signature verification inside `acc` is planned
  ([signing](docs/signing.md)).
- **A digest detects inconsistency, not forgery.** Re-validating a manifest catches edits to its
  derived parts; it does not prove that the embedded events are what the forge held, or that an
  entire export was not replaced ([model](docs/model.md), [SECURITY.md](SECURITY.md)).
- **Signing a verdict does not make its facts true.** An attestation says that this evaluator,
  run by this identity, saw these facts; a declaration inside it is still a declaration
  ([signing](docs/signing.md)).
- **`signed` review evidence cannot yet come from the forge.** Forge-observed approvals are
  `observed`; a policy that sets `minimum_review_evidence: signed` needs signed review
  attestations ([policy](docs/policy.md)).

## Collection

- **GitHub only.** A GitLab collector is planned; other forges are not supported.
- **A best-effort snapshot, not an atomic archive.** GitHub data is mutable and read in several
  requests. Identity changes, deleted users and edited declarations cannot be reconstructed
  reliably from current REST data. A review or merge whose account no longer exists is recorded
  against `unknown:unavailable`.
- **Bounded collection.** Pages are capped (`--max-pages`, default 100 per endpoint, maximum
  1000) and responses at 8 MiB. Anything not retrieved is marked incomplete (exit 4), and
  incomplete collection can never produce a clean result.
- **Dismissed reviews force `reviews_complete: false`.** The REST API does not expose when a review
  was dismissed, so that history is flagged incomplete.
- **The token's view is the limit.** Data the token cannot see is missing from the snapshot.

## Evaluation

- **Mixed human authorship is not resolved.** The pull request opener is the effective author
  unless explicit agent evidence overrides it.
- **From trailers, the operator is derived only when one human authored every commit.**
- **Unnamed AI authorship cannot be recorded.** An Agent Trace record that marks AI authorship
  without naming the tool is ignored until the specification defines an unnamed agent.
- **Deployments are not evaluated.** `acc verify` gates a deploy you run, from the commits and
  signed verdicts you give it; it does not read what was deployed. A rebase merge of several
  commits leaves all but the last uncovered ([verify](docs/verify.md#limits)).
- **Protection bypass (ACC005) is only as visible as GitHub's rule suites**: an actor a ruleset
  exempts, and an administrator's override of classic branch protection, leave no record that
  API exposes, so an empty record is `unknown`, never a pass. Rule suites need administration read
  access and reach back one month ([ACC005](docs/rules/ACC005.md#limits)).
- **Required checks (ACC004) are judged against today's rules**, not the rules in force at the
  merge, which GitHub does not record; and a check posted only to the test merge commit, whose
  SHA GitHub discards after the merge, reads as never run ([ACC004](docs/rules/ACC004.md#limits)).
- **Reviewer team membership (ACC011) is read at collection**, not at review time: GitHub exposes
  current membership only. Someone who left the team after approving does not count, and someone
  who joined afterwards does; the manifest records when membership was observed.
- **acc reads a risk tier, it does not derive one.** The tier comes from a change-risk assessment;
  `acc` checks its evidence against `minimum_risk_evidence` only when the tier would lower the
  requirements ([policy](docs/policy.md#risk-tiers)).

## Privacy

- **Manifests and attestations contain personal data**: names, usernames, approval and merge
  history, work activity. Keep real exports out of public repositories, and treat anything
  published to a transparency log as permanent ([privacy](docs/privacy.md)).
- **There is no automatic pseudonymization mode yet**; pseudonymize consistently by hand before
  sharing.
