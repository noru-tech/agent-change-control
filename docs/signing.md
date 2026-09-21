# Signing the verdict

`acc` never holds a key. It writes the evidence, in a form that is deterministic and
self-validating, and a signer your organization already trusts turns it into an attestation. This
page shows two ways to do that and one way to verify the result. The predicate itself is described
in [in-toto](in-toto.md).

Before signing anything, read the privacy note at the end: an attestation contains the manifest,
and the manifest contains names and review history.

## What gets signed

There are two subject forms, and which one you use depends on the signer.

| Form | Subject | Produced by | Signer |
| --- | --- | --- | --- |
| Commit subjects | `gitCommit` digests of the change's head and merge commits | `acc … --format in-toto` or `in-toto-jsonl` | Any DSSE signer that accepts a pre-built Statement, such as `cosign attest-blob --statement` |
| Digest of the evidence | `sha256` of the manifest file, in canonical JSON | `acc … --format json` | Signers that hash a file for you: `actions/attest`, `cosign attest-blob --predicate` |

Both forms carry the same predicate, the manifest, and `acc validate` accepts both: for the
second form it recomputes the sha256 of the canonical predicate and requires it to match the
subject, so the commits are still bound, through `head_sha` and `merge_commit_sha` inside the
predicate. The first form is what a verifier that walks commits wants; the second is what GitHub's
attestation store and `gh attestation verify` can look up, because they identify artifacts by
SHA-2 digest only and cannot store a `gitCommit` subject.

## GitHub artifact attestations

The workflow identity signs through Sigstore and the attestation is stored in the repository's
attestation store, where `gh attestation verify` finds it by the manifest's digest. This repository
runs this workflow on every merged pull request:
[`.github/workflows/attest.yml`](../.github/workflows/attest.yml). The attestations it produced are
listed at <https://github.com/noru-tech/agent-change-control/attestations>.

```yaml
on:
  pull_request_target:
    types: [closed]
permissions:
  contents: read
  pull-requests: read
  id-token: write
  attestations: write
jobs:
  attest:
    if: github.event.pull_request.merged == true
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: noru-tech/agent-change-control@v0.3.1
        with:
          fail-on-findings: false   # the verdict is recorded either way
          format: json
          output: acc-manifest.json
      - uses: actions/attest@v4
        with:
          subject-path: acc-manifest.json
          predicate-type: https://noru.tech/spec/ai-change-provenance/v0.1
          predicate-path: acc-manifest.json
```

Notes:

- `pull_request_target` rather than `pull_request`, because a merged pull request from a fork
  would otherwise run without the `id-token` permission. No code from the pull request is executed:
  the checkout is the base branch, and the action only reads the pull request through the API.
- The subject and the predicate are the same file. `actions/attest` hashes the file for the subject
  and parses it for the predicate. `acc` writes JSON in canonical form, so the digest is
  reproducible from the predicate alone, which is what `acc validate` checks.
- Pin both actions to commit SHAs in production, as the repository's own workflow does.

## cosign

To sign the commit-subject form, hand cosign the Statement `acc` wrote and let it wrap it in a
DSSE envelope:

```bash
acc pr 421 --repo acme/api --format in-toto -o pr-421.intoto.json
cosign attest-blob --statement pr-421.intoto.json --bundle pr-421.sigstore.json --yes -
```

cosign reads the Statement as is and validates its shape before signing; with `--bundle` the
output is a Sigstore bundle, with `--output-signature` and `--output-attestation` a bare DSSE
envelope. To sign the digest form instead, give cosign the manifest as both the predicate and the
blob:

```bash
acc pr 421 --repo acme/api --format json -o acc-manifest.json
cosign attest-blob --predicate acc-manifest.json \
  --type https://noru.tech/spec/ai-change-provenance/v0.1 \
  --bundle acc-manifest.sigstore.json --yes acc-manifest.json
```

Keyless signing through the public Sigstore instance writes an entry to Rekor, which is public
and permanent. See the privacy note.

## Verifying

Verification has two halves: the signature, which says who signed, and the content, which says
whether the findings follow from the facts. The signer's tooling does the first, `acc validate`
the second.

```bash
# 1. Signature and identity, against GitHub's attestation store
gh attestation verify acc-manifest.json --repo acme/api \
  --predicate-type https://noru.tech/spec/ai-change-provenance/v0.1

# 2. Content: unwrap the Statement and re-evaluate it
gh attestation download acc-manifest.json --repo acme/api    # writes <digest>.jsonl
jq -r '.dsseEnvelope.payload' <digest>.jsonl | base64 -d > statement.json
acc validate statement.json
```

For a cosign bundle, replace step 1 with `cosign verify-blob-attestation --bundle … --type …`
and read the payload from the bundle's `dsseEnvelope.payload` the same way. `acc validate` on the
Statement confirms the subjects fit the predicate, re-evaluates the embedded events under the
embedded policy, and requires byte-identical findings, assessments and summary. A Statement whose
predicate marks collection incomplete validates, but cannot be read as clean; check
`events.window.complete` and every change's `reviews_complete` before treating a verdict as pass.

What verification does not establish: that the events in the predicate are what the forge held at
the time. The signer attests that this evaluator, run by this identity, saw these facts; a
declaration inside them is still a declaration (see [agent authorship](agent-authorship.md)).
Signed authorship and review evidence, which would strengthen that, is future work.

## Privacy

The predicate is the manifest: actor identifiers, display names, pull request titles and the full
review history. Wherever the attestation goes, that data goes.

- The GitHub attestation store is readable by everyone who can read the repository. For a public
  repository that is everyone, though the same facts are already visible on the pull request.
- Keyless Sigstore signing records the signing event in Rekor. What the log stores depends on the
  entry type the client uses, but treat the payload as published: it cannot be withdrawn.
- For a private repository whose review history must stay private, sign with a key you control and
  keep the attestations in a store you control, or wait for the pseudonymization mode on the
  [roadmap](../ROADMAP.md).

See [privacy](privacy.md) for the general handling rules.
