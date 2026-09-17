use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::changes::{DiscardRequest, StageRequest, StatusOptions};
use git_backend::domain::{Generation, StatusReport};

#[tauri::command]
pub async fn git_status(
    state: SharedState<'_>,
    repo_id: u64,
    options: Option<StatusOptions>,
) -> CommandResult<StatusReport> {
    let started_at = std::time::Instant::now();
    let report = state
        .backend
        .status(
            to_repo_id(repo_id),
            options.unwrap_or_default(),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)?;
    log::info!("git_status repo {repo_id} in {:?}", started_at.elapsed());
    Ok((*report).clone())
}

#[tauri::command]
pub async fn git_stage_paths(
    state: SharedState<'_>,
    repo_id: u64,
    paths: Vec<String>,
    all: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .stage_paths(
            to_repo_id(repo_id),
            StageRequest {
                paths,
                all: all.unwrap_or(false),
            },
            expected_generation.map(Generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_unstage_paths(
    state: SharedState<'_>,
    repo_id: u64,
    paths: Vec<String>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .unstage_paths(
            to_repo_id(repo_id),
            paths,
            expected_generation.map(Generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_discard_changes(
    state: SharedState<'_>,
    repo_id: u64,
    paths: Vec<String>,
    all: Option<bool>,
    expected_generation: Option<u64>,
) -> CommandResult<()> {
    state
        .backend
        .discard_changes(
            to_repo_id(repo_id),
            DiscardRequest {
                paths,
                all: all.unwrap_or(false),
            },
            expected_generation.map(Generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn git_commit(
    state: SharedState<'_>,
    repo_id: u64,
    message: String,
    stage_all: Option<bool>,
    allow_empty: Option<bool>,
    author_name: Option<String>,
    author_email: Option<String>,
    expected_generation: Option<u64>,
) -> CommandResult<git_backend::api::mutations::CommitExecution> {
    let author = match (author_name, author_email) {
        (Some(name), Some(email)) => Some(git_backend::domain::Signature {
            name,
            email,
            time_seconds: 0,
            time_offset_minutes: 0,
        }),
        _ => None,
    };
    state
        .backend
        .commit(
            to_repo_id(repo_id),
            git_backend::api::mutations::CommitRequest {
                message,
                author,
                stage_all: stage_all.unwrap_or(false),
                // Real commits run the hook pipeline; results ride back so
                // the hook checker can display them.
                run_hooks: true,
                allow_empty: allow_empty.unwrap_or(false),
            },
            expected_generation.map(Generation),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_amend_commit(
    state: SharedState<'_>,
    repo_id: u64,
    message: Option<String>,
    expected_generation: Option<u64>,
) -> CommandResult<git_backend::domain::CommitSummary> {
    state
        .backend
        .amend_commit(
            to_repo_id(repo_id),
            git_backend::api::mutations::AmendRequest {
                message,
                author: None,
            },
            expected_generation.map(Generation),
        )
        .await
        .map_err(to_serialized)
}
