use serde::{Deserialize, Serialize};

use ansi::TextStyle;

/// Public identity of the connected GitHub account. Never carries the
/// access token; the token lives only in the OS keychain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountProfile {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
    pub html_url: String,
    /// Scopes granted to the stored token, as reported by GitHub.
    pub scopes: Vec<String>,
    /// Epoch ms when the connection was established.
    pub connected_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Org {
    pub login: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PublishRepositoryRequest {
    /// Provider to publish to. `None` uses the registry's default provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// `None` publishes under the signed-in user's personal account.
    pub owner: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub private: bool,
}

impl Default for PublishRepositoryRequest {
    fn default() -> Self {
        Self {
            provider: None,
            owner: None,
            name: String::new(),
            description: None,
            private: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub full_name: String,
    pub html_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
}

/// One GitHub notification thread. The token never crosses IPC; only this
/// non-secret projection reaches the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub unread: bool,
    pub reason: String,
    pub subject_title: String,
    pub subject_type: String,
    pub repo_full_name: String,
    /// Best-effort web URL derived from the API subject URL.
    pub html_url: Option<String>,
    /// The subject's API URL, for on-click resolution of types without a
    /// static web mapping (releases, check suites).
    pub subject_url: Option<String>,
    pub updated_at: String,
}

/// One page of the inbox. `has_more` comes from the response `Link`
/// header, so the UI stops exactly when GitHub has no next page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPage {
    pub notifications: Vec<Notification>,
    pub page: u32,
    pub has_more: bool,
}

/// Why a thread ended, from GitHub's `state_reason`. A thread that was never
/// closed carries no reason, and so does one GitHub reports as `reopened`,
/// which describes a state rather than an ending. Anything unrecognised is
/// [`ThreadStateReason::None`], so a new value degrades to "closed" instead
/// of failing the read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ThreadStateReason {
    #[default]
    None,
    /// Closed as fixed or delivered.
    Completed,
    /// Closed without the work being done, GitHub's "won't fix" / stale.
    NotPlanned,
    /// Closed in favour of another thread.
    Duplicate,
}

/// One issue from a GitHub search result. Search items carry `html_url`
/// and `repository_url` instead of a nested `repository.full_name`, so
/// the repo name is derived from the URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIssueItem {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub state_reason: ThreadStateReason,
    pub labels: Vec<Label>,
    pub comment_count: u64,
    pub assignees: Vec<User>,
    pub author: String,
    pub updated_at: String,
    pub html_url: String,
    pub repo_full_name: String,
}

/// One page of search results. `has_more` comes from the response `Link`
/// header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchIssuePage {
    pub items: Vec<SearchIssueItem>,
    pub page: u32,
    pub has_more: bool,
}

/// One row of the pull requests list. Carries the issue projection plus
/// `merged_at`, which is the only way to tell a merged pull request from a
/// closed one: search reports both as `state: "closed"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestListItem {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub state_reason: ThreadStateReason,
    pub labels: Vec<Label>,
    pub comment_count: u64,
    pub assignees: Vec<User>,
    pub author: User,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
    pub html_url: String,
    /// `owner/repo` the pull request lives in. A cross-repo search has to
    /// derive it from the item URL, since search reports no repository name.
    pub repo_full_name: String,
}

/// One page of pull request search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchPullRequestPage {
    pub items: Vec<PullRequestListItem>,
    pub page: u32,
    pub has_more: bool,
}

/// A GitHub user as shown on issues: login plus avatar, never the token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct User {
    pub login: String,
    pub avatar_url: String,
}

/// A label on an issue. `color` is the hex string without `#`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Label {
    pub name: String,
    pub color: String,
}

/// One row of the issues list. Pull requests are filtered out before
/// reaching the UI, so every item here is a plain issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueListItem {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub labels: Vec<Label>,
    pub comment_count: u64,
    pub assignees: Vec<User>,
    pub author: String,
    pub updated_at: String,
}

/// Full issue detail for the issue page header and sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueDetail {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub state_reason: ThreadStateReason,
    pub body: String,
    pub author: User,
    pub labels: Vec<Label>,
    pub assignees: Vec<User>,
    pub participants: Vec<User>,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
}

