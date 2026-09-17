use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};

#[tauri::command]
pub async fn git_list_commit_hooks(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<Vec<git_backend::api::hooks::HookInfo>> {
    state
        .backend
        .list_commit_hooks(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_run_commit_hook(
    state: SharedState<'_>,
    repo_id: u64,
    hook: String,
) -> CommandResult<git_backend::api::hooks::HookRunResult> {
    state
        .backend
        .run_commit_hook(to_repo_id(repo_id), hook)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_read_commit_hook(
    state: SharedState<'_>,
    repo_id: u64,
    hook: String,
) -> CommandResult<git_backend::api::hooks::HookContent> {
    state
        .backend
        .read_commit_hook(to_repo_id(repo_id), hook)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_write_commit_hook(
    state: SharedState<'_>,
    repo_id: u64,
    hook: String,
    content: String,
) -> CommandResult<()> {
    state
        .backend
        .write_commit_hook(to_repo_id(repo_id), hook, content)
        .await
        .map_err(to_serialized)
}
