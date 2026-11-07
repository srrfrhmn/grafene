use thiserror::Error;

/// A value from outside the process has an incorrect format.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("invalid {kind} id {value:?}")]
    Id { kind: &'static str, value: String },

    #[error("invalid branch name {value:?}: {reason}")]
    BranchName { value: String, reason: &'static str },

    #[error("invalid object id {0:?}: expected 40 or 64 hex characters")]
    ObjectId(String),

    #[error("invalid hunk hash {0:?}: expected \"b3:\" and 64 hex characters")]
    HunkHash(String),

    #[error("invalid line range {start}..={end}: lines start at 1 and start must not be after end")]
    LineRange { start: u32, end: u32 },

    #[error("invalid rule id {0:?}: use 1 to 128 characters from a-z, 0-9, '.', '_', '-'")]
    RuleId(String),
}
