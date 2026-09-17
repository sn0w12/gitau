use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::repository::{
    CreateRepositoryRequest, GitignoreTemplateInfo, LicenseTemplateInfo, OpenRepositoryRequest,
};
use git_backend::application::RemoveRepositoryResult;
use git_backend::domain::RepoSnapshot;
use git_backend::engines::git2::workflows::StashEntry;
use std::path::Path;
use tauri::ipc::Channel;
use tauri::AppHandle;

#[tauri::command]
pub async fn git_open_repository(
    state: SharedState<'_>,
    request: OpenRepositoryRequest,
) -> CommandResult<git_backend::application::OpenedRepository> {
    let started_at = std::time::Instant::now();
    let path = std::path::PathBuf::from(&request.path);
    let opened = state
        .backend
        .open_repository(&path)
        .await
        .map_err(to_serialized)?;
    log::info!(
        "git_open_repository {} in {:?}",
        request.path,
        started_at.elapsed()
    );
    Ok(opened)
}

#[tauri::command]
pub async fn git_close_repository(state: SharedState<'_>, repo_id: u64) -> CommandResult<bool> {
    state
        .backend
        .close_repository(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_remove_repository(
    state: SharedState<'_>,
    path: String,
    move_to_trash: bool,
) -> CommandResult<RemoveRepositoryResult> {
    state
        .backend
        .remove_repository(Path::new(&path), move_to_trash)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_repository_snapshot(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<RepoSnapshot> {
    state
        .backend
        .repository_snapshot(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_init_repository(
    state: SharedState<'_>,
    request: CreateRepositoryRequest,
) -> CommandResult<git_backend::application::CreateRepositoryResult> {
    state
        .backend
        .create_repository(request)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub fn git_list_gitignore_templates(state: SharedState<'_>) -> Vec<GitignoreTemplateInfo> {
    state.backend.list_gitignore_templates()
}

#[tauri::command]
pub fn git_list_licenses(state: SharedState<'_>) -> Vec<LicenseTemplateInfo> {
    state.backend.list_licenses()
}

#[tauri::command]
pub async fn git_stash_list(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<Vec<StashEntry>> {
    state
        .backend
        .stash_list(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_open_diff(
    app: AppHandle,
    state: SharedState<'_>,
    repo_id: u64,
    request: Option<git_backend::api::queries::DiffRequest>,
    on_event: Channel<git_backend::streaming::model::DiffEvent>,
) -> CommandResult<u64> {
    let (operation_id, rx) = state
        .backend
        .open_diff(
            to_repo_id(repo_id),
            request.unwrap_or_default(),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)?;

    tauri::async_runtime::spawn(async move {
        let mut rx = rx;
        while let Some(event) = rx.recv().await {
            if channel_send(&on_event, event).is_err() {
                break;
            }
            let _ = &app;
        }
    });

    Ok(operation_id.0)
}

pub(crate) fn channel_send<T>(channel: &Channel<T>, value: T) -> Result<(), tauri::Error>
where
    T: serde::Serialize + Clone,
{
    channel
        .send(value)
        .map_err(|e| tauri::Error::Anyhow(e.into()))
}
