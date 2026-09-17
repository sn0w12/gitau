use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::github::{
    AccountProfile, DeviceFlowStart, GithubOrg, PublishRepositoryRequest, PublishResult,
};

/// The connected account, `None` while signed out.
#[tauri::command]
pub fn github_account(state: SharedState<'_>) -> CommandResult<Option<AccountProfile>> {
    Ok(state.backend.github_account())
}

/// Starts a device-flow sign-in and returns the short code to display.
#[tauri::command]
pub async fn github_begin_sign_in(state: SharedState<'_>) -> CommandResult<DeviceFlowStart> {
    state
        .backend
        .github_begin_sign_in()
        .await
        .map_err(to_serialized)
}

/// Waits for browser authorization and resolves with the fresh profile.
/// Long-running by design; cancel via `github_cancel_sign_in`.
#[tauri::command]
pub async fn github_complete_sign_in(state: SharedState<'_>) -> CommandResult<AccountProfile> {
    state
        .backend
        .github_complete_sign_in()
        .await
        .map_err(to_serialized)
}

/// Aborts an in-flight sign-in; a pending complete call errors out.
#[tauri::command]
pub fn github_cancel_sign_in(state: SharedState<'_>) -> CommandResult<()> {
    state.backend.github_cancel_sign_in();
    Ok(())
}

/// Removes the stored token and profile cache.
#[tauri::command]
pub async fn github_sign_out(state: SharedState<'_>) -> CommandResult<()> {
    state.backend.github_sign_out().await.map_err(to_serialized)
}

/// Organizations the signed-in user can publish into.
#[tauri::command]
pub async fn github_list_orgs(state: SharedState<'_>) -> CommandResult<Vec<GithubOrg>> {
    state
        .backend
        .github_list_orgs()
        .await
        .map_err(to_serialized)
}

/// Creates the repository on GitHub, wires up `origin`, and pushes the
/// current branch with upstream tracking.
#[tauri::command]
pub async fn github_publish_repository(
    state: SharedState<'_>,
    repo_id: u64,
    owner: Option<String>,
    name: String,
    description: Option<String>,
    private: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<PublishResult> {
    state
        .backend
        .publish_repository(
            to_repo_id(repo_id),
            PublishRepositoryRequest {
                owner,
                name,
                description,
                private: private.unwrap_or(true),
            },
            expected_generation.map(git_backend::domain::Generation),
        )
        .await
        .map_err(to_serialized)
}
