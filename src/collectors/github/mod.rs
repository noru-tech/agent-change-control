//! Read-only GitHub transport and normalization; no policy evaluation.
//!
//! Requests are GET-only against a fixed origin with redirects disabled, a body cap and a page
//! cap. Error messages are fixed strings so response headers and bodies never leak.

use crate::model::*;
use crate::normalize::timestamp;
use crate::provenance::attestations;
use crate::{Exit, Failure, failure};
use anyhow::{Result, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::time::Duration;

const ORIGIN: &str = "https://api.github.com";
const MAX_BYTES: u64 = 8 * 1024 * 1024;
const USER_AGENT: &str = concat!("agent-change-control/", env!("CARGO_PKG_VERSION"));
/// The actor recorded when GitHub returns no account for a role (a deleted user, for example).
pub const UNAVAILABLE_ACTOR: &str = "unknown:unavailable";

pub use super::Sources;

/// The core rate limit, from `GET /rate_limit`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub remaining: u64,
    pub limit: u64,
    /// When the limit resets, in Unix seconds.
    pub reset: u64,
}

pub struct Github {
    agent: ureq::Agent,
    token: Option<String>,
    base: String,
    pub max_pages: usize,
}

enum Response {
    Found(Value, bool),
    NotFound,
    /// A 403 that is not rate limiting, returned only to a caller that tolerates it: the token
    /// cannot read this resource.
    Forbidden,
}

fn api(message: &'static str) -> anyhow::Error {
    failure(Exit::Api, message)
}

fn unsupported(message: &'static str) -> anyhow::Error {
    failure(Exit::Unsupported, message)
}

/// The token variables, for hints.
const SET_TOKEN: &str =
    "set GITHUB_TOKEN or GH_TOKEN to a token with read access to contents and pull requests";

/// What to do about a failed request: `status` is the HTTP status (0 when no response arrived),
/// `rate_limited` whether GitHub said the rate limit was the cause, `token` whether the request
/// carried one. Hints never quote the response.
pub fn hint(status: u16, rate_limited: bool, token: bool) -> String {
    match (status, rate_limited, token) {
        (0, ..) => "check the network connection to api.github.com (and HTTPS_PROXY, if you use one), then retry".into(),
        (401, _, true) => "the token in GITHUB_TOKEN or GH_TOKEN was rejected: it is expired, revoked or mistyped; replace it with one that has read access to contents and pull requests".into(),
        (401, _, false) => SET_TOKEN.into(),
        (_, true, true) => "the token's rate limit is exhausted; wait until it resets (`acc doctor --online` shows when), or narrow the window to make fewer requests".into(),
        (_, true, false) => format!("unauthenticated requests are limited to 60 an hour; {SET_TOKEN}"),
        (403, false, true) => "the token cannot read this repository: it needs read access to contents and pull requests (a fine-grained token must select the repository)".into(),
        (403, false, false) => format!("GitHub refused an unauthenticated request; {SET_TOKEN}"),
        (404, _, true) => "check OWNER/REPO and the pull request number, and that the token can read this repository".into(),
        (404, _, false) => format!("check OWNER/REPO and the pull request number; a private repository looks missing without a token: {SET_TOKEN}"),
        _ => "GitHub answered with an unexpected status; retry, and check https://www.githubstatus.com".into(),
    }
}

