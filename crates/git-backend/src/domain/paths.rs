use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{GitError, Result};

/// A validated, repository-relative path using `/` separators.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RelativePath(String);

impl RelativePath {
    pub fn parse(input: &str) -> Result<Self> {
        let normalized = normalize(input)?;
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(self.0.replace('/', std::path::MAIN_SEPARATOR_STR))
    }

    pub fn join(&self, other: &RelativePath) -> Self {
        Self(format!("{}/{}", self.0, other.0))
    }

    pub fn parent(&self) -> Option<Self> {
        self.0
            .rsplit_once('/')
            .map(|(parent, _)| Self(parent.to_owned()))
    }

    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }
}

fn normalize(input: &str) -> Result<String> {
    if input.is_empty() {
        return Err(GitError::invalid_input("path is empty"));
    }
    if input.len() > 4096 {
        return Err(GitError::invalid_input("path exceeds 4096 characters"));
    }
    if input.bytes().any(|b| b == 0 || b < 0x20 && b != b'\t') {
        return Err(GitError::invalid_input("path contains control characters"));
    }
    let replaced = input.replace('\\', "/");
    if replaced.starts_with('/') {
        return Err(GitError::invalid_input("path is absolute"));
    }
    #[cfg(windows)]
    {
        let lower = replaced.to_ascii_lowercase();
        if lower.as_bytes().get(1) == Some(&b':') {
            return Err(GitError::invalid_input("path is absolute"));
        }
        if lower.starts_with("//") {
            return Err(GitError::invalid_input("unc paths are not valid here"));
        }
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in replaced.split('/') {
        match part {
            "" | "." => continue,
            ".." => return Err(GitError::invalid_input("path contains `..`")),
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        return Err(GitError::invalid_input("path resolves to nothing"));
    }
    Ok(parts.join("/"))
}

impl fmt::Display for RelativePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&str> for RelativePath {
    type Error = GitError;

    fn try_from(value: &str) -> Result<Self> {
        Self::parse(value)
    }
}

impl TryFrom<String> for RelativePath {
    type Error = GitError;

    fn try_from(value: String) -> Result<Self> {
        Self::parse(&value)
    }
}

/// Joins a repository root with a validated relative path.
pub fn join_workdir(workdir: &Path, rel: &RelativePath) -> PathBuf {
    workdir.join(rel.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_separators_and_dot_segments() {
        let p = RelativePath::parse("src/./lib\\\\main.rs").unwrap();
        assert_eq!(p.as_str(), "src/lib/main.rs");
    }

    #[test]
    fn rejects_traversal_and_absolute_paths() {
        assert!(RelativePath::parse("../secret").is_err());
        assert!(RelativePath::parse("/etc/passwd").is_err());
        assert!(RelativePath::parse("a/../b").is_err());
        assert!(RelativePath::parse("").is_err());
    }

    #[test]
    fn parent_and_file_name() {
        let p = RelativePath::parse("src/lib.rs").unwrap();
        assert_eq!(p.parent().unwrap().as_str(), "src");
        assert_eq!(p.file_name(), "lib.rs");
        assert!(RelativePath::parse("README.md").unwrap().parent().is_none());
    }

    #[test]
    fn roundtrips_through_serde() {
        let p = RelativePath::parse("src/main.rs").unwrap();
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json, serde_json::json!("src/main.rs"));
    }
}
