//! Validated Git names: branch names and object IDs.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ParseError;

/// A short branch name, for example `feature/login`. It has no `refs/heads/` prefix.
///
/// The rules are a subset of `git check-ref-format --branch`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct BranchName(String);

impl BranchName {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn check(s: &str) -> Result<(), &'static str> {
        if s.is_empty() {
            return Err("name is empty");
        }
        if s == "@" {
            return Err("name is \"@\"");
        }
        if s.starts_with('-') {
            return Err("name starts with '-'");
        }
        if s.starts_with('/') || s.ends_with('/') {
            return Err("name starts or ends with '/'");
        }
        if s.ends_with('.') || s.ends_with(".lock") {
            return Err("name ends with '.' or \".lock\"");
        }
        if s.contains("..") || s.contains("//") || s.contains("@{") {
            return Err("name contains \"..\", \"//\", or \"@{\"");
        }
        if s.split('/').any(|part| part.starts_with('.') || part.ends_with(".lock")) {
            return Err("a path part starts with '.' or ends with \".lock\"");
        }
        if s.chars().any(|c| c.is_ascii_control() || matches!(c, ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\')) {
            return Err("name contains a space, a control character, or one of ~^:?*[\\");
        }
        Ok(())
    }
}

impl FromStr for BranchName {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::check(s)
            .map(|()| Self(s.to_owned()))
            .map_err(|reason| ParseError::BranchName { value: s.to_owned(), reason })
    }
}

impl TryFrom<String> for BranchName {
    type Error = ParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match Self::check(&s) {
            Ok(()) => Ok(Self(s)),
            Err(reason) => Err(ParseError::BranchName { value: s, reason }),
        }
    }
}

impl From<BranchName> for String {
    fn from(b: BranchName) -> Self {
        b.0
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A Git object ID: 40 hex characters (SHA-1) or 64 hex characters (SHA-256).
///
/// The value is kept in lowercase.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ObjectId(String);

impl ObjectId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for ObjectId {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ok = matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_hexdigit());
        if ok { Ok(Self(s.to_ascii_lowercase())) } else { Err(ParseError::ObjectId(s.to_owned())) }
    }
}

impl TryFrom<String> for ObjectId {
    type Error = ParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<ObjectId> for String {
    fn from(o: ObjectId) -> Self {
        o.0
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_branch_names() {
        for name in ["main", "feature/login", "fix-123", "user/a.b", "v1.2.3"] {
            assert!(name.parse::<BranchName>().is_ok(), "{name}");
        }
    }

    #[test]
    fn rejects_bad_branch_names() {
        let bad = [
            "", "@", "-x", "/x", "x/", "x.", "x.lock", "a..b", "a//b", "a@{b", "a/.b", "a.lock/b", "a b", "a~b", "a^b",
            "a:b", "a?b", "a*b", "a[b", "a\\b", "a\tb",
        ];
        for name in bad {
            assert!(name.parse::<BranchName>().is_err(), "{name:?}");
        }
    }

    #[test]
    fn branch_name_serde_validates() {
        assert!(serde_json::from_str::<BranchName>("\"ok/name\"").is_ok());
        assert!(serde_json::from_str::<BranchName>("\"bad name\"").is_err());
    }

    #[test]
    fn object_id_lengths_and_case() {
        let sha1 = "A".repeat(40);
        let id: ObjectId = sha1.parse().unwrap();
        assert_eq!(id.as_str(), "a".repeat(40));
        assert!("0".repeat(64).parse::<ObjectId>().is_ok());
        assert!("0".repeat(39).parse::<ObjectId>().is_err());
        assert!("g".repeat(40).parse::<ObjectId>().is_err());
    }
}
