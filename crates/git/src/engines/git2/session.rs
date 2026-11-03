use std::path::{Path, PathBuf};

use crate::error::{GitError, Result};

/// Handle for mutation-oriented operations backed by libgit2.
#[derive(Debug, Clone)]
pub struct Git2Session {
    root: PathBuf,
}

impl Git2Session {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Opens a fresh repository handle. libgit2 handles are cheap to open and
    /// must not be shared across threads, so each blocking call opens its own.
    pub fn repository(&self) -> Result<git2::Repository> {
        self.repository_at(&self.root)
    }

    /// Runs `f` with the thread's cached open handle for this root, keeping
    /// libgit2 object/config caches warm across calls.
    pub fn with_repository<T>(&self, f: impl FnOnce(&git2::Repository) -> Result<T>) -> Result<T> {
        super::local::with_repository(&self.root, f)
    }

    fn repository_at(&self, path: &Path) -> Result<git2::Repository> {
        match git2::Repository::discover(path) {
            Ok(repo) => Ok(repo),
            Err(e) => Err(crate::streaming::pipeline::map_open_error(path, e)),
        }
    }
}

pub fn resolve_signature(repo: &git2::Repository) -> Result<git2::Signature<'static>> {
    let config = repo.config()?;
    let name = config.get_string("user.name").ok();
    let email = config.get_string("user.email").ok();
    match (name, email) {
        (Some(name), Some(email)) if !name.trim().is_empty() && !email.trim().is_empty() => {
            Ok(git2::Signature::now(&name, &email)?)
        }
        _ => Err(GitError::invalid_input(
            "user.name / user.email are not configured",
        )),
    }
}
