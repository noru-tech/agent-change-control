# Attesting a release with `acc release`

A build's provenance says how the release artifacts were made. It does not say whether the code in
them was reviewed. `acc release` writes the second half: an in-toto Statement about the same
artifacts whose predicate lists every commit the release shipped and the verdict of the change that
landed each one.

```bash
acc release manifests/*.json \
  --range v1.4.0..v1.5.0 \
  --subject "acme-api-1.5.0.tar.gz=sha256:$(sha256sum dist/acme-api-1.5.0.tar.gz | cut -d' ' -f1)" \
  --output release.intoto.json
```

## What it records

The range is read from the local clone, `git rev-list --first-parent BASE..HEAD`, exactly as
[`acc verify`](verify.md) reads it, and covered the same way: each first-parent commit must be the
merge commit of a change in one of the manifests, and that change must pass the policy as
`acc check` judges it, dispositions included (`--as-of` when one is not open). The policy is each
manifest's own, or `--policy FILE` for all of them.

| Predicate field | What it holds |
| --- | --- |
| `range` | `base` and `head` as full commit identifiers |
| `verdict` | `pass`, `fail` or `incomplete` for the whole range |
| `commits` | every first-parent commit, newest first: its status (`covered`, `uncovered`, `failing`, `incomplete`) and each change that landed it, with its verdict, failing finding identifiers, `verdict_digest` and the digest of the manifest it came from |
| `uncovered` | the commits no change landed |
| `manifests` | the digest, `source_digest` and repository of each manifest read |
| `policy`, `as_of` | which policy judged the changes, and the date dispositions were evaluated on |

`verdict_digest` is SHA-256 over the RFC 8785 bytes of `{change_id, merge_commit_sha, verdict,
failing}`, so a reader holding the manifest can recompute it. The specification is
[§7.4](../spec/ai-change-provenance.md#74-release-attestation); the schema is
[`schemas/release.schema.json`](../schemas/release.schema.json).

The Statement is written whatever the verdict: a release that shipped a commit nobody reviewed is a
fact to attest, not to hide. `acc validate release.intoto.json` checks it against the schema.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Every commit in the range is covered by a passing change |
| 1 | A commit is uncovered or failing |
| 2 | Usage: a malformed `--subject`, or a range git cannot resolve (including a shallow clone) |
| 3 | A manifest does not validate |
| 4 | A commit's only verdicts are incomplete (takes precedence over 1) |

## Signing it

The Statement is unsigned. Sign it the way you sign acc's other attestations, for example with
GitHub artifact attestations, whose `predicate-type` input takes
`https://noru.tech/spec/ai-change-provenance/release/v0.1` ([signing](signing.md)). A consumer then
holds two attestations about one artifact digest: how it was built, and what it contains.

## Admitting an image in Kubernetes

The release attestation can stand in front of a cluster: an image runs only if the change control
behind it passed. [`examples/release-image-attest.yml`](../examples/release-image-attest.yml)
builds the image on a release tag, writes the Statement with the image as its subject, and attaches
its predicate to the image with `cosign attest --type
https://noru.tech/spec/ai-change-provenance/release/v0.1`, keyless, as the release workflow's
identity. cosign builds its own Statement around the predicate, with the image digest as subject,
the same digest acc's `--subject` names.

[`examples/kyverno-release-policy.yml`](../examples/kyverno-release-policy.yml) is the admission
side: a Kyverno `ClusterPolicy` whose `verifyImages` rule requires an attestation of that
predicate type, signed by that workflow on a tag, with the condition `verdict == pass`. An image
with no attestation, one signed by another identity, or one whose release shipped an uncovered,
failing or incomplete commit is refused.

The release job only reads verdicts it verified (`gh attestation verify` against the signing
workflow), so a manifest from anywhere else cannot make a release pass. A release with no verified
verdict at all is not attested, and the policy refuses it.

## Limits

- Only the manifests given are read. A commit whose change is in none of them is uncovered: pass
  every manifest that covers the range.
- A rebase merge of several commits leaves all but the last uncovered, as for `acc verify`.
- `--subject` digests are the caller's. acc does not read the artifacts; it states what the
  caller's build produced alongside what the commits contain.
