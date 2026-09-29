# Draft pull request for in-toto/attestation

**Status: draft for Bip's review. Not opened upstream.**

Title: `Add AI Change Provenance predicate (community contribution)`

---

This adds the **AI Change Provenance** (ACP) predicate,
`https://noru.tech/spec/ai-change-provenance/v0.3`, as a community contribution. It does not
claim vetted status.

**What it records.** For a software change (a pull request), the predicate records:

- who the effective author was: a human, or a coding agent together with the human who
  directed it;
- who reviewed the change and who merged it;
- whether a human independent of that effective author approved the reviewed head before merge.

The predicate is the evaluated evidence, and a consumer can re-evaluate it offline.

**Why.** The account-based four-eyes check ("author ≠ approver") stops working once agents open
changes under their own accounts. The engineer who directed the agent can approve its pull
request, and the platform sees two accounts. ACP is a small specification for that one question.
It has a reference implementation ([`acc`](https://github.com/noru-tech/agent-change-control))
that emits this predicate. Its own repository signs one for every merged pull request with GitHub
artifact attestations. The tool is proposed for in-toto/friends in #121.

**What this PR adds.**

- `spec/predicates/ai-change-provenance.md`, following the template. It includes a worked example
  in which every digest's preimage is shown.
- A "Community Contributed Predicates" section in `spec/predicates/README.md`, with the entry.
  There is no such section today, and the tier proposal (in-toto/ITE#63) is still a draft, so the
  entry does not go under "Vetted Predicates". I'm happy to move it wherever the maintainers
  prefer.

The type URI is on our domain and redirects to the specification text pinned by release. No
in-toto.io redirect is needed.

## The two questions reviewers ask first

**How do I reproduce the digests?**

- Every digest is SHA-256 over the [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785) (JCS)
  serialization of the value, with no trailing newline (ACP §8.2).
- Input is I-JSON: integer-only numbers in ±(2^53 − 1), no unpaired surrogates, unique member
  names, and nesting of at most 128 with the outermost container at depth 1. The depth bound and
  integer range are the same as in the AI Agent Action proposal (#588). ACP goes further and
  forbids non-integer numbers anywhere.
- The worked example shows the exact preimage of each finding identifier, of the source digest
  and of the `sha256` subject form.
- ACP's CI recomputes the digests of all its goldens with an independent JCS implementation
  (Trail of Bits' `rfc8785` for Python), without the reference implementation's code.

**How do I test an implementation?**

- Evaluator conformance is defined by a published corpus, `acp-evaluator-conformance`, suite
  revision 1: 32 accept, 18 reject and 4 incomplete vectors.
- It runs through an external-verifier contract modeled on
  [agent-evidence-vectors](https://github.com/probityai/agent-evidence-vectors): `<cmd>
  <vector-file>`, the verdict in the exit status, and a one-line JSON result.
- Every reject vector is one mutation of an accept vector. The digest list is signed at each
  release tag.
- The harness is standard-library Python and is also a GitHub Action.
- CI checks that builds with a single rule broken fail the corpus.
- The corpus is a public answer key, not a proof of correctness. Independent runs, especially
  ones that disagree with it, are the most useful thing a reviewer could give us.

## Prepared answers

**Evidence vocabulary: `derived < declared < observed < signed` versus #588's `witness` and
`asserter`.** ACP's kinds say how the evaluator knows a fact:

| ACP kind | Meaning | Closest #588 role |
| --- | --- | --- |
| `derived` | Computed from records written at authoring time: a vendor `Co-Authored-By` trailer, an Agent Trace record. Weakest. | none |
| `declared` | Asserted by a party and not verified: an inline declaration in the change description, a consumer's account mapping. | `asserter` |
| `observed` | Recorded by the forge itself, read from its API: reviews, merges, accounts. | `witness`: the forge is the intermediary that saw it happen |
| `signed` | Asserted in an attestation whose signature the consumer's operator verified before evaluation. | none. In #588 the signature is on the chain, not a property of each fact. |

Where the concepts coincide (`declared`/`asserter`, `observed`/`witness`), we are willing to
align the names or document the mapping normatively. Ordering the kinds matters to ACP, because
a policy sets the weakest kind it accepts for authorship and for approvals. That is why there
are four kinds rather than two roles.

**Identity: how "independent human" is established, and where ACP says `unknown`.**

- An actor is `human` only when the forge asserts a user account. Apps and bot accounts are
  `bot`, and a bot's approval is never a human approval.
- The effective human is the author for a human change, and the operator for an agent change.
  The operator comes from a declaration, a signed provenance document, or a derived record. It
  resolves only to an account the forge confirms is human; never to the merger, the opener, a
  reviewer, an email or a display name.
- Independence compares actor identifiers. The approval must be of the current head, it must be
  the reviewer's latest decision at or before merge, and it must not have been withdrawn.
- ACP reports `unknown`, never `pass`, when:
  - the operator is not recorded (ACC006);
  - the operator's evidence is below the policy minimum;
  - the review history was not fully retrieved;
  - an opt-in agent reviewer cannot be shown to be independent on a required dimension.
- ACP does not detect one person holding two accounts. That is an identity-management control,
  out of scope here as it is for SLSA's two-party review.

**Relationship to SLSA.**

- *SLSA Provenance* describes builds, and ACP describes the approval of the source change. They
  share a subject, the merge commit, so the two attestations travel together.
- *The SLSA Source track* requires, at Level 4, that changes to protected branches be agreed by
  two or more trusted persons. The source control system asserts this, and it is summarized in
  a Source VSA (`SLSA_SOURCE_LEVEL_4`). ACP is a finer test of the same requirement for
  agent-authored changes: an agent's operator counts as the author, so their approval is not the
  second party.
- An organization can use a clean ACP result to back an `ORG_SOURCE_` property in its Source VSA.
- SLSA allows exceptions for trusted robot contributions. ACP never counts a bot as an approver,
  and counts an agent approval only under an explicit, signed, opt-in policy.

**Why the reference implementation does not verify signatures yet.**

- `acc`'s evaluator is offline and deterministic. It reads no network, clock or environment.
- Verification needs a trust root: which identities may sign which claims. That trust root
  belongs to the consumer, not to an offline evaluator and never to the predicate.
- Today, signed input is handed over already verified. The caller's statement of who verified it
  is recorded with the evidence, and a validator rejects `signed` evidence without such a
  record.
- `acc verify`, with consumer-pinned identities and Sigstore bundles first, is on the
  [roadmap](https://github.com/noru-tech/agent-change-control/blob/main/ROADMAP.md).

**Field naming (snake_case).** The template's guidelines prefer lowerCamelCase. ACP's predicate
is the same manifest the tool writes as JSON, YAML and SARIF properties, versioned with public
schemas, so renaming the fields is a breaking change. We would rather make it once, at 1.0, if
the maintainers ask for it.

**Unrecognized fields.** The document states a deviation from the framework's parsing rules:
unknown fields inside the predicate must be rejected. The predicate must re-evaluate to exactly
the same content, so a field the consumer does not understand would make that check meaningless.

## Before opening (checklist for Bip)

- [ ] The noru.tech redirects in `URI-HOSTING.md` are live, and every type URI resolves.
- [ ] Spec 0.3 is released. The document's "Pinned by" line names that tag and commit, and the
      schema links point at the tag instead of `main`.
- [ ] The conformance corpus has been signed at that tag (`conformance-release.yml` ran), and
      `gh attestation verify` passes on `CORPUS-DIGESTS.txt`.
- [ ] Decide whether the provenance predicate (`…/provenance/v0.1`) goes first instead, or
      together (see below).
- [ ] Run `markdownlint` from the upstream repository on the document. It reports nothing
      against their configuration today.
- [ ] Open it from a fork, as with in-toto/friends#121, and mention it at the in-toto community
      meeting: recent predicate PRs (#581, #588) have waited weeks for a maintainer review.

## Which predicate to submit first

The plan's default is the evaluation predicate, because `acc` emits and signs it today, and this
draft follows that default. The case for submitting the **provenance** predicate first, or
together with this one, is reuse. Any agent integration could emit it: "this agent wrote this
commit, and this human directed it". Its reach goes well beyond `acc`, and it is small, with no
rules and no evaluation. The review predicate is best kept for later: it overlaps the stalled
human-review work (#151) and #581.
