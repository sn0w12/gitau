use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use fs_util::write_atomic;

use crate::cache::{
    entry_is_recent_enough, now_ms, touch_all, try_remove_empty_dir, CachedIcon, MetaEntry,
};
use crate::config::IconConfig;
use crate::error::IconError;
use crate::fetcher::IconFetcher;
use crate::image::{encode_data_url, sniff_image};
use crate::plan::{
    entry_file_name, file_name_of_key, plan_from_key, plan_from_remote, split_key, IconPlan,
};
use crate::source::OwnerIconSource;

struct ServiceState {
    cache_dir: Option<std::path::PathBuf>,
    /// key -> metadata for every entry found on disk or fetched this run.
    entries: HashMap<String, MetaEntry>,
}

/// Resolves owner/org icons into a per-owner disk cache.
pub struct IconService {
    config: IconConfig,
    fetcher: Arc<dyn IconFetcher>,
    source: Arc<dyn OwnerIconSource>,
    state: Mutex<ServiceState>,
    /// key -> notify handle; deduplicates concurrent fetches per key.
    inflight: Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
}

impl IconService {
    pub fn new(
        config: IconConfig,
        fetcher: Arc<dyn IconFetcher>,
        source: Arc<dyn OwnerIconSource>,
    ) -> Self {
        Self {
            config,
            fetcher,
            source,
            state: Mutex::new(ServiceState {
                cache_dir: None,
                entries: HashMap::new(),
            }),
            inflight: Mutex::new(HashMap::new()),
        }
    }

    /// Points the service at its disk cache and loads any existing entries.
    /// Missing directories are created lazily on first write.
    pub fn initialize(&self, cache_dir: std::path::PathBuf) {
        let mut state = self.lock_state();
        state.cache_dir = Some(cache_dir);
        state.entries = self.scan_disk_entries(&state.cache_dir);
    }

    /// Resolves the icon for a git remote URL. Fresh bytes are served from
    /// disk; missing entries are fetched; stale entries are served
    /// immediately while a refresh runs in the background
    /// (stale-while-revalidate). `None` means no provider supplies an icon
    /// URL for the remote.
    pub async fn resolve(
        self: &Arc<Self>,
        remote_url: &str,
    ) -> Result<Option<CachedIcon>, IconError> {
        let Some(plan) = plan_from_remote(remote_url, &*self.source)? else {
            return Ok(None);
        };

        loop {
            if let Some(cached) = self.fresh_entry(&plan.key)? {
                return Ok(Some(cached));
            }

            // Stale bytes are better than none: kick off the refresh in the
            // background but do not block the caller once we have something
            // on disk.
            if self.entry_meta(&plan.key).is_some() {
                let service = Arc::clone(self);
                let background_plan = plan.clone();
                tokio::spawn(async move {
                    let _ = service.fetch_with_dedup(background_plan).await;
                });
                return Ok(Some(
                    self.read_cached(&plan.key)?
                        .ok_or(IconError::NotInitialized)?,
                ));
            }

            if let Err(error) = self.fetch_with_dedup(plan.clone()).await {
                // A concurrent leader may have written a fresh entry while
                // we were waiting; prefer serving that.
                if let Some(cached) = self.fresh_entry(&plan.key)? {
                    return Ok(Some(cached));
                }
                return Err(error);
            }
        }
    }

    pub async fn force_refresh(&self, remote_url: &str) -> Result<Option<CachedIcon>, IconError> {
        let Some(plan) = plan_from_remote(remote_url, &*self.source)? else {
            return Ok(None);
        };
        self.fetch_with_dedup(plan.clone()).await?;
        Ok(Some(
            self.read_cached(&plan.key)?
                .ok_or(IconError::NotInitialized)?,
        ))
    }

    /// Revalidates every entry older than `entry_ttl`. Failures keep the
    /// old bytes (they stay servable); returns the number of refreshed and
    /// failed keys for logging/tests.
    pub async fn refresh_stale(&self) -> (usize, usize) {
        let now = now_ms();
        let stale: Vec<String> = {
            let state = self.lock_state();
            state
                .entries
                .iter()
                .filter(|(_, meta)| meta.is_stale(self.config.entry_ttl, now))
                .map(|(key, _)| key.clone())
                .collect()
        };

        let mut refreshed = 0;
        let mut failed = 0;
        for key in stale {
            let Some(plan) = plan_from_key(&key, &*self.source) else {
                continue;
            };
            if self.fetch_with_dedup(plan).await.is_ok() {
                refreshed += 1;
            } else {
                failed += 1;
            }
        }
        (refreshed, failed)
    }

