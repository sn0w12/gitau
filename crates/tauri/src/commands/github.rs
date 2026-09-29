use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::github::{
    AccountProfile, DeviceFlowStart, GithubCheckRun, GithubCheckRunDetail, GithubCheckRunLog,
    GithubCommitStatus, GithubIssueComment, GithubIssueDetail, GithubIssueEvent, GithubOrg,
    GithubPullRequestCommit, GithubPullRequestDetail, GithubPullRequestReview,
    GithubPullRequestReviewComment, GithubRepoPermissions, GithubWorkflowRun, MergePullRequestBody,
    MergePullRequestResult, NotificationPage, PublishRepositoryRequest, PublishResult,
    SearchIssuePage, SearchPullRequestPage, UpdateIssueBody, UpdatePullRequestBody,
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
    page: Option<u32>,
) -> CommandResult<SearchIssuePage> {
    state
        .backend
        .github_list_issues(
            owner,
            repo,
            issue_state.unwrap_or_else(|| "open".into()),
            labels.unwrap_or_default(),
            page.unwrap_or(1).max(1),
        )
        .await
        .map_err(to_serialized)
}

/// Pull requests of a GitHub repository. `state` is open/closed/all;
/// `labels` matches pull requests carrying every named label.
#[tauri::command]
pub async fn github_list_pull_requests(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    pull_state: Option<String>,
    labels: Option<Vec<String>>,
    page: Option<u32>,
) -> CommandResult<SearchPullRequestPage> {
    state
        .backend
        .github_list_pull_requests(
            owner,
            repo,
            pull_state.unwrap_or_else(|| "open".into()),
            labels.unwrap_or_default(),
            page.unwrap_or(1).max(1),
        )
        .await
        .map_err(to_serialized)
}

/// Search issues across all of GitHub matching
/// `is:issue involves:@me sort:updated-desc`.
#[tauri::command]
pub async fn github_search_issues(
    state: SharedState<'_>,
    page: Option<u32>,
) -> CommandResult<SearchIssuePage> {
    state
        .backend
        .github_search_issues(page.unwrap_or(1).max(1))
        .await
        .map_err(to_serialized)
}

/// Search pull requests across all of GitHub matching
/// `is:pr involves:@me sort:updated-desc`.
#[tauri::command]
pub async fn github_search_pull_requests(
    state: SharedState<'_>,
    page: Option<u32>,
) -> CommandResult<SearchPullRequestPage> {
    state
        .backend
        .github_search_pull_requests(page.unwrap_or(1).max(1))
        .await
        .map_err(to_serialized)
}

/// Commit statuses for a commit, one per reporter.
#[tauri::command]
pub async fn github_list_commit_statuses(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    sha: String,
) -> CommandResult<Vec<GithubCommitStatus>> {
    state
        .backend
        .github_list_commit_statuses(owner, repo, sha)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_get_pull(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<GithubPullRequestDetail> {
    state
        .backend
        .github_get_pull(owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_list_pull_reviews(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<GithubPullRequestReview>> {
    state
        .backend
        .github_list_pull_reviews(owner, repo, number)
        .await
        .map_err(to_serialized)
}

/// Commits on the pull request's head branch, oldest first.
#[tauri::command]
pub async fn github_list_pull_commits(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<GithubPullRequestCommit>> {
    state
        .backend
        .github_list_pull_commits(owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_list_pull_review_comments(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<GithubPullRequestReviewComment>> {
    state
        .backend
        .github_list_pull_review_comments(owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_update_pull(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
    body: UpdatePullRequestBody,
) -> CommandResult<GithubPullRequestDetail> {
    state
        .backend
        .github_update_pull(owner, repo, number, body)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn github_merge_pull(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    number: u64,
    body: MergePullRequestBody,
) -> CommandResult<MergePullRequestResult> {
    state
        .backend
        .github_merge_pull(owner, repo, number, body)
        .await
        .map_err(to_serialized)
}

/// CI check runs for a commit, which is a pull request's head sha.
#[tauri::command]
pub async fn github_list_check_runs(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    sha: String,
) -> CommandResult<Vec<GithubCheckRun>> {
    state
        .backend
        .github_list_check_runs(owner, repo, sha)
        .await
        .map_err(to_serialized)
}

/// One check run with its output, for the results dialog.
#[tauri::command]
pub async fn github_get_check_run(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    check_run_id: u64,
) -> CommandResult<GithubCheckRunDetail> {
    state
        .backend
        .github_get_check_run(owner, repo, check_run_id)
        .await
        .map_err(to_serialized)
}

/// A run's job log, split per step, mirroring GitHub's job view.
#[tauri::command]
pub async fn github_get_check_run_log(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    check_run_id: u64,
) -> CommandResult<GithubCheckRunLog> {
    state
        .backend
        .github_get_check_run_log(owner, repo, check_run_id)
        .await
        .map_err(to_serialized)
}

/// Workflow runs for a commit, newest first.
#[tauri::command]
pub async fn github_list_workflow_runs(
    state: SharedState<'_>,
    owner: String,
    repo: String,
    sha: String,
) -> CommandResult<Vec<GithubWorkflowRun>> {
    state
        .backend
        .github_list_workflow_runs(owner, repo, sha)
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
