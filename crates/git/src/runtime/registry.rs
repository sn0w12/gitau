use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use serde::Serialize;

use crate::domain::history::{CommitWithDetail, HistoryChart};
use crate::domain::{Generation, HistoryPage, RepoId, SnapshotId, StatusReport};
use crate::engines::gix::GixSession;
use crate::error::{GitError, Result};
use crate::runtime::cache::{Lru, params_hash};
use crate::runtime::invalidation::InvalidationHub;
use crate::runtime::watcher::WatcherHandle;

/// Ref tips plus the files git rewrites alongside them. Everything else
/// under `.git` (objects, logs, lock files) stays invisible to the watcher.
fn is_ref_tip(path: &Path, git_dir: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(git_dir) else {
        return false;
    };
    if relative == Path::new("HEAD") || relative == Path::new("packed-refs") {
        return true;
    }
    if path
        .extension()
        .is_some_and(|ext| ext == std::ffi::OsStr::new("lock"))
    {
        return false;
    }
    relative.starts_with("refs")
}

#[derive(Debug, Clone)]
pub enum CachedValue {
    Status(Arc<StatusReport>),
    History(Arc<HistoryPage>),
    CommitDetail(Arc<CommitWithDetail>),
    HistoryChart(Arc<HistoryChart>),
}

const CACHE_CAPACITY: usize = 64;

pub struct RepoEntry {
    pub id: RepoId,
    pub canonical_path: PathBuf,
    generation: AtomicU64,
    snapshot_seq: AtomicU64,
    pub gix: GixSession,
    cache: Lru<u64, CachedValue>,
    /// Commit OIDs are immutable; survives generation bumps.
    history_cache: Arc<std::sync::Mutex<crate::runtime::history_cache::HistoryCache>>,
    /// Serializes history page computations so concurrent callers share the
    /// walk/summary caches instead of redoing per-commit diff stats.
    history_lock: std::sync::Mutex<()>,
    /// Serializes mutations per repository.
    pub write_lock: tokio::sync::Mutex<()>,
}

impl RepoEntry {
    fn new(id: RepoId, canonical_path: PathBuf, gix: GixSession) -> Self {
        Self {
            id,
            canonical_path,
            generation: AtomicU64::new(1),
            snapshot_seq: AtomicU64::new(0),
            gix,
            cache: Lru::new(CACHE_CAPACITY),
            history_cache: Arc::new(std::sync::Mutex::new(
                crate::runtime::history_cache::HistoryCache::default(),
            )),
            history_lock: std::sync::Mutex::new(()),
            write_lock: tokio::sync::Mutex::new(()),
        }
    }

    pub fn generation(&self) -> Generation {
        Generation(self.generation.load(Ordering::Acquire))
    }

    pub fn bump_generation(&self) -> Generation {
        let next = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.clear_cache();
        Generation(next)
    }

    pub fn next_snapshot_id(&self) -> SnapshotId {
        SnapshotId(self.snapshot_seq.fetch_add(1, Ordering::AcqRel) + 1)
    }

    pub fn history_cache(
        &self,
    ) -> Arc<std::sync::Mutex<crate::runtime::history_cache::HistoryCache>> {
        self.history_cache.clone()
    }

