# Control mapping

Where the evidence `acc` produces lands in common frameworks. This is a map from findings to
control objectives, written to help a control owner decide what to collect and how to present
it. It is not a compliance claim, and no evaluator output replaces auditor judgment about design
and operating effectiveness.

## What the evidence is

For every change in a window, the manifest records who opened it, who is its effective author
(human or agent), who operated the agent, who reviewed it and when, against which commit, and who
merged it, with a reference to the API record for each fact. Every rule yields `pass`, `fail`,
`unknown` or `not_applicable` per change. `unknown` and *incomplete* are reported, never rounded.

Two properties matter for evidence handling:

- **Reproducible.** `acc validate` re-evaluates the embedded facts and requires byte-identical
  output. A manifest that has been edited after the fact does not validate.
- **Scoped.** A manifest covers one repository and one inclusive UTC window of merge times. The
  population is explicit, which is what a sampling procedure needs.

## Mapping

| Framework | Control objective (paraphrased) | What `acc` provides |
| --- | --- | --- |
| SOC 2 (2017 TSC) | CC8.1 — changes are authorized, designed, tested, approved and implemented under change-management procedures | Per-change evidence of independent approval before merge (ACC003), agent changes with independent review (ACC001), and self-approval (ACC002) |
| ISO/IEC 27001:2022 Annex A | A.8.32 Change management; A.5.3 Segregation of duties; A.8.25 Secure development life cycle | Population of merged changes with author, approver and merger identities; failures of segregation surfaced as findings |
| PCI DSS v4.0.1 | 6.5.1 — changes to system components follow change-control procedures including approval by authorized parties | Approval facts bound to the merged head commit; open items reported rather than assumed |
| NIST SP 800-53 Rev. 5 | CM-3 Configuration change control; CM-5 Access restrictions for change; AC-5 Separation of duties | Machine-checkable separation between the effective author (including an agent's operator) and the approver |
| NIST SP 800-218 (SSDF) | PS.1 Protect code from unauthorized access and tampering; PW.7 Review and/or analyze human-readable code | Evidence that human review occurred, by whom, and whether the reviewer was independent of the author |
| ISO/IEC 42001:2023 | Human oversight of AI systems and accountability for AI-assisted outputs (Annex A, AI system life cycle controls) | Explicit record of the human operator of each agent-written change (ACC006 when missing) and of independent human approval |
| SLSA Source track (draft) | Reviewed changes on protected branches | The independence assessment per merged change, with review-to-commit binding |

## Reading the findings

| Finding | What it says | Typical control response |
| --- | --- | --- |
| ACC001 | An agent-written change merged (or is open) without a human independent of its operator approving the current head | Treat as a missed four-eyes review; retrospective review and a disposition, or a required status check going forward |
| ACC002 | The effective human author, or the agent's operator, approved the change themselves | Self-approval; the same response as for any author-approved change |
| ACC003 | A merged change has no qualifying independent approval | Missed review or an approval that did not cover the merged head |
| ACC006 | An agent wrote the change and no human operator is recorded | Not a violation; a gap in provenance. Fix the declaration at the source (agent integration or PR template) |
| ACC007 | An agent approved the change (observation, info) | None by itself; the population of agent-approved changes for sampling |
| ACC008 | An agent wrote the change and only agents of the same vendor approved it | One model checking its own work; treat as a missed independent review |
| ACC009 | Under `agent_review`, the only agent approvals are dependent on the author (operator, vendor, signing identity or instructions) | Missed independence; the same response as for self-approval |
| ACC010 | Under `agent_review`, an agent approval is not backed by a verified signed review document | Do not count the approval; fix the reviewer's signing integration |
| exit 4 (incomplete) | Part of the population or a review history could not be retrieved | Do not report the window as clean; re-collect, or document the gap |

Dispositions (`accepted`, `remediated`, `false_positive`, with owner, date, rationale and optional
expiry) live in the manifest next to the finding, so the exception record and the evidence travel
together.

## Presenting to an auditor

1. Scan the period: `acc scan github ORG/REPO --since 2026-07-01 --until 2026-09-30 -o q3.json`.
2. Record dispositions for accepted exceptions; keep the manifest under access control (it
   contains names and work activity; see [privacy](privacy.md)).
3. Provide `acc validate q3.json` output alongside the manifest, and `acc check q3.json --as-of
   DATE` for the threshold decision on the reporting date.
4. Optionally sign `acc evaluate --format in-toto` output and store it with the release
   attestations, so the change-control evidence and the build provenance for a release share a
   subject.

The narrative behind the mapping is in [The four-eyes principle is broken for coding
agents](four-eyes.md); the exact rule conditions are in the [specification](../spec/ai-change-provenance.md).
