use std::path::Path;
use std::sync::Arc;

use git::Backend;
use serde_json::Value;

use crate::commands::{to_session_serialized, CommandResult};
use crate::state::{SessionRepository, SharedState};

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

pub(crate) async fn open_session_repositories(
    backend: Arc<Backend>,
    paths: Vec<String>,
) -> Vec<SessionRepository> {
    telemetry::scope(open_session_repositories_inner(backend, paths)).await
}

#[telemetry::trace(skip = ["scope", "spawn"], name = "app.open_session_repositories")]
async fn open_session_repositories_inner(
    backend: Arc<Backend>,
    paths: Vec<String>,
) -> Vec<SessionRepository> {
    let requested = paths.len() as u64;
    let mut set = tokio::task::JoinSet::new();
    for path in paths {
        let backend = Arc::clone(&backend);
        set.spawn(telemetry::scope(async move {
            match backend.open_repository(Path::new(&path)).await {
                Ok(opened) => SessionRepository {
                    path,
                    repo_id: Some(opened.id.0),
                    error: None,
                },
                Err(error) => SessionRepository {
                    path,
                    repo_id: None,
                    error: Some(error.to_string()),
                },
            }
        }));
    }
    let mut opened = Vec::new();
    while let Some(joined) = set.join_next().await {
        if let Ok(repo) = joined {
            opened.push(repo);
        }
    }
    let succeeded = opened.iter().filter(|repo| repo.repo_id.is_some()).count() as u64;
    telemetry::context! {
        "requested" => requested,
        "opened" => succeeded,
        "failed" => opened.len() as u64 - succeeded,
    };
    opened
}

#[tauri::command]
pub async fn session_repositories(
    state: SharedState<'_>,
    paths: Vec<String>,
) -> CommandResult<Vec<SessionRepository>> {
    telemetry::scope(restore_repositories(state, paths)).await
}

#[telemetry::trace(name = "app.session_repositories")]
async fn restore_repositories(
    state: SharedState<'_>,
    paths: Vec<String>,
) -> CommandResult<Vec<SessionRepository>> {
    let requested = paths.len() as u64;
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
            Ok(opened) => SessionRepository {
                path,
                repo_id: Some(opened.id.0),
                error: None,
            },
            Err(error) => SessionRepository {
                path,
                repo_id: None,
                error: Some(error.to_string()),
            },
        });
    }
    let opened = out.iter().filter(|repo| repo.repo_id.is_some()).count() as u64;
    telemetry::context! {
        "requested" => requested,
        "opened" => opened,
        "failed" => out.len() as u64 - opened,
    };
    Ok(out)
}