/// `http://127.0.0.1:PORT` or `http://localhost:PORT`.
fn loopback(base: &str) -> bool {
    base.strip_prefix("http://127.0.0.1:")
        .or_else(|| base.strip_prefix("http://localhost:"))
        .is_some_and(|port| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
}

impl Github {
    pub fn new(token: Option<String>, max_pages: usize) -> Result<Self> {
        Self::with_base(token, max_pages, ORIGIN)
    }

    /// A custom base is restricted to loopback HTTP without a token, for deterministic
    /// integration tests.
    pub fn with_base(token: Option<String>, max_pages: usize, base: &str) -> Result<Self> {
        if !(1..=1000).contains(&max_pages) {
            return Err(failure(Exit::Usage, "max-pages must be between 1 and 1000"));
        }
        if !(base == ORIGIN || (loopback(base) && token.is_none())) {
            return Err(failure(Exit::Usage, "unsupported API origin"));
        }
        let config = ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(USER_AGENT)
            .build();
        Ok(Self {
            agent: config.new_agent(),
            token,
            base: base.trim_end_matches('/').into(),
            max_pages,
        })
    }

    fn get(&self, path: &str) -> Result<Response> {
        self.fetch(path, false)
    }

    /// [`Self::get`]; with `tolerate`, a permission 403 is [`Response::Forbidden`] rather than
    /// an error, for data whose absence is recorded rather than fatal.
    fn fetch(&self, path: &str, tolerate: bool) -> Result<Response> {
        ensure!(
            path.starts_with('/') && !path.starts_with("//"),
            "invalid API path"
        );
        let mut request = self
            .agent
            .get(format!("{}{}", self.base, path))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(token) = &self.token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        let token = self.token.is_some();
        let mut response = request
            .call()
            .map_err(|_| self.failed(Exit::Api, "GitHub request failed", 0, false))?;
        let status = response.status().as_u16();
        match status {
            200 => {}
            404 => return Ok(Response::NotFound),
            401 => {
                return Err(self.failed(Exit::Auth, "GitHub authentication failed", 401, false));
            }
            403 | 429 => {
                let headers = response.headers();
                let limited = status == 429
                    || headers.contains_key("retry-after")
                    || headers
                        .get("x-ratelimit-remaining")
                        .is_some_and(|v| v.as_bytes() == b"0");
                if tolerate && !limited {
                    return Ok(Response::Forbidden);
                }
                return Err(self.failed(
                    Exit::Api,
                    "GitHub request forbidden or rate limited",
                    status,
                    limited,
                ));
            }
            _ => {
                return Err(
                    Failure::new(Exit::Api, "GitHub API returned an unexpected status")
                        .hint(hint(status, false, token))
                        .into(),
                );
            }
        }
        let next = response
            .headers()
            .get("link")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| s.contains("rel=\"next\""));
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| api("GitHub response read failed"))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(api("GitHub payload exceeds size limit"));
        }
        let value =
            serde_json::from_slice(&bytes).map_err(|_| api("GitHub returned invalid JSON"))?;
        Ok(Response::Found(value, next))
    }

    /// A failure with the hint for `status`.
    fn failed(
        &self,
        exit: Exit,
        message: &'static str,
        status: u16,
        limited: bool,
    ) -> anyhow::Error {
        Failure::new(exit, message)
            .hint(hint(status, limited, self.token.is_some()))
            .into()
    }

    fn not_found(&self) -> anyhow::Error {
        self.failed(Exit::Api, "GitHub resource not found", 404, false)
    }

    fn object(&self, path: &str) -> Result<Value> {
        match self.get(path)? {
            Response::Found(value, _) => Ok(value),
            Response::NotFound | Response::Forbidden => Err(self.not_found()),
        }
    }

    /// An object the token may not be able to read: `None` when it is forbidden or absent.
    fn readable(&self, path: &str) -> Result<Option<Value>> {
        match self.fetch(path, true)? {
            Response::Found(value, _) => Ok(Some(value)),
            Response::NotFound | Response::Forbidden => Ok(None),
        }
    }

    /// Walk a list endpoint the token may not be able to read. `key` names the array inside an
    /// object response (`check_runs`); `None` reads a bare array. Returns the items and whether
    /// the listing is complete: an unreadable or truncated listing is incomplete, never empty.
    fn readable_pages(&self, path: &str, key: Option<&str>) -> Result<(Vec<Value>, bool)> {
        let mut all = Vec::new();
        for page in 1..=self.max_pages {
            let sep = if path.contains('?') { '&' } else { '?' };
            let (value, next) =
                match self.fetch(&format!("{path}{sep}per_page=100&page={page}"), true)? {
                    Response::Found(value, next) => (value, next),
                    Response::NotFound | Response::Forbidden => return Ok((all, false)),
                };
            let items = match key {
                Some(k) => &value[k],
                None => &value,
            }
            .as_array()
            .ok_or_else(|| api("GitHub list response is not an array"))?;
            all.extend(items.iter().cloned());
            if !next {
                return Ok((all, true));
            }
        }
        Ok((all, false))
    }

    /// Walk a list endpoint. `done` inspects each page and may end the walk early; the walk is
    /// complete when the last page had no `next` link or `done` returned true.
    fn pages(&self, path: &str, done: impl Fn(&[Value]) -> bool) -> Result<(Vec<Value>, bool)> {
        let mut all = Vec::new();
        for page in 1..=self.max_pages {
            let sep = if path.contains('?') { '&' } else { '?' };
            let Response::Found(value, next) =
                self.get(&format!("{path}{sep}per_page=100&page={page}"))?
            else {
                return Err(self.not_found());
            };
            let items = value
                .as_array()
                .ok_or_else(|| api("GitHub list response is not an array"))?;
            let stop = done(items);
            all.extend(items.iter().cloned());
            if !next || stop {
                return Ok((all, true));
            }
        }
        Ok((all, false))
    }

    /// The checks the forge requires on `branch`, as its rules say now. Rulesets come from
    /// `GET /repos/{repository}/rules/branches/{branch}` (repository and organization rules in
    /// active enforcement; readable with metadata access), classic protection from the branch's
    /// `protection` summary. GitHub keeps no record of the rules in force at an earlier time.
    fn required_checks(&self, repository: &str, branch: &str) -> Result<Required> {
        let segment = path_segment(branch);
        let mut required = Required {
            names: BTreeSet::new(),
            sources: Vec::new(),
            complete: true,
            provenance: Vec::new(),
        };
        let rules_path = format!("/repos/{repository}/rules/branches/{segment}");
        let (rules, rules_complete) = self.readable_pages(&rules_path, None)?;
        if rules_complete {
            required.sources.push(RequiredSource::Rules);
            required.provenance.push(evidence(
                &format!("{ORIGIN}{rules_path}"),
                EvidenceKind::Observed,
            ));
            for rule in rules
                .iter()
                .filter(|r| r["type"] == "required_status_checks")
            {
                for check in rule["parameters"]["required_status_checks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    required.names.insert(string(check, "context")?);
                }
            }
        } else {
            required.complete = false;
        }
        let branch_path = format!("/repos/{repository}/branches/{segment}");
        match self.readable(&branch_path)? {
            Some(b) if b["protected"] == false => {
                required.sources.push(RequiredSource::BranchProtection);
            }
            Some(b) if b["protection"].is_object() => {
                required.sources.push(RequiredSource::BranchProtection);
                let checks = &b["protection"]["required_status_checks"];
                for context in checks["contexts"].as_array().into_iter().flatten() {
                    if let Some(name) = context.as_str() {
                        required.names.insert(name.to_string());
                    }
                }
                for check in checks["checks"].as_array().into_iter().flatten() {
                    required.names.insert(string(check, "context")?);
                }
            }
            // Protected, but the summary is not visible to this token.
            _ => required.complete = false,
        }
        required.provenance.push(evidence(
            &format!("{ORIGIN}{branch_path}"),
            EvidenceKind::Observed,
        ));
        Ok(required)
    }

    /// Ruleset evaluations of pushes to `branch` over the past month, from
    /// `GET /repos/{repository}/rulesets/rule-suites`, which needs repository administration read
    /// access. Returns the listing, whether it was read in full, and its path.
    fn rule_suites(&self, repository: &str, branch: &str) -> Result<(Vec<Value>, bool, String)> {
        let reference = path_segment(&format!("refs/heads/{branch}"));
        let path = format!(
            "/repos/{repository}/rulesets/rule-suites?ref={reference}&time_period=month&rule_suite_result=all"
        );
        let (items, listed) = self.readable_pages(&path, None)?;
        Ok((items, listed, path))
    }

    /// Completed check runs and commit statuses on the head and the merge commit.
    fn check_results(
        &self,
        repository: &str,
        head: &str,
        merge: Option<&str>,
        branch: &str,
        required: &Required,
    ) -> Result<Checks> {
        let mut runs = Vec::new();
        let mut complete = true;
        let mut provenance = required.provenance.clone();
        let commits: Vec<&str> = std::iter::once(head)
            .chain(merge.filter(|m| *m != head))
            .collect();
        for sha in commits {
            let path = format!("/repos/{repository}/commits/{sha}/check-runs?filter=all");
            let (items, listed) = self.readable_pages(&path, Some("check_runs"))?;
            complete &= listed;
            provenance.push(evidence(&format!("{ORIGIN}{path}"), EvidenceKind::Observed));
            for run in &items {
                if run["status"] != "completed" {
                    continue;
                }
                let Some(conclusion) = run["conclusion"].as_str().and_then(conclusion) else {
                    // A completed run with a conclusion this version does not know: the
                    // listing cannot be judged in full.
                    complete = false;
                    continue;
                };
                let id = number(run, "id")?;
                runs.push(CheckRun {
                    name: string(run, "name")?,
                    kind: CheckKind::CheckRun,
                    commit_sha: sha.to_string(),
                    conclusion,
                    completed_at: timestamp(&string(run, "completed_at")?)?,
                    provenance: vec![evidence(
                        &format!("{ORIGIN}/repos/{repository}/check-runs/{id}"),
                        EvidenceKind::Observed,
                    )],
                });
            }
            let path = format!("/repos/{repository}/commits/{sha}/statuses");
            let (items, listed) = self.readable_pages(&path, None)?;
            complete &= listed;
            provenance.push(evidence(&format!("{ORIGIN}{path}"), EvidenceKind::Observed));
            for status in &items {
                let conclusion = match status["state"].as_str() {
                    Some("success") => Conclusion::Success,
                    Some("failure") => Conclusion::Failure,
                    Some("error") => Conclusion::Error,
                    _ => continue,
                };
                let id = number(status, "id")?;
                runs.push(CheckRun {
                    name: string(status, "context")?,
                    kind: CheckKind::Status,
                    commit_sha: sha.to_string(),
                    conclusion,
                    completed_at: timestamp(&string(status, "created_at")?)?,
                    provenance: vec![evidence(
                        &format!("{ORIGIN}/repos/{repository}/statuses/{sha}#{id}"),
                        EvidenceKind::Observed,
                    )],
                });
            }
        }
        Ok(Checks {
            branch: branch.to_string(),
            runs,
            runs_complete: complete,
            required: required.names.iter().cloned().collect(),
            required_sources: required.sources.clone(),
            required_complete: required.complete,
            provenance,
        })
    }

    /// `GET /rate_limit` (which does not count against the limit): the core resource's
    /// remaining requests, limit and reset time (Unix seconds).
    pub fn rate_limit(&self) -> Result<RateLimit> {
        let v = self.object("/rate_limit")?;
        let core = &v["resources"]["core"];
        let field = |name: &str| {
            core[name]
                .as_u64()
                .ok_or_else(|| unsupported("GitHub rate limit response is missing a field"))
        };
        Ok(RateLimit {
            remaining: field("remaining")?,
            limit: field("limit")?,
            reset: field("reset")?,
        })
    }

    /// `GET /repos/{repository}/releases/latest`: the latest release's tag name.
    pub fn latest_release(&self, repository: &str) -> Result<String> {
        validate_repo(repository)?;
        let v = self.object(&format!("/repos/{repository}/releases/latest"))?;
        v["tag_name"]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| unsupported("GitHub release has no tag name"))
    }

    /// Collect the pull requests merged in `from..=to` (or the single pull request `pr`),
    /// establishing agent authorship from `sources` in tier order: attestation and declaration
    /// (which must agree) or verified account first, then derived evidence (trailers, Agent
    /// Trace records).
    pub fn collect(
        &self,
        repository: &str,
        from: Timestamp,
        to: Timestamp,
        pr: Option<u64>,
        sources: Sources<'_>,
    ) -> Result<Events> {
        let Sources {
            known,
            trailers,
            traces,
            attestations,
            vendors,
            checks,
            bypass_as_of,
            teams,
            branches,
            deployments,
        } = sources;
        let mut required_by_branch: BTreeMap<String, Required> = BTreeMap::new();
        let mut suites_by_branch: BTreeMap<String, (Vec<Value>, bool, String)> = BTreeMap::new();
        validate_repo(repository)?;
        if from > to {
            return Err(failure(Exit::Usage, "window is reversed"));
        }
        let mut e = Events {
            version: "0.4".into(),
            repository: repository.into(),
            window: Window {
                from,
                to,
                complete: true,
                reason: None,
            },
            actors: BTreeMap::new(),
            attestations: BTreeMap::new(),
            teams: Vec::new(),
            branches: Vec::new(),
            deployments: None,
            changes: vec![],
        };
        let (numbers, complete) = match pr {
            Some(n) => (vec![n], true),
            None => {
                // Historical scans select merged PRs by merge time. Listing newest-updated first
                // lets the walk stop at the first page that ends before the window: a merge never
                // postdates the pull request's last update.
                let (items, complete) = self.pages(
                    &format!("/repos/{repository}/pulls?state=all&sort=updated&direction=desc"),
                    |page| {
                        page.last()
                            .and_then(|p| p["updated_at"].as_str())
                            .and_then(|s| timestamp(s).ok())
                            .is_some_and(|updated| updated < from)
                    },
                )?;
                let mut numbers = Vec::new();
                for p in items {
                    if let Some(m) = p["merged_at"].as_str() {
                        let at = timestamp(m)?;
                        if at >= from && at <= to {
                            numbers.push(number(&p, "number")?);
                        }
                    }
                }
                numbers.sort_unstable();
                (numbers, complete)
            }
        };
        if !complete {
            incomplete(&mut e, "Pull request pagination limit reached");
        }
        let mut seen = BTreeSet::new();
        for n in numbers {
            if !seen.insert(n) {
                return Err(api(
                    "GitHub returned duplicate pull requests; retry collection",
                ));
            }
            let path = format!("/repos/{repository}/pulls/{n}");
            let p = self.object(&path)?;
            let head = string(&p["head"], "sha")?;
            let source = evidence(&format!("{ORIGIN}{path}"), EvidenceKind::Observed);
            let author = match actor(&p["user"], &mut e.actors, &source, known, vendors)? {
                Some(a) => a,
                None => {
                    incomplete(&mut e, "Pull request author unavailable");
                    unavailable(&mut e.actors, &source)
                }
            };
            let merger = if p["merged_at"].is_string() {
                let m = actor(&p["merged_by"], &mut e.actors, &source, known, vendors)?;
                Some(m.unwrap_or_else(|| unavailable(&mut e.actors, &source)))
            } else {
                None
            };
            let (reviews, reviews_complete) = self.pages(&format!("{path}/reviews"), |_| false)?;
            let mut normalized = Vec::new();
            let mut history_complete = reviews_complete;
            for r in reviews {
                let state = match string(&r, "state")?.to_ascii_lowercase().as_str() {
                    "pending" => continue,
                    "approved" => ReviewState::Approved,
                    "changes_requested" => ReviewState::ChangesRequested,
                    "commented" => ReviewState::Commented,
                    "dismissed" => ReviewState::Dismissed,
                    _ => return Err(unsupported("unsupported GitHub review state")),
                };
                let id = number(&r, "id")?.to_string();
                let provenance = evidence(
                    &format!("{ORIGIN}{path}/reviews/{id}"),
                    EvidenceKind::Observed,
                );
                let reviewer = match actor(&r["user"], &mut e.actors, &provenance, known, vendors)?
                {
                    Some(a) => a,
                    None => {
                        // A review without an account cannot establish independence.
                        history_complete = false;
                        unavailable(&mut e.actors, &provenance)
                    }
                };
                // REST exposes the current dismissed state without the dismissal time: never
                // claim historical completeness.
                if state == ReviewState::Dismissed {
                    history_complete = false;
                }
                normalized.push(Review {
                    id,
                    actor_id: reviewer.actor_id,
                    state,
                    at: timestamp(&string(&r, "submitted_at")?)?,
                    commit_sha: string(&r, "commit_id")?,
                    provenance: reviewer.provenance,
                    agent: None,
                });
            }
            // Review attestations upgrade the forge's reviews and never create one: a matched
            // review gains the attestation's evidence and, for an agent reviewer, the facts the
            // predicate states about it; an unmatched attestation is recorded as such.
            if let Some(a) = attestations {
                for claim in a.reviews(&head)? {
                    let mut record = claim.record.clone();
                    let matched = normalized.iter().position(|r| {
                        r.actor_id == claim.reviewer_id
                            && r.commit_sha == head
                            && r.state == claim.decision
                    });
                    let Some(index) = matched else {
                        record.matched = false;
                        e.attestations.insert(claim.id.clone(), record);
                        continue;
                    };
                    let kind = e.actors[&claim.reviewer_id].kind;
                    ensure!(
                        kind == claim.reviewer_kind,
                        "review attestation reviewer kind does not match the account"
                    );
                    let facts = if kind == ActorKind::Agent {
                        let login = claim.reviewer_id.trim_start_matches("github:");
                        if let (Some(mapped), Some(named)) = (known.get(login), &claim.agent) {
                            ensure!(
                                mapped == named,
                                "review attestation names another agent than the account mapping"
                            );
                        }
                        let mut resolve = |id: &Option<String>| -> Result<Option<String>> {
                            let Some(id) = id else { return Ok(None) };
                            let login = id.strip_prefix("github:").unwrap_or(id);
                            if valid_login(login)
                                && let Response::Found(user, _) =
                                    self.get(&format!("/users/{login}"))?
                            {
                                actor(&user, &mut e.actors, &source, known, vendors)?;
                            }
                            let id = format!("github:{}", login.to_ascii_lowercase());
                            Ok(e.actors
                                .get(&id)
                                .filter(|a| a.kind == ActorKind::Human)
                                .map(|_| id))
                        };
                        Some(ReviewAgent {
                            operator: resolve(&claim.operator)?,
                            identity: claim.identity.clone(),
                            instructions_owner: resolve(&claim.instructions_owner)?,
                            model: claim.model.clone(),
                        })
                    } else {
                        ensure!(
                            claim.operator.is_none() && claim.instructions_owner.is_none(),
                            "review attestation for a human names an operator or instructions"
                        );
                        None
                    };
                    let review = &mut normalized[index];
                    if let (Some(existing), Some(new)) = (&review.agent, &facts) {
                        ensure!(
                            existing == new,
                            "conflicting review attestations for one review"
                        );
                    }
                    if facts.is_some() {
                        review.agent = facts;
                    }
                    review.provenance.push(claim.evidence.clone());
                    e.attestations.insert(claim.id.clone(), record);
                }
            }
            let (commits, commits_complete) = self.pages(&format!("{path}/commits"), |_| false)?;
            if !commits_complete || commits.len() as u64 != number(&p, "commits")? {
                incomplete(&mut e, "Commit collection is incomplete");
            }
            let mut normalized_commits = Vec::new();
            // Derived-tier evidence per commit: the vendor trailers and Agent Trace records
            // that name an agent, each with the evidence it came from.
            let mut derived: Vec<(Evidence, BTreeSet<String>)> = Vec::new();
            for commit in commits {
                let sha = string(&commit, "sha")?;
                let commit_ref = format!("{ORIGIN}/repos/{repository}/commits/{sha}");
                let source = evidence(&commit_ref, EvidenceKind::Observed);
                let author = actor(&commit["author"], &mut e.actors, &source, known, vendors)?;
                if let Some(registry) = trailers {
                    let message = commit["commit"]["message"].as_str().unwrap_or("");
                    let agents = crate::provenance::trailer_agents(message, registry);
                    if !agents.is_empty() {
                        derived.push((
                            Evidence {
                                source: "commit_trailer".into(),
                                r#ref: commit_ref.clone(),
                                kind: EvidenceKind::Derived,
                            },
                            agents,
                        ));
                    }
                }
                for hit in traces.map(|t| t.hits(&sha)).unwrap_or_default() {
                    // A record without a tool name attests AI authorship it cannot name;
                    // the model has no unnamed agent, so it is not interpreted.
                    if let Some(agent) = &hit.agent {
                        derived.push((
                            Evidence {
                                source: "agent_trace".into(),
                                r#ref: hit.locator.clone(),
                                kind: EvidenceKind::Derived,
                            },
                            BTreeSet::from([agent.clone()]),
                        ));
                    }
                }
                normalized_commits.push(Commit { sha, author });
            }
            let merge_commit_sha: Option<String> = p["merged_at"]
                .as_str()
                .and_then(|_| p["merge_commit_sha"].as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            let collected_checks = if checks && p["merged_at"].is_string() {
                let branch = string(&p["base"], "ref")?;
                if !required_by_branch.contains_key(&branch) {
                    let required = self.required_checks(repository, &branch)?;
                    required_by_branch.insert(branch.clone(), required);
                }
                Some(self.check_results(
                    repository,
                    &head,
                    merge_commit_sha.as_deref(),
                    &branch,
                    &required_by_branch[&branch],
                )?)
            } else {
                None
            };
            let collected_protection = match (bypass_as_of, p["merged_at"].as_str()) {
                (Some(as_of), Some(merged)) => {
                    let branch = string(&p["base"], "ref")?;
                    if !suites_by_branch.contains_key(&branch) {
                        let listed = self.rule_suites(repository, &branch)?;
                        suites_by_branch.insert(branch.clone(), listed);
                    }
                    let (suites, listed, path) = &suites_by_branch[&branch];
                    Some(protection(
                        repository,
                        &branch,
                        merge_commit_sha.as_deref(),
                        timestamp(merged)?,
                        as_of,
                        suites,
                        *listed,
                        path,
                    )?)
                }
                _ => None,
            };
            let mut c = Change {
                id: format!("github:{repository}:pr:{n}"),
                repository: repository.into(),
                forge: Forge::Github,
                title: string(&p, "title")?,
                url: format!("https://github.com/{repository}/pull/{n}"),
                opened_at: timestamp(&string(&p, "created_at")?)?,
                merged_at: p["merged_at"].as_str().map(timestamp).transpose()?,
                head_sha: head.clone(),
                // GitHub reports a merge commit for open pull requests too, but that is a test
                // merge that is recreated on every push; only a merged change's is a fact.
                merge_commit_sha,
                commits: normalized_commits,
                forge_author: author.clone(),
                author,
                agent_operator: None,
                reviews: normalized,
                reviews_complete: history_complete,
                merger,
                provenance: vec![
                    source.clone(),
                    evidence(&format!("{ORIGIN}{path}/reviews"), EvidenceKind::Observed),
                ],
                labels: p["labels"]
                    .as_array()
                    .map(|labels| {
                        labels
                            .iter()
                            .filter_map(|l| l["name"].as_str())
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default(),
                checks: collected_checks,
                protection: collected_protection,
                risk: None,
            };
            if !history_complete {
                incomplete(
                    &mut e,
                    "Reviews incomplete or historical dismissal time unavailable",
                );
            }
            let body = p["body"].as_str().unwrap_or("");
            let declared = crate::provenance::declaration(body)?;
            let login = p["user"]["login"].as_str().map(|l| l.to_ascii_lowercase());
            let mapped = login.as_deref().and_then(|l| known.get(l));
            // The explicit tier: a provenance attestation bound to the head, the inline
            // declaration, or both when they agree. Disagreement is an error, never a guess.
            let attested = match attestations {
                Some(a) => a.authorship(&head)?,
                None => None,
            };
            let declaration_evidence = evidence(
                &format!(
                    "https://github.com/{repository}/pull/{n}#issue-{}",
                    number(&p, "id")?
                ),
                EvidenceKind::Declared,
            );
            let explicit = match (attested, declared) {
                (Some(att), Some((agent, op))) => {
                    ensure!(
                        att.agent == agent,
                        "conflicting attested and declared agent"
                    );
                    if let (Some(a), Some(d)) = (&att.operator, &op) {
                        ensure!(
                            attestations::same_operator(a, d, "github"),
                            "conflicting attested and declared operator"
                        );
                    }
                    e.attestations.extend(att.records);
                    let mut provenance = att.evidence;
                    provenance.push(declaration_evidence);
                    Some((agent, att.operator.or(op), provenance))
                }
                (Some(att), None) => {
                    e.attestations.extend(att.records);
                    Some((att.agent, att.operator, att.evidence))
                }
                (None, Some((agent, op))) => Some((agent, op, vec![declaration_evidence])),
                (None, None) => None,
            };
            if let Some((agent, op, provenance)) = explicit {
                if let Some(known_agent) = mapped {
                    ensure!(
                        *known_agent == agent,
                        "conflicting known account and declared agent"
                    );
                }
                // Resolve a declared GitHub login explicitly through the API, never guess from
                // the merger.
                if let Some(login) = op.as_deref() {
                    let login = login.strip_prefix("github:").unwrap_or(login);
                    if valid_login(login)
                        && let Response::Found(user, _) = self.get(&format!("/users/{login}"))?
                    {
                        actor(&user, &mut e.actors, &source, known, vendors)?;
                    }
                }
                crate::provenance::apply(
                    &mut c,
                    &mut e.actors,
                    agent,
                    op,
                    crate::provenance::Registries {
                        namespace: "github",
                        vendors,
                    },
                    provenance,
                    Confidence::Explicit,
                )?;
            } else if let (Some(login), Some(agent)) = (login.as_deref(), mapped) {
                crate::provenance::apply(
                    &mut c,
                    &mut e.actors,
                    agent.clone(),
                    None,
                    crate::provenance::Registries {
                        namespace: "github",
                        vendors,
                    },
                    vec![Evidence {
                        source: "known_agent_account".into(),
                        r#ref: format!("github:{login}={agent}"),
                        kind: EvidenceKind::Declared,
                    }],
                    Confidence::Explicit,
                )?;
                c.author.provenance.push(source);
            } else if !derived.is_empty() {
                // The lower tier: records written at authoring time. One agent across every
                // piece of derived evidence establishes agent authorship with `derived`
                // confidence; two different agents are not interpreted (mixed authorship is
                // deferred), and the forge author stays the effective author.
                let agents: BTreeSet<&String> =
                    derived.iter().flat_map(|(_, a)| a.iter()).collect();
                if agents.len() == 1 {
                    let agent = (*agents.iter().next().expect("one agent")).clone();
                    // The operator is derived only when every commit in the change was
                    // authored by the same human account. Anything else stays unknown.
                    let mut authors = c.commits.iter().map(|k| {
                        k.author
                            .as_ref()
                            .map(|a| a.actor_id.as_str())
                            .filter(|id| e.actors[*id].kind == ActorKind::Human)
                    });
                    let operator = match authors.next().flatten() {
                        Some(first) if authors.all(|a| a == Some(first)) => Some(first.to_string()),
                        _ => None,
                    };
                    let provenance = derived.into_iter().map(|(ev, _)| ev).collect();
                    crate::provenance::apply(
                        &mut c,
                        &mut e.actors,
                        agent,
                        operator,
                        crate::provenance::Registries {
                            namespace: "github",
                            vendors,
                        },
                        provenance,
                        Confidence::Derived,
                    )?;
                }
            }
            // Detect a moving head/review snapshot; this exporter does not claim atomic API
            // snapshots.
            let after = self.object(&path)?;
            if after["head"]["sha"] != p["head"]["sha"]
                || after["updated_at"] != p["updated_at"]
                || after["merged_at"] != p["merged_at"]
                || after["merge_commit_sha"] != p["merge_commit_sha"]
            {
                c.reviews_complete = false;
                incomplete(&mut e, "Pull request changed during collection; retry");
            }
            if let Some(a) = attestations
                && let Some((risk, records)) = a.risk(&c.head_sha)?
            {
                e.attestations.extend(records);
                c.risk = Some(risk);
            }
            e.changes.push(c);
        }
        if let Some((ids, observed_at)) = teams {
            for id in ids {
                e.teams.push(self.team(id, observed_at)?);
            }
        }
        if let Some((names, as_of)) = branches {
            let names = if names.is_empty() {
                self.default_branch(repository)?.into_iter().collect()
            } else {
                names.to_vec()
            };
            if names.is_empty() {
                // The default branch could not be read: record nothing, and the gap with it.
                incomplete(&mut e, "Default branch unavailable");
            }
            for name in names {
                let b = self.branch(
                    repository,
                    &name,
                    from,
                    to,
                    as_of,
                    &mut e.actors,
                    known,
                    vendors,
                )?;
                e.branches.push(b);
            }
        }
        if let Some(query) = deployments {
            let d = self.deployments(repository, query, &mut e.actors, known, vendors)?;
            e.deployments = Some(d);
        }
        crate::normalize::events(e)
    }

    /// The repository's default branch, as `GET /repos/{repository}` says now.
    fn default_branch(&self, repository: &str) -> Result<Option<String>> {
        Ok(self
            .readable(&format!("/repos/{repository}"))?
            .and_then(|r| r["default_branch"].as_str().map(String::from)))
    }

    /// Every update of `name` in `from..=to`, from the repository activity record
    /// (`GET /repos/{repository}/activity`), which lists pushes, force pushes, merges, creations
    /// and deletions, newest first. Its widest period is a year back from collection, so a window
    /// reaching further is incomplete.
    #[allow(clippy::too_many_arguments)]
    fn branch(
        &self,
        repository: &str,
        name: &str,
        from: Timestamp,
        to: Timestamp,
        as_of: Timestamp,
        actors: &mut BTreeMap<String, Actor>,
        known: &BTreeMap<String, String>,
        vendors: &BTreeMap<String, String>,
    ) -> Result<Branch> {
        let reference = path_segment(&format!("refs/heads/{name}"));
        let path = format!("/repos/{repository}/activity?ref={reference}&time_period=year");
        let mut items = Vec::new();
        let mut listed = false;
        for page in 1..=self.max_pages {
            let (value, next) =
                match self.fetch(&format!("{path}&per_page=100&page={page}"), true)? {
                    Response::Found(value, next) => (value, next),
                    Response::NotFound | Response::Forbidden => break,
                };
            let page_items = value
                .as_array()
                .ok_or_else(|| api("GitHub list response is not an array"))?;
            let older = page_items
                .last()
                .and_then(|a| a["timestamp"].as_str())
                .and_then(|s| timestamp(s).ok())
                .is_some_and(|at| at < from);
            items.extend(page_items.iter().cloned());
            if !next || older {
                listed = true;
                break;
            }
        }
        let source = evidence(&format!("{ORIGIN}{path}"), EvidenceKind::Observed);
        let mut events = Vec::new();
        let mut understood = true;
        for a in &items {
            let at = timestamp(&string(a, "timestamp")?)?;
            if at < from || at > to {
                continue;
            }
            let kind = match a["activity_type"].as_str() {
                Some("push") => BranchEventKind::Push,
                Some("force_push") => BranchEventKind::ForcePush,
                Some("branch_creation") => BranchEventKind::BranchCreation,
                Some("branch_deletion") => BranchEventKind::BranchDeletion,
                Some("pr_merge") => BranchEventKind::PrMerge,
                Some("merge_queue_merge") => BranchEventKind::MergeQueueMerge,
                // An update this version does not know cannot be judged.
                _ => {
                    understood = false;
                    continue;
                }
            };
            let id = number(a, "id")?.to_string();
            let provenance = evidence(&format!("{ORIGIN}{path}#{id}"), EvidenceKind::Observed);
            let actor_id =
                actor(&a["actor"], actors, &provenance, known, vendors)?.map(|i| i.actor_id);
            events.push(BranchEvent {
                id,
                kind,
                before: string(a, "before")?,
                after: string(a, "after")?,
                at,
                actor_id,
                provenance: vec![provenance],
            });
        }
        let in_reach = from >= as_of - chrono::Duration::days(365);
        Ok(Branch {
            id: format!("github:{repository}:branch:{name}"),
            name: name.to_string(),
            events,
            complete: listed && in_reach && understood,
            provenance: vec![source],
        })
    }

    /// The commit date of `sha`, from `GET /repos/{repository}/commits/{sha}`.
    fn committed_at(&self, repository: &str, sha: &str) -> Result<Option<Timestamp>> {
        self.readable(&format!("/repos/{repository}/commits/{sha}"))?
            .and_then(|c| c["commit"]["committer"]["date"].as_str().map(String::from))
            .map(|s| timestamp(&s))
            .transpose()
    }

    /// The first-parent commits after `base` up to and including `head`, newest first, from
    /// `GET /repos/{repository}/compare/{base}...{head}`, and whether they were read in full.
    fn first_parent(
        &self,
        repository: &str,
        base: &str,
        head: &str,
    ) -> Result<(Vec<DeployedCommit>, bool)> {
        let path = format!("/repos/{repository}/compare/{base}...{head}");
        let mut by_sha: BTreeMap<String, (Option<String>, Timestamp)> = BTreeMap::new();
        let mut total = None;
        let mut status = None;
        for page in 1..=self.max_pages {
            let Some(v) = self.readable(&format!("{path}?per_page=100&page={page}"))? else {
                return Ok((Vec::new(), false));
            };
            total = v["total_commits"].as_u64();
            status = v["status"].as_str().map(String::from);
            let commits = v["commits"].as_array().cloned().unwrap_or_default();
            if commits.is_empty() {
                break;
            }
            for c in &commits {
                let sha = string(c, "sha")?.to_ascii_lowercase();
                let parent = c["parents"][0]["sha"].as_str().map(str::to_ascii_lowercase);
                let date = timestamp(&string(&c["commit"]["committer"], "date")?)?;
                by_sha.insert(sha, (parent, date));
            }
            if total.is_some_and(|t| by_sha.len() as u64 >= t) {
                break;
            }
        }
        match status.as_deref() {
            // The head is the base, or behind it (a rollback): only the head itself shipped.
            Some("identical" | "behind") => {
                return Ok(match self.committed_at(repository, head)? {
                    Some(at) => (
                        vec![DeployedCommit {
                            sha: head.to_ascii_lowercase(),
                            committed_at: at,
                        }],
                        true,
                    ),
                    None => (Vec::new(), false),
                });
            }
            Some("ahead" | "diverged") => {}
            _ => return Ok((Vec::new(), false)),
        }
        let listed = total.is_some_and(|t| by_sha.len() as u64 == t);
        let mut out = Vec::new();
        let mut cursor = Some(head.to_ascii_lowercase());
        while let Some(sha) = cursor {
            let Some((parent, date)) = by_sha.get(&sha) else {
                break;
            };
            out.push(DeployedCommit {
                sha: sha.clone(),
                committed_at: *date,
            });
            cursor = parent.clone();
        }
        let reached_head = out
            .first()
            .is_some_and(|c| c.sha.eq_ignore_ascii_case(head));
        Ok((out, listed && reached_head))
    }

    /// The successful deployments to production environments in `query`'s window, each with the
    /// commits it shipped since the environment's previous successful deployment.
    fn deployments(
        &self,
        repository: &str,
        query: super::DeploymentQuery<'_>,
        actors: &mut BTreeMap<String, Actor>,
        known: &BTreeMap<String, String>,
        vendors: &BTreeMap<String, String>,
    ) -> Result<Deployments> {
        let mut complete = true;
        let mut provenance = Vec::new();
        let mut out = Vec::new();
        let listings: Vec<(Option<&str>, String)> = if query.environments.is_empty() {
            vec![(None, format!("/repos/{repository}/deployments"))]
        } else {
            query
                .environments
                .iter()
                .map(|env| {
                    (
                        Some(env.as_str()),
                        format!(
                            "/repos/{repository}/deployments?environment={}",
                            path_segment(env)
                        ),
                    )
                })
                .collect()
        };
        for (environment, path) in listings {
            provenance.push(evidence(&format!("{ORIGIN}{path}"), EvidenceKind::Observed));
            // Successful deployments per environment, newest first, through the first one that
            // succeeded before the window: that one bounds the earliest deployment inside it.
            let mut successful: BTreeMap<String, Vec<(Value, Timestamp)>> = BTreeMap::new();
            let mut bounded: BTreeSet<String> = BTreeSet::new();
            let mut listed = false;
            'pages: for page in 1..=self.max_pages {
                let sep = if path.contains('?') { '&' } else { '?' };
                let (value, next) =
                    match self.fetch(&format!("{path}{sep}per_page=100&page={page}"), true)? {
                        Response::Found(value, next) => (value, next),
                        Response::NotFound | Response::Forbidden => break,
                    };
                let items = value
                    .as_array()
                    .ok_or_else(|| api("GitHub list response is not an array"))?;
                for d in items {
                    let env = string(d, "environment")?;
                    if environment.is_none() && d["production_environment"] != true {
                        continue;
                    }
                    if bounded.contains(&env) {
                        continue;
                    }
                    let id = number(d, "id")?;
                    let statuses_path = format!("/repos/{repository}/deployments/{id}/statuses");
                    let Some(Value::Array(statuses)) = self.readable(&statuses_path)? else {
                        complete = false;
                        continue;
                    };
                    let Some(succeeded) = statuses
                        .iter()
                        .find(|s| s["state"] == "success")
                        .and_then(|s| s["created_at"].as_str())
                        .map(timestamp)
                        .transpose()?
                    else {
                        continue;
                    };
                    if succeeded < query.from {
                        bounded.insert(env.clone());
                    }
                    if succeeded <= query.to {
                        successful
                            .entry(env)
                            .or_default()
                            .push((d.clone(), succeeded));
                    }
                }
                let all_bounded = environment.is_some_and(|env| bounded.contains(env));
                if !next || all_bounded {
                    listed = true;
                    break 'pages;
                }
            }
            complete &= listed;
            for (env, list) in successful {
                for (i, (d, at)) in list.iter().enumerate() {
                    if *at < query.from {
                        continue;
                    }
                    let id = number(d, "id")?;
                    let sha = string(d, "sha")?.to_ascii_lowercase();
                    let source = evidence(
                        &format!("{ORIGIN}/repos/{repository}/deployments/{id}"),
                        EvidenceKind::Observed,
                    );
                    let creator =
                        actor(&d["creator"], actors, &source, known, vendors)?.map(|a| a.actor_id);
                    let previous = list
                        .get(i + 1)
                        .and_then(|(p, _)| p["sha"].as_str())
                        .map(str::to_ascii_lowercase);
                    let (commits, commits_complete) = match &previous {
                        Some(base) => self.first_parent(repository, base, &sha)?,
                        None => match self.committed_at(repository, &sha)? {
                            // Nothing bounds the range: the deployed commit alone is judged.
                            Some(at) => (
                                vec![DeployedCommit {
                                    sha: sha.clone(),
                                    committed_at: at,
                                }],
                                false,
                            ),
                            None => (Vec::new(), false),
                        },
                    };
                    if commits.is_empty() {
                        // The deployed commit itself could not be read; nothing can be judged.
                        complete = false;
                        continue;
                    }
                    out.push(Deployment {
                        id: format!("github:{repository}:deployment:{id}"),
                        environment: env.clone(),
                        sha,
                        at: *at,
                        creator,
                        previous_sha: previous,
                        commits,
                        commits_complete,
                        provenance: vec![source],
                    });
                }
            }
        }
        Ok(Deployments {
            environments: query.environments.to_vec(),
            from: query.from,
            to: query.to,
            deployments: out,
            complete,
            provenance,
        })
    }

    /// A reviewer team's members, as GitHub lists them now. A listing the token cannot read in
    /// full is recorded as incomplete, never as empty.
    fn team(&self, id: &str, observed_at: Timestamp) -> Result<Team> {
        let (org, slug) = id
            .split_once('/')
            .filter(|(o, s)| !o.is_empty() && !s.is_empty() && !s.contains('/'))
            .ok_or_else(|| failure(Exit::Usage, "a reviewer team must be ORG/SLUG"))?;
        let path = format!(
            "/orgs/{}/teams/{}/members",
            path_segment(org),
            path_segment(slug)
        );
        let (items, complete) = self.readable_pages(&path, None)?;
        let mut members: Vec<String> = items
            .iter()
            .filter_map(|m| m["login"].as_str())
            .filter(|l| valid_login(l))
            .map(|l| format!("github:{}", l.to_ascii_lowercase()))
            .collect();
        members.sort();
        members.dedup();
        Ok(Team {
            id: id.to_ascii_lowercase(),
            members,
            complete,
            observed_at,
            provenance: vec![evidence(
                &format!("https://api.github.com{path}"),
                EvidenceKind::Observed,
            )],
        })
    }
}

/// How far back the rule-suite listing reaches: GitHub's widest `time_period` is a month, so
/// anything merged more than 28 days before collection may have fallen out of it.
const RULE_SUITE_REACH_DAYS: i64 = 28;

/// The ruleset evaluations of one merged change: those whose push landed its merge commit.
#[allow(clippy::too_many_arguments)]
fn protection(
    repository: &str,
    branch: &str,
    merge: Option<&str>,
    merged_at: Timestamp,
    as_of: Timestamp,
    listing: &[Value],
    listed: bool,
    path: &str,
) -> Result<Protection> {
    let mut suites = Vec::new();
    let mut understood = true;
    if let Some(merge) = merge {
        for suite in listing.iter().filter(|s| s["after_sha"] == merge) {
            let id = number(suite, "id")?;
            let result = match suite["result"].as_str() {
                Some("bypass") => SuiteResult::Bypass,
                Some("fail") => SuiteResult::Fail,
                Some("pass") => SuiteResult::Pass,
                // A result this version does not know: the evaluation cannot be judged.
                _ => {
                    understood = false;
                    continue;
                }
            };
            suites.push(RuleSuite {
                id: id.to_string(),
                after_sha: merge.to_string(),
                result,
                actor: suite["actor_name"].as_str().map(String::from),
                pushed_at: timestamp(&string(suite, "pushed_at")?)?,
                provenance: vec![evidence(
                    &format!("{ORIGIN}/repos/{repository}/rulesets/rule-suites/{id}"),
                    EvidenceKind::Observed,
                )],
            });
        }
    }
    let in_reach = merged_at >= as_of - chrono::Duration::days(RULE_SUITE_REACH_DAYS);
    Ok(Protection {
        branch: branch.to_string(),
        suites,
        complete: listed && in_reach && understood && merge.is_some(),
        provenance: vec![evidence(&format!("{ORIGIN}{path}"), EvidenceKind::Observed)],
    })
}

/// The forge's required checks for one branch, read once per collection.
struct Required {
    names: BTreeSet<String>,
    sources: Vec<RequiredSource>,
    complete: bool,
    provenance: Vec<Evidence>,
}

/// A check run conclusion, or `None` for one this version does not know.
fn conclusion(s: &str) -> Option<Conclusion> {
    Some(match s {
        "action_required" => Conclusion::ActionRequired,
        "cancelled" => Conclusion::Cancelled,
        "failure" => Conclusion::Failure,
        "neutral" => Conclusion::Neutral,
        "skipped" => Conclusion::Skipped,
        "stale" => Conclusion::Stale,
        "success" => Conclusion::Success,
        "timed_out" => Conclusion::TimedOut,
        _ => return None,
    })
}

/// A branch name as one URL path segment: everything but RFC 3986 unreserved characters is
/// percent-encoded, so a branch like `release/1.0` stays one segment.
fn path_segment(s: &str) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            write!(out, "%{b:02X}").expect("writing to a String cannot fail");
        }
    }
    out
}

