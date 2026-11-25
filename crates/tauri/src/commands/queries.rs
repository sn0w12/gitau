use git::api::history::{
    BlameQuery, CommitDetailQuery, FileAtRevisionQuery, HistoryChartQuery, HistoryPageQuery,
};
use tauri::ipc::Channel;

use crate::commands::channel::channel_send;
use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git::domain::history::{BlameResult, FileContent, HistoryChart, HistoryPage, RepoListing};
use git::streaming::store::{DiffImage, DiffImageSide, DiffImageSideKind, RangeResult};

#[tauri::command]
pub async fn git_history_page(
    state: SharedState<'_>,
    repo_id: u64,
    query: Option<HistoryPageQuery>,
) -> CommandResult<HistoryPage> {
    telemetry::scope(history_page(state, repo_id, query)).await
}

#[telemetry::trace(skip = ["to_repo_id", "to_serialized"], name = "git.history_page")]
async fn history_page(
    state: SharedState<'_>,
    repo_id: u64,
    query: Option<HistoryPageQuery>,
) -> CommandResult<HistoryPage> {
    let started_at = std::time::Instant::now();
    let query = query.unwrap_or_default();
    let limit = query.limit;
    let skip = query.skip;
    let filtered =
        query.search.is_some() || query.path.is_some() || !query.exclude_reachable_from.is_empty();
    let page = state
        .backend
        .history_page(
            to_repo_id(repo_id),
            query,
            git::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)?;
    telemetry::context! {
        "limit" => u64::from(limit),
        "skip" => skip,
        "filtered" => filtered,
        "commits" => page.commits.len() as u64,
        "total" => u64::from(page.total),
        "has_more" => page.has_more,
    };
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
    telemetry::scope(history_chart(state, repo_id, query)).await
}

#[telemetry::trace(skip = ["to_repo_id", "to_serialized"], name = "git.history_chart")]
async fn history_chart(
    state: SharedState<'_>,
    repo_id: u64,
    query: Option<HistoryChartQuery>,
) -> CommandResult<HistoryChart> {
    let query = query.unwrap_or_default();
    let include_merges = query.include_merges;
    let chart = state
        .backend
        .history_chart(
            to_repo_id(repo_id),
            query,
            git::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)?;
    telemetry::context! {
        "buckets" => chart.buckets.len() as u64,
        "commits" => u64::from(chart.total_commits),
        "include_merges" => include_merges,
        "bucket_size" => format!("{:?}", chart.bucket_size),
    };
    Ok((*chart).clone())
}

#[tauri::command]
pub async fn git_commit_detail(
    state: SharedState<'_>,
    repo_id: u64,
    revision: String,
    detect_renames: Option<bool>,
) -> CommandResult<git::domain::CommitDetail> {
    telemetry::scope(commit_detail(state, repo_id, revision, detect_renames)).await
}

#[telemetry::trace(
    skip = ["parse", "to_repo_id", "to_serialized"],
    name = "git.commit_detail"
)]
async fn commit_detail(
    state: SharedState<'_>,
    repo_id: u64,
    revision: String,
    detect_renames: Option<bool>,
) -> CommandResult<git::domain::CommitDetail> {
    let detect_renames = detect_renames.unwrap_or(true);
    let query = CommitDetailQuery {
        revision: git::domain::RevisionSpec::parse(&revision).map_err(to_serialized)?,
        detect_renames,
    };
    let detail = state
        .backend
        .commit_detail(to_repo_id(repo_id), query, Default::default())
        .await
        .map_err(to_serialized)?;
    telemetry::context! {
        "detect_renames" => detect_renames,
        "files" => detail.detail.files.len() as u64,
        "additions" => detail.detail.total_additions,
        "deletions" => detail.detail.total_deletions,
    };
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
        revision: git::domain::RevisionSpec::parse(&revision).map_err(to_serialized)?,
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
    telemetry::scope(blame(state, repo_id, path, revision)).await
}

#[telemetry::trace(skip = ["parse", "to_repo_id", "to_serialized"], name = "git.blame")]
async fn blame(
    state: SharedState<'_>,
    repo_id: u64,
    path: String,
    revision: Option<String>,
) -> CommandResult<BlameResult> {
    let has_revision = revision.is_some();
    let revision = match revision {
        Some(spec) => Some(git::domain::RevisionSpec::parse(&spec).map_err(to_serialized)?),
        None => None,
    };
    let query = BlameQuery {
        path,
        revision,
        ..Default::default()
    };
    let result = state
        .backend
        .blame(to_repo_id(repo_id), query, Default::default())
        .await
        .map_err(to_serialized)?;
    telemetry::context! {
        "has_revision" => has_revision,
        "lines" => result.lines.len() as u64,
    };
    Ok(result)
}

