//! Grafene domain types: stacks, branches, PRs, revisions, and findings.
//!
//! This crate does no I/O and has no dependencies on other Grafene crates
//! (design section 6.4). All values that come from outside are validated
//! when they are parsed, so other crates can trust them.

mod error;
mod finding;
mod git;
mod ids;
mod pr;
mod stack;

pub use error::ParseError;
pub use finding::{Confidence, Finding, FindingState, HunkHash, HunkKind, LineRange, RuleId, Severity};
pub use git::{BranchName, ObjectId};
pub use ids::{FindingId, PrId, RepoId, RevisionId, StackId, UserId};
pub use pr::{Forge, Pr, PrNumber, PrState, Revision, UnixMillis};
pub use stack::{Branch, Stack, StackError};