fn incomplete(e: &mut Events, reason: &str) {
    e.window.complete = false;
    let existing = e.window.reason.get_or_insert_with(String::new);
    if !existing.contains(reason) {
        if !existing.is_empty() {
            existing.push_str("; ");
        }
        existing.push_str(reason);
    }
}

fn evidence(reference: &str, kind: EvidenceKind) -> Evidence {
    Evidence {
        source: "github_api".into(),
        r#ref: reference.into(),
        kind,
    }
}

fn string(v: &Value, key: &str) -> Result<String> {
    v[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| unsupported("Required GitHub string field unavailable"))
}

fn number(v: &Value, key: &str) -> Result<u64> {
    v[key]
        .as_u64()
        .ok_or_else(|| unsupported("Required GitHub numeric field unavailable"))
}

/// Register the account in `user` and return its identity, or `None` when GitHub returned no
/// account at all (`null`), which happens for deleted users.
fn actor(
    user: &Value,
    actors: &mut BTreeMap<String, Actor>,
    source: &Evidence,
    known: &BTreeMap<String, String>,
    vendors: &BTreeMap<String, String>,
) -> Result<Option<Identity>> {
    if user.is_null() {
        return Ok(None);
    }
    let login = string(user, "login")?.to_ascii_lowercase();
    let id = format!("github:{login}");
    let kind = if known.contains_key(&login) {
        ActorKind::Agent
    } else {
        match user["type"].as_str() {
            Some("User") => ActorKind::Human,
            Some("Bot") => ActorKind::Bot,
            _ => ActorKind::Unknown,
        }
    };
    actors.insert(
        id.clone(),
        Actor {
            kind,
            display_name: Some(login.clone()),
            vendor: known.get(&login).and_then(|a| vendors.get(a)).cloned(),
        },
    );
    let mut provenance = vec![source.clone()];
    if let Some(agent) = known.get(&login) {
        provenance.push(Evidence {
            source: "known_agent_account".into(),
            r#ref: format!("github:{login}={agent}"),
            kind: EvidenceKind::Declared,
        });
    }
    Ok(Some(Identity {
        actor_id: id,
        provenance,
    }))
}

