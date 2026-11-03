use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A resolved icon served from the disk cache, base64-encoded so the UI can
/// use it directly as an `<img src>` without filesystem access.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedIcon {
    pub key: String,
    pub data_url: String,
    pub content_type: String,
    /// When the cached bytes were fetched (epoch ms).
    pub fetched_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetaEntry {
    pub(crate) url: String,
    pub(crate) content_type: String,
    pub(crate) fetched_at_ms: u64,
}

impl MetaEntry {
    pub(crate) fn is_stale(&self, ttl: Duration, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.fetched_at_ms) > ttl.as_millis() as u64
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Best-effort mtime bump of every file in an entry dir, marking it used.
pub(crate) fn touch_all(dir: &std::path::Path) {
    let now = SystemTime::now();
    let Ok(files) = std::fs::read_dir(dir) else {
        return;
    };
    for file in files.flatten() {
        if let Ok(handle) = std::fs::File::options().write(true).open(file.path()) {
            let _ = handle.set_modified(now);
        }
    }
}

/// True when the entry was used recently enough to keep. Prefers the newest
/// file mtime (last serve or fetch); falls back to the metadata fetch time,
/// and keeps entries we cannot judge rather than deleting blindly.
pub(crate) fn entry_is_recent_enough(
    owner_dir: std::path::PathBuf,
    meta: Option<&MetaEntry>,
    now: SystemTime,
    max_age: Duration,
) -> bool {
    if let Some(last_use) = newest_mtime(&owner_dir) {
        return now.duration_since(last_use).unwrap_or(Duration::ZERO) <= max_age;
    }
    match meta {
        Some(meta) => now_ms().saturating_sub(meta.fetched_at_ms) <= max_age.as_millis() as u64,
        None => true,
    }
}

fn newest_mtime(dir: &std::path::Path) -> Option<SystemTime> {
    let files = std::fs::read_dir(dir).ok()?;
    let mut newest: Option<SystemTime> = None;
    for file in files.flatten() {
        if let Ok(mtime) = file.metadata().and_then(|m| m.modified()) {
            newest = Some(newest.map_or(mtime, |best| best.max(mtime)));
        }
    }
    newest
}

pub(crate) fn try_remove_empty_dir(dir: &std::path::Path) {
    if std::fs::read_dir(dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_dir(dir);
    }
}
