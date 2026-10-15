use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::error::Result;

type RepositoryHolder = Arc<Mutex<Option<git2::Repository>>>;

thread_local! {
    static HANDLES: RefCell<HashMap<PathBuf, RepositoryHolder>> = RefCell::new(HashMap::new());
}

static REPOSITORY_HANDLES: std::sync::OnceLock<Mutex<HashMap<PathBuf, Vec<RepositoryHolder>>>> =
    std::sync::OnceLock::new();

/// Runs `f` with this thread's open handle for the repository at `root`,
/// discovering and caching one on first use.
///
/// The handle is taken out of its holder while `f` runs. A nested call on the
/// same thread (rayon runs stolen tasks on the calling thread) therefore cannot
/// borrow it twice: it opens its own handle and leaves it in the holder for the
/// next call, and the outer frame drops its now-redundant handle on return.
pub(crate) fn with_repository<T>(
    root: &Path,
    f: impl FnOnce(&git2::Repository) -> Result<T>,
) -> Result<T> {
    let holder = HANDLES.with(|handles| {
        let mut handles = handles.borrow_mut();
        match handles.get(root) {
            Some(holder) => holder.clone(),
            None => {
                let holder: RepositoryHolder = Arc::new(Mutex::new(None));
                handles.insert(root.to_path_buf(), holder.clone());
                holder
            }
        }
    });
    {
        let mut registry = REPOSITORY_HANDLES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        let holders = registry.entry(root.to_path_buf()).or_default();
        if !holders.iter().any(|item| Arc::ptr_eq(item, &holder)) {
            holders.push(holder.clone());
        }
    }

    let repo = match holder.lock().unwrap().take() {
        Some(repo) => repo,
        None => git2::Repository::discover(root)
            .map_err(|e| crate::streaming::pipeline::map_open_error(root, e))?,
    };
    let result = f(&repo);
    let mut slot = holder.lock().unwrap();
    if slot.is_none() {
        *slot = Some(repo);
    }
    result
}

/// Drops every handle cached on the current thread, deregistering it so the
/// repository is closed instead of kept alive by the registry.
pub(crate) fn clear_all_cached_handles() {
    let drained: Vec<(PathBuf, RepositoryHolder)> =
        HANDLES.with(|handles| handles.borrow_mut().drain().collect());
    if let Some(all) = REPOSITORY_HANDLES.get() {
        let mut registry = all.lock().unwrap();
        for (root, holder) in &drained {
            if let Some(holders) = registry.get_mut(root) {
                holders.retain(|item| !Arc::ptr_eq(item, holder));
            }
        }
    }
}

/// Closes every open handle for `root`, on this and other threads.
pub(crate) fn clear_all_repository_handles(root: &Path) {
    let local = HANDLES.with(|handles| handles.borrow_mut().remove(root));
    if let Some(all) = REPOSITORY_HANDLES.get() {
        let holders = all.lock().unwrap().remove(root).unwrap_or_default();
        for holder in holders {
            holder.lock().unwrap().take();
        }
    }
    drop(local);
}
