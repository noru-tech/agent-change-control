# GitLab

`acc` collects merge requests from GitLab.com projects and evaluates them under the same rules,
policy and output formats as GitHub pull requests. Self-managed GitLab is not collected yet: the
collector talks to one fixed origin, `https://gitlab.com`, as the GitHub collector talks only to
`api.github.com`.

## In a merge request pipeline

The CI/CD component in [`templates/acc.yml`](../templates/acc.yml) installs a checksum-verified
`acc` release, evaluates the current merge request, prints the table to the job log, and keeps the
manifest and the rendered output as job artifacts:

```yaml
include:
  - remote: https://raw.githubusercontent.com/noru-tech/agent-change-control/v0.7.0/templates/acc.yml
    inputs:
      fail_on_findings: true
```

Add a masked, protected CI/CD variable `GITLAB_TOKEN` holding a project access token with the
`read_api` scope and the Reporter role. The job's own `CI_JOB_TOKEN` is not enough: GitLab lets it
read a merge request but not its approvals or diff versions
([job token access](https://docs.gitlab.com/ci/jobs/ci_job_token/)). Without a token only a public
project can be read.

The component runs on `merge_request_event` pipelines. It reads the policy as it stands at the
merge request's base (`policy_ref`, default `$CI_MERGE_REQUEST_DIFF_BASE_SHA`), so a merge request
that edits the policy is evaluated under the one it would replace, exactly as the GitHub Action
does. Its inputs mirror the Action's: `policy`, `agent_accounts`, `agent_vendors`, `attestations`,
`verified_by`, `ignore_trailers`, `format`, `output`, `fail_on_findings`, `max_pages` and
`version`. Lists are space-separated.

The component is included with `include: remote` from a tag of this repository. GitLab's CI/CD
Catalog lists only components hosted in a GitLab project, and there is none yet.

The release archive is checked against its published SHA-256. That proves the bytes are the ones
the release published, not who built them; to verify the build provenance as the GitHub Action
does, run `gh attestation verify` on the archive in a job image that has the `gh` CLI.

## From the command line

```sh
export GITLAB_TOKEN=...          # read_api; never pass it on the command line
acc scan gitlab acme/platform/api --since 2026-09-01 --until 2026-09-30
acc pr 42 --forge gitlab --repo acme/platform/api
```

Without a forge, `acc scan`, `acc export` and `acc pr` collect from GitLab when they run in a
gitlab.com pipeline (`GITLAB_CI` and `CI_PROJECT_PATH`) or in a clone whose `origin` remote is on
gitlab.com, and from GitHub otherwise. For a periodic evidence run, schedule a pipeline that runs
`acc scan gitlab "$CI_PROJECT_PATH"` over the last window and keeps the manifest as an artifact.

## What GitLab records, and what acc makes of it

[Spec §4.4](../spec/ai-change-provenance.md#44-gitlab) is the normative version of this section.

**An approval names no commit.** GitLab lists the approvals in force on a merge request and when
each was given ([merge request approvals API](https://docs.gitlab.com/api/merge_request_approvals/)),
but not which commit was approved. `acc` binds each approval to the head of the latest diff
version GitLab created at or before it, and records that binding as `derived` evidence naming the
version. GitLab creates a version shortly after each push, so an approval given within a second of
a version could have approved either head: it is not recorded, and the review history is
incomplete (exit 4) rather than guessed. An approval given before the last push binds to an
earlier head and does not approve the merged one, which is what ACC003 asks.

A withdrawn approval is no longer listed; it would not have qualified anyway. GitLab Community
Edition does not report when an approval was given, so on it nothing binds and every merge
request's history is incomplete.

**Accounts.** An actor is `gitlab:username`. GitLab's user record says whether an account is a
bot (project and group access tokens act through bot users); when it cannot be read, the actor is
`unknown`, which never counts as a human approval. A declared operator resolves only to an account
the users API confirms. Commits list an author name and email but no account, so commit authors
are not recorded, and an operator is never derived from them.

**Required checks (ACC004).** GitLab has no named required checks. When the project's merge
settings say "Pipelines must succeed", `acc` records the required check `pipeline`, and every
finished pipeline on the head or merge commit as a result named `pipeline`. Each finished job is
recorded under its own name, so a policy can also require a job by name. A job allowed to fail
that failed is `neutral`; a manual job that never ran has no result. A merged-results pipeline
runs on a temporary merge commit and is not recorded.

**Protection bypass (ACC005).** GitLab does not record, on every tier, an evaluation of each push
against the branch's protection. When ACC005's facts are asked for, they are recorded as
incomplete, and ACC005 is `unknown`.

**Branch updates and deployments (ACC012, ACC013).** GitLab's push events do not say whether a
push was forced or landed a merge request, and this version does not read GitLab deployments, so
both are recorded as unreadable and the rules are `unknown`.

**Reviewer teams (ACC011).** A required reviewer team is a group path (`acme/security`, or
`acme/eng/security` for a subgroup). Its members are the group's direct members, as GitLab lists
them at collection. Members inherited from a parent group are not counted.

**Merge commit.** The merge commit, or the squash commit when a squashed change was
fast-forwarded. A plain fast-forward merge reports neither, so `acc verify` cannot cover those
commits; use merge commits or squash on projects whose deployments it gates.

## Gating a deploy

[`examples/gitlab-deploy-gate.yml`](../examples/gitlab-deploy-gate.yml) signs each merged merge
request's verdict keyless with Sigstore and keeps it in the project's package registry, then
verifies the verdicts of every commit since the last production deployment before deploying.
[Verify](verify.md#a-gitlab-deploy-job) explains it.