fn unavailable(actors: &mut BTreeMap<String, Actor>, source: &Evidence) -> Identity {
    actors.insert(
        UNAVAILABLE_ACTOR.into(),
        Actor {
            kind: ActorKind::Unknown,
            display_name: None,
            vendor: None,
        },
    );
    Identity {
        actor_id: UNAVAILABLE_ACTOR.into(),
        provenance: vec![source.clone()],
    }
}

fn valid_login(s: &str) -> bool {
    !s.is_empty() && s.len() <= 39 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// `OWNER/REPO`, with a valid login as owner and a plain path segment as name.
pub fn validate_repo(s: &str) -> Result<()> {
    let ok = match s.split_once('/') {
        Some((owner, name)) => {
            valid_login(owner)
                && !name.is_empty()
                && name != "."
                && name != ".."
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        }
        None => false,
    };
    if !ok {
        return Err(failure(Exit::Usage, "repository must be OWNER/REPO"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repositories_are_owner_slash_name() {
        assert!(validate_repo("acme/api").is_ok());
        assert!(validate_repo("acme/api.v2").is_ok());
        for bad in [
            "acme", "acme/", "/api", "../evil", "acme/..", "acme/a/b", "a_b/api",
        ] {
            assert!(validate_repo(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn hints_depend_on_the_status_the_rate_limit_and_the_token() {
        assert!(hint(0, false, true).contains("network"));
        assert!(hint(401, false, true).contains("was rejected"));
        assert!(hint(401, false, false).starts_with("set GITHUB_TOKEN"));
        assert!(hint(403, true, true).contains("rate limit is exhausted"));
        assert!(hint(429, true, false).contains("60 an hour"));
        assert!(hint(403, false, true).contains("cannot read this repository"));
        assert!(hint(403, false, false).contains("unauthenticated"));
        assert!(hint(404, false, false).contains("private repository"));
        assert!(hint(404, false, true).contains("token can read"));
        assert!(hint(500, false, true).contains("unexpected status"));
    }

    #[test]
    fn logins_follow_github_rules() {
        assert!(valid_login("alice"));
        assert!(valid_login("alice-1"));
        assert!(!valid_login(""));
        assert!(!valid_login("alice_1"));
        assert!(!valid_login(&"a".repeat(40)));
    }

    #[test]
    fn only_loopback_bases_are_custom() {
        assert!(loopback("http://127.0.0.1:8080"));
        assert!(loopback("http://localhost:1"));
        assert!(!loopback("http://127.0.0.1:"));
        assert!(!loopback("https://127.0.0.1:8080"));
        assert!(!loopback("http://evil.example:80"));
        assert!(Github::with_base(Some("secret".into()), 1, "http://127.0.0.1:1").is_err());
        assert!(Github::with_base(None, 1, "https://evil.example").is_err());
        assert!(Github::with_base(None, 0, ORIGIN).is_err());
        assert!(Github::with_base(Some("secret".into()), 1, ORIGIN).is_ok());
    }

    #[test]
    fn incomplete_reasons_accumulate_once() {
        let mut e = Events {
            version: "0.1".into(),
            repository: "acme/api".into(),
            window: Window {
                from: timestamp("2026-08-01T00:00:00Z").unwrap(),
                to: timestamp("2026-08-31T00:00:00Z").unwrap(),
                complete: true,
                reason: None,
            },
            actors: BTreeMap::new(),
            attestations: BTreeMap::new(),
            teams: Vec::new(),
            branches: Vec::new(),
            deployments: None,
            changes: vec![],
        };
        incomplete(&mut e, "a");
        incomplete(&mut e, "b");
        incomplete(&mut e, "a");
        assert!(!e.window.complete);
        assert_eq!(e.window.reason.as_deref(), Some("a; b"));
    }
}
