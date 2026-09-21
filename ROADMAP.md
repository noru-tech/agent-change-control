# Roadmap

The goal is a small, boring, trustworthy standard for one question: did a human independent of
the effective author approve this change? Everything below serves that question. Items are
ordered by intent, not by date; see the [issues](https://github.com/noru-tech/agent-change-control/issues)
for status.

## Released in 0.1

GitHub collection, offline evaluation of ACC001/ACC002/ACC003/ACC006, public schemas, byte-stable
manifests, dispositions, table/JSON/YAML/SARIF output.

## Shipped since 0.1

- **AI Change Provenance 0.1** published for comment, then **0.2** with agent reviewers
  (`spec/ai-change-provenance.md`): signed authorship and review documents as pre-verified
  input, evidence strength, and the opt-in `agent_review` mode with rules ACC007 to ACC010.
- **GitHub Action** (`noru-tech/agent-change-control@v0.2.0`) and **in-toto output**
  (`--format in-toto`), released in 0.2.0.
- **Derived evidence tier** (0.3.0): vendor `Co-Authored-By` trailers and Agent Trace records
  as lower-tier agent evidence, with the operator derived from commit authorship.

## Next

- **Adapters for more third-party provenance.** Agent Trace is read today (§3.4 of the spec).
  git-ai (an Agent Trace partner) and AgentDiff next, once their record formats are reviewed
  against the same requirement: written at authoring time, bound to a revision, naming the tool.
  Reading Agent Trace records from git notes and from a path in the repository at the head
  revision (via the contents API, no clone) so the GitHub Action needs no checkout of trace files.
- **Unnamed AI authorship.** An Agent Trace record with `contributor.type: ai` but no `tool.name`
  proves an agent wrote the change without naming it. The model has no unnamed agent; a spec
  change is needed before `acc` can record it truthfully.

## Planned

- **GitLab collector.** Merge requests, approvals and resource events. Known constraint: the
  approvals API exposes the *current* approval set; the approval history with commit binding and
  reset-on-push semantics comes from resource state events and notes, and some of it is tier-gated.
  Whatever cannot be retrieved will be marked incomplete, not inferred.
- **ACP trailers** (`Agent-Author:`, `Agent-Operator:`) as an explicit-tier declaration form for
  agents that commit but do not open pull requests. Reserved in spec 0.1; defined in 0.2 with
  precedence against the inline block.
- **Signature verification inside `acc`.** Signed provenance documents are read today as
  pre-verified input with a recorded verifier (§3.6). Verifying them in `acc` needs a trust-root
  configuration (which identities may sign which claims); until then the verifier statement is
  the caller's. Likely Sigstore bundles first.
- **In-toto layout export.** A layout fragment from a policy: `write` and `review` steps with
  the author and reviewer signing identities as functionaries and a threshold, mapping the
  `identity` dimension of `agent_review` to in-toto verification. Only worth it for users who
  already run in-toto verification.
- **`acc verify`** for signed attestations: unwrap the DSSE envelope, then apply what
  `acc validate` already does for unsigned Statements (subjects match the predicate, the predicate
  re-validates), and check the subjects against a commit range.
- **Dismissal timing** via GitHub's GraphQL timeline, so dismissed reviews stop forcing
  `reviews_complete: false`.
- **Deployment and bypass rules** (ACC004, ACC005 reserved): merged without required checks,
  branch-protection bypass, deploy of an unapproved head.
- **Pseudonymization mode** for sharing manifests outside the organization while keeping finding
  identifiers stable.
- **More forges**: Bitbucket, Azure DevOps, Gerrit, via the collector proposal template.

## Not planned

- Detecting AI-written code from its content. Declarations and records written at authoring time
  only.
- Any LLM in the evaluation path.
- Treating every bot as an agent, or any merger as an operator.
- Hosted services or telemetry in this repository. Noru's platform consumes the same manifests;
  the tool stays independent of it.
