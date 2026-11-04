use std::sync::Arc;

use git::{Backend, BackendConfig, ForgeIconSource};
use icons::{HttpFetcher, IconConfig, IconService};
use serde::Serialize;

use crate::session::SessionStore;
use crate::settings::SettingsStore;

pub struct AppState {
    pub backend: Arc<Backend>,
    pub settings: Arc<SettingsStore>,
    pub session: Arc<SessionStore>,
    pub icons: Arc<IconService>,
    pub startup_repos: Arc<tokio::sync::OnceCell<Vec<PreOpenedRepo>>>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreOpenedRepo {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        let backend = Arc::new(Backend::new(BackendConfig::default()));
        let icon_source = Arc::new(ForgeIconSource::new(Arc::clone(
            backend.forge_accounts().registry(),
        )));
        Self {
            backend,
            settings: Arc::new(SettingsStore::new(crate::settings::schema::builtin())),
            session: Arc::new(SessionStore::new()),
            icons: Arc::new(IconService::new(
                IconConfig::default(),
                Arc::new(HttpFetcher::default()),
                icon_source,
            )),
            startup_repos: Arc::new(tokio::sync::OnceCell::new()),
        }
    }
}

pub type SharedState<'a> = tauri::State<'a, AppState>;

#[inline]
pub fn to_repo_id(value: u64) -> git::domain::RepoId {
    git::domain::RepoId(value)
}
