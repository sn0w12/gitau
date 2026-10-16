use crate::commands::{to_serialized, CommandResult};
use crate::state::{to_repo_id, SharedState};
use git_backend::api::{
    AccountProfile, CheckRun, CheckRunDetail, CheckRunLog, CommitStatus, IssueComment, IssueDetail,
    IssueEvent, MergePullRequestBody, MergePullRequestResult, NotificationPage, Org,
    PublishRepositoryRequest, PublishResult, PullRequestCommit, PullRequestDetail,
    PullRequestReview, PullRequestReviewComment, RepoPermissions, SearchIssuePage,
    SearchPullRequestPage, UpdateIssueBody, UpdatePullRequestBody, WorkflowRun,
};
use git_backend::{AuthChallenge, ConnectionProfile, ProviderDescriptor};

/// Providers this build can talk to, with their capabilities, for the UI to
/// gate views and build the provider picker.
#[tauri::command]
pub fn forge_providers(state: SharedState<'_>) -> CommandResult<Vec<ProviderDescriptor>> {
    Ok(state.backend.forge_providers())
}

/// Every connection that currently holds a token.
#[tauri::command]
pub fn forge_connections(state: SharedState<'_>) -> CommandResult<Vec<ConnectionProfile>> {
    Ok(state.backend.forge_connections())
}

/// The connected account for one provider, `None` while signed out.
#[tauri::command]
pub fn forge_account(
    state: SharedState<'_>,
    provider: String,
) -> CommandResult<Option<AccountProfile>> {
    Ok(state.backend.forge_account(provider))
}

/// Starts a device-flow sign-in and returns the short code to display.
#[tauri::command]
pub async fn forge_begin_sign_in(
    state: SharedState<'_>,
    provider: String,
    host: String,
) -> CommandResult<AuthChallenge> {
    state
        .backend
        .forge_begin_sign_in(provider, host)
        .await
        .map_err(to_serialized)
}

/// Waits for browser authorization and resolves with the fresh connection.
/// Long-running by design; cancel via `forge_cancel_sign_in`.
#[tauri::command]
pub async fn forge_complete_sign_in(state: SharedState<'_>) -> CommandResult<ConnectionProfile> {
    state
        .backend
        .forge_complete_sign_in()
        .await
        .map_err(to_serialized)
}

/// Aborts an in-flight sign-in; a pending complete call errors out.
#[tauri::command]
pub fn forge_cancel_sign_in(state: SharedState<'_>) -> CommandResult<()> {
    state.backend.forge_cancel_sign_in();
    Ok(())
}

/// Removes the stored token and profile cache for one connection.
#[tauri::command]
pub fn forge_sign_out(state: SharedState<'_>, provider: String, host: String) -> CommandResult<()> {
    state
        .backend
        .forge_sign_out(provider, host)
        .map_err(to_serialized)
}

/// Organizations the signed-in user can publish into.
#[tauri::command]
pub async fn forge_list_orgs(state: SharedState<'_>, provider: String) -> CommandResult<Vec<Org>> {
    state
        .backend
        .forge_list_orgs(provider)
        .await
        .map_err(to_serialized)
}

/// Creates the repository on the chosen provider, wires up `origin`, and
/// pushes the current branch with upstream tracking.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn forge_publish_repository(
    state: SharedState<'_>,
    repo_id: u64,
    provider: Option<String>,
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
                provider,
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
pub async fn forge_list_notifications(
    state: SharedState<'_>,
    provider: String,
    page: Option<u32>,
) -> CommandResult<NotificationPage> {
    state
        .backend
        .forge_list_notifications(provider, page.unwrap_or(1).max(1))
        .await
        .map_err(to_serialized)
}

/// Marks one thread read.
#[tauri::command]
pub async fn forge_mark_notification_read(
    state: SharedState<'_>,
    provider: String,
    thread_id: String,
) -> CommandResult<()> {
    state
        .backend
        .forge_mark_notification_read(provider, thread_id)
        .await
        .map_err(to_serialized)
}

/// Marks every thread read.
#[tauri::command]
pub async fn forge_mark_all_notifications_read(
    state: SharedState<'_>,
    provider: String,
) -> CommandResult<()> {
    state
        .backend
        .forge_mark_all_notifications_read(provider)
        .await
        .map_err(to_serialized)
}

/// Resolves a notification subject API URL to its web URL, for types
/// without a static mapping. `None` when the subject is gone.
#[tauri::command]
pub async fn forge_resolve_subject_url(
    state: SharedState<'_>,
    provider: String,
    subject_url: String,
) -> CommandResult<Option<String>> {
    state
        .backend
        .forge_resolve_subject_url(provider, subject_url)
        .await
        .map_err(to_serialized)
}

/// Issues of a repository. `state` is open/closed/all; `labels` matches
/// issues carrying every named label.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn forge_list_issues(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    issue_state: Option<String>,
    labels: Option<Vec<String>>,
    page: Option<u32>,
) -> CommandResult<SearchIssuePage> {
    state
        .backend
        .forge_list_issues(
            provider,
            owner,
            repo,
            issue_state.unwrap_or_else(|| "open".into()),
            labels.unwrap_or_default(),
            page.unwrap_or(1).max(1),
        )
        .await
        .map_err(to_serialized)
}

/// Pull requests of a repository. `state` is open/closed/all; `labels`
/// matches pull requests carrying every named label.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn forge_list_pull_requests(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    pull_state: Option<String>,
    labels: Option<Vec<String>>,
    page: Option<u32>,
) -> CommandResult<SearchPullRequestPage> {
    state
        .backend
        .forge_list_pull_requests(
            provider,
            owner,
            repo,
            pull_state.unwrap_or_else(|| "open".into()),
            labels.unwrap_or_default(),
            page.unwrap_or(1).max(1),
        )
        .await
        .map_err(to_serialized)
}

