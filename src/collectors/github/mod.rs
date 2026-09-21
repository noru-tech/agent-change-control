//! Read-only GitHub transport and normalization; no policy evaluation.
//!
//! Requests are GET-only against a fixed origin with redirects disabled, a body cap and a page
//! cap. Error messages are fixed strings so response headers and bodies never leak.

use crate::model::*;
use crate::normalize::timestamp;
use crate::provenance::agent_trace::AgentTraces;
use crate::provenance::attestations::{self, Attestations};
use crate::{Exit, failure};
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

/// The caller-supplied evidence sources for a collection.
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    /// Verified agent accounts, `login -> agent`.
    pub known: &'a BTreeMap<String, String>,
    /// The vendor trailer registry, or `None` to ignore commit trailers.
    pub trailers: Option<&'a BTreeMap<String, String>>,
    /// Agent Trace records to bind to commits, if any.
    pub traces: Option<&'a AgentTraces>,
    /// Attestations to bind to head commits, if any.
    pub attestations: Option<&'a Attestations>,
    /// The vendor registry, `agent -> vendor`.
    pub vendors: &'a BTreeMap<String, String>,
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
}

fn api(message: &'static str) -> anyhow::Error {
    failure(Exit::Api, message)
}

fn unsupported(message: &'static str) -> anyhow::Error {
    failure(Exit::Unsupported, message)
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
        let mut response = request.call().map_err(|_| api("GitHub request failed"))?;
        match response.status().as_u16() {
            200 => {}
            404 => return Ok(Response::NotFound),
            401 => return Err(failure(Exit::Auth, "GitHub authentication failed")),
            403 | 429 => return Err(api("GitHub request forbidden or rate limited")),
            _ => return Err(api("GitHub API returned an unexpected status")),
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

    fn object(&self, path: &str) -> Result<Value> {
        match self.get(path)? {
            Response::Found(value, _) => Ok(value),
            Response::NotFound => Err(api("GitHub resource not found")),
        }
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
                return Err(api("GitHub resource not found"));
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
        } = sources;
        validate_repo(repository)?;
        if from > to {
            return Err(failure(Exit::Usage, "window is reversed"));
        }
        let mut e = Events {
            version: "0.1".into(),
            repository: repository.into(),
            window: Window {
                from,
                to,
                complete: true,
                reason: None,
            },
            actors: BTreeMap::new(),
            attestations: BTreeMap::new(),
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
                merge_commit_sha: p["merged_at"]
                    .as_str()
                    .and_then(|_| p["merge_commit_sha"].as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from),
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
            e.changes.push(c);
        }
        crate::normalize::events(e)
    }
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
            changes: vec![],
        };
        incomplete(&mut e, "a");
        incomplete(&mut e, "b");
        incomplete(&mut e, "a");
        assert!(!e.window.complete);
        assert_eq!(e.window.reason.as_deref(), Some("a; b"));
    }
}