    /// Deletes entries untouched for longer than `max_age`. Last use is the
    /// newest mtime of the entry's files (touched on every serve), falling
    /// back to the metadata's fetch time when mtimes are unavailable.
    /// Returns the number of entries removed. Best-effort: IO failures are
    /// skipped, never propagated, so the startup cleanup cannot fail boot.
    pub fn prune_old_entries(&self, max_age: Duration) -> usize {
        let dir = self.lock_state().cache_dir.clone();
        let Some(dir) = dir else {
            return 0;
        };
        let now = SystemTime::now();
        let mut removed = 0;
        let Ok(host_dirs) = std::fs::read_dir(&dir) else {
            return 0;
        };
        for host_dir in host_dirs.flatten() {
            let host = host_dir.file_name().to_string_lossy().into_owned();
            let Ok(owner_dirs) = std::fs::read_dir(host_dir.path()) else {
                continue;
            };
            for owner_dir in owner_dirs.flatten() {
                let owner = owner_dir.file_name().to_string_lossy().into_owned();
                let key = format!("{host}/{owner}");
                let meta = self.entry_meta(&key);
                if entry_is_recent_enough(owner_dir.path(), meta.as_ref(), now, max_age) {
                    continue;
                }
                if std::fs::remove_dir_all(owner_dir.path()).is_ok() {
                    self.lock_state().entries.remove(&key);
                    removed += 1;
                }
            }
            try_remove_empty_dir(&host_dir.path());
        }
        removed
    }

