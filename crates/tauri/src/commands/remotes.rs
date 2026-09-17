use crate::commands::repository::channel_send;
use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::remotes::{
    CloneEvent, CloneRequest, CredentialRequest, FetchRequest, PullRequest, PushOutcome,
    PushRequest, RemoteAddRequest, RemoteInfo,
};
use git_backend::engines::git2::workflows::WorkflowOutcome;
use std::path::Path;
use tauri::ipc::Channel;

#[tauri::command]
pub async fn git_list_remotes(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<Vec<RemoteInfo>> {
    state
        .backend
        .list_remotes(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

/// Lists remotes for a repository addressed by path, so metadata can be
/// shown without an open session.
#[tauri::command]
pub async fn git_list_remotes_by_path(
    state: SharedState<'_>,
    path: String,
) -> CommandResult<Vec<RemoteInfo>> {
    state
        .backend
        .list_remotes_for_path(Path::new(&path))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_add_remote(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    url: String,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .add_remote(
            to_repo_id(repo_id),
            RemoteAddRequest {
                name,
                url,
                fetch_refspec: None,
            },
            expected_generation.map(git_backend::domain::Generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_remove_remote(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .remove_remote(
            to_repo_id(repo_id),
            name,
            expected_generation.map(git_backend::domain::Generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_set_remote_url(
    state: SharedState<'_>,
    repo_id: u64,
    name: String,
    url: String,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .set_remote_url(
            to_repo_id(repo_id),
            name,
            url,
            expected_generation.map(git_backend::domain::Generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_fetch(
    state: SharedState<'_>,
    repo_id: u64,
    remote: Option<String>,
    prune: Option<bool>,
    credential: Option<CredentialRequest>,
) -> CommandResult<()> {
    state
        .backend
        .fetch(
            to_repo_id(repo_id),
            FetchRequest {
                remote: remote.unwrap_or_else(|| "origin".into()),
                prune: prune.unwrap_or(false),
                credential,
                ..Default::default()
            },
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_push(
    state: SharedState<'_>,
    repo_id: u64,
    remote: Option<String>,
    refspecs: Option<Vec<String>>,
    force: Option<bool>,
    set_upstream: Option<bool>,
    credential: Option<CredentialRequest>,
) -> CommandResult<Vec<PushOutcome>> {
    state
        .backend
        .push(
            to_repo_id(repo_id),
            PushRequest {
                remote: remote.unwrap_or_else(|| "origin".into()),
                refspecs: refspecs.unwrap_or_default(),
                force: force.unwrap_or(false),
                set_upstream: set_upstream.unwrap_or(false),
                credential,
            },
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn git_pull(
    state: SharedState<'_>,
    repo_id: u64,
    remote: Option<String>,
    branch: Option<String>,
    fast_forward_only: Option<bool>,
    rebase: Option<bool>,
    credential: Option<CredentialRequest>,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    state
        .backend
        .pull(
            to_repo_id(repo_id),
            PullRequest {
                remote: remote.unwrap_or_else(|| "origin".into()),
                branch,
                fast_forward_only: fast_forward_only.unwrap_or(false),
                rebase: rebase.unwrap_or(false),
                credential,
            },
            expected_generation.map(git_backend::domain::Generation),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)
}

/// Starts a clone and streams CloneEvent updates until a terminal event.
/// Resolves with the operation id, usable with `git_cancel_operation`.
#[tauri::command]
pub async fn git_clone(
    state: SharedState<'_>,
    request: CloneRequest,
    on_event: Channel<CloneEvent>,
) -> CommandResult<u64> {
    let (operation_id, mut rx) = state
        .backend
        .open_clone(
            request,
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)?;

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if channel_send(&on_event, event).is_err() {
                break;
            }
        }
    });

    Ok(operation_id.0)
}
