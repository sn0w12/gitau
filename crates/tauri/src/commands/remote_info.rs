use crate::commands::{to_serialized, CommandResult};
use crate::state::SharedState;
use git_backend::api::remote_info::RemoteRepoInfo;
use std::path::Path;

#[tauri::command]
pub async fn git_remote_repo_info(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<RemoteRepoInfo> {
    state
        .backend
        .remote_repo_info(git_backend::domain::RepoId(repo_id))
        .await
        .map_err(to_serialized)
}

/// Remote info for a repository addressed by path; used by the sidebar and
/// home page so descriptions survive closing the last tab.
#[tauri::command]
pub async fn git_remote_repo_info_by_path(
    state: SharedState<'_>,
    path: String,
) -> CommandResult<RemoteRepoInfo> {
    state
        .backend
        .remote_repo_info_for_path(Path::new(&path))
        .await
        .map_err(to_serialized)
}
