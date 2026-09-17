//! Remote owner/org icon resolution with a per-owner disk cache and
//! periodic background refresh.
//!
//! Given a git remote URL (`https://github.com/{owner}/{repo}.git`,
//! `git@github.com:{owner}/{repo}.git`, ...), the service derives a stable
//! cache key from `{host}/{owner}`, resolves the provider's avatar URL
//! (GitHub direct; any other host falls back to its favicon), downloads it
//! once, stores the bytes beside small JSON metadata, and revalidates
//! entries older than [`IconConfig::entry_ttl`], on demand (stale-while-
//! revalidate: stale bytes are still served while the refresh runs) and
//! periodically via [`IconService::spawn_periodic_refresh`].

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::runtime::disk::{sanitize_component, write_atomic};

#[derive(Debug, Clone)]
pub struct IconConfig {
    pub entry_ttl: Duration,
    pub refresh_interval: Duration,
    pub request_timeout: Duration,
    pub max_bytes: usize,
    /// Entries untouched for longer than this are deleted by the startup
    /// cleanup. Tracks last use (file mtime, touched on every serve), not
    /// last fetch, so the periodic refresh cannot keep dead entries alive.
    pub prune_after: Duration,
}

impl Default for IconConfig {
    fn default() -> Self {
        Self {
            entry_ttl: Duration::from_secs(24 * 60 * 60),
            refresh_interval: Duration::from_secs(6 * 60 * 60),
            request_timeout: Duration::from_secs(10),
            max_bytes: 5 * 1024 * 1024,
            prune_after: Duration::from_secs(30 * 24 * 60 * 60),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IconError {
    #[error("icon service not initialized")]
    NotInitialized,
    #[error("invalid remote url `{0}`")]
    InvalidRemote(String),
    #[error("network error fetching `{url}`: {message}")]
    Network { url: String, message: String },
    #[error("response for `{url}` is not a supported image")]
    NotAnImage { url: String },
    #[error("response for `{url}` exceeds the size limit")]
    TooLarge { url: String },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

impl IconError {
    pub fn code(&self) -> &'static str {
        match self {
            IconError::NotInitialized => "iconsNotInitialized",
            IconError::InvalidRemote(_) => "invalidRemoteUrl",
            IconError::Network { .. } => "iconNetwork",
            IconError::NotAnImage { .. } => "iconNotAnImage",
            IconError::TooLarge { .. } => "iconTooLarge",
            IconError::Io(_) => "io",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconPlan {
    /// `{host}/{owner}` with host lowercased; both parts sanitized.
    pub key: String,
    pub url: String,
}

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
struct MetaEntry {
    url: String,
    content_type: String,
    fetched_at_ms: u64,
}

impl MetaEntry {
    fn is_stale(&self, ttl: Duration, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.fetched_at_ms) > ttl.as_millis() as u64
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Derives the cache key + remote URL for a git remote string. Supports
/// `https://`, `http://`, and scp-like `git@host:owner/repo(.git)` forms.
pub fn plan_from_remote(remote: &str) -> Result<IconPlan, IconError> {
    let trimmed = remote.trim();
    if trimmed.is_empty() {
        return Err(IconError::InvalidRemote(remote.to_owned()));
    }

    let without_git_suffix = trimmed.strip_suffix(".git").unwrap_or(trimmed);

    // Split off scheme if present.
    let (scheme, rest) = match without_git_suffix.split_once("://") {
        Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
        None => (String::new(), without_git_suffix),
    };

    let (host, path) = if !scheme.is_empty() {
        if scheme != "https" && scheme != "http" {
            return Err(IconError::InvalidRemote(remote.to_owned()));
        }
        let (host, path) = rest
            .split_once(['/'])
            .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?;
        (host, path)
    } else {
        // scp-like syntax: git@github.com:owner/repo
        if !trimmed.contains('@') || !without_git_suffix.contains(':') {
            return Err(IconError::InvalidRemote(remote.to_owned()));
        }
        let after_user = without_git_suffix.split_once('@').unwrap().1;
        after_user
            .split_once(':')
            .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?
    };

    let host = sanitize_component(&host.to_ascii_lowercase());
    let owner = path
        .split('/')
        .find(|segment| !segment.is_empty())
        .ok_or_else(|| IconError::InvalidRemote(remote.to_owned()))?;
    // Owners are case-insensitively unique on the providers we special-case,
    // so normalize to keep one cache entry per owner.
    let owner = sanitize_component(&owner.to_lowercase());
    if host.is_empty() || owner.is_empty() {
        return Err(IconError::InvalidRemote(remote.to_owned()));
    }

    let url = avatar_url_for(&host, &owner);

    Ok(IconPlan {
        key: format!("{host}/{owner}"),
        url,
    })
}

/// Provider-specific owner/org avatar endpoints. Anything unrecognized
/// falls back to the site favicon.
fn avatar_url_for(host: &str, owner: &str) -> String {
    let is_github = host == "github.com" || host.ends_with(".github.com");
    let is_gitlab =
        host == "gitlab.com" || host.contains(".gitlab.") || host.starts_with("gitlab.");
    let is_bitbucket = host == "bitbucket.org";

    if is_github {
        // Works for users and orgs; redirects to the CDN avatar.
        format!("https://{host}/{owner}.png?size=128")
    } else if is_gitlab {
        // Serves the namespace's avatar, or an identicon when none exists.
        format!("https://{host}/{owner}.png")
    } else if is_bitbucket {
        format!("https://bitbucket.org/account/{owner}/avatar/128/")
    } else {
        format!("https://{host}/favicon.ico")
    }
}

/// Light magic-byte sniffing; some CDNs serve avatars as generic octet
/// streams, so we trust bytes over headers.
pub(crate) fn sniff_image(bytes: &[u8]) -> Option<&'static str> {
    if sniff_svg(bytes) {
        return Some("image/svg+xml");
    }
    if bytes.len() < 12 {
        return None;
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF8") {
        return Some("image/gif");
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("image/x-icon");
    }
    if &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

/// SVG is text, so sniff for an `<svg` tag after an optional XML
/// declaration, BOM, and leading whitespace.
fn sniff_svg(bytes: &[u8]) -> bool {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
    let start = bytes
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let head = &bytes[start..];
    if head.starts_with(b"<svg") {
        return true;
    }
    if !head.starts_with(b"<?xml") {
        return false;
    }
    let window = &head[..head.len().min(512)];
    window
        .to_ascii_lowercase()
        .windows(4)
        .any(|window| window == b"<svg")
}

/// Encodes image bytes as a data URL the webview can use in `<img src>`.
pub(crate) fn encode_data_url(content_type: &str, bytes: &[u8]) -> String {
    use base64::Engine as _;
    format!(
        "data:{content_type};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

pub type FetchFuture = Pin<Box<dyn Future<Output = Result<Vec<u8>, IconError>> + Send>>;

/// Downloads one URL's raw bytes. Object-safe via boxed futures so tests
/// can inject fakes without a network.
pub trait IconFetcher: Send + Sync {
    fn fetch(&self, url: String, max_bytes: usize) -> FetchFuture;
}

pub struct HttpFetcher {
    client: reqwest::Client,
}

impl HttpFetcher {
    pub fn new(timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("gitau/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds with rustls-tls");
        Self { client }
    }
}

impl Default for HttpFetcher {
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}

impl IconFetcher for HttpFetcher {
    fn fetch(&self, url: String, max_bytes: usize) -> FetchFuture {
        let client = self.client.clone();
        Box::pin(async move {
            let mut response =
                client
                    .get(&url)
                    .send()
                    .await
                    .map_err(|error| IconError::Network {
                        url: url.clone(),
                        message: error.to_string(),
                    })?;
            if !response.status().is_success() {
                return Err(IconError::Network {
                    url: url.clone(),
                    message: format!("HTTP {}", response.status()),
                });
            }

            if let Some(length) = response.content_length() {
                if length as usize > max_bytes {
                    return Err(IconError::TooLarge { url });
                }
            }

            // Stream with a hard cap regardless of advertised length.
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|error| IconError::Network {
                url: url.clone(),
                message: error.to_string(),
            })? {
                if bytes.len() + chunk.len() > max_bytes {
                    return Err(IconError::TooLarge { url });
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(bytes)
        })
    }
}

struct ServiceState {
    cache_dir: Option<std::path::PathBuf>,
    /// key -> metadata for every entry found on disk or fetched this run.
    entries: HashMap<String, MetaEntry>,
}

/// Resolves owner/org icons into a per-owner disk cache.
pub struct IconService {
    config: IconConfig,
    fetcher: Arc<dyn IconFetcher>,
    state: Mutex<ServiceState>,
    /// key -> notify handle; deduplicates concurrent fetches per key.
    inflight: Mutex<HashMap<String, Arc<tokio::sync::Notify>>>,
}

impl IconService {
    pub fn new(config: IconConfig, fetcher: Arc<dyn IconFetcher>) -> Self {
        Self {
            config,
            fetcher,
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
    /// (stale-while-revalidate).
    pub async fn resolve(self: &Arc<Self>, remote_url: &str) -> Result<CachedIcon, IconError> {
        let plan = plan_from_remote(remote_url)?;

        loop {
            if let Some(cached) = self.fresh_entry(&plan.key)? {
                return Ok(cached);
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
                return self
                    .read_cached(&plan.key)?
                    .ok_or(IconError::NotInitialized);
            }

            if let Err(error) = self.fetch_with_dedup(plan.clone()).await {
                // A concurrent leader may have written a fresh entry while
                // we were waiting; prefer serving that.
                if let Some(cached) = self.fresh_entry(&plan.key)? {
                    return Ok(cached);
                }
                return Err(error);
            }
        }
    }

    pub async fn force_refresh(&self, remote_url: &str) -> Result<CachedIcon, IconError> {
        let plan = plan_from_remote(remote_url)?;
        self.fetch_with_dedup(plan.clone()).await?;
        self.read_cached(&plan.key)?
            .ok_or(IconError::NotInitialized)
    }

    /// Revalidates every entry older than `entry_ttl`. Failures keep the
    /// old bytes (they stay servable); returns the number of refreshed and
    /// failed keys for logging/tests.
    pub async fn refresh_stale(&self) -> (usize, usize) {
        let now = now_ms();
        let stale: Vec<(String, String)> = {
            let state = self.lock_state();
            state
                .entries
                .iter()
                .filter(|(_, meta)| meta.is_stale(self.config.entry_ttl, now))
                .map(|(key, meta)| (key.clone(), meta.url.clone()))
                .collect()
        };

        let mut refreshed = 0;
        let mut failed = 0;
        for (key, url) in stale {
            let plan = IconPlan { key, url };
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

fn split_key(key: &str) -> (&str, &str) {
    match key.split_once('/') {
        Some((host, owner)) => (host, owner),
        None => ("x", key),
    }
}

/// Per-entry file name: the sanitized owner part is unique within its
/// per-host directory, so it alone names the files.
fn entry_file_name(plan: &IconPlan) -> String {
    file_name_of_key(&plan.key)
}

fn file_name_of_key(key: &str) -> String {
    let (_, owner) = split_key(key);
    owner.to_owned()
}

fn unwrap_poisoned<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Best-effort mtime bump of every file in an entry dir, marking it used.
fn touch_all(dir: &std::path::Path) {
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
fn entry_is_recent_enough(
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

fn try_remove_empty_dir(dir: &std::path::Path) {
    if std::fs::read_dir(dir)
        .map(|mut entries| entries.next().is_none())
        .unwrap_or(false)
    {
        let _ = std::fs::remove_dir(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG_BYTES: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
    ];

    #[test]
    fn plans_github_https_remotes() {
        let plan = plan_from_remote("https://github.com/Facebook/react.git").unwrap();
        assert_eq!(plan.key, "github.com/facebook");
        assert_eq!(plan.url, "https://github.com/facebook.png?size=128");
    }

    #[test]
    fn plans_handle_suffixes_and_slashes() {
        let plan = plan_from_remote("https://github.com/rust-lang/rust/").unwrap();
        assert_eq!(plan.key, "github.com/rust-lang");
    }

    #[test]
    fn plans_scp_like_remotes() {
        let plan = plan_from_remote("git@github.com:tokio-rs/tokio.git").unwrap();
        assert_eq!(plan.key, "github.com/tokio-rs");
        assert_eq!(plan.url, "https://github.com/tokio-rs.png?size=128");
    }

    #[test]
    fn supports_gitlab_and_bitbucket() {
        let gitlab = plan_from_remote("https://gitlab.com/gitlab-org/gitlab.git").unwrap();
        assert_eq!(gitlab.key, "gitlab.com/gitlab-org");
        assert_eq!(gitlab.url, "https://gitlab.com/gitlab-org.png");

        let scp = plan_from_remote("git@gitlab.com:kernel-team/linux.git").unwrap();
        assert_eq!(scp.url, "https://gitlab.com/kernel-team.png");

        let self_hosted = plan_from_remote("https://gitlab.company.dev/team/app.git").unwrap();
        assert_eq!(self_hosted.url, "https://gitlab.company.dev/team.png");

        let bitbucket = plan_from_remote("https://bitbucket.org/atlassian/atlaskit.git").unwrap();
        assert_eq!(
            bitbucket.url,
            "https://bitbucket.org/account/atlassian/avatar/128/"
        );
    }

    #[test]
    fn unknown_hosts_fall_back_to_favicon() {
        let plan = plan_from_remote("https://git.example.org/team/project.git").unwrap();
        assert_eq!(plan.key, "git.example.org/team");
        assert_eq!(plan.url, "https://git.example.org/favicon.ico");
    }

    #[test]
    fn rejects_invalid_remotes() {
        for bad in ["", "not-a-url", "ftp://host/x/y", "https://", "git@host"] {
            assert!(plan_from_remote(bad).is_err(), "`{bad}` should not plan");
        }
    }

    #[test]
    fn sanitized_keys_cannot_escape_the_cache_dir() {
        let evil = sanitize_component("../../evil");
        assert!(!evil.contains('/'));
        assert_ne!(evil, "..");

        let plan = plan_from_remote("https://github.com/../etc/passwd").unwrap();
        assert!(plan.key.starts_with("github.com/"));
        assert!(!plan.key.contains(".."));
    }

    #[test]
    fn sniffs_supported_formats() {
        let webp = b"RIFF\x00\x00\x00\x00WEBPVP8 ".to_vec();
        assert_eq!(sniff_image(PNG_BYTES), Some("image/png"));
        assert_eq!(
            sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some("image/jpeg")
        );
        assert_eq!(sniff_image(b"GIF89axxxxxxx"), Some("image/gif"));
        assert_eq!(
            sniff_image(&[0x00, 0x00, 0x01, 0x00, 0, 0, 0, 0, 0, 0, 0, 0]),
            Some("image/x-icon")
        );
        assert_eq!(sniff_image(&webp), Some("image/webp"));
        assert_eq!(sniff_image(b"<html>not an img"), None);
    }

    #[test]
    fn sniffs_svg() {
        assert_eq!(
            sniff_image(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            Some("image/svg+xml")
        );
        assert_eq!(
            sniff_image(b"\xEF\xBB\xBF  <?xml version=\"1.0\"?><svg viewBox=\"0 0 1 1\"/>"),
            Some("image/svg+xml")
        );
        assert_eq!(sniff_image(b"<?xml version=\"1.0\"?><HTML></HTML>"), None);
        assert_eq!(sniff_image(b"not svg at all"), None);
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
        let service = Arc::new(IconService::new(config, fetcher));
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

    /// Manual diagnostic: hits the real GitHub avatar endpoint through
    /// HttpFetcher. Run with:
    /// `cargo test -p git-backend http_fetcher_fetches -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "requires network"]
    async fn http_fetcher_fetches_github_avatar() {
        let fetcher = HttpFetcher::default();
        let bytes = fetcher
            .fetch(
                "https://github.com/octocat.png?size=128".to_owned(),
                5 * 1024 * 1024,
            )
            .await
            .expect("real fetch failed");
        assert_eq!(sniff_image(&bytes), Some("image/png"));
    }
}
