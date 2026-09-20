use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::github::{
    AccountProfile, DeviceFlowStart, GithubIssueComment, GithubIssueDetail, GithubIssueEvent,
    GithubIssueListItem, GithubOrg, GithubRepoPermissions, NotificationPage,
    PublishRepositoryRequest, PublishResult, UpdateIssueBody,
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

/// One page of notification threads (read + unread), newest first. The
/// frontend derives unread counts and filters from these pages. Pages are
/// 1-based, 100 threads each.
#[tauri::command]
pub async fn github_list_notifications(
    state: SharedState<'_>,
    page: Option<u32>,
) -> CommandResult<NotificationPage> {
    state
        .backend
        .github_list_notifications(page.unwrap_or(1).max(1))
        .await
        .map_err(to_serialized)
}

/// Marks one thread read.
#[tauri::command]
pub async fn github_mark_notification_read(
    state: SharedState<'_>,
    thread_id: String,
) -> CommandResult<()> {
    state
        .backend
        .github_mark_notification_read(thread_id)
        .await
        .map_err(to_serialized)
}

/// Marks every thread read.
#[tauri::command]
pub async fn github_mark_all_notifications_read(state: SharedState<'_>) -> CommandResult<()> {
    state
        .backend
        .github_mark_all_notifications_read()
        .await
        .map_err(to_serialized)
}

/// Resolves a notification subject API URL to its web URL, for types
/// without a static mapping. `None` when the subject is gone.
#[tauri::command]
pub async fn github_resolve_subject_url(
    state: SharedState<'_>,
    subject_url: String,
) -> CommandResult<Option<String>> {
    state
        .backend
        .github_resolve_subject_url(subject_url)
        .await
        .map_err(to_serialized)
}

/// Issues of a GitHub repository. `state` is open/closed/all;
/// `labels` matches issues carrying every named label.
#[tauri::command]
pub async fn github_list_issues(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    issue_state: Option<String>,
    labels: Option<Vec<String>>,
) -> CommandResult<Vec<GithubIssueListItem>> {
    state
        .backend
        .github_list_issues(
            owner,
            repo,
            issue_state.unwrap_or_else(|| "open".into()),
            labels.unwrap_or_default(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_get_issue(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<GithubIssueDetail> {
    state
        .backend
        .github_get_issue(owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_list_issue_comments(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<GithubIssueComment>> {
    state
        .backend
        .github_list_issue_comments(owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_list_issue_events(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<GithubIssueEvent>> {
    state
        .backend
        .github_list_issue_events(owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_create_issue_comment(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
    body: String,
) -> CommandResult<GithubIssueComment> {
    state
        .backend
        .github_create_issue_comment(owner, repo, number, body)
        .await
        .map_err(to_serialized)
}

/// Partial issue update: `body` carries state (open/closed),
/// description, and replacement label and assignee name lists.
#[tauri::command]
pub async fn github_update_issue(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
    body: UpdateIssueBody,
) -> CommandResult<GithubIssueDetail> {
    state
        .backend
        .github_update_issue(owner, repo, number, body)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_update_issue_comment(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    comment_id: u64,
    body: String,
) -> CommandResult<GithubIssueComment> {
    state
        .backend
        .github_update_issue_comment(owner, repo, comment_id, body)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_delete_issue_comment(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    comment_id: u64,
) -> CommandResult<()> {
    state
        .backend
        .github_delete_issue_comment(owner, repo, comment_id)
        .await
        .map_err(to_serialized)
}

/// The signed-in user's access on the repository, for gating comment
/// moderation in the UI.
#[tauri::command]
pub async fn github_repo_permissions(
    state: SharedState<'_>,
    owner: String,
    repo: String,
) -> CommandResult<GithubRepoPermissions> {
    state
        .backend
        .github_repo_permissions(owner, repo)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_create_issue(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    title: String,
    body: Option<String>,
    labels: Option<Vec<String>>,
) -> CommandResult<GithubIssueDetail> {
    state
        .backend
        .github_create_issue(owner, repo, title, body, labels.unwrap_or_default())
        .await
        .map_err(to_serialized)
}
