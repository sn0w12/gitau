use git_backend::api::history::{
    BlameQuery, CommitDetailQuery, FileAtRevisionQuery, HistoryChartQuery, HistoryPageQuery,
};
use tauri::ipc::Channel;

use crate::commands::repository::channel_send;
use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::domain::history::{
    BlameResult, FileContent, HistoryChart, HistoryPage, RepoListing,
};
use git_backend::streaming::store::RangeResult;

#[tauri::command]
pub async fn git_history_page(
    state: SharedState<'_>,
    repo_id: u64,
    query: Option<HistoryPageQuery>,
) -> CommandResult<HistoryPage> {
    let started_at = std::time::Instant::now();
    let page = state
        .backend
        .history_page(
            to_repo_id(repo_id),
            query.unwrap_or_default(),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)?;
    log::info!(
        "git_history_page repo {repo_id} in {:?}",
        started_at.elapsed()
    );
    Ok((*page).clone())
}

#[tauri::command]
pub async fn git_history_chart(
    state: SharedState<'_>,
    repo_id: u64,
    query: Option<HistoryChartQuery>,
) -> CommandResult<HistoryChart> {
    let chart = state
        .backend
        .history_chart(
            to_repo_id(repo_id),
            query.unwrap_or_default(),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)?;
    Ok((*chart).clone())
}

#[tauri::command]
pub async fn git_commit_detail(
    state: SharedState<'_>,
    repo_id: u64,
    revision: String,
    detect_renames: Option<bool>,
) -> CommandResult<git_backend::domain::CommitDetail> {
    let query = CommitDetailQuery {
        revision: git_backend::domain::RevisionSpec::parse(&revision).map_err(to_serialized)?,
        detect_renames: detect_renames.unwrap_or(true),
    };
    let detail = state
        .backend
        .commit_detail(to_repo_id(repo_id), query, Default::default())
        .await
        .map_err(to_serialized)?;
    Ok(detail.detail.clone())
}

#[tauri::command]
pub async fn git_file_at_revision(
    state: SharedState<'_>,
    repo_id: u64,
    revision: String,
    path: String,
) -> CommandResult<FileContent> {
    let query = FileAtRevisionQuery {
        revision: git_backend::domain::RevisionSpec::parse(&revision).map_err(to_serialized)?,
        path,
    };
    state
        .backend
        .file_at_revision(to_repo_id(repo_id), query, Default::default())
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_blame(
    state: SharedState<'_>,
    repo_id: u64,
    path: String,
    revision: Option<String>,
) -> CommandResult<BlameResult> {
    let revision = match revision {
        Some(spec) => Some(git_backend::domain::RevisionSpec::parse(&spec).map_err(to_serialized)?),
        None => None,
    };
    let query = BlameQuery {
        path,
        revision,
        ..Default::default()
    };
    state
        .backend
        .blame(to_repo_id(repo_id), query, Default::default())
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_list_branches_and_tags(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<RepoListing> {
    state
        .backend
        .list_branches_and_tags(to_repo_id(repo_id), Default::default())
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub fn git_read_diff_image(
    state: SharedState<'_>,
    operation_id: u64,
    section_id: u32,
) -> CommandResult<Option<git_backend::streaming::store::DiffImage>> {
    state
        .backend
        .read_diff_image(git_backend::domain::OperationId(operation_id), section_id)
        .map_err(to_serialized)
}

#[tauri::command]
pub fn git_read_diff_range(
    state: SharedState<'_>,
    operation_id: u64,
    start_row: u64,
    max_rows: u32,
) -> CommandResult<RangeResult> {
    state
        .backend
        .read_diff_range(
            git_backend::domain::OperationId(operation_id),
            start_row,
            max_rows,
        )
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_open_graph(
    state: SharedState<'_>,
    repo_id: u64,
    query: Option<git_backend::api::graph::GraphQuery>,
    on_event: Channel<git_backend::streaming::model::GraphEvent>,
) -> CommandResult<u64> {
    let (operation_id, rx) = state
        .backend
        .open_graph(
            to_repo_id(repo_id),
            query.unwrap_or_default(),
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
        }
    });

    Ok(operation_id.0)
}

#[tauri::command]
pub fn git_read_graph_range(
    state: SharedState<'_>,
    operation_id: u64,
    start_row: u64,
    max_rows: u32,
) -> CommandResult<git_backend::streaming::graph::GraphRangeResult> {
    state
        .backend
        .read_graph_range(
            git_backend::domain::OperationId(operation_id),
            start_row,
            max_rows,
        )
        .map_err(to_serialized)
}

#[tauri::command]
pub fn git_cancel_operation(state: SharedState<'_>, operation_id: u64) -> CommandResult<bool> {
    state
        .backend
        .cancel_operation(git_backend::domain::OperationId(operation_id))
        .map_err(to_serialized)
}
