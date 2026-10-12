use std::path::Path;
use std::sync::Arc;

use git_backend::Backend;
use serde_json::Value;

use crate::commands::{to_session_serialized, CommandResult};
use crate::state::{PreOpenedRepo, SharedState};

/// Loads the persisted app session (tabs, active tab, open repos).
#[tauri::command]
pub fn session_load(state: SharedState<'_>) -> CommandResult<Value> {
    Ok(state.session.load())
}

/// Persists the whole session document. The frontend owns its schema; the
/// backend only enforces the version field.
#[tauri::command]
pub fn session_save(state: SharedState<'_>, doc: Value) -> CommandResult<()> {
    state.session.save(doc).map_err(to_session_serialized)
}

/// Opens every repository concurrently. Shared by the setup pre-open and the
/// command so the work happens at most once per process.
pub(crate) async fn open_session_repositories(
    backend: Arc<Backend>,
    paths: Vec<String>,
) -> Vec<PreOpenedRepo> {
    let mut set = tokio::task::JoinSet::new();
    for path in paths {
        let backend = Arc::clone(&backend);
        set.spawn(async move {
            match backend.open_repository(Path::new(&path)).await {
                Ok(opened) => PreOpenedRepo {
                    path,
                    repo_id: Some(opened.id.0),
                    error: None,
                },
                Err(error) => PreOpenedRepo {
                    path,
                    repo_id: None,
                    error: Some(error.to_string()),
                },
            }
        });
    }
    let mut opened = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(repo) = joined {
            opened.push(repo);
        }
    }
    opened
}

/// Restores the session's repositories. Reuses the setup pre-open when it has
/// already run; any path it did not cover is opened here.
#[tauri::command]
pub async fn session_repositories(
    state: SharedState<'_>,
    paths: Vec<String>,
) -> CommandResult<Vec<PreOpenedRepo>> {
    let backend = Arc::clone(&state.backend);
    let cell = Arc::clone(&state.startup_repos);
    let session_paths = crate::session::repo_paths(&state.session.load());
    let preopened = cell
        .get_or_init(|| open_session_repositories(Arc::clone(&backend), session_paths))
        .await;

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        if let Some(found) = preopened.iter().find(|repo| repo.path == path) {
            out.push(found.clone());
            continue;
        }
        out.push(match backend.open_repository(Path::new(&path)).await {
            Ok(opened) => PreOpenedRepo {
                path,
                repo_id: Some(opened.id.0),
                error: None,
            },
            Err(error) => PreOpenedRepo {
                path,
                repo_id: None,
                error: Some(error.to_string()),
            },
        });
    }
    Ok(out)
}
