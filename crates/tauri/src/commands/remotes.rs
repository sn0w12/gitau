use crate::commands::channel::channel_send;
use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git::api::remotes::{
    CloneEvent, CloneRequest, CredentialRequest, FetchRequest, GitPullRequest, PushOutcome,
    PushRequest, RemoteAddRequest, RemoteInfo,
};
use git::engines::git2::workflows::WorkflowOutcome;
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
            expected_generation.map(git::domain::Generation),
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
            expected_generation.map(git::domain::Generation),
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
            expected_generation.map(git::domain::Generation),
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
    telemetry::scope(fetch(state, repo_id, remote, prune, credential)).await
}

#[telemetry::trace(skip = ["to_repo_id", "to_serialized", "span_detailed"], name = "git.fetch")]
async fn fetch(
    state: SharedState<'_>,
    repo_id: u64,
    remote: Option<String>,
    prune: Option<bool>,
    credential: Option<CredentialRequest>,
) -> CommandResult<()> {
    let prune = prune.unwrap_or(false);
    telemetry::context! {
        "prune" => prune,
        "has_credential" => credential.is_some(),
    };
    state
        .backend
        .fetch(
            to_repo_id(repo_id),
            FetchRequest {
                remote: remote.unwrap_or_else(|| "origin".into()),
                prune,
                credential,
                ..Default::default()
            },
            git::runtime::cancellation::CancellationToken::new(),
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
    telemetry::scope(push(
        state,
        repo_id,
        remote,
        refspecs,
        force,
        set_upstream,
        credential,
    ))
    .await
}

#[telemetry::trace(skip = ["to_repo_id", "to_serialized", "span_detailed"], name = "git.push")]
#[allow(clippy::too_many_arguments)]
async fn push(
    state: SharedState<'_>,
    repo_id: u64,
    remote: Option<String>,
    refspecs: Option<Vec<String>>,
    force: Option<bool>,
    set_upstream: Option<bool>,
    credential: Option<CredentialRequest>,
) -> CommandResult<Vec<PushOutcome>> {
    let refspecs = refspecs.unwrap_or_default();
    let force = force.unwrap_or(false);
    let set_upstream = set_upstream.unwrap_or(false);
    telemetry::context! {
        "refspecs" => refspecs.len() as u64,
        "force" => force,
        "set_upstream" => set_upstream,
        "has_credential" => credential.is_some(),
    };
    state
        .backend
        .push(
            to_repo_id(repo_id),
            PushRequest {
                remote: remote.unwrap_or_else(|| "origin".into()),
                refspecs,
                force,
                set_upstream,
                credential,
            },
            git::runtime::cancellation::CancellationToken::new(),
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
    telemetry::scope(pull(
        state,
        repo_id,
        remote,
        branch,
        fast_forward_only,
        rebase,
        credential,
        expected_generation,
    ))
    .await
}

#[telemetry::trace(skip = ["to_repo_id", "to_serialized", "span_detailed"], name = "git.pull")]
#[allow(clippy::too_many_arguments)]
async fn pull(
    state: SharedState<'_>,
    repo_id: u64,
    remote: Option<String>,
    branch: Option<String>,
    fast_forward_only: Option<bool>,
    rebase: Option<bool>,
    credential: Option<CredentialRequest>,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    let fast_forward_only = fast_forward_only.unwrap_or(false);
    let rebase = rebase.unwrap_or(false);
    telemetry::context! {
        "has_branch" => branch.is_some(),
        "fast_forward_only" => fast_forward_only,
        "rebase" => rebase,
        "has_credential" => credential.is_some(),
    };
    state
        .backend
        .pull(
            to_repo_id(repo_id),
            GitPullRequest {
                remote: remote.unwrap_or_else(|| "origin".into()),
                branch,
                fast_forward_only,
                rebase,
                credential,
            },
            expected_generation.map(git::domain::Generation),
            git::runtime::cancellation::CancellationToken::new(),
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
    telemetry::scope(clone(state, request, on_event)).await
}

#[telemetry::trace(skip = ["to_repo_id", "to_serialized", "span_detailed"], name = "git.clone")]
async fn clone(
    state: SharedState<'_>,
    request: CloneRequest,
    on_event: Channel<CloneEvent>,
) -> CommandResult<u64> {
    telemetry::context! {
        "bare" => request.bare,
        "depth" => u64::from(request.depth.unwrap_or(0)),
        "has_credential" => request.credential.is_some(),
    };
    let (operation_id, mut rx) = state
        .backend
        .open_clone(
            request,
            git::runtime::cancellation::CancellationToken::new(),
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
