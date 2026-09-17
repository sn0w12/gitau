use std::sync::Arc;

use git_backend::icons::{HttpFetcher, IconConfig, IconService};
use git_backend::{Backend, BackendConfig};

use crate::session::SessionStore;
use crate::settings::SettingsStore;

pub struct AppState {
    pub backend: Arc<Backend>,
    pub settings: Arc<SettingsStore>,
    pub session: Arc<SessionStore>,
    pub icons: Arc<IconService>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            backend: Arc::new(Backend::new(BackendConfig::default())),
            settings: Arc::new(SettingsStore::new(crate::settings::schema::builtin())),
            session: Arc::new(SessionStore::new()),
            icons: Arc::new(IconService::new(
                IconConfig::default(),
                Arc::new(HttpFetcher::default()),
            )),
        }
    }
}

pub type SharedState<'a> = tauri::State<'a, AppState>;

#[inline]
pub fn to_repo_id(value: u64) -> git_backend::domain::RepoId {
    git_backend::domain::RepoId(value)
}