    /// Runs the periodic refresh loop, revalidating stale entries every
    /// `refresh_interval`. The first tick is skipped so a fresh process does
    /// not hammer remotes at boot; resolve() already revalidates on demand.
    ///
    /// Must be awaited from within a Tokio runtime context, e.g.:
    /// `tauri::async_runtime::spawn(async move { icons.run_periodic_refresh().await })`.
    pub async fn run_periodic_refresh(self: Arc<Self>) {
        let interval = self.config.refresh_interval;
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // The first tick fires immediately; skip it.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            let _ = self.refresh_stale().await;
        }
    }

    async fn fetch_with_dedup(&self, plan: IconPlan) -> Result<(), IconError> {
        let waiter;
        {
            let mut inflight = self.lock_inflight();
            if let Some(existing) = inflight.get(&plan.key) {
                waiter = Some(Arc::clone(existing));
            } else {
                inflight.insert(plan.key.clone(), Arc::new(tokio::sync::Notify::new()));
                waiter = None;
            }
        }

        if let Some(notify) = waiter {
            notify.notified().await;
            // Leader finished (or failed). Either way our caller re-checks
            // freshness; if it failed and nothing is cached, resolve() will
            // retry with us as leader.
            return Ok(());
        }

        let result = self.fetch_and_store(&plan).await;

        // Wake waiters and release leadership.
        let notify = self.lock_inflight().remove(&plan.key);
        if let Some(notify) = notify {
            notify.notify_waiters();
        }

        result
    }

    async fn fetch_and_store(&self, plan: &IconPlan) -> Result<(), IconError> {
        let max_bytes = self.config.max_bytes;
        let bytes = self.fetcher.fetch(plan.url.clone(), max_bytes).await?;

        let content_type = sniff_image(&bytes).ok_or_else(|| IconError::NotAnImage {
            url: plan.url.clone(),
        })?;

        let fetched_at_ms = now_ms();
        let dir = self.cache_dir_for(plan)?;

        // Atomic writes: temp file + persist, matching the settings store.
        let image_path = dir.join(format!("{}.img", entry_file_name(plan)));
        let meta_path = dir.join(format!("{}.json", entry_file_name(plan)));

        write_atomic(&image_path, &bytes)?;
        let meta = MetaEntry {
            url: plan.url.clone(),
            content_type: content_type.to_owned(),
            fetched_at_ms,
        };
        let meta_json = serde_json::to_vec_pretty(&meta)
            .map_err(|error| IconError::Io(std::io::Error::other(error)))?;
        write_atomic(&meta_path, &meta_json)?;

        self.lock_state().entries.insert(plan.key.clone(), meta);
        Ok(())
    }

    /// Returns the cached icon when it exists AND is within TTL.
    fn fresh_entry(&self, key: &str) -> Result<Option<CachedIcon>, IconError> {
        let meta = {
            let state = self.lock_state();
            state.entries.get(key).cloned()
        };
        match meta {
            Some(meta) if !meta.is_stale(self.config.entry_ttl, now_ms()) => self.read_cached(key),
            _ => Ok(None),
        }
    }

    fn entry_meta(&self, key: &str) -> Option<MetaEntry> {
        self.lock_state().entries.get(key).cloned()
    }

    fn read_cached(&self, key: &str) -> Result<Option<CachedIcon>, IconError> {
        let (dir, meta) = {
            let state = self.lock_state();
            let Some(dir) = state.cache_dir.clone() else {
                return Ok(None);
            };
            let meta = state.entries.get(key).cloned();
            (dir, meta)
        };

        let Some(meta) = meta else {
            return Ok(None);
        };
        let (host, owner) = split_key(key);
        let owner_dir = dir.join(host).join(owner);
        let path = owner_dir.join(format!("{owner}.img"));
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };
        // Record the use so the startup cleanup can tell live entries from
        // dead ones. Failure is ignored: a missed touch only delays pruning.
        touch_all(&owner_dir);
        let content_type =
            sniff_image(&bytes).map_or_else(|| meta.content_type.clone(), str::to_owned);
        let data_url = encode_data_url(&content_type, &bytes);

        Ok(Some(CachedIcon {
            key: key.to_owned(),
            data_url,
            content_type,
            fetched_at_ms: meta.fetched_at_ms,
        }))
    }

    fn cache_dir_for(&self, plan: &IconPlan) -> Result<std::path::PathBuf, IconError> {
        let dir = self.lock_state().cache_dir.clone();
        let Some(dir) = dir else {
            return Err(IconError::NotInitialized);
        };
        let (host, owner) = split_key(&plan.key);
        let dir = dir.join(host).join(owner);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn scan_disk_entries(
        &self,
        cache_dir: &Option<std::path::PathBuf>,
    ) -> HashMap<String, MetaEntry> {
        let Some(cache_dir) = cache_dir else {
            return HashMap::new();
        };
        let mut entries = HashMap::new();
        let Ok(host_dirs) = std::fs::read_dir(cache_dir) else {
            return entries;
        };
        for host_dir in host_dirs.flatten() {
            let host = host_dir.file_name().to_string_lossy().into_owned();
            let Ok(owner_dirs) = std::fs::read_dir(host_dir.path()) else {
                continue;
            };
            for owner_dir in owner_dirs.flatten() {
                let owner = owner_dir.file_name().to_string_lossy().into_owned();
                let meta_path = owner_dir.path().join(format!(
                    "{}.json",
                    file_name_of_key(&format!("{host}/{owner}"))
                ));
                let Ok(raw) = std::fs::read_to_string(&meta_path) else {
                    continue;
                };
                if let Ok(meta) = serde_json::from_str::<MetaEntry>(&raw) {
                    entries.insert(format!("{host}/{owner}"), meta);
                }
            }
        }
        entries
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, ServiceState> {
        unwrap_poisoned(&self.state)
    }

    fn lock_inflight(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<String, Arc<tokio::sync::Notify>>> {
        unwrap_poisoned(&self.inflight)
    }
}

fn unwrap_poisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::FetchFuture;

    const PNG_BYTES: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
    ];

    struct FakeSource;

    impl OwnerIconSource for FakeSource {
        fn icon_url(&self, host: &str, owner: &str) -> Option<String> {
            (host == "github.com").then(|| format!("https://github.com/{owner}.png?size=128"))
        }
    }

    struct FakeFetcher {
        state: Mutex<FakeState>,
    }

    struct FakeState {
        calls: Vec<String>,
        responses: HashMap<String, Result<Vec<u8>, String>>,
        delay_ms: u64,
    }

    impl FakeFetcher {
        fn with_responses(responses: HashMap<String, Result<Vec<u8>, String>>) -> Self {
            Self {
                state: Mutex::new(FakeState {
                    calls: Vec::new(),
                    responses,
                    delay_ms: 0,
                }),
            }
        }

        fn png(url: &str) -> HashMap<String, Result<Vec<u8>, String>> {
            let mut map = HashMap::new();
            map.insert(url.to_owned(), Ok(PNG_BYTES.to_vec()));
            map
        }

        fn call_count(&self) -> usize {
            self.state.lock().unwrap().calls.len()
        }
    }

    impl IconFetcher for FakeFetcher {
        fn fetch(&self, url: String, max_bytes: usize) -> FetchFuture {
            {
                let mut state = self.state.lock().unwrap();
                state.calls.push(url.clone());
                if state.delay_ms > 0 {
                    std::thread::sleep(Duration::from_millis(state.delay_ms));
                }
            }
            let result = self
                .state
                .lock()
                .unwrap()
                .responses
                .get(&url)
                .cloned()
                .unwrap_or_else(|| Err("no scripted response".to_owned()));
            Box::pin(async move {
                match result {
                    Ok(bytes) => {
                        assert!(bytes.len() <= max_bytes);
                        Ok(bytes)
                    }
                    Err(message) => Err(IconError::Network { url, message }),
                }
            })
        }
    }

    fn service_in(dir: &std::path::Path, fetcher: Arc<dyn IconFetcher>) -> Arc<IconService> {
        let config = IconConfig {
            entry_ttl: Duration::from_secs(3600),
            ..IconConfig::default()
        };
        let service = Arc::new(IconService::new(config, fetcher, Arc::new(FakeSource)));
        service.initialize(dir.to_path_buf());
        service
    }

    #[tokio::test]
    async fn caches_to_disk_per_owner_and_serves_repeat_reads() {
        let dir = tempfile::tempdir().unwrap();
        let fetcher = Arc::new(FakeFetcher::with_responses(FakeFetcher::png(
            "https://github.com/octocat.png?size=128",
        )));
        let service = service_in(dir.path(), Arc::clone(&fetcher) as _);

        let first = service
            .resolve("https://github.com/octocat/hello-world.git")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(first.key, "github.com/octocat");
        assert!(first.data_url.starts_with("data:image/png;base64,"));

        // Bytes live under host/owner on disk.
        let image = dir
            .path()
            .join("github.com")
            .join("octocat")
            .join("octocat.img");
        assert_eq!(std::fs::read(&image).unwrap(), PNG_BYTES);

        // Second resolve is served from cache without another request.
        service
            .resolve("git@github.com:octocat/hello-world.git")
            .await
            .unwrap();
        assert_eq!(fetcher.call_count(), 1);
    }

    #[tokio::test]
    async fn hosts_without_a_provider_resolve_to_none_without_fetching() {
        let dir = tempfile::tempdir().unwrap();
        let fetcher = Arc::new(FakeFetcher::with_responses(HashMap::new()));
        let service = service_in(dir.path(), Arc::clone(&fetcher) as _);

        assert!(matches!(
            service.resolve("https://gitlab.com/team/repo.git").await,
            Ok(None)
        ));
        assert_eq!(fetcher.call_count(), 0);
    }

    #[tokio::test]
    async fn refresh_stale_refetches_and_failures_keep_old_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://github.com/octo.png?size=128";
        let mut responses = HashMap::new();
        responses.insert(url.to_owned(), Ok(PNG_BYTES.to_vec()));
        let fetcher = Arc::new(FakeFetcher::with_responses(responses));
        let service = service_in(dir.path(), Arc::clone(&fetcher) as _);

        service
            .resolve("https://github.com/octo/repo.git")
            .await
            .unwrap();
        assert_eq!(fetcher.call_count(), 1);

        // Age the entry beyond the TTL by rewriting its metadata, then
        // reload from disk exactly as a restart would.
        let meta_path = dir.path().join("github.com").join("octo").join("octo.json");
        let raw = std::fs::read_to_string(&meta_path).unwrap();
        let mut meta: serde_json::Value = serde_json::from_str(&raw).unwrap();
        meta["fetchedAtMs"] = serde_json::json!(0);
        std::fs::write(&meta_path, serde_json::to_string(&meta).unwrap()).unwrap();

        let reloaded = service_in(dir.path(), Arc::clone(&fetcher) as _);
        let (refreshed, failed) = reloaded.refresh_stale().await;
        assert_eq!((refreshed, failed), (1, 0));
        assert_eq!(fetcher.call_count(), 2);

        // A failing network keeps the previously cached bytes servable.
        fetcher
            .state
            .lock()
            .unwrap()
            .responses
            .insert(url.to_owned(), Err("connection refused".to_owned()));
        std::fs::write(
            &meta_path,
            serde_json::to_string(&{
                let mut m = meta.clone();
                m["fetchedAtMs"] = serde_json::json!(0);
                m
            })
            .unwrap(),
        )
        .unwrap();
        let reloaded2 = service_in(dir.path(), Arc::clone(&fetcher) as _);
        let (refreshed, failed) = reloaded2.refresh_stale().await;
        assert_eq!((refreshed, failed), (0, 1));
        let image = dir.path().join("github.com").join("octo").join("octo.img");
        assert_eq!(std::fs::read(&image).unwrap(), PNG_BYTES);
    }

    #[tokio::test]
    async fn stale_bytes_are_served_while_refresh_runs() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://github.com/octo.png?size=128";
        let mut responses = HashMap::new();
        responses.insert(url.to_owned(), Ok(PNG_BYTES.to_vec()));
        let fetcher = Arc::new(FakeFetcher::with_responses(responses));
        let service = service_in(dir.path(), Arc::clone(&fetcher) as _);
        service
            .resolve("https://github.com/octo/repo.git")
            .await
            .unwrap();

        // Age it in-memory only: next resolve must return stale bytes and
        // schedule exactly one background refresh.
        service
            .lock_state()
            .entries
            .get_mut("github.com/octo")
            .unwrap()
            .fetched_at_ms = 0;

        let icon = service
            .resolve("https://github.com/octo/repo.git")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(icon.fetched_at_ms, 0); // stale copy served immediately

        // Give the spawned refresh a beat to complete.
        tokio::time::timeout(Duration::from_secs(1), async {
            while fetcher.call_count() < 2 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("background refresh should run");

        let refreshed = service
            .resolve("https://github.com/octo/repo.git")
            .await
            .unwrap()
            .unwrap();
        assert_ne!(refreshed.fetched_at_ms, 0);
    }

    #[tokio::test]
    async fn concurrent_resolves_fetch_only_once() {
        let dir = tempfile::tempdir().unwrap();
        // All four remotes share the owner `a`, so all map to one URL.
        let url = "https://github.com/a.png?size=128";
        let mut responses = HashMap::new();
        responses.insert(url.to_owned(), Ok(PNG_BYTES.to_vec()));
        let fetcher = Arc::new(FakeFetcher::with_responses(responses));
        {
            let mut state = fetcher.state.lock().unwrap();
            state.delay_ms = 150;
        }
        let service = service_in(dir.path(), Arc::clone(&fetcher) as _);

        let (a, b, c, d) = tokio::join!(
            service.resolve("https://github.com/a/b.git"),
            service.resolve("https://github.com/a/c.git"),
            service.resolve("https://github.com/a/d.git"),
            service.resolve("git@github.com:a/e.git"),
        );
        for (label, result) in [("a", a), ("b", b), ("c", c), ("d", d)] {
            if let Err(error) = &result {
                panic!("resolve {label} failed: {error}");
            }
        }
        assert_eq!(fetcher.call_count(), 1, "in-flight requests must dedupe");
    }

    #[tokio::test]
    async fn prune_removes_only_untouched_entries() {
        let dir = tempfile::tempdir().unwrap();
        let mut responses = HashMap::new();
        responses.insert(
            "https://github.com/old.png?size=128".to_owned(),
            Ok(PNG_BYTES.to_vec()),
        );
        responses.insert(
            "https://github.com/new.png?size=128".to_owned(),
            Ok(PNG_BYTES.to_vec()),
        );
        let fetcher = Arc::new(FakeFetcher::with_responses(responses));
        let service = service_in(dir.path(), Arc::clone(&fetcher) as _);
        service
            .resolve("https://github.com/old/repo.git")
            .await
            .unwrap();
        service
            .resolve("https://github.com/new/repo.git")
            .await
            .unwrap();
        assert_eq!(fetcher.call_count(), 2);

        // Age the `old` entry past the prune horizon via mtimes.
        let old_time = SystemTime::now() - Duration::from_secs(31 * 24 * 60 * 60);
        for file in std::fs::read_dir(dir.path().join("github.com").join("old"))
            .unwrap()
            .flatten()
        {
            std::fs::File::options()
                .write(true)
                .open(file.path())
                .unwrap()
                .set_modified(old_time)
                .unwrap();
        }

        let removed = service.prune_old_entries(Duration::from_secs(30 * 24 * 60 * 60));
        assert_eq!(removed, 1);
        assert!(!dir.path().join("github.com").join("old").exists());
        assert!(dir.path().join("github.com").join("new").exists());

        // Pruned keys leave the in-memory map, so the next resolve refetches.
        service
            .resolve("https://github.com/new/repo.git")
            .await
            .unwrap();
        assert_eq!(fetcher.call_count(), 2, "fresh entry must not refetch");
    }

    #[tokio::test]
    async fn non_image_responses_are_rejected_and_not_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let mut responses = HashMap::new();
        responses.insert(
            "https://github.com/text.png?size=128".to_owned(),
            Ok(b"<html>definitely not an image</html>".to_vec()),
        );
        let fetcher = Arc::new(FakeFetcher::with_responses(responses));
        let service = service_in(dir.path(), Arc::clone(&fetcher) as _);

        assert!(matches!(
            service.resolve("https://github.com/text/repo.git").await,
            Err(IconError::NotAnImage { .. })
        ));
        assert!(!dir.path().join("github.com").join("text").exists());
    }
}
