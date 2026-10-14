//! Read-only GitLab transport and normalization; no policy evaluation.
//!
//! The same posture as the GitHub collector: GET-only against a fixed origin (GitLab.com) with
//! redirects disabled, a body cap and a page cap, and fixed error messages so response headers
//! and bodies never leak.
//!
//! GitLab differs from GitHub in three ways that shape what is recorded:
//!
//! - **An approval names no commit.** `GET …/merge_requests/:iid/approvals` lists the approvals in
//!   force with `approved_at` and no SHA. Each is bound to the head of the latest diff version
//!   (`GET …/versions`) created at or before it, recorded as `derived` evidence. A version created
//!   within a second of the approval leaves the binding ambiguous, and the approval is then not
//!   recorded at all: the review history is incomplete rather than guessed (spec §4).
//! - **There are no named required checks.** "Pipelines must succeed" in the project's merge
//!   settings requires the `pipeline` check; every completed pipeline on the head or merge commit
//!   is recorded under that name, and each of its jobs under the job's own name.
//! - **There is no ruleset bypass record** on every tier, so ACC005's facts are recorded as
//!   incomplete when asked for.

use super::Sources;
use super::github::UNAVAILABLE_ACTOR;
use crate::model::*;
use crate::normalize::timestamp;
use crate::provenance::attestations;
use crate::{Exit, Failure, failure};
use anyhow::{Result, ensure};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::time::Duration;

const ORIGIN: &str = "https://gitlab.com";
const API: &str = "/api/v4";
const MAX_BYTES: u64 = 8 * 1024 * 1024;
const USER_AGENT: &str = concat!("agent-change-control/", env!("CARGO_PKG_VERSION"));
/// The account GitLab moves a deleted user's contributions to. It names nobody.
const GHOST: &str = "ghost";
/// The name of the check a project's "Pipelines must succeed" setting requires.
pub const PIPELINE_CHECK: &str = "pipeline";

pub struct Gitlab {
    agent: ureq::Agent,
    token: Option<String>,
    base: String,
    pub max_pages: usize,
    /// `bot` for each user id already looked up, so an account is asked about once.
    bots: RefCell<BTreeMap<u64, Option<bool>>>,
}

enum Response {
    Found(Value, bool),
    NotFound,
    Forbidden,
}

fn api(message: &'static str) -> anyhow::Error {
    failure(Exit::Api, message)
}

fn unsupported(message: &'static str) -> anyhow::Error {
    failure(Exit::Unsupported, message)
}

const SET_TOKEN: &str =
    "set GITLAB_TOKEN to a personal, project or group access token with the read_api scope";

/// What to do about a failed request. Hints never quote the response.
pub fn hint(status: u16, rate_limited: bool, token: bool) -> String {
    match (status, rate_limited, token) {
        (0, ..) => "check the network connection to gitlab.com (and HTTPS_PROXY, if you use one), then retry".into(),
        (401, _, true) => "the token in GITLAB_TOKEN was rejected: it is expired, revoked or mistyped; replace it with one that has the read_api scope".into(),
        (401, _, false) => SET_TOKEN.into(),
        (_, true, _) => "GitLab's rate limit is exhausted; wait a minute and retry, or narrow the window to make fewer requests".into(),
        (403, false, true) => "the token cannot read this project: it needs the read_api scope and at least the Reporter role".into(),
        (403, false, false) => format!("GitLab refused an unauthenticated request; {SET_TOKEN}"),
        (404, _, true) => "check GROUP/PROJECT and the merge request number, and that the token can read this project".into(),
        (404, _, false) => format!("check GROUP/PROJECT and the merge request number; a private project looks missing without a token: {SET_TOKEN}"),
        _ => "GitLab answered with an unexpected status; retry, and check https://status.gitlab.com".into(),
    }
}

/// `http://127.0.0.1:PORT` or `http://localhost:PORT`.
fn loopback(base: &str) -> bool {
    base.strip_prefix("http://127.0.0.1:")
        .or_else(|| base.strip_prefix("http://localhost:"))
        .is_some_and(|port| !port.is_empty() && port.chars().all(|c| c.is_ascii_digit()))
}

