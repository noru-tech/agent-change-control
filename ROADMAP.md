# Roadmap

The goal is a small, boring, trustworthy standard for one question: did a human independent of
the effective author approve this change? Everything below serves that question. Items are
ordered by intent, not by date; see the [issues](https://github.com/noru-tech/agent-change-control/issues)
for status.

## Released in 0.1

GitHub collection, offline evaluation of ACC001/ACC002/ACC003/ACC006, public schemas, byte-stable
manifests, dispositions, table/JSON/YAML/SARIF output.

## Shipped since 0.1

- **AI Change Provenance 0.1** published for comment: `spec/ai-change-provenance.md`.
- **GitHub Action** (`noru-tech/agent-change-control@v0.2.0`) and **in-toto output**
  (`--format in-toto`), released in 0.2.0.
- **Derived evidence tier** (0.3.0): vendor `Co-Authored-By` trailers and Agent Trace records
  as lower-tier agent evidence, with the operator derived from commit authorship.

## Next

- **AI reviewer classification.** Teams already let an AI reviewer approve low-risk changes, and
  `acc` can currently only say "not a human". Proposed design, for discussion in an issue before
  any rule ships:
  - *Classification.* A reviewer account mapped with `--agent-account` is already an agent and
    never counts as a human approval. Add a vendor to agent actors (registry: `claude-code` and
    `claude-code-review` → `anthropic`, `copilot` → `github`, …) so two agents can be compared.
  - *ACC007, agent approval recorded* (`info`): an agent's approval is on the current head. Not a
    violation; makes the population of AI-reviewed changes visible.
  - *ACC008, same-vendor write and review* (`high`): the effective author is an agent and the only
    approvals of the head are by agents of the same vendor. One model checking its own work is
    the agent-era self-approval and deserves its own identifier.
  - *Policy.* `agent_review.satisfies_independence: false` by default. When a team sets it to
    `true` for changes carrying an allow-listed forge label (collected as a new `labels` field),
    an agent approval by a *different* vendor may satisfy ACC001/ACC003 for those changes only,
    and the manifest records that the policy, not a human, made the call. Risk classification
    itself stays outside `acc`.
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
- **Signed review evidence.** The review predicates in circulation carry the reviewer in the
  signature, so they cannot be read without verification. Either a review predicate that names
  the reviewer, or in-`acc` verification, unlocks `minimum_review_evidence: signed`.
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