    pub fn history_lock(&self) -> &std::sync::Mutex<()> {
        &self.history_lock
    }
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    pub fn cached(&self, kind: &'static str, params: &impl Serialize) -> Option<CachedValue>
where {
        self.cache.get(&params_hash(kind, params))
    }

    pub fn store_cached(&self, kind: &'static str, params: &impl Serialize, value: CachedValue) {
        self.cache.insert(params_hash(kind, params), value);
    }
}

struct RepoHandle {
    entry: Arc<RepoEntry>,
    _watcher: Option<WatcherHandle>,
}

pub struct Registry {
    entries: RwLock<HashMap<RepoId, RepoHandle>>,
    by_path: RwLock<HashMap<PathBuf, RepoId>>,
    next_repo_id: AtomicU64,
    hub: Arc<InvalidationHub>,
}

impl Registry {
    pub fn new(hub: Arc<InvalidationHub>) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            by_path: RwLock::new(HashMap::new()),
            next_repo_id: AtomicU64::new(1),
            hub,
        }
    }

    pub fn hub(&self) -> Arc<InvalidationHub> {
        self.hub.clone()
    }

    /// Registers a repository, deduplicating on its canonical repository root
    /// (the worktree directory, or git dir for bare repositories).
    pub fn register(
        &self,
        discovered_path: &Path,
        watch_paths: Vec<PathBuf>,
    ) -> Result<(RepoId, Arc<RepoEntry>, bool)> {
        let input = std::fs::canonicalize(discovered_path)
            .unwrap_or_else(|_| discovered_path.to_path_buf());

        if let Some(existing) = self.by_path.read().unwrap().get(&input).copied() {
            if let Some(handle) = self.entries.read().unwrap().get(&existing) {
                return Ok((existing, handle.entry.clone(), false));
            }
        }

        let session = GixSession::discover(&input)?;
        let repo_root = session
            .workdir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| session.git_dir().to_path_buf());
        let repo_root = std::fs::canonicalize(&repo_root).unwrap_or(repo_root);

        if let Some(existing) = self.by_path.read().unwrap().get(&repo_root).copied() {
            if let Some(handle) = self.entries.read().unwrap().get(&existing) {
                return Ok((existing, handle.entry.clone(), false));
            }
        }

        let id = RepoId(self.next_repo_id.fetch_add(1, Ordering::AcqRel));
        let entry = Arc::new(RepoEntry::new(id, repo_root.clone(), session.clone()));

        let weak = Arc::downgrade(&entry);
        let hub = self.hub.clone();
        let watcher = if watch_paths.is_empty() {
            None
        } else {
            // The app's own writes land in `.git` and in gitignored paths
            // (`target/`, `node_modules/`); reacting to those would bump the
            // generation again and race the frontend's snapshot. Worktree
            // changes that git ignores carry the same risk when a build
            // tool or IDE is running, so drop every event the repo itself
            // would ignore. Ref tips are the exception: external commits,
            // tags, checkouts, and fetches only touch `.git`, so ignoring
            // all of it would leave branches and tags stale until reload.
            // Exclusion runs through the same gix stack as status (including
            // the global excludes override), so the two cannot disagree
            // about what counts as ignored.
            let git_dir = session.git_dir().to_path_buf();
            let git_dir_for_tips = git_dir.clone();
            let workdir = session.workdir().map(Path::to_path_buf);
            let check = session.clone();
            let is_ignored = move |path: &PathBuf| {
                let Some(workdir) = workdir.as_deref() else {
                    // Bare repositories watch the git dir itself; those
                    // events are the whole point.
                    return false;
                };
                if path.starts_with(&git_dir) {
                    return !is_ref_tip(path, &git_dir);
                }
                let Ok(relative) = path.strip_prefix(workdir) else {
                    return false;
                };
                check.is_excluded(relative, path.is_dir())
            };
            let is_tip = move |path: &PathBuf| is_ref_tip(path, &git_dir_for_tips);
            let watcher = crate::runtime::watcher::spawn_watch(
                &watch_paths,
                std::time::Duration::from_millis(150),
                is_ignored,
                is_tip,
                move |refs_changed| {
                    if let Some(entry) = weak.upgrade() {
                        let generation = entry.bump_generation();
                        if refs_changed {
                            entry.history_cache().lock().unwrap().clear_ref_dependent();
                        }
                        hub.publish(id.0, generation.0, "watcher");
                    }
                },
            );
            watcher.ok()
        };

        self.entries.write().unwrap().insert(
            id,
            RepoHandle {
                entry: entry.clone(),
                _watcher: watcher,
            },
        );
        self.by_path.write().unwrap().insert(repo_root, id);

        Ok((id, entry, true))
    }

    pub fn get(&self, id: RepoId) -> Result<Arc<RepoEntry>> {
        self.entries
            .read()
            .unwrap()
            .get(&id)
            .map(|handle| handle.entry.clone())
            .ok_or(GitError::RepositoryNotFound {
                path: PathBuf::from(format!("repo id {id}")),
            })
    }

    /// Bumps every open repository and publishes an invalidation per repo,
    /// so process-wide changes (like the global excludes file) take effect
    /// immediately without reopening anything.
    pub fn bump_all_generations(&self, reason: &str) {
        let entries: Vec<Arc<RepoEntry>> = self
            .entries
            .read()
            .unwrap()
            .values()
            .map(|handle| handle.entry.clone())
            .collect();
        for entry in entries {
            let generation = entry.bump_generation();
            self.hub.publish(entry.id.0, generation.0, reason);
        }
    }

    pub fn remove(&self, id: RepoId) -> bool {
        let removed = self
            .entries
            .write()
            .unwrap()
            .remove(&id)
            .map(|_handle| true)
            .unwrap_or(false);
        if removed {
            self.by_path.write().unwrap().retain(|_, v| *v != id);
        }
        removed
    }

    /// Closes the session registered for `path` (the canonical repo root),
    /// stopping its watcher. Returns the id when a session was open.
    pub fn remove_by_path(&self, path: &Path) -> Option<RepoId> {
        let id = {
            let by_path = self.by_path.read().unwrap();
            by_path.get(path).copied()
        }?;
        self.remove(id).then_some(id)
    }

    /// Returns the live session for `path` (the canonical repo root), if any.
    pub fn get_by_path(&self, path: &Path) -> Option<Arc<RepoEntry>> {
        let id = self.by_path.read().unwrap().get(path).copied()?;
        self.entries
            .read()
            .unwrap()
            .get(&id)
            .map(|handle| handle.entry.clone())
    }

    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_repo(dir: &Path) -> PathBuf {
        git2::Repository::init(dir).unwrap();
        dir.to_path_buf()
    }

    #[test]
    fn registers_and_deduplicates_by_canonical_path() {
        let temp = tempfile::tempdir().unwrap();
        let root = init_repo(temp.path());
        let registry = Registry::new(Arc::new(InvalidationHub::new()));

        let (id_a, entry_a, created) = registry.register(&root, vec![]).unwrap();
        assert!(created);
        assert_eq!(entry_a.generation(), Generation(1));

        let (id_b, _, created_b) = registry.register(&root, vec![]).unwrap();
        assert!(!created_b);
        assert_eq!(id_a, id_b);

        let nested = root.join("src");
        std::fs::create_dir_all(&nested).unwrap();
        let (id_c, entry_c, _) = registry.register(&nested, vec![]).unwrap();
        assert_eq!(id_c, id_a);
        assert_eq!(entry_c.canonical_path, entry_a.canonical_path);
    }

    #[test]
    fn rejects_non_repositories() {
        let temp = tempfile::tempdir().unwrap();
        let registry = Registry::new(Arc::new(InvalidationHub::new()));
        assert!(registry.register(temp.path(), vec![]).is_err());
    }

    #[test]
    fn remove_drops_handle_and_path_index() {
        let temp = tempfile::tempdir().unwrap();
        let root = init_repo(temp.path());
        let registry = Registry::new(Arc::new(InvalidationHub::new()));
        let (id, entry, _) = registry.register(&root, vec![]).unwrap();
        assert!(registry.remove(id));
        assert!(registry.get(id).is_err());
        drop(entry);

        let (id_again, _, created) = registry.register(&root, vec![]).unwrap();
        assert!(created);
        assert_ne!(id_again, id);
    }

    #[test]
    fn remove_by_path_closes_session_and_reports_id() {
        let temp = tempfile::tempdir().unwrap();
        let root = init_repo(temp.path());
        let registry = Registry::new(Arc::new(InvalidationHub::new()));
        let (id, entry, _) = registry.register(&root, vec![]).unwrap();
        let canonical = std::fs::canonicalize(&root).unwrap();

        assert_eq!(registry.remove_by_path(&canonical), Some(id));
        assert!(registry.get(id).is_err());
        drop(entry);
        assert_eq!(registry.remove_by_path(&canonical), None);
    }

    #[test]
    fn generation_invalidation_clears_cache() {
        let temp = tempfile::tempdir().unwrap();
        let root = init_repo(temp.path());
        let registry = Registry::new(Arc::new(InvalidationHub::new()));
        let (_id, entry, _) = registry.register(&root, vec![]).unwrap();

        entry.store_cached(
            "status",
            &serde_json::json!({}),
            CachedValue::Status(Arc::new(StatusReport {
                snapshot_id: SnapshotId(0),
                generation: entry.generation(),
                entries: vec![],
                conflicts: vec![],
            })),
        );
        assert!(entry.cached("status", &serde_json::json!({})).is_some());

        let bumped = entry.bump_generation();
        assert!(bumped.0 >= 2);
        assert!(entry.cached("status", &serde_json::json!({})).is_none());
    }

    #[test]
    fn ref_tips_cover_heads_tags_head_and_packed_refs() {
        let git_dir = Path::new("/repo/.git");
        for tip in [
            "refs/heads/master",
            "refs/tags/v0.1.3",
            "refs/remotes/origin/master",
            "refs/stash",
            "HEAD",
            "packed-refs",
        ] {
            assert!(
                is_ref_tip(&git_dir.join(tip), git_dir),
                "{tip} must count as a ref tip"
            );
        }
    }

    #[test]
    fn ref_tips_exclude_objects_logs_locks_and_worktree() {
        let git_dir = Path::new("/repo/.git");
        for other in [
            "/repo/.git/objects/ab/cdef",
            "/repo/.git/logs/HEAD",
            "/repo/.git/logs/refs/heads/master",
            "/repo/.git/index",
            "/repo/.git/index.lock",
            "/repo/package.json",
            "/other/.git/refs/tags/v0.1.3",
        ] {
            assert!(
                !is_ref_tip(Path::new(other), git_dir),
                "{other} must not count as a ref tip"
            );
        }
    }

    #[test]
    fn ref_tip_lock_files_stay_ignored() {
        // Lock files appear and vanish around every ref write; the debounce
        // window already coalesces them, but they must never mark a burst
        // as a ref change on their own.
        let git_dir = Path::new("/repo/.git");
        assert!(!is_ref_tip(
            &git_dir.join("refs/heads/master.lock"),
            git_dir
        ));
    }
}