impl Gitlab {
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
            bots: RefCell::new(BTreeMap::new()),
        })
    }

    /// GET `path` (under `/api/v4`); with `tolerate`, a permission 403 is
    /// [`Response::Forbidden`] rather than an error.
    fn fetch(&self, path: &str, tolerate: bool) -> Result<Response> {
        ensure!(
            path.starts_with('/') && !path.starts_with("//"),
            "invalid API path"
        );
        let mut request = self
            .agent
            .get(format!("{}{API}{path}", self.base))
            .header("Accept", "application/json");
        if let Some(token) = &self.token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }
        let token = self.token.is_some();
        let mut response = request
            .call()
            .map_err(|_| self.failed(Exit::Api, "GitLab request failed", 0, false))?;
        let status = response.status().as_u16();
        match status {
            200 => {}
            404 => return Ok(Response::NotFound),
            401 => {
                return Err(self.failed(Exit::Auth, "GitLab authentication failed", 401, false));
            }
            403 | 429 => {
                let limited = status == 429
                    || response
                        .headers()
                        .get("ratelimit-remaining")
                        .is_some_and(|v| v.as_bytes() == b"0");
                if tolerate && !limited {
                    return Ok(Response::Forbidden);
                }
                return Err(self.failed(
                    Exit::Api,
                    "GitLab request forbidden or rate limited",
                    status,
                    limited,
                ));
            }
            _ => {
                return Err(
                    Failure::new(Exit::Api, "GitLab API returned an unexpected status")
                        .hint(hint(status, false, token))
                        .into(),
                );
            }
        }
        // Offset pagination announces the next page both ways; either is enough.
        let headers = response.headers();
        let next = headers
            .get("link")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|s| s.contains("rel=\"next\""))
            || headers
                .get("x-next-page")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|s| !s.trim().is_empty());
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| api("GitLab response read failed"))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(api("GitLab payload exceeds size limit"));
        }
        let value =
            serde_json::from_slice(&bytes).map_err(|_| api("GitLab returned invalid JSON"))?;
        Ok(Response::Found(value, next))
    }

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
        self.failed(Exit::Api, "GitLab resource not found", 404, false)
    }

    fn object(&self, path: &str) -> Result<Value> {
        match self.fetch(path, false)? {
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

    /// Walk a list endpoint. With `tolerate`, an unreadable listing ends the walk as incomplete
    /// instead of failing. `done` inspects each page and may end the walk early. Returns the
    /// items and whether the listing is complete: an unreadable or truncated listing is
    /// incomplete, never empty.
    fn pages(
        &self,
        path: &str,
        tolerate: bool,
        done: impl Fn(&[Value]) -> bool,
    ) -> Result<(Vec<Value>, bool)> {
        let mut all = Vec::new();
        for page in 1..=self.max_pages {
            let sep = if path.contains('?') { '&' } else { '?' };
            let (value, next) =
                match self.fetch(&format!("{path}{sep}per_page=100&page={page}"), tolerate)? {
                    Response::Found(value, next) => (value, next),
                    Response::NotFound | Response::Forbidden if tolerate => {
                        return Ok((all, false));
                    }
                    Response::NotFound | Response::Forbidden => return Err(self.not_found()),
                };
            let items = value
                .as_array()
                .ok_or_else(|| api("GitLab list response is not an array"))?;
            let stop = done(items);
            all.extend(items.iter().cloned());
            if !next || stop {
                return Ok((all, true));
            }
        }
        Ok((all, false))
    }

    /// Whether the account with this id is a bot, as `GET /users/:id` says; `None` when GitLab
    /// does not say.
    fn bot(&self, id: u64) -> Result<Option<bool>> {
        if let Some(known) = self.bots.borrow().get(&id) {
            return Ok(*known);
        }
        let bot = self
            .readable(&format!("/users/{id}"))?
            .and_then(|u| u["bot"].as_bool());
        self.bots.borrow_mut().insert(id, bot);
        Ok(bot)
    }

    /// Register the account in `user` and return its identity, or `None` when GitLab returned
    /// no account (`null`) or the ghost account it gives a deleted user's contributions.
    fn actor(
        &self,
        user: &Value,
        actors: &mut BTreeMap<String, Actor>,
        source: &Evidence,
        sources: &Sources<'_>,
    ) -> Result<Option<Identity>> {
        if user.is_null() {
            return Ok(None);
        }
        let username = string(user, "username")?.to_ascii_lowercase();
        if username == GHOST {
            return Ok(None);
        }
        ensure!(valid_username(&username), "invalid GitLab username");
        let id = format!("gitlab:{username}");
        let kind = if sources.known.contains_key(&username) {
            ActorKind::Agent
        } else {
            // A user object embedded in a merge request does not say whether it is a bot; the
            // user endpoint does. When it cannot be read the kind stays unknown, which never
            // counts as a human approval.
            match self.bot(number(user, "id")?)? {
                Some(false) => ActorKind::Human,
                Some(true) => ActorKind::Bot,
                None => ActorKind::Unknown,
            }
        };
        actors.insert(
            id.clone(),
            Actor {
                kind,
                display_name: Some(username.clone()),
                vendor: sources
                    .known
                    .get(&username)
                    .and_then(|a| sources.vendors.get(a))
                    .cloned(),
            },
        );
        let mut provenance = vec![source.clone()];
        if let Some(agent) = sources.known.get(&username) {
            provenance.push(Evidence {
                source: "known_agent_account".into(),
                r#ref: format!("gitlab:{username}={agent}"),
                kind: EvidenceKind::Declared,
            });
        }
        Ok(Some(Identity {
            actor_id: id,
            provenance,
        }))
    }

    /// Resolve a declared account name through `GET /users?username=`, never by guessing.
    /// Returns the actor ID when GitLab confirms a human with exactly that username.
    fn declared_human(
        &self,
        name: &str,
        actors: &mut BTreeMap<String, Actor>,
        source: &Evidence,
        sources: &Sources<'_>,
    ) -> Result<Option<String>> {
        let name = name.strip_prefix("gitlab:").unwrap_or(name);
        if !valid_username(name) {
            return Ok(None);
        }
        let Some(Value::Array(users)) = self.readable(&format!("/users?username={name}"))? else {
            return Ok(None);
        };
        let Some(user) = users.iter().find(|u| {
            u["username"]
                .as_str()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        }) else {
            return Ok(None);
        };
        self.actor(user, actors, source, sources)?;
        let id = format!("gitlab:{}", name.to_ascii_lowercase());
        Ok(actors
            .get(&id)
            .filter(|a| a.kind == ActorKind::Human)
            .map(|_| id))
    }

    /// The checks the project's merge settings require: `pipeline` when "Pipelines must
    /// succeed" is on. GitLab keeps no record of the settings in force at an earlier time.
    fn required_checks(&self, project: &str) -> Result<Required> {
        let path = format!("/projects/{}", path_segment(project));
        let mut required = Required {
            names: BTreeSet::new(),
            sources: Vec::new(),
            complete: false,
            provenance: vec![evidence(&path, EvidenceKind::Observed)],
        };
        if let Some(p) = self.readable(&path)?
            && let Some(on) = p["only_allow_merge_if_pipeline_succeeds"].as_bool()
        {
            required.complete = true;
            required.sources.push(RequiredSource::ProjectSettings);
            if on {
                required.names.insert(PIPELINE_CHECK.into());
            }
        }
        Ok(required)
    }

    /// Completed pipelines and their jobs on the head and the merge commit.
    fn check_results(
        &self,
        project: &str,
        head: &str,
        merge: Option<&str>,
        branch: &str,
        required: &Required,
    ) -> Result<Checks> {
        let encoded = path_segment(project);
        let mut runs = Vec::new();
        let mut complete = true;
        let mut provenance = required.provenance.clone();
        let commits: Vec<&str> = std::iter::once(head)
            .chain(merge.filter(|m| *m != head))
            .collect();
        for sha in commits {
            let path = format!("/projects/{encoded}/pipelines?sha={sha}");
            let (pipelines, listed) = self.pages(&path, true, |_| false)?;
            complete &= listed;
            provenance.push(evidence(&path, EvidenceKind::Observed));
            for listed in &pipelines {
                let id = number(listed, "id")?;
                let detail_path = format!("/projects/{encoded}/pipelines/{id}");
                let Some(pipeline) = self.readable(&detail_path)? else {
                    complete = false;
                    continue;
                };
                match pipeline_conclusion(&pipeline) {
                    Outcome::Done(conclusion, at) => runs.push(CheckRun {
                        name: PIPELINE_CHECK.into(),
                        kind: CheckKind::Pipeline,
                        commit_sha: sha.to_string(),
                        conclusion,
                        completed_at: at,
                        provenance: vec![evidence(&detail_path, EvidenceKind::Observed)],
                    }),
                    Outcome::Running => {}
                    Outcome::Unknown => complete = false,
                }
                let jobs_path =
                    format!("/projects/{encoded}/pipelines/{id}/jobs?include_retried=true");
                let (jobs, jobs_listed) = self.pages(&jobs_path, true, |_| false)?;
                complete &= jobs_listed;
                for job in &jobs {
                    match job_conclusion(job) {
                        Outcome::Done(conclusion, at) => {
                            let job_id = number(job, "id")?;
                            runs.push(CheckRun {
                                name: string(job, "name")?,
                                kind: CheckKind::Job,
                                commit_sha: sha.to_string(),
                                conclusion,
                                completed_at: at,
                                provenance: vec![evidence(
                                    &format!("/projects/{encoded}/jobs/{job_id}"),
                                    EvidenceKind::Observed,
                                )],
                            });
                        }
                        Outcome::Running => {}
                        Outcome::Unknown => complete = false,
                    }
                }
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

    /// Collect the merge requests merged in `from..=to` (or the single merge request `mr`),
    /// establishing agent authorship from `sources` in the same tier order as the GitHub
    /// collector.
    pub fn collect(
        &self,
        project: &str,
        from: Timestamp,
        to: Timestamp,
        mr: Option<u64>,
        sources: Sources<'_>,
    ) -> Result<Events> {
        validate_project(project)?;
        if from > to {
            return Err(failure(Exit::Usage, "window is reversed"));
        }
        let project = project.to_string();
        let encoded = path_segment(&project);
        let mut e = Events {
            version: "0.4".into(),
            repository: project.clone(),
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
        let (numbers, complete) = match mr {
            Some(n) => (vec![n], true),
            None => {
                // Merged merge requests newest-updated first: the walk stops at the first page
                // that ends before the window, because a merge never postdates the last update.
                let (items, complete) = self.pages(
                    &format!(
                        "/projects/{encoded}/merge_requests?state=merged&order_by=updated_at&sort=desc"
                    ),
                    false,
                    |page| {
                        page.last()
                            .and_then(|p| p["updated_at"].as_str())
                            .and_then(|s| timestamp(s).ok())
                            .is_some_and(|updated| updated < from)
                    },
                )?;
                let mut numbers = Vec::new();
                for m in items {
                    if let Some(at) = m["merged_at"].as_str() {
                        let at = timestamp(at)?;
                        if at >= from && at <= to {
                            numbers.push(number(&m, "iid")?);
                        }
                    }
                }
                numbers.sort_unstable();
                (numbers, complete)
            }
        };
        if !complete {
            incomplete(&mut e, "Merge request pagination limit reached");
        }
        let mut required: Option<Required> = None;
        let mut seen = BTreeSet::new();
        for n in numbers {
            if !seen.insert(n) {
                return Err(api(
                    "GitLab returned duplicate merge requests; retry collection",
                ));
            }
            let path = format!("/projects/{encoded}/merge_requests/{n}");
            let m = self.object(&path)?;
            let c = self.change(&project, n, &path, &m, &mut e, &sources, &mut required)?;
            e.changes.push(c);
        }
        if let Some((ids, observed_at)) = sources.teams {
            for id in ids {
                e.teams.push(self.team(id, observed_at)?);
            }
        }
        // GitLab's push events say neither whether a push was forced nor whether a merge request
        // landed it, and this collector does not read its deployments yet: both are recorded as
        // unreadable, so ACC012 and ACC013 are unknown rather than passing on silence.
        if let Some((names, _)) = sources.branches {
            let names: Vec<String> = if names.is_empty() {
                self.readable(&format!("/projects/{encoded}"))?
                    .and_then(|p| p["default_branch"].as_str().map(String::from))
                    .into_iter()
                    .collect()
            } else {
                names.to_vec()
            };
            if names.is_empty() {
                incomplete(&mut e, "Default branch unavailable");
            }
            for name in names {
                e.branches.push(Branch {
                    id: format!("gitlab:{project}:branch:{name}"),
                    name,
                    events: Vec::new(),
                    complete: false,
                    provenance: Vec::new(),
                });
            }
        }
        if let Some(query) = sources.deployments {
            e.deployments = Some(Deployments {
                environments: query.environments.to_vec(),
                from: query.from,
                to: query.to,
                deployments: Vec::new(),
                complete: false,
                provenance: Vec::new(),
            });
        }
        crate::normalize::events(e)
    }

    /// One merge request as a [`Change`].
    #[allow(clippy::too_many_arguments)]
    fn change(
        &self,
        project: &str,
        n: u64,
        path: &str,
        m: &Value,
        e: &mut Events,
        sources: &Sources<'_>,
        required: &mut Option<Required>,
    ) -> Result<Change> {
        let encoded = path_segment(project);
        let head = string(m, "sha")?;
        let source = evidence(path, EvidenceKind::Observed);
        let merged = m["state"] == "merged" && m["merged_at"].is_string();
        let author = match self.actor(&m["author"], &mut e.actors, &source, sources)? {
            Some(a) => a,
            None => {
                incomplete(e, "Merge request author unavailable");
                unavailable(&mut e.actors, &source)
            }
        };
        let merger = if merged {
            // `merge_user` is who merged or set the change to merge automatically; `merged_by`
            // is its deprecated predecessor.
            let user = if m["merge_user"].is_null() {
                &m["merged_by"]
            } else {
                &m["merge_user"]
            };
            let who = self.actor(user, &mut e.actors, &source, sources)?;
            Some(who.unwrap_or_else(|| unavailable(&mut e.actors, &source)))
        } else {
            None
        };

        let (reviews, mut history_complete) = self.approvals(path, &head, e, sources)?;

        let (commits, commits_complete) =
            self.pages(&format!("{path}/commits"), false, |_| false)?;
        if !commits_complete {
            incomplete(e, "Commit collection is incomplete");
        }
        let mut normalized_commits = Vec::new();
        let mut derived: Vec<(Evidence, BTreeSet<String>)> = Vec::new();
        for commit in commits {
            let sha = string(&commit, "id")?;
            let commit_ref = format!("{ORIGIN}{API}/projects/{encoded}/repository/commits/{sha}");
            if let Some(registry) = sources.trailers {
                let message = commit["message"].as_str().unwrap_or("");
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
            for hit in sources.traces.map(|t| t.hits(&sha)).unwrap_or_default() {
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
            // GitLab's commit listing carries the author's name and email and no account, and
            // an email never resolves an identity (spec §4).
            normalized_commits.push(Commit { sha, author: None });
        }

        // The commit the merge left on the target branch: the merge commit, or the squash
        // commit when the merge was a fast-forward of a squashed change. A plain fast-forward
        // reports neither, and the commit is then not known.
        let merge_commit_sha: Option<String> = if merged {
            m["merge_commit_sha"]
                .as_str()
                .or_else(|| m["squash_commit_sha"].as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        } else {
            None
        };
        let branch = string(m, "target_branch")?;
        let checks = if sources.checks && merged {
            if required.is_none() {
                *required = Some(self.required_checks(project)?);
            }
            Some(self.check_results(
                project,
                &head,
                merge_commit_sha.as_deref(),
                &branch,
                required.as_ref().expect("read above"),
            )?)
        } else {
            None
        };
        // GitLab records no ruleset evaluation of a merge on every tier, so a request for
        // ACC005's facts is answered honestly: not readable here.
        let protection = (sources.bypass_as_of.is_some() && merged).then(|| Protection {
            branch: branch.clone(),
            suites: Vec::new(),
            complete: false,
            provenance: Vec::new(),
        });
        let mut c = Change {
            id: format!("gitlab:{project}:mr:{n}"),
            repository: project.into(),
            forge: Forge::Gitlab,
            title: string(m, "title")?,
            url: format!("{ORIGIN}/{project}/-/merge_requests/{n}"),
            opened_at: timestamp(&string(m, "created_at")?)?,
            merged_at: if merged {
                Some(timestamp(&string(m, "merged_at")?)?)
            } else {
                None
            },
            head_sha: head.clone(),
            merge_commit_sha,
            commits: normalized_commits,
            forge_author: author.clone(),
            author,
            agent_operator: None,
            reviews: Vec::new(),
            reviews_complete: false,
            merger,
            provenance: vec![
                source.clone(),
                evidence(&format!("{path}/approvals"), EvidenceKind::Observed),
                evidence(&format!("{path}/versions"), EvidenceKind::Observed),
            ],
            labels: m["labels"]
                .as_array()
                .map(|labels| {
                    labels
                        .iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
            checks,
            protection,
            risk: None,
        };
        c.reviews = reviews;
        self.authorship(&mut c, m, &head, &source, derived, e, sources)?;
        // Detect a moving head or approval set; this collector does not claim atomic snapshots.
        let after = self.object(path)?;
        if after["sha"] != m["sha"]
            || after["updated_at"] != m["updated_at"]
            || after["merged_at"] != m["merged_at"]
            || after["merge_commit_sha"] != m["merge_commit_sha"]
        {
            history_complete = false;
            incomplete(e, "Merge request changed during collection; retry");
        }
        c.reviews_complete = history_complete;
        if !history_complete {
            incomplete(
                e,
                "Approvals incomplete or not bound to a commit unambiguously",
            );
        }
        if let Some(a) = sources.attestations
            && let Some((risk, records)) = a.risk(&c.head_sha)?
        {
            e.attestations.extend(records);
            c.risk = Some(risk);
        }
        Ok(c)
    }

    /// The approvals in force, each bound to the commit that was the head when it was given.
    /// Returns the reviews and whether every approval was recorded.
    fn approvals(
        &self,
        path: &str,
        head: &str,
        e: &mut Events,
        sources: &Sources<'_>,
    ) -> Result<(Vec<Review>, bool)> {
        let approvals_path = format!("{path}/approvals");
        let Some(approvals) = self.readable(&approvals_path)? else {
            return Ok((Vec::new(), false));
        };
        let (versions, versions_complete) =
            self.pages(&format!("{path}/versions"), true, |_| false)?;
        let versions_path = format!("{path}/versions");
        let mut timeline = Vec::new();
        for v in &versions {
            timeline.push(Version {
                id: number(v, "id")?,
                head: string(v, "head_commit_sha")?,
                at: timestamp(&string(v, "created_at")?)?,
            });
        }
        let mut complete = versions_complete;
        let mut reviews = Vec::new();
        let source = evidence(&approvals_path, EvidenceKind::Observed);
        for a in approvals["approved_by"].as_array().into_iter().flatten() {
            let reviewer = match self.actor(&a["user"], &mut e.actors, &source, sources)? {
                Some(r) => r,
                None => {
                    // An approval without an account cannot establish independence.
                    complete = false;
                    continue;
                }
            };
            // GitLab Community Edition omits `approved_at`; without it nothing binds.
            let Some(at) = a["approved_at"].as_str().map(timestamp).transpose()? else {
                complete = false;
                continue;
            };
            let Some(version) = bind(&timeline, at) else {
                complete = false;
                continue;
            };
            let user_id = number(&a["user"], "id")?;
            let mut provenance = reviewer.provenance;
            provenance.push(Evidence {
                source: "gitlab_api".into(),
                r#ref: format!("{ORIGIN}{API}{versions_path}#{}", version.id),
                kind: EvidenceKind::Derived,
            });
            reviews.push(Review {
                id: format!("approval:{user_id}"),
                actor_id: reviewer.actor_id,
                state: ReviewState::Approved,
                at,
                commit_sha: version.head.clone(),
                provenance,
                agent: None,
            });
        }
        // Review attestations upgrade an approval of the head and never create one.
        if let Some(att) = sources.attestations {
            for claim in att.reviews(head)? {
                let mut record = claim.record.clone();
                let matched = reviews.iter().position(|r| {
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
                    let username = claim.reviewer_id.trim_start_matches("gitlab:");
                    if let (Some(mapped), Some(named)) = (sources.known.get(username), &claim.agent)
                    {
                        ensure!(
                            mapped == named,
                            "review attestation names another agent than the account mapping"
                        );
                    }
                    let mut resolve = |id: &Option<String>| -> Result<Option<String>> {
                        match id {
                            Some(id) => self.declared_human(id, &mut e.actors, &source, sources),
                            None => Ok(None),
                        }
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
                let review = &mut reviews[index];
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
        Ok((reviews, complete))
    }

    /// Establish the effective author: an attestation and declaration (which must agree) or a
    /// verified account first, then derived evidence. Identical in meaning to the GitHub
    /// collector; only account resolution differs.
    #[allow(clippy::too_many_arguments)]
    fn authorship(
        &self,
        c: &mut Change,
        m: &Value,
        head: &str,
        source: &Evidence,
        derived: Vec<(Evidence, BTreeSet<String>)>,
        e: &mut Events,
        sources: &Sources<'_>,
    ) -> Result<()> {
        let registries = || crate::provenance::Registries {
            namespace: "gitlab",
            vendors: sources.vendors,
        };
        let body = m["description"].as_str().unwrap_or("");
        let declared = crate::provenance::declaration(body)?;
        let username = m["author"]["username"]
            .as_str()
            .map(str::to_ascii_lowercase);
        let mapped = username.as_deref().and_then(|u| sources.known.get(u));
        let attested = match sources.attestations {
            Some(a) => a.authorship(head)?,
            None => None,
        };
        let declaration_evidence = evidence(
            &format!(
                "/projects/{}/merge_requests/{}#description",
                path_segment(&c.repository),
                number(m, "iid")?
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
                        attestations::same_operator(a, d, "gitlab"),
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
            // A declared operator resolves only to an account GitLab confirms.
            if let Some(name) = op.as_deref() {
                self.declared_human(name, &mut e.actors, source, sources)?;
            }
            crate::provenance::apply(
                c,
                &mut e.actors,
                agent,
                op,
                registries(),
                provenance,
                Confidence::Explicit,
            )?;
        } else if let (Some(username), Some(agent)) = (username.as_deref(), mapped) {
            crate::provenance::apply(
                c,
                &mut e.actors,
                agent.clone(),
                None,
                registries(),
                vec![Evidence {
                    source: "known_agent_account".into(),
                    r#ref: format!("gitlab:{username}={agent}"),
                    kind: EvidenceKind::Declared,
                }],
                Confidence::Explicit,
            )?;
            c.author.provenance.push(source.clone());
        } else if !derived.is_empty() {
            // One agent across every piece of derived evidence establishes agent authorship
            // with `derived` confidence. GitLab's commits carry no accounts, so the operator is
            // never derived here and stays unknown.
            let agents: BTreeSet<&String> = derived.iter().flat_map(|(_, a)| a.iter()).collect();
            if agents.len() == 1 {
                let agent = (*agents.iter().next().expect("one agent")).clone();
                let provenance = derived.into_iter().map(|(ev, _)| ev).collect();
                crate::provenance::apply(
                    c,
                    &mut e.actors,
                    agent,
                    None,
                    registries(),
                    provenance,
                    Confidence::Derived,
                )?;
            }
        }
        Ok(())
    }

    /// A reviewer team's direct members, as GitLab lists them now. Members inherited from an
    /// ancestor group are not counted: they would make the whole parent group the team. A
    /// listing the token cannot read in full is incomplete, never empty.
    fn team(&self, id: &str, observed_at: Timestamp) -> Result<Team> {
        let segments: Vec<&str> = id.split('/').collect();
        if segments.len() < 2 || !segments.iter().all(|s| valid_segment(s)) {
            return Err(failure(
                Exit::Usage,
                "a GitLab reviewer team must be a group path, GROUP/SUBGROUP",
            ));
        }
        let path = format!("/groups/{}/members", path_segment(id));
        let (items, complete) = self.pages(&path, true, |_| false)?;
        let mut members: Vec<String> = items
            .iter()
            .filter_map(|m| m["username"].as_str())
            .map(str::to_ascii_lowercase)
            .filter(|u| valid_username(u) && u != GHOST)
            .map(|u| format!("gitlab:{u}"))
            .collect();
        members.sort();
        members.dedup();
        Ok(Team {
            id: id.to_ascii_lowercase(),
            members,
            complete,
            observed_at,
            provenance: vec![evidence(&path, EvidenceKind::Observed)],
        })
    }
}

/// One diff version of a merge request: the head it showed, and when it was created.
struct Version {
    id: u64,
    head: String,
    at: Timestamp,
}

/// The version an approval given at `at` was given against: the latest created at or before it.
/// `None` when no version precedes it, or when any version was created within a second of it
/// either way: GitLab creates a version shortly after a push, so an approval that close to one
/// could have been given against either head.
fn bind(timeline: &[Version], at: Timestamp) -> Option<&Version> {
    let close = chrono::Duration::seconds(1);
    if timeline.iter().any(|v| (v.at - at).abs() < close) {
        return None;
    }
    timeline
        .iter()
        .filter(|v| v.at <= at)
        .max_by_key(|v| (v.at, v.id))
}

/// A pipeline or job's state, as far as ACC004 is concerned.
enum Outcome {
    Done(Conclusion, Timestamp),
    /// Not finished, or waiting for a person: no result yet.
    Running,
    /// A state this version does not know: the listing cannot be judged in full.
    Unknown,
}

fn finished(v: &Value, conclusion: Conclusion) -> Outcome {
    match v["finished_at"].as_str().map(timestamp) {
        Some(Ok(at)) => Outcome::Done(conclusion, at),
        Some(Err(_)) => Outcome::Unknown,
        None => Outcome::Running,
    }
}

fn pipeline_conclusion(p: &Value) -> Outcome {
    match p["status"].as_str() {
        Some("success") => finished(p, Conclusion::Success),
        Some("failed") => finished(p, Conclusion::Failure),
        Some("canceled") => finished(p, Conclusion::Cancelled),
        Some("skipped") => finished(p, Conclusion::Skipped),
        Some(
            "created"
            | "waiting_for_resource"
            | "preparing"
            | "pending"
            | "running"
            | "manual"
            | "scheduled"
            | "canceling"
            | "waiting_for_callback",
        ) => Outcome::Running,
        _ => Outcome::Unknown,
    }
}

fn job_conclusion(j: &Value) -> Outcome {
    match j["status"].as_str() {
        Some("success") => finished(j, Conclusion::Success),
        // A job allowed to fail does not fail its pipeline; it is recorded as neutral, the way
        // GitHub records a check that reports without passing or failing.
        Some("failed") if j["allow_failure"] == true => finished(j, Conclusion::Neutral),
        Some("failed") => finished(j, Conclusion::Failure),
        Some("canceled") => finished(j, Conclusion::Cancelled),
        Some("skipped") => finished(j, Conclusion::Skipped),
        Some(
            "created"
            | "waiting_for_resource"
            | "preparing"
            | "pending"
            | "running"
            | "manual"
            | "scheduled"
            | "canceling"
            | "waiting_for_callback",
        ) => Outcome::Running,
        _ => Outcome::Unknown,
    }
}

/// The project's required checks, read once per collection.
struct Required {
    names: BTreeSet<String>,
    sources: Vec<RequiredSource>,
    complete: bool,
    provenance: Vec<Evidence>,
}

/// A project or group path as one URL path segment, the form GitLab's API takes it in:
/// everything but RFC 3986 unreserved characters is percent-encoded, `/` included.
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

/// Evidence for an API path under `/api/v4`.
fn evidence(path: &str, kind: EvidenceKind) -> Evidence {
    Evidence {
        source: "gitlab_api".into(),
        r#ref: format!("{ORIGIN}{API}{path}"),
        kind,
    }
}

fn string(v: &Value, key: &str) -> Result<String> {
    v[key]
        .as_str()
        .map(String::from)
        .ok_or_else(|| unsupported("Required GitLab string field unavailable"))
}

fn number(v: &Value, key: &str) -> Result<u64> {
    v[key]
        .as_u64()
        .ok_or_else(|| unsupported("Required GitLab numeric field unavailable"))
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

/// A GitLab username: letters, digits, `_`, `-` and `.`, at most 255 characters.
fn valid_username(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c))
}

/// One segment of a group or project path.
fn valid_segment(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && !s.starts_with('-')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

/// `GROUP/PROJECT` or `GROUP/SUBGROUP/…/PROJECT`.
pub fn validate_project(s: &str) -> Result<()> {
    let segments: Vec<&str> = s.split('/').collect();
    if segments.len() < 2 || segments.len() > 20 || !segments.iter().all(|s| valid_segment(s)) {
        return Err(failure(
            Exit::Usage,
            "project must be GROUP/PROJECT or GROUP/SUBGROUP/PROJECT",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Timestamp {
        timestamp(s).unwrap()
    }

    fn version(id: u64, head: &str, at: &str) -> Version {
        Version {
            id,
            head: head.into(),
            at: t(at),
        }
    }

    #[test]
    fn projects_are_group_paths() {
        for ok in ["acme/api", "acme/platform/api", "acme/api.v2", "a_b/c-d"] {
            assert!(validate_project(ok).is_ok(), "{ok}");
        }
        for bad in [
            "acme", "acme/", "/api", "../evil", "acme/..", "acme/-x", "acme/a b",
        ] {
            assert!(validate_project(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn paths_are_one_encoded_segment() {
        assert_eq!(path_segment("acme/platform/api"), "acme%2Fplatform%2Fapi");
    }

    #[test]
    fn an_approval_binds_to_the_latest_version_before_it() {
        let timeline = [
            version(1, "aaa", "2026-09-01T10:00:00.000Z"),
            version(2, "bbb", "2026-09-01T11:00:00.000Z"),
            version(3, "ccc", "2026-09-01T12:00:00.000Z"),
        ];
        assert_eq!(
            bind(&timeline, t("2026-09-01T11:30:00Z")).unwrap().head,
            "bbb"
        );
        assert_eq!(
            bind(&timeline, t("2026-09-01T13:00:00Z")).unwrap().head,
            "ccc"
        );
        // Before any version: nothing to bind to.
        assert!(bind(&timeline, t("2026-09-01T09:00:00Z")).is_none());
        // Within a second of a version, either side: ambiguous.
        assert!(bind(&timeline, t("2026-09-01T12:00:00.500Z")).is_none());
        assert!(bind(&timeline, t("2026-09-01T11:59:59.600Z")).is_none());
        assert!(bind(&timeline, t("2026-09-01T12:00:01.000Z")).is_some());
    }

    #[test]
    fn pipeline_and_job_states_map_to_conclusions() {
        let done = |v: Value| match job_conclusion(&v) {
            Outcome::Done(c, _) => Some(c),
            _ => None,
        };
        let at = "2026-09-01T10:00:00Z";
        assert_eq!(
            done(serde_json::json!({"status": "failed", "allow_failure": true, "finished_at": at})),
            Some(Conclusion::Neutral)
        );
        assert_eq!(
            done(
                serde_json::json!({"status": "failed", "allow_failure": false, "finished_at": at})
            ),
            Some(Conclusion::Failure)
        );
        assert!(matches!(
            job_conclusion(&serde_json::json!({"status": "manual", "finished_at": null})),
            Outcome::Running
        ));
        assert!(matches!(
            pipeline_conclusion(&serde_json::json!({"status": "teleported", "finished_at": at})),
            Outcome::Unknown
        ));
    }

    #[test]
    fn hints_name_gitlab() {
        assert!(hint(401, false, false).starts_with("set GITLAB_TOKEN"));
        assert!(hint(404, false, false).contains("private project"));
        assert!(hint(429, true, true).contains("rate limit"));
    }

    #[test]
    fn only_loopback_bases_are_custom() {
        assert!(Gitlab::with_base(Some("secret".into()), 1, "http://127.0.0.1:1").is_err());
        assert!(Gitlab::with_base(None, 1, "https://evil.example").is_err());
        assert!(Gitlab::with_base(None, 1, "http://127.0.0.1:1").is_ok());
        assert!(Gitlab::with_base(Some("secret".into()), 1, ORIGIN).is_ok());
    }
}
