use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::error::Result;

thread_local! {
    static HANDLES: RefCell<HashMap<PathBuf, git2::Repository>> = RefCell::new(HashMap::new());
}

type RepositoryHolder = Arc<Mutex<Option<git2::Repository>>>;
type RepositoryHandles = Mutex<HashMap<PathBuf, Vec<RepositoryHolder>>>;

static REPOSITORY_HANDLES: std::sync::OnceLock<RepositoryHandles> = std::sync::OnceLock::new();

/// Runs `f` with the thread's cached handle for the repository containing
/// `root`.
///
/// The handle is removed from the cache while `f` runs so that nested calls
/// on the same thread (rayon executes stolen tasks on the calling thread)
/// cannot double-borrow the TLS map; the first handle to finish re-populates
/// the slot.
pub(crate) fn with_repository<T>(
    root: &Path,
    f: impl FnOnce(&git2::Repository) -> Result<T>,
) -> Result<T> {
    let cached = HANDLES.with_borrow_mut(|handles| handles.remove(root));
    let holder = Arc::new(Mutex::new(None));
    REPOSITORY_HANDLES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap()
        .entry(root.to_path_buf())
        .or_default()
        .push(holder.clone());
    let repo = match cached {
        Some(repo) => repo,
        None => git2::Repository::discover(root)
            .map_err(|e| crate::streaming::pipeline::map_open_error(root, e))?,
    };
    let result = f(&repo);
    *holder.lock().unwrap() = Some(repo);
    REPOSITORY_HANDLES
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .entry(root.to_path_buf())
        .or_default()
        .retain(|item| !Arc::ptr_eq(item, &holder));
    result
}

pub(crate) fn clear_all_cached_handles() {
    HANDLES.with_borrow_mut(|handles| handles.clear());
}

pub(crate) fn clear_all_repository_handles(root: &Path) {
    HANDLES.with_borrow_mut(|handles| {
        handles.remove(root);
    });
    if let Some(all) = REPOSITORY_HANDLES.get() {
        let holders = all.lock().unwrap().remove(root).unwrap_or_default();
        for holder in holders {
            holder.lock().unwrap().take();
        }
    }
}
