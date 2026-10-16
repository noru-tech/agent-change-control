# Gating a deploy with `acc verify`

`acc check` asks whether a set of changes passes the policy. A deploy asks a narrower question:
**is every commit about to go out the result of a change that passed?** `acc verify` answers it
from two things the deploy job already has: the git history it is about to deploy, and signed
verdicts made when each change merged.

```bash
acc verify --range "$LAST_DEPLOYED..$GITHUB_SHA" \
  --verification verdicts/*.json \
  --signer https://github.com/acme/api/.github/workflows/attest.yml@refs/heads/main \
  --issuer https://token.actions.githubusercontent.com
```

It exits `0` only when every commit in the range is covered by a passing change.

## What "covered" means

`acc verify` reads the range from the local clone with `git rev-list --first-parent BASE..HEAD`:
the commits the target branch moved through after `BASE`, up to and including `HEAD`. Each of them
must be the commit a change landed on the branch, its `merge_commit_sha`:

- **Merge commit:** the merge commit is on the first-parent history, and the commits it brought in
  are not, so the change covers all of them.
- **Squash merge, or a rebase merge of one commit:** the single commit the forge reports as the
  merge commit is the one on the branch.
- **A rebase merge of several commits:** only the last of them is the commit the forge reports. The
  others are uncovered. Use merge commits or squash merges on a branch you gate.
- **A direct push:** no change landed it, so it is uncovered. This is what the gate is for.

The change must then **pass** the policy, judged exactly as `acc check` judges a manifest, one
change at a time:

| Verdict | When |
| --- | --- |
| `pass` | No unsuppressed finding at or above `fail_on` for the change, a complete review history, and the facts every enabled opt-in rule (ACC004, ACC005) needs |
| `fail` | At least one unsuppressed finding at or above `fail_on` for the change |
| `incomplete` | No failing finding, but the review history or an enabled opt-in rule's facts are incomplete |

The policy is the one embedded in each attestation, or `--policy FILE` for all of them. Dispositions
are the ones the signed manifest carries; a non-open one needs `--as-of`, as for `acc check`. The
window's own completeness is not part of a change's verdict: it says whether every change in a
window was listed, and a change that was not listed simply covers nothing.

When several attestations judge the same change, a `fail` from any of them decides it, and a
`pass` outranks an `incomplete` (a later collection may have filled the gap).

| Commit status | Meaning |
| --- | --- |
| `covered` | A change landed it and passes |
| `uncovered` | No verified attestation records a change that landed it |
| `failing` | A change that landed it fails |
| `incomplete` | The only verdicts on the changes that landed it are incomplete |

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Every commit in the range is `covered` (an empty range is too) |
| 1 | A commit is `uncovered` or `failing` |
| 2 | Usage: no attestations, `--verification` without `--signer`, a range that is not `BASE..HEAD` or that git cannot resolve, a shallow clone |
| 3 | An attestation is invalid, unsigned, signed by an identity no `--signer` names, or through another `--issuer` |
| 4 | A commit is `incomplete`; takes precedence over 1, as everywhere in `acc` |

A shallow clone is refused because it cannot show that the range is whole. Check out with full
history (`fetch-depth: 0`).

## Who signed the verdicts

`acc` does not check signatures, here or anywhere else. The deploy job verifies them with the
signer's tooling first, and `acc verify` accepts two kinds of input:

- **`--verification FILE`**: the JSON `gh attestation verify --format json` writes. `acc` reads the
  certificate identity the verifier established and refuses any attestation whose identity is not
  one `--signer` names exactly, or whose issuer is not `--issuer` when given. Name the workflow
  that signs verdicts, at the branch it runs on, so that a verdict signed anywhere else cannot
  open the gate.
- **`--attestations PATH` with `--verified-by TEXT`**: signed DSSE envelopes or Sigstore bundles you
  verified with your own identity flags (for example `cosign verify-blob-attestation
  --certificate-identity … --certificate-oidc-issuer …`). `acc` records your statement in the
  result and refuses anything that carries no signature.

Every Statement is then validated as `acc validate` validates one: the statement schema, subjects
that fit the predicate, and a byte-identical re-evaluation of the embedded facts. A Statement
edited after signing, even by its signer, fails here as well as at the signature.

## Result

`--format text` (the default) prints one line per commit on stdout and a status line on stderr.
`--format json` (or an output name ending in `.json`) prints one object, as RFC 8785 bytes:

```json
{
  "attestations": [
    {"file": "verdicts/1a2b3c4.json#1", "payload_digest": "sha256:…",
     "predicate_type": "https://noru.tech/spec/ai-change-provenance/v0.4",
     "signer": {"identity": "https://github.com/acme/api/.github/workflows/attest.yml@refs/heads/main",
                "issuer": "https://token.actions.githubusercontent.com"},
     "verified_by": null}
  ],
  "commits": [
    {"commit": "1a2b3c4…", "status": "covered",
     "changes": [{"attestation": "verdicts/1a2b3c4.json#1", "change_id": "github:acme/api:pr:421",
                  "failing": [], "verdict": "pass"}]}
  ],
  "range": {"base": "0f9e8d7…", "head": "1a2b3c4…"},
  "verified": true
}
```

`commits` is newest first. `failing` lists the identifiers of the findings that fail the change.
`verified_by` is set for `--attestations` input and null when the signer came from a verifier's
output.

## A GitHub deploy job

The signing workflow from [signing](signing.md#github-artifact-attestations) attests each merged
pull request's manifest. Add one step to it so the deploy job can find the manifest by the commit
it landed:

```yaml
      - uses: actions/upload-artifact@v4
        with:
          name: acc-verdict-${{ github.event.pull_request.merge_commit_sha }}
          path: acc-manifest.json
          retention-days: 90
```

[`examples/deploy-gate.yml`](../examples/deploy-gate.yml) is a complete deploy workflow. For each
first-parent commit since the last successful production deployment, it downloads that verdict,
verifies its attestation with `gh attestation verify --signer-workflow`, and hands the results to
`acc verify`. A commit with no verdict is not an error at that step: `acc verify` reports it
`uncovered` and the job stops before deploying.

Two things to keep in mind:

- **Retention.** A workflow artifact lasts as long as the repository's retention setting allows. A
  verdict that has expired leaves its commit uncovered, which fails closed. Deploy more often than
  the retention period, or keep the verdicts somewhere that lasts longer.
- **The first deploy** has no previous deployment to start from. Pass the base explicitly once.

## A GitLab deploy job

[`examples/gitlab-deploy-gate.yml`](../examples/gitlab-deploy-gate.yml) is the same gate on
GitLab.com, with both halves in one pipeline:

- **On every push to the default branch**, `acc-attest` finds the merge request that landed the
  commit, evaluates it with `acc pr --forge gitlab`, and signs the manifest keyless with Sigstore,
  as the job's GitLab identity (`id_tokens` with `aud: sigstore`). The manifest is both predicate
  and blob, the [digest form](signing.md#what-gets-signed). Manifest and bundle go to the project's
  generic package registry under the merge commit, where they outlive artifact retention. A commit
  no merge request landed gets nothing, and the deploy job reports it uncovered.
- **The deploy job** takes the last successful production deployment from GitLab's deployments
  API, fetches the verdict of every first-parent commit since, and keeps only those that
  `cosign verify-blob-attestation` accepts: the signature, the signer
  (`https://gitlab.com/GROUP/PROJECT//.gitlab-ci.yml@refs/heads/main`, double slash included,
  issuer `https://gitlab.com`) and the manifest's digest against the signed subject. It then runs
  `acc verify --attestations` with `--verified-by` naming that check, and deploys only on exit 0.

The job token reads the deployments API and the package registry. Reading a merge request's
approvals needs `GITLAB_TOKEN` (`read_api`), as for every GitLab collection. The pipeline installs
cosign 3, because acc reads the Sigstore bundle format (a `dsseEnvelope`) that cosign 3 writes and
older releases do not, and pins both downloads by SHA-256. Use merge commits or squash merges: a
plain fast-forward merge reports no merge commit, and its commits are uncovered.

## Elsewhere

Any CI that can run `git`, a signature verifier and `acc` can run the gate:

```bash
git fetch --unshallow 2>/dev/null || true
for sha in $(git rev-list --first-parent "$BASE..$HEAD"); do
  fetch-verdict "$sha" > "verdicts/$sha.sigstore.json" || continue   # your storage
  cosign verify-blob-attestation --bundle "verdicts/$sha.sigstore.json" … # your identity flags
done
acc verify --range "$BASE..$HEAD" --attestations verdicts \
  --verified-by "cosign verify-blob-attestation, identity attest.yml@refs/heads/main"
```

A verdict that fails verification must not be passed on: remove it, and `acc verify` reports its
commit uncovered.

## Limits

- **Rebase merges of several commits** leave all but the last commit uncovered (above).
- **What was deployed last is the caller's to say.** `acc verify` covers the range it is given; a
  wrong `BASE` covers the wrong commits. Take it from the deployment record, not from a branch
  someone can move.
- **Signatures are checked by the caller.** `--signer` makes the trusted identity explicit in the
  gate, but the cryptographic check is `gh`'s or cosign's.
- **The verdict is as of signing.** A finding dispositioned after the attestation was made is not
  visible here; sign the updated manifest again, or decide it in `--policy`.
