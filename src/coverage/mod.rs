//! Coverage of a commit range by change verdicts: the decision `acc verify` makes before a deploy.
//!
//! The range is a first-parent history, newest first, as `git rev-list --first-parent` prints it.
//! Every commit on it must be the commit a change landed on the target branch (its merge commit;
//! for a squash or a single-commit rebase, the commit the forge reports as such), and that change
//! must pass the policy. Commits a merge commit brought in are not on the first-parent history, so
//! the merge's change covers them. Nothing else covers a commit: a direct push, or the earlier
//! commits of a multi-commit rebase merge, are uncovered.
//!
//! The same change may be judged by more than one attestation. A failing verdict from any of them
//! decides it, because a signer that saw a failing fact is not overruled by one that did not; an
//! incomplete verdict yields to a complete one, because a later collection may have filled the gap.

use crate::Exit;
use crate::manifest::{ChangeVerdict, Verdict};

/// One change's verdict as one attestation reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judged {
    /// Which attestation judged it (a file and position, for the report).
    pub source: String,
    pub change: ChangeVerdict,
}

/// What covers one commit of the range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitCoverage {
    pub commit: String,
    pub status: Status,
    /// Every verdict on a change that landed this commit.
    pub judged: Vec<Judged>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Status {
    /// A change landed this commit and passes.
    Covered,
    /// The only verdicts on the changes that landed it are incomplete.
    Incomplete,
    /// No verified attestation records a change that landed this commit.
    Uncovered,
    /// A change that landed it fails the policy.
    Failing,
}

impl Status {
    pub const fn as_str(self) -> &'static str {
        match self {
            Status::Covered => "covered",
            Status::Incomplete => "incomplete",
            Status::Uncovered => "uncovered",
            Status::Failing => "failing",
        }
    }
}

/// The coverage of every commit in `commits` by the verdicts in `judged`.
pub fn cover(commits: &[String], judged: &[Judged]) -> Vec<CommitCoverage> {
    commits
        .iter()
        .map(|commit| {
            let mine: Vec<Judged> = judged
                .iter()
                .filter(|j| {
                    j.change
                        .merge_commit_sha
                        .as_deref()
                        .is_some_and(|sha| sha.eq_ignore_ascii_case(commit))
                })
                .cloned()
                .collect();
            let worst = |v: Verdict| mine.iter().any(|j| j.change.verdict == v);
            let status = if mine.is_empty() {
                Status::Uncovered
            } else if worst(Verdict::Fail) {
                Status::Failing
            } else if worst(Verdict::Pass) {
                Status::Covered
            } else {
                Status::Incomplete
            };
            CommitCoverage {
                commit: commit.to_ascii_lowercase(),
                status,
                judged: mine,
            }
        })
        .collect()
}

/// The exit a coverage result decides: incomplete takes precedence over a failure, as it does for
/// `acc check`; anything uncovered or failing is a policy failure.
pub fn exit(coverage: &[CommitCoverage]) -> Exit {
    if coverage.iter().any(|c| c.status == Status::Incomplete) {
        Exit::Incomplete
    } else if coverage.iter().any(|c| c.status != Status::Covered) {
        Exit::PolicyFailed
    } else {
        Exit::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judged(source: &str, change: &str, merge: Option<&str>, verdict: Verdict) -> Judged {
        Judged {
            source: source.into(),
            change: ChangeVerdict {
                change_id: change.into(),
                merge_commit_sha: merge.map(String::from),
                verdict,
                failing: Vec::new(),
            },
        }
    }

    fn commits(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_commit_is_covered_only_by_the_change_that_landed_it() {
        let j = [
            judged("a#1", "pr:1", Some("AAAA"), Verdict::Pass),
            judged("a#1", "pr:2", None, Verdict::Pass),
        ];
        let c = cover(&commits(&["aaaa", "bbbb"]), &j);
        assert_eq!(c[0].status, Status::Covered);
        assert_eq!(c[0].commit, "aaaa");
        assert_eq!(c[1].status, Status::Uncovered);
        assert!(c[1].judged.is_empty());
        assert_eq!(exit(&c), Exit::PolicyFailed);
    }

    #[test]
    fn a_failing_verdict_decides_and_a_complete_one_beats_incomplete() {
        let c = cover(
            &commits(&["aaaa"]),
            &[
                judged("a#1", "pr:1", Some("aaaa"), Verdict::Pass),
                judged("b#1", "pr:1", Some("aaaa"), Verdict::Fail),
            ],
        );
        assert_eq!(c[0].status, Status::Failing);
        let c = cover(
            &commits(&["aaaa"]),
            &[
                judged("a#1", "pr:1", Some("aaaa"), Verdict::Incomplete),
                judged("b#1", "pr:1", Some("aaaa"), Verdict::Pass),
            ],
        );
        assert_eq!(c[0].status, Status::Covered);
        assert_eq!(exit(&c), Exit::Ok);
    }

    #[test]
    fn incomplete_takes_precedence_over_failure() {
        let c = cover(
            &commits(&["aaaa", "bbbb"]),
            &[judged("a#1", "pr:1", Some("aaaa"), Verdict::Incomplete)],
        );
        assert_eq!(c[0].status, Status::Incomplete);
        assert_eq!(c[1].status, Status::Uncovered);
        assert_eq!(exit(&c), Exit::Incomplete);
    }

    #[test]
    fn an_empty_range_needs_nothing() {
        assert_eq!(exit(&cover(&[], &[])), Exit::Ok);
    }
}
