# Enforcing the four-eyes principle for coding agents

*Why different accounts do not necessarily represent independent humans, and how to check for
independent human approval when coding agents contribute changes.*

The four-eyes principle still applies: a change needs review and approval from an independent
human. Coding agents expose a gap in how platforms enforce it, because the account that opens a
pull request may belong to an agent directed by the same human who approves it. This project
preserves the principle by recording the human operator and checking reviewer independence
against that person.

## The control everyone has

Every change-management framework contains the same sentence in different words: a change to
production code must be reviewed and approved by someone other than the person who made it. SOC 2
puts it in CC8.1. ISO/IEC 27001:2022 puts it in A.8.32 and A.5.3. PCI DSS puts it in 6.5.1. NIST
SP 800-53 spreads it over CM-3 and AC-5. Auditors call it separation of duties, engineers call it
the four-eyes rule, and platforms implement it as a branch-protection setting: *require one
approving review; the author cannot approve their own pull request.*

That setting has quietly been carrying the entire control. When an auditor samples merged pull
requests, the test is: does the approver's account differ from the author's account? If yes, the
control operated. The account is the proxy for the person, and the person is the proxy for the
judgment.

## Where the proxy breaks

A coding agent is not an account. It is a tool that turns a human's direction into code. The
human who directs it, the **operator**, chooses what to build, reads the plan, steers the edits and
decides when it is done. Their judgment is the judgment embodied in the change. The agent has
none to contribute to a review.

Now watch the branch-protection rule under three arrangements that are all common today.

**The operator opens the pull request.** Alice prompts an agent, reviews what it wrote, and opens
the PR from her own account. Bob approves. Two accounts, two humans, two judgments. The rule works.
Note what made it work: Alice was, in effect, honest about being the author.

**The agent opens the pull request.** The agent runs as `acme-agent[bot]`, or as a GitHub App, or
from a shared automation user, and opens the PR itself. Alice, who directed it, approves it. The
platform sees author `acme-agent[bot]` and approver `alice`. Different accounts: the rule is
satisfied. One human judgment: the control is gone. No log will ever show it, because every record
is accurate. The failure is in what the records mean.

**Nobody recorded who directed it.** Same as above, but now Alice is on holiday and Carol approves.
Did Carol independently review agent-written code? Or did Carol direct the agent from a shared
session and approve her own work? The records cannot say. An evaluator that assumes the merger is
the operator invents a separation that may not exist. One that exempts bots waives the control
entirely. Both produce a green checkmark.

The second arrangement is the one that should worry a control owner most, because it is the one
that scales. Agents that open their own pull requests are the productivity feature every vendor is
shipping. Each one moves the operator from the author column to the reviewer column, and the
platform rule rewards it.

## What enforcing the principle requires

The fix is not to detect AI-written code. Detection is unreliable, adversarial and, more to the
point, irrelevant: agent-written code with a genuinely independent review is exactly what the
control wants. The fix is to stop asking "are the accounts different?" and start asking the
question the control was always about:

> Did a human who is independent of the change's *effective author* approve the *current head* of
> the change, *before* it merged, and can we show the evidence?

Making that question machine-readable takes four things.

1. **Record the effective human, not the account.** When an agent wrote the change, the human
   whose judgment it embodies is the operator. That fact has to be declared, explicitly and in a
   delimited place, by the agent or the operator. Then independence is measured against the
   operator, and `acme-agent[bot]` → `alice` is recognized for what it is: self-approval.
2. **Bind approvals to commits and time.** An approval of an earlier head is not an approval of
   the head that merged. An approval submitted after the merge is not an approval. A comment does
   not withdraw an approval; a later request for changes does.
3. **Make "unknown" a first-class result.** If nobody declared the operator, the answer is
   *unknown*, reported as such, never rounded to pass because bots are exempt or to fail because
   bots are suspicious. If the review history could not be fully retrieved, no change in it can be
   clean.
4. **Make it reproducible.** The same inputs must give the same findings, byte for byte, on any
   machine, offline, without a model in the loop. Otherwise the evidence is an opinion.

Those four requirements are the whole of [AI Change Provenance 0.1](../spec/ai-change-provenance.md).
They fit in a small schema, a fenced block in a pull request description, and a few pages of rules
with exact conditions.

## What it looks like in practice

The agent, or the engineer operating it, adds one block to the pull request body:

````text
```agent-change-control
author: claude-code
operator: alice
```
````

A collector records the pull request, its commits, its full review history and its merger, and
resolves `alice` against the platform. An evaluator then asks whether a human other than Alice
approved the head commit before merge. The answer is one of four things per rule (`pass`, `fail`,
`unknown`, `not_applicable`), with every finding pointing back at the API records it was derived
from. The output is a manifest that can be re-evaluated to prove it was not edited, a SARIF file
for the code-scanning tab, or an in-toto statement to sign and file next to the release's build
provenance.

Wired into CI as a required check, it is a merge gate: an agent-written change waits for a human
who did not operate the agent. Run over last quarter's merges, it is the population an auditor
samples from, with the independence question already answered for every item.

## What it deliberately does not do

- It does not decide whether code was written by an AI. It only reads explicit declarations.
- It does not assume any bot is an agent, or that any agent's operator is the merger.
- It does not call a model, read the diff, or score the prose.
- It does not certify compliance. It produces evidence about a recorded scope, with its gaps
  marked, so a human can make that judgment.

The four-eyes principle is a good control. It just needs to know which eyes it is counting.

---

*Written by the team at [Noru](https://noru.tech). Discussion and contributions are welcome in the
[repository](https://github.com/noru-tech/agent-change-control).*
