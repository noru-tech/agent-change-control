//! Forge collectors. Each one normalizes into [`crate::model::Events`] and never evaluates policy.

use crate::model::Timestamp;
use crate::provenance::agent_trace::AgentTraces;
use crate::provenance::attestations::Attestations;
use std::collections::BTreeMap;

pub mod github;
pub mod gitlab;

/// The caller-supplied evidence sources for a collection.
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    /// Verified agent accounts, `login -> agent` (a GitHub login or a GitLab username).
    pub known: &'a BTreeMap<String, String>,
    /// The vendor trailer registry, or `None` to ignore commit trailers.
    pub trailers: Option<&'a BTreeMap<String, String>>,
    /// Agent Trace records to bind to commits, if any.
    pub traces: Option<&'a AgentTraces>,
    /// Attestations to bind to head commits, if any.
    pub attestations: Option<&'a Attestations>,
    /// The vendor registry, `agent -> vendor`.
    pub vendors: &'a BTreeMap<String, String>,
    /// Collect check results and required checks for merged changes (ACC004).
    pub checks: bool,
    /// Collect ruleset evaluations of merged changes (ACC005), as of this instant: GitHub lists
    /// them only for the past month, so an older merge is recorded as out of reach.
    pub bypass_as_of: Option<Timestamp>,
    /// Reviewer teams to record (GitHub `ORG/SLUG` or a GitLab group path, ACC011), and the
    /// instant their membership is observed: forges expose current membership only.
    pub teams: Option<(&'a [String], Timestamp)>,
    /// Protected branches whose updates to record (ACC012; empty names the default branch), and
    /// the instant of collection: the forge's activity record reaches back a year from it.
    pub branches: Option<(&'a [String], Timestamp)>,
    /// Production deployments to record (ACC013).
    pub deployments: Option<DeploymentQuery<'a>>,
}

/// Which deployments to record, and over which window.
#[derive(Debug, Clone, Copy)]
pub struct DeploymentQuery<'a> {
    /// Environments that count as production; empty means those the forge marks as production.
    pub environments: &'a [String],
    pub from: Timestamp,
    pub to: Timestamp,
}
