//! Pull requests and their revisions (design section 9).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ObjectId, PrId, RepoId, RevisionId, StackId, UserId};

/// The Git host that holds the canonical repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Forge {
    GitHub,
    GitLab,
    Forgejo,
}

/// The PR number on the forge, for example `#42`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrNumber(pub u64);

impl fmt::Display for PrNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// UTC time as milliseconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UnixMillis(pub i64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Draft,
    Open,
    Merged,
    Closed,
}

impl PrState {
    /// True if no more changes can occur to the PR.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Merged | Self::Closed)
    }
}

/// One pull request. It is one branch of a stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pr {
    pub id: PrId,
    pub repo_id: RepoId,
    pub stack_id: StackId,
    pub number: PrNumber,
    /// The PR below this PR in the stack. `None` if the PR is on trunk.
    pub parent_pr_id: Option<PrId>,
    /// Position in the stack. 0 is the PR on trunk.
    pub position: u32,
    pub state: PrState,
    pub author_id: UserId,
}

/// One pushed version of a PR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    pub id: RevisionId,
    pub pr_id: PrId,
    /// Sequence number. The first revision is 1.
    pub seq: u32,
    pub head_sha: ObjectId,
    pub base_sha: ObjectId,
    pub pushed_at: UnixMillis,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forge_serde_names() {
        assert_eq!(serde_json::to_string(&Forge::GitHub).unwrap(), "\"github\"");
        assert_eq!(serde_json::from_str::<Forge>("\"forgejo\"").unwrap(), Forge::Forgejo);
    }

    #[test]
    fn terminal_states() {
        assert!(PrState::Merged.is_terminal());
        assert!(PrState::Closed.is_terminal());
        assert!(!PrState::Open.is_terminal());
        assert!(!PrState::Draft.is_terminal());
    }

    #[test]
    fn pr_number_display() {
        assert_eq!(PrNumber(42).to_string(), "#42");
    }
}