#[tauri::command]
pub async fn git_list_branches_and_tags(
    state: SharedState<'_>,
    repo_id: u64,
) -> CommandResult<RepoListing> {
    telemetry::scope(repo_listing(state, repo_id)).await
}

#[telemetry::trace(
    skip = ["to_repo_id", "to_serialized"],
    name = "git.list_branches_and_tags"
)]
async fn repo_listing(state: SharedState<'_>, repo_id: u64) -> CommandResult<RepoListing> {
    let listing = state
        .backend
        .list_branches_and_tags(to_repo_id(repo_id), Default::default())
        .await
        .map_err(to_serialized)?;
    telemetry::context! {
        "branches" => listing.branches.len() as u64,
        "tags" => listing.tags.len() as u64,
    };
    Ok(listing)
}

#[tauri::command]
pub async fn git_branch_ahead_behind(
    state: SharedState<'_>,
    repo_id: u64,
    head: String,
    base: String,
) -> CommandResult<git::domain::AheadBehind> {
    telemetry::scope(ahead_behind(state, repo_id, head, base)).await
}

#[telemetry::trace(skip = ["to_repo_id", "to_serialized"], name = "git.ahead_behind")]
async fn ahead_behind(
    state: SharedState<'_>,
    repo_id: u64,
    head: String,
    base: String,
) -> CommandResult<git::domain::AheadBehind> {
    let result = state
        .backend
        .branch_ahead_behind(
            to_repo_id(repo_id),
            head,
            base,
            git::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .map_err(to_serialized)?;
    telemetry::context! {
        "ahead" => u64::from(result.ahead),
        "behind" => u64::from(result.behind),
    };
    Ok(result)
}

/// Returns `gitau-image` protocol URLs for an image section. The webview
/// fetches the bytes itself, so no image data crosses the IPC payload.
#[tauri::command]
pub fn git_read_diff_image(
    state: SharedState<'_>,
    webview: tauri::Webview,
    operation_id: u64,
    section_id: u32,
) -> CommandResult<Option<DiffImage>> {
    let Some(source) = state
        .backend
        .diff_image_source(git::domain::OperationId(operation_id), section_id)
        .map_err(to_serialized)?
    else {
        return Ok(None);
    };

    let side = |kind| -> CommandResult<Option<DiffImageSide>> {
        if !source.has_side(kind) {
            return Ok(None);
        }
        let url = crate::commands::diff_image::url(&webview, operation_id, section_id, kind)
            .map_err(|error| to_serialized(git::error::GitError::internal(error.to_string())))?;
        Ok(Some(DiffImageSide { url }))
    };

    Ok(Some(DiffImage {
        old: side(DiffImageSideKind::Old)?,
        new: side(DiffImageSideKind::New)?,
    }))
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
        .read_diff_range(git::domain::OperationId(operation_id), start_row, max_rows)
        .map_err(to_serialized)
}

#[tauri::command]
pub fn git_read_diff_context(
    state: SharedState<'_>,
    operation_id: u64,
    section_id: u32,
    old_start: u32,
    new_start: u32,
    count: u32,
) -> CommandResult<git::api::queries::DiffContextResult> {
    state
        .backend
        .read_diff_context(
            git::domain::OperationId(operation_id),
            section_id,
            old_start,
            new_start,
            count,
        )
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn git_open_graph(
    state: SharedState<'_>,
    repo_id: u64,
    query: Option<git::api::graph::GraphQuery>,
    on_event: Channel<git::streaming::model::GraphEvent>,
) -> CommandResult<u64> {
    telemetry::scope(open_graph(state, repo_id, query, on_event)).await
}

#[telemetry::trace(skip = ["to_repo_id", "to_serialized"], name = "git.open_graph")]
async fn open_graph(
    state: SharedState<'_>,
    repo_id: u64,
    query: Option<git::api::graph::GraphQuery>,
    on_event: Channel<git::streaming::model::GraphEvent>,
) -> CommandResult<u64> {
    let query = query.unwrap_or_default();
    telemetry::context! {
        "has_revision" => query.revision.is_some(),
        "exclusions" => query.exclude_reachable_from.len() as u64,
    };
    let (operation_id, rx) = state
        .backend
        .open_graph(
            to_repo_id(repo_id),
            query,
            git::runtime::cancellation::CancellationToken::new(),
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
) -> CommandResult<git::streaming::graph::GraphRangeResult> {
    state
        .backend
        .read_graph_range(git::domain::OperationId(operation_id), start_row, max_rows)
        .map_err(to_serialized)
}

#[tauri::command]
pub fn git_cancel_operation(state: SharedState<'_>, operation_id: u64) -> CommandResult<bool> {
    state
        .backend
        .cancel_operation(git::domain::OperationId(operation_id))
        .map_err(to_serialized)
}
