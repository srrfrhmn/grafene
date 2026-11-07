//! Findings from the review engine (design sections 5 and 8.5).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{FindingId, ParseError};

/// How bad the issue is, if it is real. Ordered from low to critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// How sure the engine is that the issue is real (D-04). Ordered from possible to proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Possible,
    Likely,
    /// A generated test failed in CI.
    Proven,
}

/// What a human did with a finding (D-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingState {
    Open,
    Accepted,
    Dismissed,
    Resolved,
}

/// The class of a diff hunk (design section 8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HunkKind {
    /// Only whitespace or layout changes.
    Format,
    /// The same subtree is in a different location.
    Move,
    /// Only an identifier changes, at all uses.
    Rename,
    /// All other changes.
    Logic,
}

/// Content hash of one hunk plus its context lines: `b3:` and 64 lowercase hex characters (BLAKE3).
///
/// Findings link to the hunk hash, not to a revision. This keeps findings stable across rebases (D-02).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HunkHash(String);

impl HunkHash {
    pub const PREFIX: &'static str = "b3:";

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for HunkHash {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ok = s
            .strip_prefix(Self::PREFIX)
            .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()));
        if ok { Ok(Self(s.to_ascii_lowercase())) } else { Err(ParseError::HunkHash(s.to_owned())) }
    }
}

impl TryFrom<String> for HunkHash {
    type Error = ParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<HunkHash> for String {
    fn from(h: HunkHash) -> Self {
        h.0
    }
}

impl fmt::Display for HunkHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// ID of a rule or AI check, for example `ai.null-deref`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RuleId(String);

impl RuleId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RuleId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ok = (1..=128).contains(&s.len())
            && s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'));
        if ok { Ok(Self(s.to_owned())) } else { Err(ParseError::RuleId(s.to_owned())) }
    }
}

impl TryFrom<String> for RuleId {
    type Error = ParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<RuleId> for String {
    fn from(r: RuleId) -> Self {
        r.0
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An inclusive range of 1-based line numbers. Serialized as `[start, end]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "(u32, u32)", into = "(u32, u32)")]
pub struct LineRange {
    start: u32,
    end: u32,
}

impl LineRange {
    /// # Errors
    ///
    /// Returns an error if `start` is 0 or `start` is after `end`.
    pub const fn new(start: u32, end: u32) -> Result<Self, ParseError> {
        if start == 0 || start > end { Err(ParseError::LineRange { start, end }) } else { Ok(Self { start, end }) }
    }

    #[must_use]
    pub const fn start(self) -> u32 {
        self.start
    }

    #[must_use]
    pub const fn end(self) -> u32 {
        self.end
    }

    /// Number of lines in the range. Always 1 or more.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end - self.start + 1
    }

    /// Always false. A range has at least one line.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        false
    }
}

impl TryFrom<(u32, u32)> for LineRange {
    type Error = ParseError;

    fn try_from((start, end): (u32, u32)) -> Result<Self, Self::Error> {
        Self::new(start, end)
    }
}

impl From<LineRange> for (u32, u32) {
    fn from(r: LineRange) -> Self {
        (r.start, r.end)
    }
}

/// One issue that the AI reviewer or a rule reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub id: FindingId,
    pub rule: RuleId,
    pub rule_version: u32,
    pub severity: Severity,
    pub confidence: Confidence,
    /// Path relative to the repository root, with `/` separators.
    pub file: String,
    pub lines: LineRange,
    pub title: String,
    /// Why the engine thinks the issue is real (D-04).
    pub evidence: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    pub hunk_hash: HunkHash,
    /// The model that made the finding. `None` for deterministic rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Cost in millionths of a US dollar. Integer, so that totals are exact.
    #[serde(default)]
    pub cost_usd_micros: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "b3:9f2c4e1a7b3d5f6081928374655647382910abcdef0123456789abcdef012345";

    #[test]
    fn hunk_hash_format() {
        assert!(HASH.parse::<HunkHash>().is_ok());
        assert!("b3:9f2c".parse::<HunkHash>().is_err());
        assert!(HASH.replace("b3:", "md5:").parse::<HunkHash>().is_err());
        assert_eq!(HASH.to_uppercase().replace("B3:", "b3:").parse::<HunkHash>().unwrap().as_str(), HASH);
    }

    #[test]
    fn rule_id_format() {
        assert!("ai.null-deref".parse::<RuleId>().is_ok());
        assert!("team_rules.no-unwrap2".parse::<RuleId>().is_ok());
        assert!("".parse::<RuleId>().is_err());
        assert!("Ai.Bad".parse::<RuleId>().is_err());
        assert!("a b".parse::<RuleId>().is_err());
        assert!("a".repeat(129).parse::<RuleId>().is_err());
    }

    #[test]
    fn line_range_rules() {
        assert_eq!(LineRange::new(118, 124).unwrap().len(), 7);
        assert_eq!(LineRange::new(5, 5).unwrap().len(), 1);
        assert!(LineRange::new(0, 3).is_err());
        assert!(LineRange::new(9, 3).is_err());
        assert!(serde_json::from_str::<LineRange>("[9, 3]").is_err());
    }

    #[test]
    fn ordering_supports_ranking() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::Medium > Severity::Low);
        assert!(Confidence::Proven > Confidence::Likely);
        assert!(Confidence::Likely > Confidence::Possible);
    }

    /// The example in design section 8.5 must parse.
    #[test]
    fn design_doc_example_round_trips() {
        let id = FindingId::new();
        let json = format!(
            r#"{{
                "id": "{id}",
                "rule": "ai.null-deref",
                "rule_version": 1,
                "severity": "high",
                "confidence": "likely",
                "file": "src/auth/session.rs",
                "lines": [118, 124],
                "title": "Session can be None after expiry check",
                "evidence": "Line 118 returns early only for Expired. Line 124 unwraps the Option.",
                "suggestion": "Replace unwrap() with a match on None.",
                "hunk_hash": "{HASH}",
                "model": "claude-sonnet-5-5",
                "cost_usd_micros": 4100
            }}"#
        );
        let f: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(f.id, id);
        assert_eq!(f.severity, Severity::High);
        assert_eq!(f.lines, LineRange::new(118, 124).unwrap());
        assert_eq!(f.cost_usd_micros, 4100);

        let again: Finding = serde_json::from_str(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(again, f);
    }

    #[test]
    fn optional_fields_default() {
        let json = format!(
            r#"{{"id":"{}","rule":"lint.x","rule_version":2,"severity":"low","confidence":"proven",
                "file":"a.rs","lines":[1,1],"title":"t","evidence":"e","hunk_hash":"{HASH}"}}"#,
            FindingId::new()
        );
        let f: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(f.model, None);
        assert_eq!(f.suggestion, None);
        assert_eq!(f.cost_usd_micros, 0);
        let out = serde_json::to_string(&f).unwrap();
        assert!(!out.contains("model"));
        assert!(!out.contains("suggestion"));
    }
}
