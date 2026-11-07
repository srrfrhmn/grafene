//! Typed, prefixed ULID identifiers.
//!
//! Each ID shows as `<prefix>_<ulid>`, for example `f_01JC8X0Y5R7Q3M6T9V2W4Z8B1D`.
//! The prefix makes logs easier to read and stops one kind of ID from being
//! used where a different kind is necessary.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ulid::Ulid;

use crate::ParseError;

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal, $kind:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Ulid);

        impl $name {
            /// The text before the `_` separator.
            pub const PREFIX: &'static str = $prefix;

            /// Make a new ID from the current time and random bits.
            #[must_use]
            #[allow(clippy::new_without_default)]
            pub fn new() -> Self {
                Self(Ulid::generate())
            }

            /// Wrap an existing ULID.
            #[must_use]
            pub const fn from_ulid(ulid: Ulid) -> Self {
                Self(ulid)
            }

            /// The ULID without the prefix.
            #[must_use]
            pub const fn ulid(self) -> Ulid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}_{}", $prefix, self.0)
            }
        }

        impl FromStr for $name {
            type Err = ParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let err = || ParseError::Id { kind: $kind, value: s.to_owned() };
                let rest = s
                    .strip_prefix($prefix)
                    .and_then(|r| r.strip_prefix('_'))
                    .ok_or_else(err)?;
                Ulid::from_string(rest).map(Self).map_err(|_| err())
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = <std::borrow::Cow<'de, str>>::deserialize(d)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

define_id!(
    /// ID of a repository.
    RepoId, "repo", "repo"
);
define_id!(
    /// ID of a user.
    UserId, "user", "user"
);
define_id!(
    /// ID of a stack.
    StackId, "stk", "stack"
);
define_id!(
    /// ID of a pull request.
    PrId, "pr", "pr"
);
define_id!(
    /// ID of one revision of a pull request.
    RevisionId, "rev", "revision"
);
define_id!(
    /// ID of a finding.
    FindingId, "f", "finding"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_and_parse_round_trip() {
        let id = FindingId::new();
        let text = id.to_string();
        assert!(text.starts_with("f_"));
        assert_eq!(text.parse::<FindingId>().unwrap(), id);
    }

    #[test]
    fn parse_rejects_wrong_prefix() {
        let pr = PrId::new().to_string();
        assert!(pr.parse::<FindingId>().is_err());
        assert!(pr.parse::<PrId>().is_ok());
    }

    #[test]
    fn parse_rejects_bad_ulid() {
        assert!("f_".parse::<FindingId>().is_err());
        assert!("f_not-a-ulid".parse::<FindingId>().is_err());
        assert!("f01JC8X0Y5R7Q3M6T9V2W4Z8B1D".parse::<FindingId>().is_err());
    }

    #[test]
    fn serde_uses_prefixed_string() {
        let id = RevisionId::new();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<RevisionId>(&json).unwrap(), id);
    }
}
