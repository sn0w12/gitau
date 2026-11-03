use std::time::Duration;

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
