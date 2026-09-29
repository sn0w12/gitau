use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::ansi;
use super::device_flow::{DeviceCodeResponse, TokenPoll, parse_device_code, parse_token_poll};
use super::{
    AccountProfile, GitHubError, GithubActionStep, GithubCheckAnnotation, GithubCheckRun,
    GithubCheckRunDetail, GithubCheckRunLog, GithubCheckRunOutput, GithubIssueComment,
    GithubIssueDetail, GithubIssueEvent, GithubLabel, GithubNotification, GithubOrg,
    GithubPullRequestCommit, GithubPullRequestDetail, GithubPullRequestListItem,
    GithubPullRequestRef, GithubPullRequestReview, GithubPullRequestReviewComment,
    GithubRepoPermissions, GithubUser, GithubWorkflowRun, MergePullRequestBody,
    MergePullRequestResult, NotificationPage, PullRequestMergeMethod, SearchIssueItem,
    SearchIssuePage, SearchPullRequestPage, UpdateIssueBody, UpdatePullRequestBody,
};

/// Object-safe async surface: boxed futures let tests inject fakes without
/// a network.
pub type GithubFuture<T> =
    Pin<Box<dyn Future<Output = std::result::Result<T, GitHubError>> + Send>>;

/// Minimal GitHub REST surface used by the integration.
pub trait GithubApi: Send + Sync {
    fn request_device_code(
        &self,
        client_id: &str,
        scopes: &str,
    ) -> GithubFuture<DeviceCodeResponse>;
    fn poll_token(&self, client_id: &str, device_code: &str) -> GithubFuture<TokenPoll>;
    fn authenticated_user(&self, token: &str) -> GithubFuture<AccountProfile>;
    fn list_orgs(&self, token: &str) -> GithubFuture<Vec<GithubOrg>>;
    fn create_repository(
        &self,
        token: &str,
        owner: Option<&str>,
        body: &CreateRepoBody,
    ) -> GithubFuture<CreatedRepository>;
    fn list_notifications(&self, token: &str, page: u32) -> GithubFuture<NotificationPage>;
    fn mark_notification_read(&self, token: &str, thread_id: &str) -> GithubFuture<()>;
    fn mark_all_notifications_read(&self, token: &str) -> GithubFuture<()>;
    /// Resolves a notification subject API URL to its web URL by fetching
    /// the subject and reading `html_url`. Returns `None` when the subject
    /// is gone (404). Used for types with no static URL mapping, like
    /// releases (addressed by tag, not id) and check suites.
    fn fetch_subject_html_url(
        &self,
        token: &str,
        subject_url: &str,
    ) -> GithubFuture<Option<String>>;
    /// Issues of a repository, open/closed/all. `labels` matches issues
    /// carrying every named label.
    fn list_issues(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: &str,
        labels: &[String],
        page: u32,
    ) -> GithubFuture<SearchIssuePage>;
    /// Search issues across all of GitHub. The query is fixed to
    /// `is:issue involves:@me sort:updated-desc`.
    fn search_issues(&self, token: &str, page: u32) -> GithubFuture<SearchIssuePage>;
    /// Search pull requests across all of GitHub, the pull request twin of
    /// [`GithubApi::search_issues`]: `is:pr involves:@me
    /// sort:updated-desc`.
    fn search_pull_requests(&self, token: &str, page: u32) -> GithubFuture<SearchPullRequestPage>;
    /// Pull requests of a repository, open/closed/all. `labels` matches
    /// pull requests carrying every named label. Search carries no head or
    /// base ref, so the rows are limited to what the issue projection has.
    fn list_pull_requests(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: &str,
        labels: &[String],
        page: u32,
    ) -> GithubFuture<SearchPullRequestPage>;
    /// Full pull request detail, including draft, mergeability, refs, and
    /// diff stats. The `mergeable` field is null while GitHub computes the
    /// merge in the background, so the UI must re-read before offering merge.
    fn get_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<GithubPullRequestDetail>;
    /// Submitted reviews, oldest first.
    fn list_pull_reviews(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubPullRequestReview>>;
    /// Commits on the pull request's head branch, oldest first.
    fn list_pull_commits(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubPullRequestCommit>>;
    /// Inline diff comments, including replies to other review comments.
    fn list_pull_review_comments(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubPullRequestReviewComment>>;
    /// Partial update: state (open/closed), base branch, draft flag.
    fn update_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdatePullRequestBody,
    ) -> GithubFuture<GithubPullRequestDetail>;
    fn merge_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &MergePullRequestBody,
    ) -> GithubFuture<MergePullRequestResult>;
    /// CI check runs for a commit, which is the pull request's head sha.
    fn list_check_runs(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> GithubFuture<Vec<GithubCheckRun>>;
    /// One check run with its output, for the results dialog.
    fn get_check_run(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        check_run_id: u64,
    ) -> GithubFuture<GithubCheckRunDetail>;
    /// A run's job log, split per step, mirroring GitHub's job view.
    fn get_check_run_log(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        check_run_id: u64,
    ) -> GithubFuture<GithubCheckRunLog>;
    /// Workflow runs for a commit, newest first.
    fn list_workflow_runs(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> GithubFuture<Vec<GithubWorkflowRun>>;
    fn get_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<GithubIssueDetail>;
    fn list_issue_comments(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubIssueComment>>;
    fn list_issue_events(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubIssueEvent>>;
    fn create_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> GithubFuture<GithubIssueComment>;
    /// Partial update: state (open/closed), labels, assignees.
    fn update_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdateIssueBody,
    ) -> GithubFuture<GithubIssueDetail>;
    fn create_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        labels: &[String],
    ) -> GithubFuture<GithubIssueDetail>;
    fn update_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> GithubFuture<GithubIssueComment>;
    fn delete_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> GithubFuture<()>;
    /// The signed-in user's access on the repository, for gating
    /// comment moderation in the UI.
    fn repo_permissions(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> GithubFuture<GithubRepoPermissions>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateRepoBody {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub private: bool,
}

/// GitHub's merge endpoint takes `merge_method` in snake_case, unlike the
/// camelCase the IPC DTOs use, so the wire form is built separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
struct MergePullPayload {
    merge_method: PullRequestMergeMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit_message: Option<String>,
}

impl From<&MergePullRequestBody> for MergePullPayload {
    fn from(body: &MergePullRequestBody) -> Self {
        Self {
            merge_method: body.merge_method,
            commit_title: body.commit_title.clone(),
            commit_message: body.commit_message.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedRepository {
    pub full_name: String,
    pub html_url: String,
    pub default_branch: Option<String>,
    /// HTTPS clone URL as reported by GitHub; publish wires this up as
    /// `origin`. Tests substitute a local path here.
    pub clone_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawUser {
    login: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawOrg {
    login: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawRepo {
    full_name: String,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    default_branch: Option<String>,
    #[serde(default)]
    clone_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawNotification {
    id: String,
    #[serde(default)]
    unread: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    subject: Option<RawNotificationSubject>,
    #[serde(default)]
    repository: Option<RawNotificationRepo>,
    #[serde(default)]
    updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawNotificationSubject {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawNotificationRepo {
    #[serde(default)]
    full_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSubject {
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawIssueUser {
    #[serde(default)]
    login: String,
    #[serde(default)]
    avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct RawIssueLabel {
    #[serde(default)]
    name: String,
    #[serde(default)]
    color: String,
}

#[derive(Debug, Deserialize)]
struct RawIssue {
    #[serde(default)]
    number: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    user: Option<RawIssueUser>,
    #[serde(default)]
    labels: Vec<RawIssueLabel>,
    #[serde(default)]
    assignees: Vec<RawIssueUser>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSearchIssueResponse {
    items: Vec<RawSearchIssue>,
}

#[derive(Debug, Deserialize)]
struct RawSearchIssue {
    #[serde(default)]
    number: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    user: Option<RawIssueUser>,
    #[serde(default)]
    labels: Vec<RawIssueLabel>,
    #[serde(default)]
    assignees: Vec<RawIssueUser>,
    #[serde(default)]
    comments: u64,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    repository_url: Option<String>,
    #[serde(default)]
    pull_request: Option<RawSearchPullRef>,
}

/// Search items carry this block only for pull requests, and `merged_at` is
/// the sole signal that a `closed` pull request was in fact merged.
#[derive(Debug, Deserialize)]
struct RawSearchPullRef {
    #[serde(default)]
    merged_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawIssueComment {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    user: Option<RawIssueUser>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

/// The repository payload carries far more than permissions; only the
/// access block is parsed.
#[derive(Debug, Deserialize)]
struct RawPermissionsEnvelope {
    #[serde(default)]
    permissions: Option<RawRepoPermissions>,
}

#[derive(Debug, Deserialize, Default)]
struct RawRepoPermissions {
    #[serde(default)]
    push: bool,
    #[serde(default)]
    admin: bool,
}

#[derive(Debug, Deserialize)]
struct RawIssueEvent {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    event: String,
    #[serde(default)]
    actor: Option<RawIssueUser>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    label: Option<RawIssueLabel>,
    #[serde(default)]
    assignee: Option<RawIssueUser>,
    /// Review requests name the user in this slot instead of `assignee`.
    #[serde(default)]
    requested_reviewer: Option<RawIssueUser>,
}

#[derive(Debug, Deserialize)]
struct RawPullRef {
    #[serde(default, rename = "ref")]
    git_ref: String,
    #[serde(default)]
    sha: String,
    #[serde(default)]
    label: String,
}

#[derive(Debug, Deserialize)]
struct RawPull {
    #[serde(default)]
    number: u64,
    #[serde(default)]
    title: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    user: Option<RawIssueUser>,
    #[serde(default)]
    labels: Vec<RawIssueLabel>,
    #[serde(default)]
    assignees: Vec<RawIssueUser>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    merged_at: Option<String>,
    /// null while GitHub computes it, so the tri-state must survive.
    #[serde(default)]
    mergeable: Option<bool>,
    #[serde(default)]
    mergeable_state: Option<String>,
    #[serde(default)]
    head: Option<RawPullRef>,
    #[serde(default)]
    base: Option<RawPullRef>,
    #[serde(default)]
    additions: u64,
    #[serde(default)]
    deletions: u64,
    #[serde(default)]
    changed_files: u64,
    #[serde(default)]
    commits: u64,
}

/// The commits endpoint nests the message and dates under `commit`, with
/// `author`/`committer` holding a linked account only when the email matches
/// one, so the git-level author is what fills the gap.
#[derive(Debug, Deserialize)]
struct RawPullCommit {
    #[serde(default)]
    sha: String,
    #[serde(default)]
    commit: Option<RawCommitDetail>,
    #[serde(default)]
    author: Option<RawIssueUser>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCommitDetail {
    #[serde(default)]
    message: String,
    #[serde(default)]
    author: Option<RawCommitSignature>,
}

#[derive(Debug, Deserialize)]
struct RawCommitSignature {
    #[serde(default)]
    name: String,
    #[serde(default)]
    date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPullReview {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    user: Option<RawIssueUser>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    submitted_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawPullReviewComment {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    user: Option<RawIssueUser>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    path: String,
    #[serde(default)]
    line: Option<u64>,
    #[serde(default)]
    diff_hunk: String,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    in_reply_to_id: Option<u64>,
    pull_request_review_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawMergeResult {
    #[serde(default)]
    sha: String,
    #[serde(default)]
    merged: bool,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct RawCheckRun {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    details_url: Option<String>,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCheckRunOutput {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    annotations_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RawCheckAnnotation {
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    start_line: Option<u64>,
    #[serde(default)]
    end_line: Option<u64>,
    #[serde(default)]
    start_column: Option<u64>,
    #[serde(default)]
    annotation_level: Option<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawActionStep {
    #[serde(default)]
    number: u64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    started_at: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawActionJob {
    #[serde(default)]
    steps: Vec<RawActionStep>,
}

#[derive(Debug, Deserialize)]
struct RawCheckRunDetail {
    #[serde(flatten)]
    run: RawCheckRun,
    #[serde(default)]
    output: Option<RawCheckRunOutput>,
}

#[derive(Debug, Deserialize)]
struct RawCheckRunsResponse {
    #[serde(default)]
    check_runs: Vec<RawCheckRun>,
}

#[derive(Debug, Deserialize)]
struct RawWorkflowRun {
    #[serde(default)]
    id: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    event: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    conclusion: Option<String>,
    #[serde(default)]
    run_number: u64,
    #[serde(default)]
    head_branch: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    created_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawWorkflowRunsResponse {
    #[serde(default)]
    workflow_runs: Vec<RawWorkflowRun>,
}

fn map_check_run(raw: RawCheckRun) -> GithubCheckRun {
    GithubCheckRun {
        id: raw.id,
        name: raw.name,
        status: raw.status,
        conclusion: raw.conclusion,
        details_url: raw.details_url,
        started_at: raw.started_at.unwrap_or_default(),
        completed_at: raw.completed_at,
    }
}

/// A timestamp in either shape GitHub uses.
///
/// A log line is stamped in RFC 3339 with a zone and fractional seconds
/// (`2026-09-26T19:12:21.6689146Z`), while a step's `started_at` arrives as
/// `2026-09-28 01:06:36` with a space and no zone. Both are UTC. The step
/// stamps are not used to split the log, only to label it.
fn parse_stamp(stamp: &str) -> Option<Timestamp> {
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(stamp) {
        return Some(parsed.with_timezone(&chrono::Utc));
    }
    chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|naive| naive.and_utc())
}

type Timestamp = chrono::DateTime<chrono::Utc>;

/// The leading stamp on a log line, and the message that follows it.
///
/// GitHub separates them with a space, so the first word is the stamp. A line
/// carrying only a stamp is blank output: it is still stamped, since the stamp
/// is the entire line, and its message is empty.
fn split_log_line(line: &str) -> (Option<Timestamp>, &str) {
    let (stamp, message) = line.split_once(' ').unwrap_or((line, ""));
    match parse_stamp(stamp) {
        Some(when) => (Some(when), message),
        // An unstamped continuation line is its own body, not a message whose
        // stamp went missing.
        None => (None, line),
    }
}

/// Whether a step is a real workflow step, as opposed to the runner's own
/// `Set up job` / `Complete job` and the `Post ...` teardown steps.
///
/// Only a real step gets a `##[group]Run <command>` line, and only the real
/// steps carry user output worth showing, so the markers line up with these
/// and with nothing else.
fn is_workflow_step(name: &str) -> bool {
    !matches!(name, "Set up job" | "Complete job") && !name.starts_with("Post ")
}

/// The name the collapsed teardown step is shown under.
const CLEANUP_STEP_NAME: &str = "Post job cleanup";

/// The first line of the teardown, which the runner writes once every step has
/// finished. It carries no marker, so this line is what the teardown is found
/// by. A run that never reaches it has no teardown to show.
const CLEANUP_MARKER: &str = "Post job cleanup.";

/// The last line a runner writes, as it tears down the processes the job
/// started. Without a teardown to hold it, this belongs to `Complete job`,
/// which is the step the runner is in when it writes the line.
const COMPLETE_MARKER: &str = "Cleaning up orphan processes";

/// Collapses the runner's trailing steps into one.
///
/// The runner emits a `Post <action>` step for every action that registered a
/// teardown, plus a `Complete job` step, and none of them write a marker, so
/// the teardown cannot be split between them. Left alone they are N steps that
/// are all empty while their output sits in the last workflow step, so they
/// become one step that holds all of it, whatever the workflow happens to have.
///
/// A run that never writes the teardown has no such output to gather, and
/// `Complete job` is then where the runner's own last line belongs, so the
/// steps are left as they are.
fn collapse_teardown(steps: Vec<GithubActionStep>, log: &str) -> Vec<GithubActionStep> {
    let Some(last_workflow) = steps.iter().rposition(|step| is_workflow_step(&step.name)) else {
        return steps;
    };
    let trailing = &steps[last_workflow + 1..];
    if trailing.len() < 2 || !log.lines().any(|line| has_message(line, CLEANUP_MARKER)) {
        return steps;
    }
    let mut collapsed: Vec<GithubActionStep> = steps[..=last_workflow].to_vec();
    collapsed.push(GithubActionStep {
        number: trailing[0].number,
        name: CLEANUP_STEP_NAME.to_string(),
        status: trailing[0].status.clone(),
        // The teardown only fails when the job was cancelled or the runner
        // itself broke, so the steps agreeing on success is the common case and
        // the first failure is the one worth reporting.
        conclusion: trailing.iter().find_map(|step| step.conclusion.clone()),
        started_at: trailing
            .iter()
            .filter_map(|step| step.started_at.clone())
            .min(),
        completed_at: trailing
            .iter()
            .filter_map(|step| step.completed_at.clone())
            .max(),
        log: String::new(),
        spans_by_line: Vec::new(),
    });
    collapsed
}

/// Routes each log line to the step that wrote it.
///
/// A `##[group]Run <command>` line is the first line of a step, and everything
/// up to the next one is that step's output. Those markers are the real
/// boundaries. The step stamps from the job endpoint are not usable on their
/// own: they are second-resolution while the lines carry sub-second stamps, so
/// a step that ran inside a single second has a window that other steps'
/// output lands inside, and a step's final second overlaps the next step's
/// first. In a real run `Oxfmt`, `Oxlint` and `Typecheck` all report
/// `19:12:40`, which leaves no way to tell their output apart by time.
///
/// The markers appear in the same order as the workflow steps, so the two are
/// zipped. The lines before the first marker are the runner's own setup, which
/// belongs to the `Set up job` step that precedes the first workflow step.
///
/// The teardown is found by the runner's own `Post job cleanup.` line rather
/// than by time, because the stamps cannot place it: the last workflow step and
/// the teardown both report the same second, and in a real run the last step's
/// final summary is written 130ms before the teardown begins.
fn slice_log_by_steps(
    steps: &[GithubActionStep],
    log: &str,
    table: &mut ansi::AnsiTable,
) -> Vec<StepLines> {
    let coloured = log.contains('\u{1b}');
    let mut buckets: Vec<StepLines> = steps.iter().map(|_| StepLines::default()).collect();

    // The steps a marker can claim, paired with the index of the bucket each
    // one writes to.
    let real: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| is_workflow_step(&step.name))
        .map(|(index, _)| index)
        .collect();
    if real.is_empty() {
        return buckets;
    }

    // The runner's own steps sit before and after the workflow steps and get
    // the output that no marker claims. A run with neither, which is a
    // composite action, falls back to the first workflow step.
    let setup = steps[..real[0]]
        .iter()
        .position(|step| step.name == "Set up job")
        .unwrap_or(0);
    // The step the teardown belongs to, which is the first of the runner's
    // trailing steps, and the step the runner's own last line belongs to when
    // the run never wrote a teardown.
    let trailing = (real[real.len() - 1] + 1..steps.len()).next();
    let complete = steps
        .iter()
        .position(|step| step.name == "Complete job")
        .unwrap_or(trailing.unwrap_or(setup));

    // The step the current marker opened, which starts on the setup step so
    // the runner's own output has somewhere to go.
    let mut current = setup;
    let mut markers = 0;
    for line in log.lines() {
        let (_, message) = split_log_line(line);
        // A marker opens the step it names. Every other `##[group]` title is a
        // runner section or an action's internal, which the runner shows as
        // its own collapsible heading and which says nothing about the step, so
        // those lines are hidden along with the `##[endgroup]` that closes
        // them.
        if let Some(title) = message.strip_prefix("##[group]") {
            if is_step_title(title) {
                current = *real.get(markers).unwrap_or(&current);
                markers += 1;
            }
            continue;
        }
        // The teardown starts at the runner's own line, so the switch happens
        // before the line is stored and that line opens the cleanup step. With
        // no teardown the runner's last line goes to `Complete job` instead,
        // which is the step it is in when it writes it.
        if has_message(line, CLEANUP_MARKER) {
            if let Some(trailing) = trailing {
                current = trailing;
            }
        } else if has_message(line, COMPLETE_MARKER) {
            current = complete;
        }
        // A line the runner echoed is shown without its `[command]` prefix and
        // in the runner's own colour, so it reads as the runner's line rather
        // than as something a build tool printed.
        if let Some(parsed) = ansi::parse_command_line(message, table) {
            buckets[current].push(parsed);
            continue;
        }
        let body = ansi::strip_workflow_commands(message);
        // A line that was blank apart from its stamp is blank output, so it
        // keeps its place and the step's spacing survives.
        if !body.is_empty() || message.is_empty() {
            buckets[current].push(ansi::parse_line(&body, table, coloured));
        }
    }
    buckets
}

/// Whether a `##[group]` title names a workflow step's command. The other
/// titles are the runner's own sections and an action's internals, and they
/// nest inside a step rather than starting one.
fn is_step_title(title: &str) -> bool {
    title.starts_with("Run ")
}

/// Whether a log line carries `marker` as its whole message, ignoring the
/// leading timestamp and any surrounding space.
fn has_message(line: &str, marker: &str) -> bool {
    split_log_line(line).1.trim() == marker
}

/// Splits a job log across the steps the job endpoint reported, from the
/// tab-separated dump the dev-only dump button produces.
///
/// The integration test drives the real split through this, so the fixture in
/// `tests/fixtures` is checked against the same code path the API uses rather
/// than against a reimplementation of it.
pub fn split_job_log_for_test(step_dump: &str, log: &str) -> Vec<GithubActionStep> {
    let steps: Vec<GithubActionStep> = step_dump
        .lines()
        .filter_map(|line| {
            let field = |key: &str| {
                line.split('\t')
                    .find_map(|part| part.strip_prefix(key))
                    .map(str::to_string)
            };
            Some(GithubActionStep {
                number: field("number: ")?.parse().ok()?,
                name: field("name: ")?,
                status: field("status: ").unwrap_or_default(),
                conclusion: field("conclusion: "),
                started_at: field("started_at: "),
                completed_at: field("completed_at: "),
                log: String::new(),
                spans_by_line: Vec::new(),
            })
        })
        .collect();
    let mut steps = collapse_teardown(steps, log);
    let mut table = ansi::AnsiTable::default();
    let buckets = slice_log_by_steps(&steps, log, &mut table);
    for (step, bucket) in steps.iter_mut().zip(buckets) {
        bucket.into_step(step);
    }
    steps
}

/// One step's log as parallel lines and span triples, so the frontend can
/// render it with the same component as a code fence.
#[derive(Default)]
struct StepLines {
    lines: Vec<String>,
    spans_by_line: Vec<Vec<u32>>,
}

impl StepLines {
    fn push(&mut self, parsed: ansi::ParsedLine) {
        self.lines.push(parsed.text);
        self.spans_by_line.push(parsed.spans);
    }

    fn into_step(self, step: &mut GithubActionStep) {
        step.log = self.lines.join("\n");
        step.spans_by_line = self.spans_by_line;
    }
}

fn map_check_annotation(raw: RawCheckAnnotation) -> GithubCheckAnnotation {
    GithubCheckAnnotation {
        path: raw.path.unwrap_or_default(),
        start_line: raw.start_line,
        end_line: raw.end_line,
        start_column: raw.start_column,
        annotation_level: raw.annotation_level.unwrap_or_default(),
        message: raw.message.unwrap_or_default(),
        title: raw.title.unwrap_or_default(),
    }
}

/// Annotations live on their own endpoint, not inside the check run, and are
/// paginated. A run that fails a suite reports one annotation per failed test.
async fn list_check_annotations(
    client: &reqwest::Client,
    authorization: &str,
    owner: &str,
    repo: &str,
    check_run_id: u64,
) -> Vec<RawCheckAnnotation> {
    let mut collected = Vec::new();
    for page in 1..=MAX_ISSUE_PAGES {
        let url = format!(
            "{API_ROOT}/repos/{owner}/{repo}/check-runs/{check_run_id}/annotations?per_page={ISSUES_PER_PAGE}&page={page}"
        );
        let Ok((body, headers)) = send_full(
            client
                .get(url)
                .header("Authorization", authorization.to_string()),
        )
        .await
        else {
            // Annotations are supplementary; a failed read leaves the run
            // readable rather than failing the whole detail call.
            break;
        };
        let Ok(parsed) = serde_json::from_str::<Vec<RawCheckAnnotation>>(&body) else {
            break;
        };
        let count = parsed.len();
        collected.extend(parsed);
        if !page_has_more(&headers, count) {
            break;
        }
    }
    collected
}

fn map_check_run_detail(
    raw: RawCheckRunDetail,
    annotations: Vec<RawCheckAnnotation>,
) -> GithubCheckRunDetail {
    let output = raw.output.unwrap_or(RawCheckRunOutput {
        title: None,
        summary: None,
        text: None,
        annotations_count: None,
    });
    GithubCheckRunDetail {
        run: map_check_run(raw.run),
        output: GithubCheckRunOutput {
            title: output.title.unwrap_or_default(),
            summary: output.summary.unwrap_or_default(),
            text: output.text.unwrap_or_default(),
            annotations_count: output.annotations_count.unwrap_or_default(),
        },
        annotations: annotations.into_iter().map(map_check_annotation).collect(),
    }
}

fn map_workflow_run(raw: RawWorkflowRun) -> GithubWorkflowRun {
    GithubWorkflowRun {
        id: raw.id,
        name: raw.name,
        event: raw.event,
        status: raw.status,
        conclusion: raw.conclusion,
        run_number: raw.run_number,
        head_branch: raw.head_branch,
        html_url: raw.html_url,
        created_at: raw.created_at.unwrap_or_default(),
    }
}

fn map_issue_user(raw: Option<RawIssueUser>) -> crate::api::github::GithubUser {
    let raw = raw.unwrap_or(RawIssueUser {
        login: String::new(),
        avatar_url: String::new(),
    });
    crate::api::github::GithubUser {
        login: raw.login,
        avatar_url: raw.avatar_url,
    }
}

fn map_issue_label(raw: &RawIssueLabel) -> crate::api::github::GithubLabel {
    crate::api::github::GithubLabel {
        name: raw.name.clone(),
        color: raw.color.clone(),
    }
}

/// The repository a search item belongs to. Search carries
/// `repository_url` but no repository name, and an item's web URL names the
/// repository two segments ahead of the `issues` or `pull` path segment.
fn search_item_repo_full_name(raw: &RawSearchIssue) -> String {
    raw.html_url
        .as_deref()
        .and_then(|url| {
            let parts: Vec<&str> = url.split('/').collect();
            let pos = parts
                .iter()
                .position(|part| *part == "issues" || *part == "pull")?;
            if pos >= 2 {
                Some(format!("{}/{}", parts[pos - 2], parts[pos - 1]))
            } else {
                None
            }
        })
        .or_else(|| {
            raw.repository_url.as_deref().and_then(|url| {
                let prefix = "https://api.github.com/repos/";
                url.strip_prefix(prefix)
                    .map(|rest| rest.trim_end_matches('/').to_owned())
            })
        })
        .unwrap_or_default()
}

fn map_search_issue_item(raw: RawSearchIssue) -> SearchIssueItem {
    let repo_full_name = search_item_repo_full_name(&raw);
    SearchIssueItem {
        number: raw.number,
        title: raw.title,
        state: raw.state,
        labels: raw.labels.iter().map(map_issue_label).collect(),
        comment_count: raw.comments,
        assignees: raw
            .assignees
            .into_iter()
            .map(|user| crate::api::github::GithubUser {
                login: user.login,
                avatar_url: user.avatar_url,
            })
            .collect(),
        author: raw.user.map(|user| user.login).unwrap_or_default(),
        updated_at: raw.updated_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
        repo_full_name,
    }
}

/// Pull requests off search items. `merged_at` is the only thing separating
/// a merged pull request from a merely closed one, since both report
/// `state: "closed"`. A repository-scoped search names the repo itself, since
/// the item URL carries no repository name for it.
fn map_search_pull_item(
    raw: RawSearchIssue,
    repo_full_name: Option<String>,
) -> GithubPullRequestListItem {
    let repo_full_name = match repo_full_name {
        Some(name) => name,
        None => search_item_repo_full_name(&raw),
    };
    GithubPullRequestListItem {
        number: raw.number,
        title: raw.title,
        state: raw.state,
        labels: raw.labels.iter().map(map_issue_label).collect(),
        comment_count: raw.comments,
        assignees: raw
            .assignees
            .into_iter()
            .map(|user| crate::api::github::GithubUser {
                login: user.login,
                avatar_url: user.avatar_url,
            })
            .collect(),
        author: map_issue_user(raw.user),
        updated_at: raw.updated_at.unwrap_or_default(),
        merged_at: raw
            .pull_request
            .and_then(|reference| reference.merged_at)
            .filter(|at| !at.is_empty()),
        html_url: raw.html_url.unwrap_or_default(),
        repo_full_name,
    }
}

/// `list_issues` and `list_pull_requests` hit the same endpoint and differ
/// only in the `is:` qualifier and the projection, so both build their query
/// here. Returns the raw items plus whether another page exists.
async fn search_repo(
    client: &reqwest::Client,
    token: String,
    terms: &[String],
    page: u32,
) -> std::result::Result<(Vec<RawSearchIssue>, bool), GitHubError> {
    let (body, headers) = send_full(
        client
            .get(format!("{API_ROOT}/search/issues"))
            .header("Authorization", bearer(&token))
            .query(&[
                ("q", terms.join(" ")),
                ("per_page", ISSUES_PER_PAGE.to_string()),
                ("page", page.to_string()),
                ("sort", "created".to_owned()),
                ("order", "desc".to_owned()),
            ]),
    )
    .await?;
    let raw: RawSearchIssueResponse = serde_json::from_str(&body).map_err(malformed)?;
    let has_more = page_has_more(&headers, raw.items.len());
    Ok((raw.items, has_more))
}

/// Qualifiers shared by both repo-scoped listings: scope to the repository,
/// narrow to one state when asked, and require every named label.
fn repo_search_terms(
    qualifier: &str,
    owner: &str,
    repo: &str,
    state: &str,
    labels: &[String],
) -> Vec<String> {
    let mut terms = vec![qualifier.to_owned(), format!("repo:{owner}/{repo}")];
    if state == "open" || state == "closed" {
        terms.push(format!("state:{state}"));
    }
    for label in labels {
        terms.push(format!("label:\"{label}\""));
    }
    terms
}

fn map_issue_detail(
    raw: RawIssue,
    participants: Vec<crate::api::github::GithubUser>,
) -> GithubIssueDetail {
    GithubIssueDetail {
        number: raw.number,
        title: raw.title,
        state: raw.state,
        body: raw.body.unwrap_or_default(),
        author: map_issue_user(raw.user),
        labels: raw.labels.iter().map(map_issue_label).collect(),
        assignees: raw
            .assignees
            .into_iter()
            .map(|user| crate::api::github::GithubUser {
                login: user.login,
                avatar_url: user.avatar_url,
            })
            .collect(),
        participants,
        created_at: raw.created_at.unwrap_or_default(),
        updated_at: raw.updated_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
    }
}

fn map_issue_comment(raw: RawIssueComment) -> GithubIssueComment {
    GithubIssueComment {
        id: raw.id,
        author: map_issue_user(raw.user),
        body: raw.body.unwrap_or_default(),
        created_at: raw.created_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
    }
}

/// Query pairs for listing issues. An empty `labels` param filters to
/// unlabeled issues on GitHub's side, so it is only sent when filtering.
/// Order-preserving dedup by login, dropping empty logins. `dedup_by`
/// only collapses adjacent entries, so it cannot be used here.
fn dedup_users(
    users: impl IntoIterator<Item = crate::api::github::GithubUser>,
) -> Vec<crate::api::github::GithubUser> {
    let mut seen = std::collections::HashSet::new();
    users
        .into_iter()
        .filter(|user| !user.login.is_empty() && seen.insert(user.login.clone()))
        .collect()
}

fn map_issue_event(raw: RawIssueEvent) -> GithubIssueEvent {
    let (actor, actor_avatar_url) = raw
        .actor
        .map(|user| (user.login, user.avatar_url))
        .unwrap_or_default();
    GithubIssueEvent {
        id: raw.id,
        kind: raw.event,
        actor,
        actor_avatar_url,
        created_at: raw.created_at.unwrap_or_default(),
        label: raw.label.as_ref().map(|label| label.name.clone()),
        label_color: raw.label.as_ref().map(|label| label.color.clone()),
        assignee: raw
            .assignee
            .or(raw.requested_reviewer)
            .map(|user| user.login),
    }
}

fn empty_pull_ref() -> GithubPullRequestRef {
    GithubPullRequestRef {
        r#ref: String::new(),
        sha: String::new(),
        label: String::new(),
    }
}

fn map_pull_ref(raw: Option<RawPullRef>) -> GithubPullRequestRef {
    raw.map(|reference| GithubPullRequestRef {
        r#ref: reference.git_ref,
        sha: reference.sha,
        label: reference.label,
    })
    .unwrap_or_else(empty_pull_ref)
}

/// GitHub sends `mergeable` as a nullable boolean, so the tri-state survives
/// the wire intact: null means the merge is still being computed and the UI
/// has to re-read before it can offer the merge button.
fn map_pull_detail(raw: RawPull) -> GithubPullRequestDetail {
    GithubPullRequestDetail {
        number: raw.number,
        title: raw.title,
        state: raw.state,
        body: raw.body.unwrap_or_default(),
        author: map_issue_user(raw.user),
        labels: raw.labels.iter().map(map_issue_label).collect(),
        assignees: raw
            .assignees
            .into_iter()
            .map(|user| crate::api::github::GithubUser {
                login: user.login,
                avatar_url: user.avatar_url,
            })
            .collect(),
        created_at: raw.created_at.unwrap_or_default(),
        updated_at: raw.updated_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
        draft: raw.draft,
        merged_at: raw.merged_at.filter(|at| !at.is_empty()),
        mergeable: raw.mergeable,
        mergeable_state: raw.mergeable_state.unwrap_or_default(),
        head: map_pull_ref(raw.head),
        base: map_pull_ref(raw.base),
        additions: raw.additions,
        deletions: raw.deletions,
        changed_files: raw.changed_files,
        commits: raw.commits,
    }
}

/// Commits come back oldest first, and a commit made outside GitHub has no
/// linked account, so the git author name stands in for the login.
fn map_pull_commit(raw: RawPullCommit) -> GithubPullRequestCommit {
    let detail = raw.commit.unwrap_or_else(|| RawCommitDetail {
        message: String::new(),
        author: None,
    });
    let signature = detail.author.unwrap_or_else(|| RawCommitSignature {
        name: String::new(),
        date: None,
    });
    let account = raw.author.map(|user| (user.login, user.avatar_url));
    let (login, avatar_url) = account.unwrap_or((signature.name, String::new()));
    GithubPullRequestCommit {
        sha: raw.sha,
        message: detail.message,
        author: crate::api::github::GithubUser { login, avatar_url },
        authored_at: signature.date.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
    }
}

fn map_pull_review(raw: RawPullReview) -> GithubPullRequestReview {
    GithubPullRequestReview {
        id: raw.id,
        author: map_issue_user(raw.user),
        state: raw.state,
        body: raw.body.unwrap_or_default(),
        submitted_at: raw.submitted_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
    }
}

fn map_pull_review_comment(raw: RawPullReviewComment) -> GithubPullRequestReviewComment {
    GithubPullRequestReviewComment {
        id: raw.id,
        author: map_issue_user(raw.user),
        body: raw.body.unwrap_or_default(),
        path: raw.path,
        line: raw.line,
        diff_hunk: raw.diff_hunk,
        created_at: raw.created_at.unwrap_or_default(),
        html_url: raw.html_url.unwrap_or_default(),
        in_reply_to_id: raw.in_reply_to_id,
        pull_request_review_id: raw.pull_request_review_id,
    }
}

/// Converts an API subject URL into a best-effort web URL, so rows link
/// out without a second round trip. Only types with a proven static
/// mapping qualify; releases (addressed by tag, not id) and check suites
/// (no web equivalent) yield `None` and resolve on click instead.
fn subject_html_url(api_url: Option<&str>, subject_type: &str) -> Option<String> {
    match subject_type {
        "Issue" | "PullRequest" | "Commit" | "Discussion" => {}
        _ => return None,
    }
    let url = api_url?;
    let rest = url.strip_prefix("https://api.github.com/repos/")?;
    let mut html = String::from("https://github.com/");
    html.push_str(
        &rest
            .replace("/pulls/", "/pull/")
            .replace("/commits/", "/commit/"),
    );
    Some(html)
}

fn map_notification(raw: RawNotification) -> GithubNotification {
    let subject_url = raw.subject.as_ref().and_then(|s| s.url.as_deref());
    let subject_type = raw
        .subject
        .as_ref()
        .and_then(|s| s.kind.clone())
        .unwrap_or_else(|| "Unknown".to_owned());
    let repo_full_name = raw
        .repository
        .as_ref()
        .and_then(|r| r.full_name.clone())
        .unwrap_or_default();
    GithubNotification {
        id: raw.id,
        unread: raw.unread,
        reason: raw.reason.unwrap_or_else(|| "subscribed".to_owned()),
        subject_title: raw
            .subject
            .as_ref()
            .and_then(|s| s.title.clone())
            .unwrap_or_default(),
        subject_type: subject_type.clone(),
        repo_full_name,
        html_url: subject_html_url(subject_url, &subject_type),
        subject_url: subject_url.map(str::to_owned),
        updated_at: raw.updated_at.unwrap_or_default(),
    }
}

#[derive(Debug, Deserialize)]
struct RawApiError {
    #[serde(default)]
    message: Option<String>,
}

const API_ROOT: &str = "https://api.github.com";
const LOGIN_ROOT: &str = "https://github.com";
const API_VERSION: &str = "2022-11-28";
const DEVICE_GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:device_code";
/// GitHub's maximum page size. Full pages keep `has_more` true as a
/// fallback when the `Link` header is missing.
const NOTIFICATIONS_PER_PAGE: u32 = 100;
const ISSUES_PER_PAGE: u32 = 100;
const MAX_ISSUE_PAGES: u32 = 10;

pub struct HttpGithubApi {
    client: reqwest::Client,
    /// The job log endpoint answers with a 302 to a blob store that rejects
    /// the API auth header, so the redirect is followed by hand.
    no_redirect: reqwest::Client,
}

impl HttpGithubApi {
    pub fn new(timeout: Duration) -> Self {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("gitau/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds with rustls-tls");
        let no_redirect = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("gitau/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("reqwest client builds with rustls-tls");
        Self {
            client,
            no_redirect,
        }
    }
}

impl Default for HttpGithubApi {
    fn default() -> Self {
        Self::new(Duration::from_secs(30))
    }
}

/// Fetches a job's log. The endpoint answers a 302 to a blob store holding
/// plain text: it rejects a `text/plain` accept header with 415, and the
/// redirect target rejects the API auth header, so the hop is made by hand.
async fn fetch_job_log(
    no_redirect: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
    job_id: u64,
) -> std::result::Result<String, GitHubError> {
    let api_url = format!("{API_ROOT}/repos/{owner}/{repo}/actions/jobs/{job_id}/logs");
    let redirect = no_redirect
        .get(&api_url)
        .header("Accept", "application/vnd.github+json")
        .header("Authorization", bearer(token))
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await?;
    let status = redirect.status();
    let location = redirect
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let Some(location) = location else {
        let body = redirect.text().await.unwrap_or_default();
        return Err(api_error(status.as_u16(), &body));
    };
    let blob = no_redirect.get(location).send().await?;
    let blob_status = blob.status();
    if !blob_status.is_success() {
        let body = blob.text().await.unwrap_or_default();
        return Err(api_error(blob_status.as_u16(), &body));
    }
    Ok(blob.text().await?)
}

async fn send(request: reqwest::RequestBuilder) -> std::result::Result<String, GitHubError> {
    send_full(request).await.map(|(body, _)| body)
}

async fn send_full(
    request: reqwest::RequestBuilder,
) -> std::result::Result<(String, reqwest::header::HeaderMap), GitHubError> {
    let response = request
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .text()
        .await
        .map_err(|error| GitHubError::Network {
            message: error.to_string(),
        })?;
    if status.is_success() {
        Ok((body, headers))
    } else {
        Err(api_error(status.as_u16(), &body))
    }
}

/// True when another page likely exists: the `Link` header advertises it,
/// or the page came back full (headers can go missing behind proxies).
fn page_has_more(headers: &reqwest::header::HeaderMap, count: usize) -> bool {
    has_next_page(headers) || count == NOTIFICATIONS_PER_PAGE as usize
}

/// True when the response `Link` header advertises a next page, per
/// RFC 8288 (`<url>; rel="next", ...`).
fn has_next_page(headers: &reqwest::header::HeaderMap) -> bool {
    let Some(link) = headers.get(reqwest::header::LINK) else {
        return false;
    };
    let Ok(link) = link.to_str() else {
        return false;
    };
    link.split(',').any(|part| {
        let mut segments = part.split(';').map(str::trim);
        let _url = segments.next();
        segments.any(|param| param.eq_ignore_ascii_case(r#"rel="next""#))
    })
}

fn api_error(status: u16, body: &str) -> GitHubError {
    let message = serde_json::from_str::<RawApiError>(body)
        .ok()
        .and_then(|raw| raw.message)
        .unwrap_or_else(|| format!("HTTP {status}"));
    match status {
        401 => GitHubError::Unauthorized,
        403 => GitHubError::Forbidden { message },
        404 => GitHubError::NotFound { message },
        _ => GitHubError::Api { status, message },
    }
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

fn malformed(error: serde_json::Error) -> GitHubError {
    GitHubError::MalformedResponse {
        message: error.to_string(),
    }
}

fn internal(error: serde_json::Error) -> GitHubError {
    GitHubError::Internal {
        message: error.to_string(),
    }
}

impl GithubApi for HttpGithubApi {
    fn request_device_code(
        &self,
        client_id: &str,
        scopes: &str,
    ) -> GithubFuture<DeviceCodeResponse> {
        let client = self.client.clone();
        let client_id = client_id.to_owned();
        let scopes = scopes.to_owned();
        Box::pin(async move {
            let form = [("client_id", client_id), ("scope", scopes)];
            let body = send(
                client
                    .post(format!("{LOGIN_ROOT}/login/device/code"))
                    .form(&form),
            )
            .await?;
            parse_device_code(&body).map_err(|message| GitHubError::MalformedResponse { message })
        })
    }

    fn poll_token(&self, client_id: &str, device_code: &str) -> GithubFuture<TokenPoll> {
        let client = self.client.clone();
        let client_id = client_id.to_owned();
        let device_code = device_code.to_owned();
        Box::pin(async move {
            let form = [
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", DEVICE_GRANT_TYPE.to_owned()),
            ];
            let body = send(
                client
                    .post(format!("{LOGIN_ROOT}/login/oauth/access_token"))
                    .form(&form),
            )
            .await?;
            parse_token_poll(&body).map_err(|message| GitHubError::MalformedResponse { message })
        })
    }

    fn authenticated_user(&self, token: &str) -> GithubFuture<AccountProfile> {
        let client = self.client.clone();
        let authorization = bearer(token);
        Box::pin(async move {
            let body = send(
                client
                    .get(format!("{API_ROOT}/user"))
                    .header("Authorization", authorization),
            )
            .await?;
            let raw: RawUser = serde_json::from_str(&body).map_err(malformed)?;
            Ok(AccountProfile {
                login: raw.login,
                name: raw.name,
                avatar_url: raw.avatar_url.unwrap_or_default(),
                html_url: raw.html_url.unwrap_or_default(),
                scopes: vec![],
                connected_at_ms: 0,
            })
        })
    }

    fn list_orgs(&self, token: &str) -> GithubFuture<Vec<GithubOrg>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        Box::pin(async move {
            let body = send(
                client
                    .get(format!("{API_ROOT}/user/orgs"))
                    .header("Authorization", authorization),
            )
            .await?;
            let raw: Vec<RawOrg> = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw
                .into_iter()
                .map(|org| GithubOrg {
                    login: org.login,
                    avatar_url: org.avatar_url,
                })
                .collect())
        })
    }

    fn create_repository(
        &self,
        token: &str,
        owner: Option<&str>,
        body: &CreateRepoBody,
    ) -> GithubFuture<CreatedRepository> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = match owner {
            Some(owner) => format!("{API_ROOT}/orgs/{owner}/repos"),
            None => format!("{API_ROOT}/user/repos"),
        };
        let payload = serde_json::to_vec(body).map_err(internal);
        Box::pin(async move {
            let payload = payload?;
            let text = send(
                client
                    .post(url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(payload),
            )
            .await?;
            let raw: RawRepo = serde_json::from_str(&text).map_err(malformed)?;
            Ok(CreatedRepository {
                full_name: raw.full_name,
                html_url: raw.html_url.unwrap_or_default(),
                default_branch: raw.default_branch,
                clone_url: raw.clone_url,
            })
        })
    }

    fn list_notifications(&self, token: &str, page: u32) -> GithubFuture<NotificationPage> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let page = page.max(1);
        Box::pin(async move {
            let (body, headers) = send_full(
                client
                    .get(format!(
                        "{API_ROOT}/notifications?all=true&participating=false&per_page={NOTIFICATIONS_PER_PAGE}&page={page}"
                    ))
                    .header("Authorization", authorization),
            )
            .await?;
            let raw: Vec<RawNotification> = serde_json::from_str(&body).map_err(malformed)?;
            let notifications: Vec<GithubNotification> =
                raw.into_iter().map(map_notification).collect();
            let has_more = page_has_more(&headers, notifications.len());
            Ok(NotificationPage {
                notifications,
                page,
                has_more,
            })
        })
    }

    fn mark_notification_read(&self, token: &str, thread_id: &str) -> GithubFuture<()> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/notifications/threads/{thread_id}");
        Box::pin(async move {
            send(client.patch(url).header("Authorization", authorization)).await?;
            Ok(())
        })
    }

    fn mark_all_notifications_read(&self, token: &str) -> GithubFuture<()> {
        let client = self.client.clone();
        let authorization = bearer(token);
        Box::pin(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let body = serde_json::json!({
                "last_read_at": chrono_lite_rfc3339(now).to_string()
            });
            send(
                client
                    .put(format!("{API_ROOT}/notifications"))
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(body.to_string()),
            )
            .await?;
            Ok(())
        })
    }

    fn fetch_subject_html_url(
        &self,
        token: &str,
        subject_url: &str,
    ) -> GithubFuture<Option<String>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let subject_url = subject_url.to_owned();
        Box::pin(async move {
            if !subject_url.starts_with("https://api.github.com/") {
                return Err(GitHubError::Internal {
                    message: "refusing to fetch a non-GitHub subject URL".into(),
                });
            }
            let body = match send(
                client
                    .get(subject_url)
                    .header("Authorization", authorization),
            )
            .await
            {
                Ok(body) => body,
                // The subject is gone; the row falls back to the repo page.
                Err(GitHubError::NotFound { .. }) => return Ok(None),
                Err(error) => return Err(error),
            };
            let raw: RawSubject = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw.html_url.filter(|url| !url.is_empty()))
        })
    }

    fn list_issues(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: &str,
        labels: &[String],
        page: u32,
    ) -> GithubFuture<SearchIssuePage> {
        let client = self.client.clone();
        let token = token.to_owned();
        let terms = repo_search_terms("is:issue", owner, repo, state, labels);
        let repo_full_name = format!("{owner}/{repo}");
        let page = page.max(1);
        Box::pin(async move {
            let (items, has_more) = search_repo(&client, token, &terms, page).await?;
            let items: Vec<SearchIssueItem> = items
                .into_iter()
                .map(|item| {
                    let labels: Vec<GithubLabel> =
                        item.labels.iter().map(map_issue_label).collect();
                    let assignees: Vec<GithubUser> = item
                        .assignees
                        .into_iter()
                        .map(|user| GithubUser {
                            login: user.login,
                            avatar_url: user.avatar_url,
                        })
                        .collect();
                    SearchIssueItem {
                        number: item.number,
                        title: item.title,
                        state: item.state,
                        labels,
                        comment_count: item.comments,
                        assignees,
                        author: item.user.map(|user| user.login).unwrap_or_default(),
                        updated_at: item.updated_at.unwrap_or_default(),
                        html_url: item.html_url.unwrap_or_default(),
                        repo_full_name: repo_full_name.clone(),
                    }
                })
                .collect();
            Ok(SearchIssuePage {
                items,
                page,
                has_more,
            })
        })
    }

    fn list_pull_requests(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        state: &str,
        labels: &[String],
        page: u32,
    ) -> GithubFuture<SearchPullRequestPage> {
        let client = self.client.clone();
        let token = token.to_owned();
        let terms = repo_search_terms("is:pr", owner, repo, state, labels);
        let repo_full_name = format!("{owner}/{repo}");
        let page = page.max(1);
        Box::pin(async move {
            let (items, has_more) = search_repo(&client, token, &terms, page).await?;
            let items = items
                .into_iter()
                .map(|item| map_search_pull_item(item, Some(repo_full_name.clone())))
                .collect();
            Ok(SearchPullRequestPage {
                items,
                page,
                has_more,
            })
        })
    }

    fn search_issues(&self, token: &str, page: u32) -> GithubFuture<SearchIssuePage> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let page = page.max(1);
        Box::pin(async move {
            let (body, headers) = send_full(
                client
                    .get(format!(
                        "{API_ROOT}/search/issues?q=is:issue+involves:@me+sort:updated-desc&per_page={ISSUES_PER_PAGE}&page={page}"
                    ))
                    .header("Authorization", authorization),
            )
            .await?;
            let raw: RawSearchIssueResponse = serde_json::from_str(&body).map_err(malformed)?;
            let items: Vec<SearchIssueItem> =
                raw.items.into_iter().map(map_search_issue_item).collect();
            let has_more = page_has_more(&headers, items.len());
            Ok(SearchIssuePage {
                items,
                page,
                has_more,
            })
        })
    }

    fn search_pull_requests(&self, token: &str, page: u32) -> GithubFuture<SearchPullRequestPage> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let page = page.max(1);
        Box::pin(async move {
            let (body, headers) = send_full(
                client
                    .get(format!(
                        "{API_ROOT}/search/issues?q=is:pr+involves:@me+sort:updated-desc&per_page={ISSUES_PER_PAGE}&page={page}"
                    ))
                    .header("Authorization", authorization),
            )
            .await?;
            let raw: RawSearchIssueResponse = serde_json::from_str(&body).map_err(malformed)?;
            let items: Vec<GithubPullRequestListItem> = raw
                .items
                .into_iter()
                .map(|item| map_search_pull_item(item, None))
                .collect();
            let has_more = page_has_more(&headers, items.len());
            Ok(SearchPullRequestPage {
                items,
                page,
                has_more,
            })
        })
    }

    fn get_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<GithubPullRequestDetail> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}");
        Box::pin(async move {
            let body = send(client.get(url).header("Authorization", authorization)).await?;
            let raw: RawPull = serde_json::from_str(&body).map_err(malformed)?;
            Ok(map_pull_detail(raw))
        })
    }

    fn list_pull_reviews(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubPullRequestReview>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}/reviews");
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let (body, headers) = send_full(
                    client
                        .get(&url)
                        .header("Authorization", authorization.clone())
                        .query(&[
                            ("per_page", ISSUES_PER_PAGE.to_string()),
                            ("page", page.to_string()),
                        ]),
                )
                .await?;
                let raw: Vec<RawPullReview> = serde_json::from_str(&body).map_err(malformed)?;
                let has_more = page_has_more(&headers, raw.len());
                items.extend(raw.into_iter().map(map_pull_review));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn list_pull_commits(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubPullRequestCommit>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}/commits");
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let (body, headers) = send_full(
                    client
                        .get(&url)
                        .header("Authorization", authorization.clone())
                        .query(&[
                            ("per_page", ISSUES_PER_PAGE.to_string()),
                            ("page", page.to_string()),
                        ]),
                )
                .await?;
                let raw: Vec<RawPullCommit> = serde_json::from_str(&body).map_err(malformed)?;
                let has_more = page_has_more(&headers, raw.len());
                items.extend(raw.into_iter().map(map_pull_commit));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn list_pull_review_comments(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubPullRequestReviewComment>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}/comments");
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let (body, headers) = send_full(
                    client
                        .get(&url)
                        .header("Authorization", authorization.clone())
                        .query(&[
                            ("per_page", ISSUES_PER_PAGE.to_string()),
                            ("page", page.to_string()),
                        ]),
                )
                .await?;
                let raw: Vec<RawPullReviewComment> =
                    serde_json::from_str(&body).map_err(malformed)?;
                let has_more = page_has_more(&headers, raw.len());
                items.extend(raw.into_iter().map(map_pull_review_comment));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn update_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdatePullRequestBody,
    ) -> GithubFuture<GithubPullRequestDetail> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}");
        let payload = serde_json::to_vec(body).map_err(internal);
        Box::pin(async move {
            let response = send(
                client
                    .patch(url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(payload?),
            )
            .await?;
            let raw: RawPull = serde_json::from_str(&response).map_err(malformed)?;
            Ok(map_pull_detail(raw))
        })
    }

    fn merge_pull(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &MergePullRequestBody,
    ) -> GithubFuture<MergePullRequestResult> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/pulls/{number}/merge");
        let payload = serde_json::to_vec(&MergePullPayload::from(body)).map_err(internal);
        Box::pin(async move {
            let response = send(
                client
                    .put(url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(payload?),
            )
            .await?;
            let raw: RawMergeResult = serde_json::from_str(&response).map_err(malformed)?;
            Ok(MergePullRequestResult {
                sha: raw.sha,
                merged: raw.merged,
                message: raw.message,
            })
        })
    }

    fn list_check_runs(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> GithubFuture<Vec<GithubCheckRun>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let sha = sha.to_owned();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/commits/{sha}/check-runs");
        Box::pin(async move {
            let body = send(
                client
                    .get(url)
                    .header("Authorization", authorization)
                    .query(&[("per_page", ISSUES_PER_PAGE.to_string())]),
            )
            .await?;
            let raw: RawCheckRunsResponse = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw.check_runs.into_iter().map(map_check_run).collect())
        })
    }

    fn get_check_run(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        check_run_id: u64,
    ) -> GithubFuture<GithubCheckRunDetail> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/check-runs/{check_run_id}");
        let owner = owner.to_string();
        let repo = repo.to_string();
        Box::pin(async move {
            let body = send(
                client
                    .get(url)
                    .header("Authorization", authorization.clone()),
            )
            .await?;
            let raw: RawCheckRunDetail = serde_json::from_str(&body).map_err(malformed)?;
            let annotations =
                list_check_annotations(&client, &authorization, &owner, &repo, check_run_id).await;
            Ok(map_check_run_detail(raw, annotations))
        })
    }

    fn get_check_run_log(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        check_run_id: u64,
    ) -> GithubFuture<GithubCheckRunLog> {
        let client = self.client.clone();
        let no_redirect = self.no_redirect.clone();
        let token = token.to_string();
        let owner = owner.to_string();
        let repo = repo.to_string();
        Box::pin(async move {
            // A GitHub Actions check run and its job share one id, so the
            // check run id addresses the job directly.
            let job_id = check_run_id;
            let job_url = format!("{API_ROOT}/repos/{owner}/{repo}/actions/jobs/{job_id}");
            let job_body =
                send(client.get(job_url).header("Authorization", bearer(&token))).await?;
            let job: RawActionJob = serde_json::from_str(&job_body).map_err(malformed)?;

            let log = match fetch_job_log(&no_redirect, &token, &owner, &repo, job_id).await {
                Ok(text) => text,
                Err(error) => {
                    return Ok(GithubCheckRunLog {
                        unavailable: Some(error.to_string()),
                        ..Default::default()
                    });
                }
            };

            let steps: Vec<GithubActionStep> = job
                .steps
                .into_iter()
                .map(|step| GithubActionStep {
                    number: step.number,
                    name: step.name.unwrap_or_default(),
                    status: step.status.unwrap_or_default(),
                    conclusion: step.conclusion,
                    started_at: step.started_at,
                    completed_at: step.completed_at,
                    log: String::new(),
                    spans_by_line: Vec::new(),
                })
                .collect();
            let mut steps = collapse_teardown(steps, &log);
            let mut table = ansi::AnsiTable::default();
            let buckets = slice_log_by_steps(&steps, &log, &mut table);
            for (step, bucket) in steps.iter_mut().zip(buckets) {
                bucket.into_step(step);
            }
            Ok(GithubCheckRunLog {
                steps,
                styles: table.styles().to_vec(),
                raw: log,
                unavailable: None,
            })
        })
    }

    fn list_workflow_runs(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        sha: &str,
    ) -> GithubFuture<Vec<GithubWorkflowRun>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let sha = sha.to_owned();
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/actions/runs");
        Box::pin(async move {
            let body = send(
                client
                    .get(url)
                    .header("Authorization", authorization)
                    .query(&[("head_sha", sha), ("per_page", "50".to_owned())]),
            )
            .await?;
            let raw: RawWorkflowRunsResponse = serde_json::from_str(&body).map_err(malformed)?;
            Ok(raw
                .workflow_runs
                .into_iter()
                .map(map_workflow_run)
                .collect())
        })
    }

    fn get_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<GithubIssueDetail> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}");
        Box::pin(async move {
            let body = send(client.get(url).header("Authorization", authorization)).await?;
            let raw: RawIssue = serde_json::from_str(&body).map_err(malformed)?;
            // The issue endpoint has no participants list; the detail
            // carries author + assignees and the UI merges comment authors.
            let mut participants = vec![
                raw.user
                    .as_ref()
                    .map(|user| crate::api::github::GithubUser {
                        login: user.login.clone(),
                        avatar_url: user.avatar_url.clone(),
                    })
                    .unwrap_or_default(),
            ];
            participants.extend(
                raw.assignees
                    .iter()
                    .map(|user| crate::api::github::GithubUser {
                        login: user.login.clone(),
                        avatar_url: user.avatar_url.clone(),
                    }),
            );
            Ok(map_issue_detail(raw, dedup_users(participants)))
        })
    }

    fn list_issue_comments(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubIssueComment>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}/comments");
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let (body, headers) = send_full(
                    client
                        .get(&url)
                        .header("Authorization", authorization.clone())
                        .query(&[
                            ("per_page", ISSUES_PER_PAGE.to_string()),
                            ("page", page.to_string()),
                        ]),
                )
                .await?;
                let raw: Vec<RawIssueComment> = serde_json::from_str(&body).map_err(malformed)?;
                let has_more = page_has_more(&headers, raw.len());
                items.extend(raw.into_iter().map(map_issue_comment));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn list_issue_events(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
    ) -> GithubFuture<Vec<GithubIssueEvent>> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}/events");
        Box::pin(async move {
            let mut items = Vec::new();
            for page in 1..=MAX_ISSUE_PAGES {
                let (body, headers) = send_full(
                    client
                        .get(&url)
                        .header("Authorization", authorization.clone())
                        .query(&[
                            ("per_page", ISSUES_PER_PAGE.to_string()),
                            ("page", page.to_string()),
                        ]),
                )
                .await?;
                let raw: Vec<RawIssueEvent> = serde_json::from_str(&body).map_err(malformed)?;
                let has_more = page_has_more(&headers, raw.len());
                items.extend(raw.into_iter().map(map_issue_event));
                if !has_more {
                    break;
                }
            }
            Ok(items)
        })
    }

    fn create_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &str,
    ) -> GithubFuture<GithubIssueComment> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}/comments");
        let payload = serde_json::json!({ "body": body }).to_string();
        Box::pin(async move {
            let text = send(
                client
                    .post(url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(payload),
            )
            .await?;
            let raw: RawIssueComment = serde_json::from_str(&text).map_err(malformed)?;
            Ok(map_issue_comment(raw))
        })
    }

    fn update_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        number: u64,
        body: &UpdateIssueBody,
    ) -> GithubFuture<GithubIssueDetail> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/{number}");
        let payload = serde_json::to_vec(body).map_err(internal);
        Box::pin(async move {
            let payload = payload?;
            let text = send(
                client
                    .patch(url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(payload),
            )
            .await?;
            let raw: RawIssue = serde_json::from_str(&text).map_err(malformed)?;
            let participants =
                dedup_users(
                    raw.assignees
                        .iter()
                        .map(|user| crate::api::github::GithubUser {
                            login: user.login.clone(),
                            avatar_url: user.avatar_url.clone(),
                        }),
                );
            Ok(map_issue_detail(raw, participants))
        })
    }

    fn create_issue(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        title: &str,
        body: Option<&str>,
        labels: &[String],
    ) -> GithubFuture<GithubIssueDetail> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues");
        let mut payload = serde_json::json!({ "title": title });
        if let Some(body) = body {
            payload["body"] = serde_json::Value::String(body.to_owned());
        }
        if !labels.is_empty() {
            payload["labels"] = serde_json::json!(labels);
        }
        let payload = payload.to_string();
        Box::pin(async move {
            let text = send(
                client
                    .post(url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(payload),
            )
            .await?;
            let raw: RawIssue = serde_json::from_str(&text).map_err(malformed)?;
            Ok(map_issue_detail(raw, vec![]))
        })
    }

    fn update_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        comment_id: u64,
        body: &str,
    ) -> GithubFuture<GithubIssueComment> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/comments/{comment_id}");
        let payload = serde_json::json!({ "body": body }).to_string();
        Box::pin(async move {
            let text = send(
                client
                    .patch(url)
                    .header("Authorization", authorization)
                    .header("Content-Type", "application/json")
                    .body(payload),
            )
            .await?;
            let raw: RawIssueComment = serde_json::from_str(&text).map_err(malformed)?;
            Ok(map_issue_comment(raw))
        })
    }

    fn delete_issue_comment(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
        comment_id: u64,
    ) -> GithubFuture<()> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}/issues/comments/{comment_id}");
        Box::pin(async move {
            send(client.delete(url).header("Authorization", authorization)).await?;
            Ok(())
        })
    }

    fn repo_permissions(
        &self,
        token: &str,
        owner: &str,
        repo: &str,
    ) -> GithubFuture<GithubRepoPermissions> {
        let client = self.client.clone();
        let authorization = bearer(token);
        let url = format!("{API_ROOT}/repos/{owner}/{repo}");
        Box::pin(async move {
            let body = send(client.get(url).header("Authorization", authorization)).await?;
            let raw: RawPermissionsEnvelope = serde_json::from_str(&body).map_err(malformed)?;
            let permissions = raw.permissions.unwrap_or_default();
            Ok(GithubRepoPermissions {
                push: permissions.push,
                admin: permissions.admin,
            })
        })
    }
}

/// Minimal UTC timestamp formatter, avoiding a chrono dependency for one
/// `last_read_at` field.
fn chrono_lite_rfc3339(epoch_secs: u64) -> String {
    let days = epoch_secs / 86_400;
    let rem = epoch_secs % 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

/// Howard Hinnant's civil_from_days, valid for all post-1970 dates.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(name: &str, start: &str, end: &str) -> GithubActionStep {
        GithubActionStep {
            number: 1,
            name: name.to_string(),
            status: "completed".into(),
            conclusion: Some("success".into()),
            started_at: Some(start.to_string()),
            completed_at: Some(end.to_string()),
            log: String::new(),
            spans_by_line: Vec::new(),
        }
    }

    /// Slices and renders, since the point of the change is what a step's log
    /// looks like rather than which bucket a line lands in. The teardown is
    /// collapsed first, so this drives the same path the API does.
    fn slice(steps: &[GithubActionStep], log: &str) -> Vec<GithubActionStep> {
        let mut table = ansi::AnsiTable::default();
        let mut steps = collapse_teardown(steps.to_vec(), log);
        let buckets = slice_log_by_steps(&steps, log, &mut table);
        for (step, bucket) in steps.iter_mut().zip(buckets) {
            bucket.into_step(step);
        }
        steps
    }

    /// The real step list and log from the run whose split was wrong, trimmed
    /// to the lines that matter. The step stamps are copied verbatim,
    /// including the three steps that all report `19:12:40`, because that
    /// collision is what a timestamp-based split cannot resolve.
    const REAL_LOG: &str = concat!(
        // Setup output, before any step marker.
        "2026-09-26T19:12:21.3522162Z Current runner version: '2.337.0'\n",
        "2026-09-26T19:12:21.3549723Z ##[group]Runner Image Provisioner\n",
        "2026-09-26T19:12:21.3550662Z Hosted Compute Agent\n",
        "2026-09-26T19:12:21.3554915Z ##[endgroup]\n",
        // Step 1, checkout.
        "2026-09-26T19:12:22.0354624Z ##[group]Run actions/checkout@v7\n",
        "2026-09-26T19:12:22.0355590Z with:\n",
        "2026-09-26T19:12:22.0356326Z   repository: sn0w12/gitau\n",
        "2026-09-26T19:12:22.0367279Z ##[endgroup]\n",
        "2026-09-26T19:12:22.6990933Z ##[group]Checking out the ref\n",
        "2026-09-26T19:12:22.7477192Z   git switch -\n",
        "2026-09-26T19:12:22.7480774Z HEAD is now at e27a631\n",
        "2026-09-26T19:12:22.7484497Z ##[endgroup]\n",
        "2026-09-26T19:12:22.7528669Z [command]/usr/bin/git log -1 --format=%H\n",
        "2026-09-26T19:12:22.8068627Z ##[group]Run actions/setup-node@v7\n",
        "2026-09-26T19:12:22.8070163Z with:\n",
        "2026-09-26T19:12:22.8088013Z ##[endgroup]\n",
        "2026-09-26T19:12:23.3945855Z ##[group]Environment details\n",
        "2026-09-26T19:12:23.8114175Z node: v24.21.0\n",
        "2026-09-26T19:12:23.8114175Z ##[endgroup]\n",
        // Step 2, the install.
        "2026-09-26T19:12:24.9518805Z ##[group]Run npm ci\n",
        "2026-09-26T19:12:24.9808755Z ##[endgroup]\n",
        "2026-09-26T19:12:25.1000000Z \u{1b}[36;1mnpm ci\u{1b}[0m\n",
        "2026-09-26T19:12:38.9722213Z found 0 vulnerabilities\n",
        // Step 3, oxfmt. Its last line is stamped 40.153, inside the single
        // second oxlint reports, so a timestamp split gives it to oxlint.
        "2026-09-26T19:12:39.0831110Z ##[group]Run npm run format:check\n",
        "2026-09-26T19:12:39.0900212Z ##[endgroup]\n",
        "2026-09-26T19:12:39.2514996Z Checking formatting...\n",
        "2026-09-26T19:12:40.1530110Z All matched files use the correct format.\n",
        // Step 4, oxlint, whose whole window is that same second.
        "2026-09-26T19:12:40.1848978Z ##[group]Run npm run lint\n",
        "2026-09-26T19:12:40.1914984Z ##[endgroup]\n",
        "2026-09-26T19:12:40.5859108Z Finished in 245ms on 292 files with 128 rules.\n",
        // Step 5, typecheck.
        "2026-09-26T19:12:40.6040189Z ##[group]Run npm run typecheck\n",
        "2026-09-26T19:12:40.6106442Z ##[endgroup]\n",
        "2026-09-26T19:12:41.1000000Z tsc --noEmit\n",
        // Step 6, the tests, which start after typecheck's reported end.
        "2026-09-26T19:12:43.2560593Z ##[group]Run npm test\n",
        "2026-09-26T19:12:43.2624553Z ##[endgroup]\n",
        "2026-09-26T19:12:43.4000000Z Test Files  55 passed (55)\n",
        // The teardown, which the runner opens with its own line.
        "2026-09-26T19:13:20.1866212Z Post job cleanup.\n",
        "2026-09-26T19:13:20.3017463Z Cache hit occurred on the primary key.\n",
        "2026-09-26T19:13:20.5709221Z Cleaning up orphan processes\n",
    );

    /// The real step list for `REAL_LOG`, with the stamps GitHub reported for
    /// it copied verbatim. The three steps reporting `19:12:40` are the ones a
    /// timestamp split cannot separate, and the `Set up job` / `Post ...` /
    /// `Complete job` steps are the ones that carry no marker.
    fn real_steps() -> Vec<GithubActionStep> {
        vec![
            step("Set up job", "2026-09-26T19:12:21Z", "2026-09-26T19:12:21Z"),
            step(
                "Run actions/checkout@v7",
                "2026-09-26T19:12:21Z",
                "2026-09-26T19:12:22Z",
            ),
            step("Setup Node", "2026-09-26T19:12:22Z", "2026-09-26T19:12:24Z"),
            step(
                "Install frontend dependencies",
                "2026-09-26T19:12:24Z",
                "2026-09-26T19:12:39Z",
            ),
            step("Oxfmt", "2026-09-26T19:12:39Z", "2026-09-26T19:12:40Z"),
            step("Oxlint", "2026-09-26T19:12:40Z", "2026-09-26T19:12:40Z"),
            step("Typecheck", "2026-09-26T19:12:40Z", "2026-09-26T19:12:43Z"),
            step(
                "Frontend tests",
                "2026-09-26T19:12:43Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Post Setup Node",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Post Run actions/checkout@v7",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Complete job",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
        ]
    }

    #[test]
    fn both_timestamp_shapes_parse() {
        assert_eq!(
            parse_stamp("2026-09-26 19:12:21").map(|at| at.to_rfc3339()),
            Some("2026-09-26T19:12:21+00:00".to_string())
        );
        assert_eq!(
            parse_stamp("2026-09-26T19:12:21.6689146Z").map(|at| at.to_rfc3339()),
            Some("2026-09-26T19:12:21.668914600+00:00".to_string())
        );
        assert_eq!(parse_stamp("not a time"), None);
    }

    #[test]
    fn every_step_of_a_real_log_receives_its_own_output() {
        let steps = slice(&real_steps(), REAL_LOG);
        // The marker names the command, not the step, so the step list is
        // matched in order. The runner's own setup output precedes every
        // marker, so it belongs to `Set up job`, and the first marker opens
        // checkout.
        assert_eq!(
            steps[0].log,
            "Current runner version: '2.337.0'\nHosted Compute Agent"
        );
        assert_eq!(
            steps[1].log,
            "with:\n  repository: sn0w12/gitau\n  git switch -\nHEAD is now at e27a631\n\
             /usr/bin/git log -1 --format=%H"
        );
        assert_eq!(steps[2].log, "with:\nnode: v24.21.0");
        assert_eq!(steps[3].log, "npm ci\nfound 0 vulnerabilities");
        assert_eq!(
            steps[4].log,
            "Checking formatting...\nAll matched files use the correct format."
        );
        assert_eq!(
            steps[5].log,
            "Finished in 245ms on 292 files with 128 rules."
        );
        assert_eq!(steps[6].log, "tsc --noEmit");
        assert_eq!(steps[7].log, "Test Files  55 passed (55)");
        assert_eq!(
            steps[8].log,
            "Post job cleanup.\nCache hit occurred on the primary key.\n\
             Cleaning up orphan processes"
        );
    }

    /// The bug this split exists to fix: oxfmt's last line is stamped
    /// `40.153`, oxlint's whole window is the second `40`, and typecheck's runs
    /// to `43` while the tests start at `43.2`. A timestamp split puts the
    /// formatter's output in oxlint, leaves oxlint empty, and puts the test
    /// output in typecheck.
    #[test]
    fn steps_sharing_a_reported_second_still_get_their_own_output() {
        let steps = slice(&real_steps(), REAL_LOG);
        assert!(
            steps[4]
                .log
                .contains("All matched files use the correct format.")
        );
        assert!(!steps[5].log.contains("correct format"));
        assert!(!steps[6].log.contains("Test Files"));
        assert_eq!(
            steps[5].log,
            "Finished in 245ms on 292 files with 128 rules."
        );
        assert!(steps[7].log.contains("Test Files  55 passed (55)"));
    }

    #[test]
    fn output_before_the_first_marker_joins_set_up_job() {
        // The runner's own setup lines carry no marker of their own, and they
        // precede every step, so they belong to `Set up job` rather than to the
        // first workflow step.
        let steps = slice(&real_steps(), REAL_LOG);
        assert!(
            steps[0]
                .log
                .starts_with("Current runner version: '2.337.0'")
        );
        assert!(!steps[1].log.contains("Current runner version"));
    }

    /// The `Post ...` and `Complete job` steps write no marker and all report
    /// the same second, so they collapse into one cleanup step holding the
    /// teardown, whatever number of them the workflow has.
    #[test]
    fn the_post_steps_collapse_into_one_cleanup_step() {
        let steps = slice(&real_steps(), REAL_LOG);
        let names: Vec<&str> = steps.iter().map(|step| step.name.as_str()).collect();
        assert_eq!(names.last(), Some(&CLEANUP_STEP_NAME));
        assert_eq!(
            steps
                .iter()
                .filter(|step| step.name == CLEANUP_STEP_NAME)
                .count(),
            1
        );
        assert!(!names.iter().any(|name| name.starts_with("Post Setup")));
        assert!(!names.contains(&"Complete job"));
        assert!(steps[8].log.starts_with("Post job cleanup."));
    }

    /// A run that never writes the teardown has nothing to collapse, and its
    /// last line is the runner's, which belongs to `Complete job`.
    #[test]
    fn without_a_teardown_the_last_line_goes_to_complete_job() {
        // One marker for one workflow step, so the marker opens the tests.
        let steps_list = vec![
            step("Set up job", "2026-09-26T19:12:43Z", "2026-09-26T19:12:43Z"),
            step(
                "Frontend tests",
                "2026-09-26T19:12:43Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Post Setup Node",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Complete job",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
        ];
        let log = concat!(
            "2026-09-26T19:12:43.2560593Z ##[group]Run npm test\n",
            "2026-09-26T19:12:43.4000000Z Test Files  55 passed (55)\n",
            "2026-09-26T19:13:20.5709221Z Cleaning up orphan processes\n",
        );
        let steps = slice(&steps_list, log);
        let names: Vec<&str> = steps.iter().map(|step| step.name.as_str()).collect();
        // Nothing to gather, so the runner's own steps are left as they are.
        assert_eq!(
            names,
            vec![
                "Set up job",
                "Frontend tests",
                "Post Setup Node",
                "Complete job"
            ]
        );
        assert!(!names.contains(&CLEANUP_STEP_NAME));

        assert_eq!(steps[1].log, "Test Files  55 passed (55)");
        assert_eq!(steps[2].log, "");
        assert_eq!(steps[3].log, "Cleaning up orphan processes");
    }

    #[test]
    fn a_blank_stamped_line_keeps_its_place_in_the_step() {
        // A stamp with no message is a blank line, not a line whose stamp went
        // missing. It sits between two markers, so it belongs to the step they
        // bound, and dropping it would reflow that step's output.
        let steps = slice(
            &real_steps(),
            concat!(
                "2026-09-26T19:12:24.9518805Z ##[group]Run npm ci\n",
                "2026-09-26T19:12:25.0000000Z\n",
                "2026-09-26T19:12:38.9722213Z found 0 vulnerabilities\n",
            ),
        );
        // The only marker opens checkout, the first real step, so the lines
        // after it are that step's output.
        assert_eq!(steps[1].log, "\nfound 0 vulnerabilities");
        assert_eq!(steps[1].spans_by_line, vec![Vec::<u32>::new(), Vec::new()]);
    }

    #[test]
    fn an_unstamped_line_joins_the_step_it_falls_in() {
        let steps = slice(
            &real_steps(),
            concat!(
                "2026-09-26T19:12:24.9518805Z ##[group]Run npm ci\n",
                "hint: names commonly chosen\n",
                "instead of 'master' are 'main'\n",
            ),
        );
        assert_eq!(
            steps[1].log,
            "hint: names commonly chosen\ninstead of 'master' are 'main'"
        );
    }

    #[test]
    fn the_runner_sections_and_an_action_internals_stay_with_their_step() {
        // `##[group]` also wraps the runner's own sections and an action's
        // internals, all nested inside a step. Only the `Run` title opens a
        // step, so these lines stay in the step they are nested in rather than
        // starting a bucket of their own. The titles themselves are hidden,
        // since they are the runner's headings rather than output.
        let steps = slice(
            &real_steps(),
            concat!(
                "2026-09-26T19:12:22.0354624Z ##[group]Run actions/checkout@v7\n",
                "2026-09-26T19:12:22.6990933Z ##[group]Checking out the ref\n",
                "2026-09-26T19:12:22.7477192Z   git switch -\n",
                "2026-09-26T19:12:22.7484497Z ##[endgroup]\n",
            ),
        );
        assert_eq!(steps[1].log, "  git switch -");
        assert_eq!(steps[2].log, "");
    }

    #[test]
    fn a_log_with_no_markers_lands_in_the_first_real_step() {
        // A run whose log carries no markers at all cannot be split, so the
        // text goes to the first real step rather than being dropped.
        let steps = vec![step("only", "", ""), step("other", "", "")];
        let log = "first line\nsecond line\n";
        let steps = slice(&steps, log);
        assert_eq!(steps[0].log, "first line\nsecond line");
        assert_eq!(steps[1].log, "");
    }

    #[test]
    fn a_run_with_only_runner_steps_writes_nothing() {
        // No step is a workflow step, so there is nowhere to put the log.
        let steps = vec![step("Set up job", "", ""), step("Complete job", "", "")];
        let steps = slice(
            &steps,
            "2026-09-26T19:12:21.3522162Z Current runner version\n",
        );
        assert_eq!(steps[0].log, "");
        assert_eq!(steps[1].log, "");
    }

    #[test]
    fn ansi_colour_is_split_out_of_the_log_text() {
        let steps = slice(&real_steps(), REAL_LOG);
        // The install step's first line is the cyan `npm ci` echo. The style id
        // is resolved rather than hardcoded, since ids depend on the order
        // styles were interned in.
        let spans = steps[3].spans_by_line[0].clone();
        assert_eq!(spans.len(), 3);
        assert_eq!(&spans[0..2], [0, 6]);
        assert!(steps[3].log.starts_with("npm ci"));
        assert_eq!(steps[4].spans_by_line, vec![Vec::<u32>::new(); 2]);
    }

    #[test]
    fn a_command_line_is_shown_without_its_prefix_in_blue() {
        let steps = slice(&real_steps(), REAL_LOG);
        // The checkout step's last line is a command the runner echoed.
        let checkout = &steps[1];
        assert!(
            checkout.log.ends_with("/usr/bin/git log -1 --format=%H"),
            "the prefix was not stripped: {:?}",
            checkout.log
        );
        assert!(!checkout.log.contains("[command]"));
        let last = checkout.spans_by_line.last().expect("the line has spans");
        assert_eq!(last.len(), 3, "the command line is not styled");
        let command_len = checkout.log.lines().last().unwrap().chars().count() as u32;
        assert_eq!(&last[0..2], [0, command_len]);
    }

    #[test]
    fn the_leading_stamp_is_dropped_but_indentation_is_kept() {
        assert_eq!(split_log_line("2026-09-26T19:12:25.5Z hello").1, "hello");
        assert_eq!(split_log_line("  indented").1, "  indented");
        assert_eq!(split_log_line("not a stamp").1, "not a stamp");
    }

    #[test]
    fn maps_status_codes_to_error_kinds() {
        assert_eq!(
            api_error(401, r#"{"message":"Bad credentials"}"#).code(),
            "authenticationRequired"
        );
        assert!(matches!(
            api_error(403, r#"{"message":"rate limit"}"#),
            GitHubError::Forbidden { .. }
        ));
        assert!(matches!(
            api_error(404, r#"{"message":"Not Found"}"#),
            GitHubError::NotFound { .. }
        ));
        assert!(matches!(
            api_error(422, r#"{"message":"name already exists on this account"}"#),
            GitHubError::Api { status: 422, .. }
        ));
        assert!(matches!(
            api_error(500, "oops"),
            GitHubError::Api { status: 500, .. }
        ));
    }

    #[test]
    fn falls_back_to_status_line_when_body_is_not_json() {
        match api_error(502, "<html>bad gateway</html>") {
            GitHubError::Api { status, message } => {
                assert_eq!(status, 502);
                assert_eq!(message, "HTTP 502");
            }
            other => panic!("expected api error, got {other:?}"),
        }
    }

    #[test]
    fn create_body_serializes_snake_case_without_nulls() {
        let json = serde_json::to_value(CreateRepoBody {
            name: "repo".into(),
            description: None,
            private: true,
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({"name": "repo", "private": true}));
    }

    #[test]
    fn repo_search_terms_scope_to_one_kind_and_repository() {
        let terms = repo_search_terms("is:pr", "octocat", "repo", "open", &[]);
        assert_eq!(terms, vec!["is:pr", "repo:octocat/repo", "state:open"]);

        let with_labels = repo_search_terms("is:issue", "octocat", "repo", "all", &["bug".into()]);
        assert_eq!(
            with_labels,
            vec!["is:issue", "repo:octocat/repo", "label:\"bug\""]
        );
    }

    #[test]
    fn maps_pull_requests_and_keeps_merge_state() {
        let raw: RawSearchIssue = serde_json::from_value(serde_json::json!({
            "number": 42,
            "title": "Add a thing",
            "state": "closed",
            "user": { "login": "octocat", "avatar_url": "https://a/1" },
            "labels": [{ "name": "enhancement", "color": "a2eeef" }],
            "assignees": [{ "login": "hubot", "avatar_url": "https://a/2" }],
            "comments": 3,
            "updated_at": "2026-09-01T12:00:00Z",
            "html_url": "https://github.com/octocat/repo/pull/42",
            "pull_request": { "merged_at": "2026-09-02T09:00:00Z" }
        }))
        .unwrap();
        let mapped = map_search_pull_item(raw, Some("octocat/repo".to_owned()));
        assert_eq!(mapped.number, 42);
        assert_eq!(mapped.author.login, "octocat");
        assert_eq!(mapped.author.avatar_url, "https://a/1");
        assert_eq!(mapped.assignees.len(), 1);
        assert_eq!(mapped.labels[0].name, "enhancement");
        assert_eq!(mapped.merged_at.as_deref(), Some("2026-09-02T09:00:00Z"));
        assert_eq!(mapped.repo_full_name, "octocat/repo");
    }

    /// A cross-repo search has no repo name to hand the mapper, so the
    /// repository comes from the item's own URL.
    #[test]
    fn cross_repo_pull_request_names_its_repository_from_the_url() {
        let raw: RawSearchIssue = serde_json::from_value(serde_json::json!({
            "number": 9,
            "title": "From somewhere else",
            "state": "open",
            "html_url": "https://github.com/hubot/tools/pull/9"
        }))
        .unwrap();
        assert_eq!(
            map_search_pull_item(raw, None).repo_full_name,
            "hubot/tools"
        );
    }

    /// A closed pull request with no merge time was abandoned, not merged.
    #[test]
    fn closed_pull_request_without_merge_time_has_no_merged_at() {
        let raw: RawSearchIssue = serde_json::from_value(serde_json::json!({
            "number": 7,
            "title": "Abandoned",
            "state": "closed",
            "html_url": "https://github.com/octocat/repo/pull/7",
            "pull_request": { "merged_at": null }
        }))
        .unwrap();
        assert_eq!(map_search_pull_item(raw, None).merged_at, None);
    }

    #[test]
    fn maps_pull_detail_including_refs_and_merge_verdict() {
        let raw: RawPull = serde_json::from_value(serde_json::json!({
            "number": 42,
            "title": "Add a thing",
            "state": "open",
            "body": "why",
            "user": { "login": "octocat", "avatar_url": "https://a/1" },
            "labels": [{ "name": "enhancement", "color": "a2eeef" }],
            "assignees": [{ "login": "hubot", "avatar_url": "https://a/2" }],
            "created_at": "2026-09-01T12:00:00Z",
            "updated_at": "2026-09-02T12:00:00Z",
            "html_url": "https://github.com/octocat/repo/pull/42",
            "draft": true,
            "merged_at": null,
            "mergeable": true,
            "mergeable_state": "clean",
            "head": { "ref": "feature", "sha": "aaa", "label": "octocat:feature" },
            "base": { "ref": "main", "sha": "bbb", "label": "octocat:main" },
            "additions": 10,
            "deletions": 3,
            "changed_files": 2,
            "commits": 4
        }))
        .unwrap();
        let mapped = map_pull_detail(raw);
        assert_eq!(mapped.number, 42);
        assert!(mapped.draft);
        assert_eq!(mapped.merged_at, None);
        assert_eq!(mapped.mergeable, Some(true));
        assert_eq!(mapped.mergeable_state, "clean");
        assert_eq!(mapped.head.r#ref, "feature");
        assert_eq!(mapped.base.r#ref, "main");
        assert_eq!(mapped.additions, 10);
        assert_eq!(mapped.commits, 4);
    }

    /// GitHub computes mergeability in the background and reports null until
    /// it finishes, so the tri-state has to survive the mapping.
    #[test]
    fn keeps_pending_mergeability_distinct_from_a_verdict() {
        let raw: RawPull = serde_json::from_value(serde_json::json!({
            "number": 1,
            "title": "t",
            "state": "open",
            "mergeable": null,
            "mergeable_state": "unknown"
        }))
        .unwrap();
        let mapped = map_pull_detail(raw);
        assert_eq!(mapped.mergeable, None);
        assert_eq!(mapped.mergeable_state, "unknown");
    }

    #[test]
    fn pull_detail_tolerates_missing_refs_and_body() {
        let raw: RawPull = serde_json::from_value(serde_json::json!({
            "number": 9,
            "title": "sparse",
            "state": "open"
        }))
        .unwrap();
        let mapped = map_pull_detail(raw);
        assert_eq!(mapped.body, "");
        assert_eq!(mapped.head.r#ref, "");
        assert_eq!(mapped.base.label, "");
        assert_eq!(mapped.additions, 0);
    }

    /// A review request names the user in `requested_reviewer`, not
    /// `assignee`, and the UI renders both through the same slot.
    #[test]
    fn review_request_events_reuse_the_assignee_slot() {
        let raw: RawIssueEvent = serde_json::from_value(serde_json::json!({
            "id": 5,
            "event": "review_requested",
            "actor": { "login": "octocat", "avatar_url": "https://a/1" },
            "created_at": "2026-09-01T12:00:00Z",
            "requested_reviewer": { "login": "hubot", "avatar_url": "https://a/2" }
        }))
        .unwrap();
        assert_eq!(map_issue_event(raw).assignee.as_deref(), Some("hubot"));
    }

    #[test]
    fn maps_review_comments_with_optional_line_and_reply() {
        let raw: RawPullReviewComment = serde_json::from_value(serde_json::json!({
            "id": 77,
            "user": { "login": "hubot", "avatar_url": "https://a/2" },
            "body": "nit",
            "path": "src/lib.rs",
            "line": 42,
            "diff_hunk": "@@ -1 +1 @@\n-old\n+new",
            "created_at": "2026-09-03T12:00:00Z",
            "html_url": "https://github.com/octocat/repo/pull/42#discussion_r77",
            "in_reply_to_id": 70,
            "pull_request_review_id": 80
        }))
        .unwrap();
        let mapped = map_pull_review_comment(raw);
        assert_eq!(mapped.path, "src/lib.rs");
        assert_eq!(mapped.line, Some(42));
        assert_eq!(mapped.in_reply_to_id, Some(70));
        assert_eq!(mapped.pull_request_review_id, Some(80));
        assert!(mapped.diff_hunk.contains("+new"));
    }

    /// An outdated review comment reports no line, so the option must stay
    /// optional instead of defaulting to a bogus line 0.
    #[test]
    fn outdated_review_comments_have_no_line() {
        let raw: RawPullReviewComment = serde_json::from_value(serde_json::json!({
            "id": 78,
            "user": { "login": "hubot", "avatar_url": "" },
            "body": "old",
            "path": "src/lib.rs",
            "diff_hunk": "",
            "created_at": "2026-09-03T12:00:00Z",
            "html_url": ""
        }))
        .unwrap();
        assert_eq!(map_pull_review_comment(raw).line, None);
    }

    #[test]
    fn maps_pull_reviews() {
        let raw: RawPullReview = serde_json::from_value(serde_json::json!({
            "id": 3,
            "user": { "login": "hubot", "avatar_url": "https://a/2" },
            "state": "CHANGES_REQUESTED",
            "body": "please fix",
            "submitted_at": "2026-09-03T12:00:00Z",
            "html_url": "https://github.com/octocat/repo/pull/42#pullrequestreview-3"
        }))
        .unwrap();
        let mapped = map_pull_review(raw);
        assert_eq!(mapped.state, "CHANGES_REQUESTED");
        assert_eq!(mapped.author.login, "hubot");
        assert_eq!(mapped.body, "please fix");
    }

    #[test]
    fn maps_pull_commits() {
        let raw: RawPullCommit = serde_json::from_value(serde_json::json!({
            "sha": "abc123",
            "commit": {
                "message": "Fix the thing\n\nWhy it mattered.",
                "author": { "name": "Hubot", "date": "2026-09-03T12:00:00Z" }
            },
            "author": { "login": "hubot", "avatar_url": "https://a/2" },
            "html_url": "https://github.com/octocat/repo/commit/abc123"
        }))
        .unwrap();
        let mapped = map_pull_commit(raw);
        assert_eq!(mapped.sha, "abc123");
        assert_eq!(mapped.message, "Fix the thing\n\nWhy it mattered.");
        assert_eq!(mapped.author.login, "hubot");
        assert_eq!(mapped.author.avatar_url, "https://a/2");
        assert_eq!(mapped.authored_at, "2026-09-03T12:00:00Z");
        assert_eq!(
            mapped.html_url,
            "https://github.com/octocat/repo/commit/abc123"
        );
    }

    /// A commit whose email matches no account has no `author`, so the git
    /// author name is what the row shows.
    #[test]
    fn unlinked_pull_commit_falls_back_to_the_git_author() {
        let raw: RawPullCommit = serde_json::from_value(serde_json::json!({
            "sha": "def456",
            "commit": {
                "message": "From a laptop",
                "author": { "name": "Someone Else", "date": "2026-09-04T09:30:00Z" }
            },
            "author": null
        }))
        .unwrap();
        let mapped = map_pull_commit(raw);
        assert_eq!(mapped.author.login, "Someone Else");
        assert_eq!(mapped.author.avatar_url, "");
        assert_eq!(mapped.html_url, "");
    }

    /// A commit payload with nothing but a sha still deserializes, so one
    /// malformed row cannot take the whole list down.
    #[test]
    fn pull_commit_tolerates_a_missing_commit_object() {
        let raw: RawPullCommit =
            serde_json::from_value(serde_json::json!({ "sha": "000" })).unwrap();
        let mapped = map_pull_commit(raw);
        assert_eq!(mapped.sha, "000");
        assert_eq!(mapped.message, "");
        assert_eq!(mapped.author.login, "");
        assert_eq!(mapped.authored_at, "");
    }

    /// The IPC DTO is camelCase but GitHub's merge endpoint takes
    /// `merge_method`, so the wire form is built separately and must not
    /// inherit the IPC naming.
    #[test]
    fn merge_payload_uses_snake_case_for_the_github_wire() {
        let payload = MergePullPayload::from(&MergePullRequestBody {
            merge_method: PullRequestMergeMethod::Squash,
            commit_title: None,
            commit_message: None,
        });
        let json = serde_json::to_value(payload).unwrap();
        assert_eq!(json, serde_json::json!({ "merge_method": "squash" }));

        let with_message = MergePullPayload::from(&MergePullRequestBody {
            merge_method: PullRequestMergeMethod::Rebase,
            commit_title: Some("subject".into()),
            commit_message: Some("body".into()),
        });
        assert_eq!(
            serde_json::to_value(with_message).unwrap(),
            serde_json::json!({
                "merge_method": "rebase",
                "commit_title": "subject",
                "commit_message": "body"
            })
        );
    }

    #[test]
    fn merge_body_defaults_to_a_merge_commit() {
        assert_eq!(
            MergePullRequestBody::default().merge_method,
            PullRequestMergeMethod::Merge
        );
    }

    #[test]
    fn maps_notification_threads() {
        let raw: RawNotification = serde_json::from_value(serde_json::json!({
            "id": "123",
            "unread": true,
            "reason": "mention",
            "subject": {
                "title": "Fix the bug",
                "type": "PullRequest",
                "url": "https://api.github.com/repos/octocat/repo/pulls/42"
            },
            "repository": { "full_name": "octocat/repo" },
            "updated_at": "2026-09-01T12:00:00Z"
        }))
        .unwrap();
        let mapped = map_notification(raw);
        assert_eq!(mapped.id, "123");
        assert!(mapped.unread);
        assert_eq!(mapped.reason, "mention");
        assert_eq!(mapped.subject_title, "Fix the bug");
        assert_eq!(mapped.repo_full_name, "octocat/repo");
        assert_eq!(
            mapped.html_url.as_deref(),
            Some("https://github.com/octocat/repo/pull/42")
        );
    }
    #[test]
    fn notification_mapping_tolerates_missing_fields() {
        let raw: RawNotification = serde_json::from_value(serde_json::json!({"id": "9"})).unwrap();
        let mapped = map_notification(raw);
        assert_eq!(mapped.reason, "subscribed");
        assert!(!mapped.unread);
        assert_eq!(mapped.html_url, None);
    }

    #[test]
    fn release_notifications_keep_subject_url_for_on_click_resolution() {
        let raw: RawNotification = serde_json::from_value(serde_json::json!({
            "id": "7",
            "unread": true,
            "reason": "subscribed",
            "subject": {
                "title": "v2.0",
                "type": "Release",
                "url": "https://api.github.com/repos/octocat/repo/releases/7"
            },
            "repository": { "full_name": "octocat/repo" },
            "updated_at": "2026-09-01T12:00:00Z"
        }))
        .unwrap();
        let mapped = map_notification(raw);
        assert_eq!(mapped.html_url, None);
        assert_eq!(
            mapped.subject_url.as_deref(),
            Some("https://api.github.com/repos/octocat/repo/releases/7")
        );
    }

    #[test]
    fn page_has_more_combines_link_header_and_full_pages() {
        use reqwest::header::{HeaderMap, HeaderValue, LINK};

        let headers = HeaderMap::new();
        assert!(!page_has_more(&headers, 0));
        assert!(!page_has_more(&headers, 12));
        assert!(page_has_more(&headers, NOTIFICATIONS_PER_PAGE as usize));

        let mut linked = HeaderMap::new();
        linked.insert(
            LINK,
            HeaderValue::from_static(
                r#"<https://api.github.com/notifications?page=2>; rel="next""#,
            ),
        );
        assert!(page_has_more(&linked, 3));
    }

    #[test]
    fn subject_url_mapping_rejects_non_api_hosts() {
        assert_eq!(subject_html_url(None, "Issue"), None);
        assert_eq!(
            subject_html_url(Some("https://example.com/repos/o/r/issues/1"), "Issue"),
            None
        );
        assert_eq!(
            subject_html_url(Some("https://api.github.com/repos/o/r/issues/7"), "Issue").as_deref(),
            Some("https://github.com/o/r/issues/7")
        );
    }

    #[test]
    fn subject_url_mapping_only_covers_directly_linked_types() {
        let api = "https://api.github.com/repos/o/r/pulls/42";
        assert_eq!(
            subject_html_url(Some(api), "PullRequest").as_deref(),
            Some("https://github.com/o/r/pull/42")
        );
        assert_eq!(subject_html_url(Some(api), "Release"), None);
        assert_eq!(subject_html_url(Some(api), "CheckSuite"), None);
        assert_eq!(subject_html_url(Some(api), "Unknown"), None);
    }

    #[test]
    fn participants_dedup_is_global_and_order_preserving() {
        use crate::api::github::GithubUser;
        let users = vec![
            GithubUser {
                login: "b".into(),
                avatar_url: String::new(),
            },
            GithubUser {
                login: "a".into(),
                avatar_url: String::new(),
            },
            GithubUser {
                login: "b".into(),
                avatar_url: "other".into(),
            },
            GithubUser {
                login: String::new(),
                avatar_url: String::new(),
            },
        ];
        let deduped = dedup_users(users);
        let logins: Vec<&str> = deduped.iter().map(|user| user.login.as_str()).collect();
        assert_eq!(logins, ["b", "a"]);
    }

    #[test]
    fn issue_event_mapping_carries_label_and_assignee() {
        let labeled: RawIssueEvent = serde_json::from_value(serde_json::json!({
            "id": 1,
            "event": "labeled",
            "actor": { "login": "octocat", "avatar_url": "https://a/u/1" },
            "created_at": "2026-09-01T12:00:00Z",
            "label": { "name": "bug", "color": "d73a4a" }
        }))
        .unwrap();
        let mapped = map_issue_event(labeled);
        assert_eq!(mapped.kind, "labeled");
        assert_eq!(mapped.actor, "octocat");
        assert_eq!(mapped.label.as_deref(), Some("bug"));
        assert_eq!(mapped.label_color.as_deref(), Some("d73a4a"));
        assert_eq!(mapped.assignee, None);

        let assigned: RawIssueEvent = serde_json::from_value(serde_json::json!({
            "id": 2,
            "event": "assigned",
            "actor": { "login": "octocat", "avatar_url": "" },
            "created_at": "2026-09-01T12:00:00Z",
            "assignee": { "login": "k" }
        }))
        .unwrap();
        let mapped = map_issue_event(assigned);
        assert_eq!(mapped.assignee.as_deref(), Some("k"));
        assert_eq!(mapped.label, None);
    }

    #[test]
    fn rfc3339_formatter_matches_known_date() {
        // 2026-01-01T00:00:00Z
        assert_eq!(chrono_lite_rfc3339(1_767_225_600), "2026-01-01T00:00:00Z");
        assert_eq!(chrono_lite_rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn detects_next_page_from_link_header() {
        use reqwest::header::{HeaderMap, HeaderValue, LINK};

        let mut headers = HeaderMap::new();
        assert!(!has_next_page(&headers));

        headers.insert(
            LINK,
            HeaderValue::from_static(
                r#"<https://api.github.com/notifications?page=2>; rel="next", <https://api.github.com/notifications?page=5>; rel="last""#,
            ),
        );
        assert!(has_next_page(&headers));

        headers.insert(
            LINK,
            HeaderValue::from_static(
                r#"<https://api.github.com/notifications?page=1>; rel="prev", <https://api.github.com/notifications?page=1>; rel="first""#,
            ),
        );
        assert!(!has_next_page(&headers));
    }
}