/// One comment on the issue timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueComment {
    pub id: u64,
    pub author: User,
    pub body: String,
    pub created_at: String,
    pub html_url: String,
}

/// One non-comment timeline entry (labeled, assigned, closed, ...).
/// `kind` is the raw GitHub event name; the UI maps known kinds to icons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueEvent {
    pub id: u64,
    pub kind: String,
    pub actor: String,
    pub actor_avatar_url: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,
}

/// Partial update to an issue: every field left `None` is untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdateIssueBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignees: Option<Vec<String>>,
}

/// The output a check run produced. Only the single-run read reports this;
/// the list omits it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CheckRunOutput {
    pub title: String,
    pub summary: String,
    /// The raw log text, which can be very large.
    pub text: String,
    pub annotations_count: u64,
}

/// One step of a run's job, in the order it ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionStep {
    pub number: u64,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    /// The log lines this step produced, sliced out of the job log by
    /// timestamp, with any ANSI colour escapes removed.
    pub log: String,
    /// Span triples per line of `log`, indexing `CheckRunLog::styles`.
    pub spans_by_line: Vec<Vec<u32>>,
}

/// A run's job log, split the way GitHub's job view splits it. The check run
/// endpoint does not carry log text: `output.text` is null for Actions jobs
/// and the real log only exists on the job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CheckRunLog {
    pub steps: Vec<ActionStep>,
    /// The 1-based table every step's spans index into, carrying the log's
    /// own colours rather than any syntax theme.
    pub styles: Vec<TextStyle>,
    /// The job log exactly as the endpoint returned it, before any splitting.
    /// Kept so the real shape of a log can be read from a live run instead of
    /// inferred, since the step stamps and the `##[group]` markers do not line
    /// up with the steps the way the splitting assumes.
    #[serde(default)]
    pub raw: String,
    /// Set when the run has no job log to show, with the reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

/// One annotation on a check run. Test runners report failures here, so this
/// is where the reason a run failed actually lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckAnnotation {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_column: Option<u64>,
    pub annotation_level: String,
    pub message: String,
    pub title: String,
}

/// One check run with its output and annotations, for the results dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRunDetail {
    #[serde(flatten)]
    pub run: CheckRun,
    pub output: CheckRunOutput,
    pub annotations: Vec<CheckAnnotation>,
}

/// One CI check run on a commit. `status` is GitHub's progress
/// (`queued`, `in_progress`, `completed`); `conclusion` stays absent until
/// the run completes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
    pub started_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
}

/// One commit status, the older sibling of a check run. Apps that have not
/// moved to the checks API (CodeRabbit among them) report here instead, and
/// GitHub shows both in one place, so the pull request page has to read both.
/// The endpoint already returns only the newest status per context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitStatus {
    pub id: u64,
    /// The reporter's name for the check, e.g. `CodeRabbit`.
    pub context: String,
    /// `pending`, `success`, `failure`, or `error`.
    pub state: String,
    /// Free text the reporter attaches, e.g. `Review in progress`.
    pub description: String,
}

/// One workflow run for a commit. Runs are started from GitHub or the app's
/// own CI, not from here, so this is a read model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRun {
    pub id: u64,
    pub name: String,
    pub event: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conclusion: Option<String>,
    pub run_number: u64,
    pub head_branch: String,
    pub html_url: String,
    pub created_at: String,
}

/// The signed-in user's access level on a repository. The UI gates
/// editing and deleting other people's comments on `push`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RepoPermissions {
    pub push: bool,
    pub admin: bool,
}

/// One side of a pull request, as reported by the pulls endpoint. Only the
/// ref and owning repo are projected; the API also reports a full `repo`
/// object per side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestRef {
    pub r#ref: String,
    pub sha: String,
    pub label: String,
}

