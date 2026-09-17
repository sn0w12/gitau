use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::ids::ObjectId;
use crate::error::{GitError, Result};

/// A validated revision expression. Engines resolve it against a repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RevisionSpec(String);

impl RevisionSpec {
    pub fn parse(input: &str) -> Result<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(GitError::invalid_input("revision is empty"));
        }
        if trimmed.len() > 512 {
            return Err(GitError::invalid_input("revision exceeds 512 characters"));
        }
        if trimmed
            .bytes()
            .any(|b| b == 0 || b == b'\n' || b == b'\r' || b < 0x20)
        {
            return Err(GitError::invalid_input(
                "revision contains control characters",
            ));
        }
        if let Some(first) = trimmed.chars().next() {
            if first == '-' {
                return Err(GitError::invalid_input("revision must not start with `-`"));
            }
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn head() -> Self {
        Self("HEAD".to_owned())
    }

    pub fn from_oid(oid: ObjectId) -> Self {
        Self(oid.hex())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevisionSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Default for RevisionSpec {
    fn default() -> Self {
        Self("HEAD".to_owned())
    }
}

impl Serialize for RevisionSpec {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RevisionSpec {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// Validated git reference-ish names (branch, tag, remote).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RefName(String);

impl RefName {
    pub const MAX_LEN: usize = 512;

    pub fn parse(input: &str, kind: &str) -> Result<Self> {
        if input.is_empty() {
            return Err(GitError::invalid_input(format!("{kind} name is empty")));
        }
        if input.len() > Self::MAX_LEN {
            return Err(GitError::invalid_input(format!(
                "{kind} name exceeds {} characters",
                Self::MAX_LEN
            )));
        }
        if input.starts_with('-') {
            return Err(GitError::invalid_input(format!(
                "{kind} name must not start with `-`"
            )));
        }
        if input.starts_with('/') || input.ends_with('/') || input.contains("//") {
            return Err(GitError::invalid_input(format!(
                "{kind} name has invalid slashes"
            )));
        }
        if input.contains("..") || input.ends_with('.') {
            return Err(GitError::invalid_input(format!(
                "{kind} name has invalid dots"
            )));
        }
        for component in input.split('/') {
            match component {
                "" | "." | ".." => {
                    return Err(GitError::invalid_input(format!(
                        "{kind} name has invalid component `{component}`"
                    )));
                }
                c if c.ends_with(".lock") => {
                    return Err(GitError::invalid_input(format!(
                        "{kind} name must not end with `.lock`"
                    )));
                }
                _ => {}
            }
        }
        if input.bytes().any(|b| {
            b < 0x20
                || b == 0x7f
                || matches!(b, b' ' | b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\')
        }) {
            return Err(GitError::invalid_input(format!(
                "{kind} name contains forbidden characters"
            )));
        }
        Ok(Self(input.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn full_branch_ref(&self) -> String {
        format!("refs/heads/{}", self.0)
    }

    pub fn full_tag_ref(&self) -> String {
        format!("refs/tags/{}", self.0)
    }

    pub fn short_from_full(full: &str) -> Option<&str> {
        full.strip_prefix("refs/heads/")
            .or_else(|| full.strip_prefix("refs/tags/"))
            .or_else(|| full.strip_prefix("refs/remotes/"))
    }
}

impl fmt::Display for RefName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub type BranchName = RefName;
pub type TagName = RefName;
pub type RemoteName = RefName;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_common_revision_syntax() {
        assert!(RevisionSpec::parse("HEAD").is_ok());
        assert!(RevisionSpec::parse("HEAD~3").is_ok());
        assert!(RevisionSpec::parse("origin/main@{upstream}").is_ok());
        assert!(RevisionSpec::parse("0123456789012345678901234567890123456789").is_ok());
        assert!(RevisionSpec::parse("feature/x:y^2").is_ok());
    }

    #[test]
    fn rejects_hostile_revisions() {
        assert!(RevisionSpec::parse("").is_err());
        assert!(RevisionSpec::parse("-oProxyCommand=evil").is_err());
        assert!(RevisionSpec::parse("a\nb").is_err());
    }

    #[test]
    fn branch_name_validation_matches_git_rules() {
        assert!(RefName::parse("feature/login", "branch").is_ok());
        assert!(RefName::parse("release-1.2.3", "branch").is_ok());
        assert!(RefName::parse("", "branch").is_err());
        assert!(RefName::parse("a//b", "branch").is_err());
        assert!(RefName::parse("a..b", "branch").is_err());
        assert!(RefName::parse("has space", "branch").is_err());
        assert!(RefName::parse("ends.lock", "branch").is_err());
        assert!(RefName::parse("a:b", "branch").is_err());
        assert!(RefName::parse("back\\slash", "branch").is_err());
    }

    #[test]
    fn full_refs_are_derived() {
        let b = RefName::parse("main", "branch").unwrap();
        assert_eq!(b.full_branch_ref(), "refs/heads/main");
        assert_eq!(RefName::short_from_full("refs/heads/main"), Some("main"));
        assert_eq!(RefName::short_from_full("refs/tags/v1"), Some("v1"));
        assert_eq!(RefName::short_from_full("refs/remotes/o/b"), Some("o/b"));
    }
}
