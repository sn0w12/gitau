use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::worktrees::{
    WorktreeCreateRequest, WorktreeInfo, WorktreeLockRequest, WorktreeRemoveRequest,
};
use git_backend::domain::Generation;

fn gen(value: Option<u64>) -> Option<Generation> {
    value.map(Generation)
}

#[tauri::command]
pub async fn git_list_worktrees(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<Vec<WorktreeInfo>> {
    state
        .backend
        .list_worktrees(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_create_worktree(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    path: Option<String>,
    start_point: Option<String>,
    expected_generation: Option<u64>,
) -> CommandResult<WorktreeInfo> {
    state
        .backend
        .create_worktree(
            to_repo_id(repo_id),
            WorktreeCreateRequest {
                name,
                path,
                start_point,
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_remove_worktree(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    force: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .remove_worktree(
            to_repo_id(repo_id),
            WorktreeRemoveRequest {
                name,
                force: force.unwrap_or(false),
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_lock_worktree(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    reason: Option<String>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .lock_worktree(
            to_repo_id(repo_id),
            WorktreeLockRequest { name, reason },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_unlock_worktree(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .unlock_worktree(to_repo_id(repo_id), name, gen(expected_generation))
        .await
        .map_err(to_serialized)
}