/// Full pull request detail for the pull request page header and sidebar.
/// Carries the issue projection plus everything only the pulls endpoint
/// reports: draft state, mergeability, the two refs, and diff stats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestDetail {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub state_reason: ThreadStateReason,
    pub body: String,
    pub author: User,
    pub labels: Vec<Label>,
    /// Reviewers requested on the pull request; GitHub returns them in the
    /// same slot as issue assignees.
    pub assignees: Vec<User>,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
    pub draft: bool,
    /// Merge commit time. Set only once merged, which is also how a merged
    /// pull request is told apart from a closed one, since both report
    /// `state: "closed"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
    /// GitHub's mergeable verdict: null while the merge is still being
    /// computed, so the UI must re-read before offering the merge button.
    pub mergeable: Option<bool>,
    /// Why the merge landed where it did, e.g. `clean`, `dirty`, or
    /// `blocked`. Empty until the computation finishes.
    pub mergeable_state: String,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
    pub additions: u64,
    pub deletions: u64,
    pub changed_files: u64,
    pub commits: u64,
}

/// One commit on a pull request's head branch. `author` falls back to the
/// git author name when the commit is not linked to a GitHub account, so a
/// row always names someone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestCommit {
    pub sha: String,
    pub message: String,
    pub author: User,
    pub authored_at: String,
    pub html_url: String,
}

/// One submitted review. `state` is GitHub's raw verdict: `APPROVED`,
/// `CHANGES_REQUESTED`, `COMMENTED`, or `DISMISSED`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReview {
    pub id: u64,
    pub author: User,
    pub state: String,
    pub body: String,
    pub submitted_at: String,
    pub html_url: String,
}

/// One inline comment anchored to a line in the diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestReviewComment {
    pub id: u64,
    pub author: User,
    pub body: String,
    /// Repository-relative path the comment is anchored to.
    pub path: String,
    /// Line in the new file, absent once the comment is outdated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// The diff hunk the comment was written against.
    pub diff_hunk: String,
    pub created_at: String,
    pub html_url: String,
    /// Set when this comment replies to another review comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_reply_to_id: Option<u64>,
    /// The review this comment belongs to. A reviewer writes inline comments
    /// before submitting, so timestamps alone place them before their own
    /// review.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pull_request_review_id: Option<u64>,
}

/// New pull request. `head` is the branch carrying the changes and `base` the
/// branch it merges into, both as branch names rather than refs. The field
/// names are single words, so the camelCase and snake_case wire conventions
/// agree and this serializes as GitHub expects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CreatePullRequestBody {
    pub title: String,
    pub head: String,
    pub base: String,
    pub body: Option<String>,
    pub draft: bool,
}

/// Partial update to a pull request: every field left `None` is untouched.
/// The field names are single words, so the camelCase and snake_case wire
/// conventions agree and this serializes as GitHub expects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UpdatePullRequestBody {
    pub state: Option<String>,
    pub body: Option<String>,
    pub base: Option<String>,
    pub draft: Option<bool>,
}

/// How GitHub should combine the branch. Anything but `merge` discards the
/// branch's own commit structure, which is why the UI defaults to `merge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PullRequestMergeMethod {
    #[default]
    Merge,
    Squash,
    Rebase,
}

/// Merge options. An empty `commit_title` or `commit_message` lets GitHub
/// pick its own default, which is what an unset field means on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct MergePullRequestBody {
    pub merge_method: PullRequestMergeMethod,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit_message: Option<String>,
}

/// Outcome of a merge attempt. `merged` is false when GitHub accepted the
/// call but declined to merge (a policy block, or the sha no longer matches).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergePullRequestResult {
    pub sha: String,
    pub merged: bool,
    pub message: String,
}

/// Request body for creating a repository. The wire form is snake_case, which
/// the provider serializes directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateRepositoryBody {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub private: bool,
}

/// A repository the provider created, projected to what publish needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedRepository {
    pub full_name: String,
    pub html_url: String,
    pub default_branch: Option<String>,
    /// HTTPS clone URL as reported by the provider; publish wires this up as
    /// `origin`. Tests substitute a local path here.
    pub clone_url: Option<String>,
}

/// Public repository metadata, read without a token so it works before any
/// account is connected. Fields the provider does not expose stay `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RepositoryMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forks: Option<u64>,
}