/// Search issues across a provider matching `is:issue involves:@me`.
#[tauri::command]
pub async fn forge_search_issues(
    state: SharedState<'_>,
    provider: String,
    page: Option<u32>,
) -> CommandResult<SearchIssuePage> {
    state
        .backend
        .forge_search_issues(provider, page.unwrap_or(1).max(1))
        .await
        .map_err(to_serialized)
}

/// Search pull requests across a provider matching `is:pr involves:@me`.
#[tauri::command]
pub async fn forge_search_pull_requests(
    state: SharedState<'_>,
    provider: String,
    page: Option<u32>,
) -> CommandResult<SearchPullRequestPage> {
    state
        .backend
        .forge_search_pull_requests(provider, page.unwrap_or(1).max(1))
        .await
        .map_err(to_serialized)
}

/// Commit statuses for a commit, one per reporter.
#[tauri::command]
pub async fn forge_list_commit_statuses(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    sha: String,
) -> CommandResult<Vec<CommitStatus>> {
    state
        .backend
        .forge_list_commit_statuses(provider, owner, repo, sha)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_get_pull(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<PullRequestDetail> {
    state
        .backend
        .forge_get_pull(provider, owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_list_pull_reviews(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<PullRequestReview>> {
    state
        .backend
        .forge_list_pull_reviews(provider, owner, repo, number)
        .await
        .map_err(to_serialized)
}

/// Commits on the pull request's head branch, oldest first.
#[tauri::command]
pub async fn forge_list_pull_commits(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<PullRequestCommit>> {
    state
        .backend
        .forge_list_pull_commits(provider, owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_list_pull_review_comments(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<PullRequestReviewComment>> {
    state
        .backend
        .forge_list_pull_review_comments(provider, owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_update_pull(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
    body: UpdatePullRequestBody,
) -> CommandResult<PullRequestDetail> {
    state
        .backend
        .forge_update_pull(provider, owner, repo, number, body)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_merge_pull(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
    body: MergePullRequestBody,
) -> CommandResult<MergePullRequestResult> {
    state
        .backend
        .forge_merge_pull(provider, owner, repo, number, body)
        .await
        .map_err(to_serialized)
}

/// CI check runs for a commit, which is a pull request's head sha.
#[tauri::command]
pub async fn forge_list_check_runs(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    sha: String,
) -> CommandResult<Vec<CheckRun>> {
    state
        .backend
        .forge_list_check_runs(provider, owner, repo, sha)
        .await
        .map_err(to_serialized)
}

/// One check run with its output, for the results dialog.
#[tauri::command]
pub async fn forge_get_check_run(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    check_run_id: u64,
) -> CommandResult<CheckRunDetail> {
    state
        .backend
        .forge_get_check_run(provider, owner, repo, check_run_id)
        .await
        .map_err(to_serialized)
}

/// A run's job log, split per step, mirroring the provider's job view.
#[tauri::command]
pub async fn forge_get_check_run_log(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    check_run_id: u64,
) -> CommandResult<CheckRunLog> {
    state
        .backend
        .forge_get_check_run_log(provider, owner, repo, check_run_id)
        .await
        .map_err(to_serialized)
}

/// Workflow runs for a commit, newest first.
#[tauri::command]
pub async fn forge_list_workflow_runs(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    sha: String,
) -> CommandResult<Vec<WorkflowRun>> {
    state
        .backend
        .forge_list_workflow_runs(provider, owner, repo, sha)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_get_issue(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<IssueDetail> {
    state
        .backend
        .forge_get_issue(provider, owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_list_issue_comments(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<IssueComment>> {
    state
        .backend
        .forge_list_issue_comments(provider, owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_list_issue_events(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
) -> CommandResult<Vec<IssueEvent>> {
    state
        .backend
        .forge_list_issue_events(provider, owner, repo, number)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_create_issue_comment(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
    body: String,
) -> CommandResult<IssueComment> {
    state
        .backend
        .forge_create_issue_comment(provider, owner, repo, number, body)
        .await
        .map_err(to_serialized)
}

/// Partial issue update: `body` carries state (open/closed), description,
/// and replacement label and assignee name lists.
#[tauri::command]
pub async fn forge_update_issue(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    number: u64,
    body: UpdateIssueBody,
) -> CommandResult<IssueDetail> {
    state
        .backend
        .forge_update_issue(provider, owner, repo, number, body)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn forge_create_issue(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    title: String,
    body: Option<String>,
    labels: Option<Vec<String>>,
) -> CommandResult<IssueDetail> {
    state
        .backend
        .forge_create_issue(
            provider,
            owner,
            repo,
            title,
            body,
            labels.unwrap_or_default(),
        )
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_update_issue_comment(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    comment_id: u64,
    body: String,
) -> CommandResult<IssueComment> {
    state
        .backend
        .forge_update_issue_comment(provider, owner, repo, comment_id, body)
        .await
        .map_err(to_serialized)
}

#[tauri::command]
pub async fn forge_delete_issue_comment(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
    comment_id: u64,
) -> CommandResult<()> {
    state
        .backend
        .forge_delete_issue_comment(provider, owner, repo, comment_id)
        .await
        .map_err(to_serialized)
}

/// The signed-in user's access on the repository, for gating comment
/// moderation in the UI.
#[tauri::command]
pub async fn forge_repo_permissions(
    state: SharedState<'_>,
    provider: String,
    owner: String,
    repo: String,
) -> CommandResult<RepoPermissions> {
    state
        .backend
        .forge_repo_permissions(provider, owner, repo)
        .await
        .map_err(to_serialized)
}
