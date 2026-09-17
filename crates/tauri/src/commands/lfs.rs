use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::lfs::LfsStatus;
use git_backend::domain::Generation;

fn gen(value: Option<u64>) -> Option<Generation> {
    value.map(Generation)
}

#[tauri::command]
pub async fn git_lfs_status(state: SharedState<'_>, repo_id: u64) -> CommandResult<LfsStatus> {
    state
        .backend
        .lfs_status(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_lfs_smudge(
    state: SharedState<'_>,
    repo_id: u64,
    expected_generation: Option<u64>,
) -> CommandResult<usize> {
    state
        .backend
        .lfs_smudge(to_repo_id(repo_id), gen(expected_generation))
        .await
        .map_err(to_serialized)
}
