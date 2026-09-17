use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::mutations::{
    ConflictFile, MergeAbortRequest, MergeContinueRequest, MergeRequest, OperationState,
    ResolveConflictRequest, ResolveSide, RevertRequest, StashAction, StashPopRequest,
    StashPushRequest,
};
use git_backend::domain::{Generation, ObjectId, RevisionSpec};
use git_backend::engines::git2::workflows::{StashEntry, WorkflowOutcome};

fn spec(value: String) -> Result<RevisionSpec, crate::commands::SerializedError> {
    RevisionSpec::parse(&value).map_err(to_serialized)
}

fn gen(value: Option<u64>) -> Option<Generation> {
    value.map(Generation)
}

#[tauri::command]
pub async fn git_merge(
    state: SharedState<'_>,
    repo_id: u64,
    target: String,
    fast_forward_only: Option<bool>,
    no_fast_forward: Option<bool>,
    message: Option<String>,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    state
        .backend
        .merge(
            to_repo_id(repo_id),
            MergeRequest {
                target: spec(target)?,
                fast_forward_only: fast_forward_only.unwrap_or(false),
                no_fast_forward: no_fast_forward.unwrap_or(false),
                message,
            },
            gen(expected_generation),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_merge_continue(
    state: SharedState<'_>,
    repo_id: u64,
    message: Option<String>,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    state
        .backend
        .merge_continue(
            to_repo_id(repo_id),
            MergeContinueRequest { message },
            gen(expected_generation),
            Default::default(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_merge_abort(
    state: SharedState<'_>,
    repo_id: u64,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    state
        .backend
        .merge_abort(
            to_repo_id(repo_id),
            MergeAbortRequest::default(),
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_start_rebase(
    state: SharedState<'_>,
    repo_id: u64,
    upstream: String,
    onto: Option<String>,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    let onto = match onto {
        Some(value) => Some(spec(value)?),
        None => None,
    };
    state
        .backend
        .start_rebase(
            to_repo_id(repo_id),
            spec(upstream)?,
            onto,
            gen(expected_generation),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_continue_rebase(
    state: SharedState<'_>,
    repo_id: u64,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    state
        .backend
        .continue_rebase(
            to_repo_id(repo_id),
            gen(expected_generation),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_abort_rebase(
    state: SharedState<'_>,
    repo_id: u64,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    state
        .backend
        .abort_rebase(to_repo_id(repo_id), gen(expected_generation))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_cherry_pick(
    state: SharedState<'_>,
    repo_id: u64,
    target: String,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    state
        .backend
        .cherry_pick(to_repo_id(repo_id), spec(target)?, gen(expected_generation))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_revert(
    state: SharedState<'_>,
    repo_id: u64,
    target: String,
    parent_index: Option<u32>,
    expected_generation: Option<u64>,
) -> CommandResult<WorkflowOutcome> {
    state
        .backend
        .revert(
            to_repo_id(repo_id),
            RevertRequest {
                target: spec(target)?,
                parent_index: parent_index.unwrap_or(0),
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_operation_state(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<OperationState> {
    state
        .backend
        .operation_state(to_repo_id(repo_id))
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_resolve_conflict(
    state: SharedState<'_>,
    repo_id: u64,
    path: String,
    side: ResolveSide,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .resolve_conflict(
            to_repo_id(repo_id),
            ResolveConflictRequest { path, side },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_conflict_file(
    state: SharedState<'_>,
    repo_id: u64,
    path: String,
    stage: u8,
) -> CommandResult<ConflictFile> {
    state
        .backend
        .conflict_file(
            to_repo_id(repo_id),
            path,
            stage,
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_stash_push(
    state: SharedState<'_>,
    repo_id: u64,
    message: Option<String>,
    include_untracked: Option<bool>,
    keep_index: Option<bool>,
    paths: Option<Vec<String>>,
    expected_generation: Option<u64>,
) -> CommandResult<ObjectId> {
    state
        .backend
        .stash_push(
            to_repo_id(repo_id),
            StashPushRequest {
                message,
                include_untracked: include_untracked.unwrap_or(false),
                keep_index: keep_index.unwrap_or(false),
                paths: paths.unwrap_or_default(),
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_stash_pop(
    state: SharedState<'_>,
    repo_id: u64,
    index: usize,
    action: Option<StashAction>,
    expected_generation: Option<u64>,
) -> CommandResult<Vec<StashEntry>> {
    state
        .backend
        .stash_pop(
            to_repo_id(repo_id),
            StashPopRequest {
                index,
                action: action.unwrap_or_default(),
            },
            gen(expected_generation),
        )
        .await
        .map_err(to_serialized)
}
