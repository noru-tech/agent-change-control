# Examples

| File | What it shows |
| --- | --- |
| `pull-request-body.md` | The inline declaration a producer puts in a pull request description (spec §3.1) |
| `provenance.json` | A standalone provenance document bound to a head commit (spec §3.2); validate with the `provenance` schema |
| `provenance.intoto.json` | The same document as an in-toto Statement to sign and hand back with `--attestations` (spec §3.2, §3.6) |
| `review.intoto.json` | A review document for an agent reviewer, as a Statement to sign (spec §3.7); validate the predicate with the `review` schema |
| `policy.yml` | A policy that fails on unknown operators |
| `workflow-required-check.yml` | The GitHub Action as a merge gate with SARIF upload |
| `deploy-gate.yml` | A deploy job that runs `acc verify` on every commit since the last production deployment ([verify](../docs/verify.md)) |
| `gitlab-deploy-gate.yml` | The same gate on GitLab.com: each merged merge request's verdict signed keyless and kept in the package registry, then verified with cosign and `acc verify` before deploying ([verify](../docs/verify.md#a-gitlab-deploy-job)) |
| `release-image-attest.yml` | A release workflow that attaches an `acc release` attestation to a container image with `cosign attest` ([release](../docs/release.md#admitting-an-image-in-kubernetes)) |
| `kyverno-release-policy.yml` | A Kyverno policy that admits an image only when that attestation says `verdict: pass` ([release](../docs/release.md#admitting-an-image-in-kubernetes)) |

Synthetic inputs for the offline commands live in `tests/fixtures/`; for instance:

```bash
acc evaluate tests/fixtures/claude-operator-self-approved/events.json --format table
acc evaluate tests/fixtures/claude-clean/events.json --format in-toto
acc evaluate tests/fixtures/claude-clean/events.json -o clean.intoto.jsonl && acc validate clean.intoto.jsonl
```
