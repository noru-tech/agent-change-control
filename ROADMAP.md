# Roadmap

The goal is a small, boring, trustworthy standard for one question: did a human independent of
the effective author approve this change? Everything below serves that question. Items are
ordered by intent, not by date; see the [issues](https://github.com/noru-tech/agent-change-control/issues)
for status.

## Released in 0.1

GitHub collection, offline evaluation of ACC001/ACC002/ACC003/ACC006, public schemas, byte-stable
manifests, dispositions, table/JSON/YAML/SARIF output.

## Next

- **AI Change Provenance 0.1 published for comment** — `spec/ai-change-provenance.md`. Feedback
  wanted from control owners, auditors and agent vendors before it is frozen.
- **GitHub Action** — `noru-tech/agent-change-control@v…` evaluates the current pull request,
  writes SARIF and a job summary. Included in the repository; released with the next tag.
- **in-toto attestation output** — `--format in-toto`, an unsigned Statement v1 with the manifest
  as predicate, ready for DSSE signing.

## Planned

- **GitLab collector.** Merge requests, approvals and resource events. Known constraint: the
  approvals API exposes the *current* approval set; the approval history with commit binding and
  reset-on-push semantics comes from resource state events and notes, and some of it is tier-gated.
  Whatever cannot be retrieved will be marked incomplete, not inferred.
- **Commit trailers** (`Agent-Author:`, `Agent-Operator:`) as a third declaration form, so agents
  that commit but do not open pull requests can declare authorship where it happens. Reserved in
  spec 0.1, to be defined in 0.2 with precedence rules against the inline block.
- **Signed provenance.** Verification of DSSE-wrapped provenance documents (§3.2) so that a
  declaration can be authenticated, not merely recorded. Likely Sigstore first.
- **`acc verify`** for attestations produced by `--format in-toto`: unwrap, check subjects against
  a commit range, re-validate the predicate.
- **Dismissal timing** via GitHub's GraphQL timeline, so dismissed reviews stop forcing
  `reviews_complete: false`.
- **Deployment and bypass rules** (ACC004, ACC005 reserved): merged without required checks,
  branch-protection bypass, deploy of an unapproved head.
- **Pseudonymization mode** for sharing manifests outside the organization while keeping finding
  identifiers stable.
- **More forges**: Bitbucket, Azure DevOps, Gerrit, via the collector proposal template.

## Not planned

- Detecting AI-written code from its content. Declarations only.
- Any LLM in the evaluation path.
- Treating every bot as an agent, or any merger as an operator.
- Hosted services or telemetry in this repository. Noru's platform consumes the same manifests;
  the tool stays independent of it.
