use std::path::Path;

use gix::ThreadSafeRepository;

use crate::domain::{HeadState, ObjectId, ShaKind};
use crate::error::{GitError, Result};

/// Read-optimized session on top of gitoxide.
#[derive(Clone)]
pub struct GixSession {
    tsr: ThreadSafeRepository,
    workdir: Option<std::path::PathBuf>,
}

/// The on-disk index, re-read on every call. gix's shared index snapshot is
/// keyed on the file mtime; the app's own git2 writes can land on the same
/// tick and leave that cache serving the pre-write index, so status and diffs
/// must always read the file as it is now. Unborn repositories have no index
/// file yet; those get an empty index so untracked files still surface.
pub fn open_index(repo: &gix::Repository) -> Result<gix::index::File> {
    match repo.open_index() {
        Ok(index) => Ok(index),
        Err(err) if index_file_missing(&err) => Ok(gix::index::File::from_state(
            gix::index::State::new(repo.object_hash()),
            repo.index_path(),
        )),
        Err(err) => Err(GitError::Internal {
            message: err.to_string(),
        }),
    }
}

/// A pathspec that matches every path, independent of the process CWD.
/// gix's internal `tree_index_status` pathspec is built with
/// `empty_patterns_match_prefix=true`, so it only matches paths under
/// `repo.prefix()` (the process CWD relative to the workdir). The app's CWD
/// sits inside the worktree, which would silently drop every staged entry
/// outside that prefix while the worktree side keeps them; the engine must
/// compute the same full-tree status either way.
pub fn unrestricted_pathspec(repo: &gix::Repository) -> Result<gix::Pathspec<'_>> {
    let empty_index = gix::index::State::new(repo.object_hash());
    repo.pathspec(
        false,
        None::<&str>,
        false,
        &empty_index,
        gix::worktree::stack::state::attributes::Source::IdMapping,
    )
    .map_err(|e| GitError::Internal {
        message: e.to_string(),
    })
}

fn index_file_missing(err: &gix::worktree::open_index::Error) -> bool {
    matches!(
        err,
        gix::worktree::open_index::Error::IndexFile(gix::index::file::init::Error::Io(
            err
        )) if err.kind() == std::io::ErrorKind::NotFound
    )
}

impl GixSession {
    pub fn discover(input: &Path) -> Result<Self> {
        // core.fscache builds a per-thread directory-enumeration cache; with
        // the parallel status check each thread re-enumerates the same
        // directories, costing more than plain lstat calls (measured on
        // Windows), so it stays off. index.skipHash skips hashing the index
        // file on every open; git 2.30+ reads such indexes fine.
        let mut trust_map = <gix::sec::trust::Mapping<gix::open::Options>>::default();
        trust_map.full = trust_map
            .full
            .config_overrides(["core.fscache=false", "index.skipHash=true"]);
        trust_map.reduced = trust_map
            .reduced
            .config_overrides(["core.fscache=false", "index.skipHash=true"]);
        match gix::ThreadSafeRepository::discover_opts(input, Default::default(), trust_map) {
            Ok(tsr) => {
                let workdir = tsr.work_dir().map(std::path::Path::to_path_buf);
                Ok(Self { tsr, workdir })
            }
            Err(_) => Err(GitError::NotARepository {
                path: input.to_owned(),
            }),
        }
    }

    pub fn handle(&self) -> gix::Repository {
        let mut repo: gix::Repository = self.tsr.clone().into();
        if let Err(error) = crate::global_ignore::apply_to_repo(&mut repo) {
            crate::global_ignore::warn_once(format!("global gitignore not applied: {error}"));
        }
        repo
    }

    /// Thread-local handle for parallel status work. Carries the same
    /// global excludes override as [`Self::handle`].
    pub(crate) fn thread_local(&self) -> gix::Repository {
        let mut repo = self.tsr.to_thread_local();
        if let Err(error) = crate::global_ignore::apply_to_repo(&mut repo) {
            crate::global_ignore::warn_once(format!("global gitignore not applied: {error}"));
        }
        repo
    }

    pub fn workdir(&self) -> Option<&std::path::Path> {
        self.workdir.as_deref()
    }

    pub fn git_dir(&self) -> &std::path::Path {
        self.tsr.git_dir()
    }

    pub fn is_bare(&self) -> bool {
        self.workdir().is_none()
    }

    /// libgit2 `status_should_ignore` equivalent over the same per-handle
    /// config status and diff use, including the global excludes override.
    /// `relative` is workdir-relative; `is_dir` lets directory-only
    /// patterns (`target/`) match directories. Fail-open: any error means
    /// "not ignored" so the caller processes the path instead of dropping
    /// it.
    pub fn is_excluded(&self, relative: &Path, is_dir: bool) -> bool {
        let repo = self.handle();
        if repo.worktree().is_none() {
            return false;
        }
        // The tolerant open: unborn repositories have no index file yet,
        // but their worktree files are still subject to exclusion.
        let Ok(index) = open_index(&repo) else {
            return false;
        };
        let Ok(mut stack) = repo.excludes(
            &index,
            None,
            gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
        ) else {
            return false;
        };
        let mode = is_dir.then_some(gix::index::entry::Mode::DIR);
        stack
            .at_path(relative, mode)
            .map(|platform| platform.is_excluded())
            .unwrap_or(false)
    }

    pub fn sha_kind(&self) -> ShaKind {
        let _ = self;
        ShaKind::Sha1
    }

    pub fn head_state(&self) -> Result<HeadState> {
        let repo = self.handle();
        let head = repo.head().map_err(|e| GitError::Internal {
            message: e.to_string(),
        })?;
        if let Some(full_name) = head.referent_name() {
            let branch = full_name.shorten().to_string();
            let peeled = head
                .try_into_referent()
                .and_then(|mut r| r.peel_to_id().ok());
            return Ok(match peeled {
                Some(id) => HeadState::Attached {
                    branch,
                    target: ObjectId::from(id.detach()),
                },
                None => HeadState::Unborn { branch },
            });
        }
        if head.is_detached() {
            let id = repo.head_id().map_err(|e| GitError::Internal {
                message: e.to_string(),
            })?;
            return Ok(HeadState::Detached {
                target: ObjectId::from(id.detach()),
            });
        }
        Err(GitError::internal("unrecognized HEAD state"))
    }
}
