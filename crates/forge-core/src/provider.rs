use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use crate::accounts::{AuthChallenge, AuthPoll};
use crate::capabilities::Capabilities;
use crate::dto::{
    AccountProfile, CheckRun, CheckRunDetail, CheckRunLog, CommitStatus, CreatePullRequestBody,
    CreateRepositoryBody, CreatedRepository, IssueComment, IssueDetail, IssueEvent, Label,
    MergePullRequestBody, MergePullRequestResult, NotificationPage, Org, PullRequestCommit,
    PullRequestDetail, PullRequestReview, PullRequestReviewComment, RepoPermissions,
    RepositoryMetadata, SearchIssuePage, SearchPullRequestPage, UpdateIssueBody,
    UpdatePullRequestBody, WorkflowRun,
};
use crate::error::ProviderError;
use crate::id::RemoteKind;

/// Object-safe async surface: boxed futures let tests inject fakes without a
/// network, and the trait stays dyn-compatible.
pub type ForgeFuture<T> = Pin<Box<dyn Future<Output = Result<T, ProviderError>> + Send>>;

/// The username half of an https credential. Providers differ (GitHub uses
/// `x-access-token`, GitLab `oauth2`), and the token is always the password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialSecret {
    pub username: String,
    pub password: String,
}

fn unsupported<T: 'static>() -> ForgeFuture<T> {
    Box::pin(async { Err(ProviderError::Unsupported) })
}

/// One forge integration. Every data method has a default that reports
/// [`ProviderError::Unsupported`], so a provider overrides only what it
/// serves and advertises the rest through [`ForgeProvider::capabilities`].
pub trait ForgeProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn label(&self) -> &'static str;
    fn capabilities(&self) -> Capabilities;

    /// Hosts this provider serves. Self-hosted instances are matched once
    /// configured, since a user adds them.
    fn hosts(&self) -> &'static [&'static str];

    /// Host suffixes whose images a body from this provider may load directly.
    /// A host matches exactly or as a subdomain; the renderer turns every
    /// other image into a link, so a body cannot pull in arbitrary remote
    /// images.
    fn image_hosts(&self) -> &'static [&'static str] {
        &[]
    }

    fn matches_host(&self, host: &str) -> bool {
        let host = host.trim_start_matches("www.");
        self.hosts().iter().any(|h| h.eq_ignore_ascii_case(host))
    }

    /// Parses `(host, owner, repo)` out of a remote URL, or `None` when the
    /// URL belongs to another provider.
    fn parse_remote(&self, url: &str) -> Option<(String, String, String)>;

    /// The web URL of a page on this provider, or `None` when the kind has no
    /// direct mapping.
    fn web_url(
        &self,
        _host: &str,
        _owner: &str,
        _repo: &str,
        _kind: RemoteKind,
        _number: u64,
    ) -> Option<String> {
        None
    }

    /// The https username paired with the access token.
    fn credential_username(&self) -> &'static str {
        "x-access-token"
    }

    /// Secret-store ids earlier versions used for this provider, tried when
    /// the connection key has no token, so an upgrade keeps users signed in.
    fn legacy_token_ids(&self) -> &'static [&'static str] {
        &[]
    }

    /// Profile cache files written before connections were keyed by host.
    fn legacy_profile_files(&self, _dir: &Path) -> Vec<(String, PathBuf)> {
        Vec::new()
    }

    /// The OAuth client id for device-flow sign-in, when the provider uses it.
    fn client_id(&self) -> Option<String> {
        None
    }

    /// Scopes to request during device-flow sign-in.
    fn oauth_scopes(&self) -> &'static str {
        ""
    }

    /// Starts a device flow, returning the user-facing challenge and the
    /// device code the backend keeps to poll with.
    fn request_device_code(
        &self,
        _client_id: &str,
        _scopes: &str,
    ) -> ForgeFuture<(AuthChallenge, String)> {
        unsupported()
    }

    fn poll_token(&self, _client_id: &str, _device_code: &str) -> ForgeFuture<AuthPoll> {
        unsupported()
    }

    fn authenticated_user(&self, _token: &str) -> ForgeFuture<AccountProfile> {
        unsupported()
    }

    fn list_orgs(&self, _token: &str) -> ForgeFuture<Vec<Org>> {
        unsupported()
    }

    fn create_repository(
        &self,
        _token: &str,
        _owner: Option<&str>,
        _body: &CreateRepositoryBody,
    ) -> ForgeFuture<CreatedRepository> {
        unsupported()
    }

    fn list_notifications(&self, _token: &str, _page: u32) -> ForgeFuture<NotificationPage> {
        unsupported()
    }

    fn mark_notification_read(&self, _token: &str, _thread_id: &str) -> ForgeFuture<()> {
        unsupported()
    }

    fn mark_all_notifications_read(&self, _token: &str) -> ForgeFuture<()> {
        unsupported()
    }

    fn fetch_subject_html_url(
        &self,
        _token: &str,
        _subject_url: &str,
    ) -> ForgeFuture<Option<String>> {
        unsupported()
    }

    fn list_issues(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _state: &str,
        _labels: &[String],
        _page: u32,
    ) -> ForgeFuture<SearchIssuePage> {
        unsupported()
    }

    fn search_issues(&self, _token: &str, _page: u32) -> ForgeFuture<SearchIssuePage> {
        unsupported()
    }

    fn get_issue(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> ForgeFuture<IssueDetail> {
        unsupported()
    }

    fn list_issue_comments(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> ForgeFuture<Vec<IssueComment>> {
        unsupported()
    }

    fn list_issue_events(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> ForgeFuture<Vec<IssueEvent>> {
        unsupported()
    }

    fn create_issue_comment(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &str,
    ) -> ForgeFuture<IssueComment> {
        unsupported()
    }

    fn update_issue(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &UpdateIssueBody,
    ) -> ForgeFuture<IssueDetail> {
        unsupported()
    }

    fn create_issue(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _title: &str,
        _body: Option<&str>,
        _labels: &[String],
    ) -> ForgeFuture<IssueDetail> {
        unsupported()
    }

    fn update_issue_comment(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _comment_id: u64,
        _body: &str,
    ) -> ForgeFuture<IssueComment> {
        unsupported()
    }

    fn delete_issue_comment(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _comment_id: u64,
    ) -> ForgeFuture<()> {
        unsupported()
    }

    fn repo_permissions(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
    ) -> ForgeFuture<RepoPermissions> {
        unsupported()
    }

    /// Public repository metadata, read without a token. `None` when the host
    /// has no such endpoint or the repository is not visible; providers that
    /// expose nothing keep the default.
    fn repo_metadata(
        &self,
        _host: &str,
        _owner: &str,
        _repo: &str,
    ) -> ForgeFuture<Option<RepositoryMetadata>> {
        Box::pin(async { Ok(None) })
    }

    fn list_pull_requests(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _state: &str,
        _labels: &[String],
        _page: u32,
    ) -> ForgeFuture<SearchPullRequestPage> {
        unsupported()
    }

    fn search_pull_requests(&self, _token: &str, _page: u32) -> ForgeFuture<SearchPullRequestPage> {
        unsupported()
    }

    fn get_pull(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> ForgeFuture<PullRequestDetail> {
        unsupported()
    }

    fn list_pull_reviews(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> ForgeFuture<Vec<PullRequestReview>> {
        unsupported()
    }

    fn list_pull_commits(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> ForgeFuture<Vec<PullRequestCommit>> {
        unsupported()
    }

    fn list_pull_review_comments(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
    ) -> ForgeFuture<Vec<PullRequestReviewComment>> {
        unsupported()
    }

    fn create_pull(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _body: &CreatePullRequestBody,
    ) -> ForgeFuture<PullRequestDetail> {
        unsupported()
    }

    fn list_labels(&self, _token: &str, _owner: &str, _repo: &str) -> ForgeFuture<Vec<Label>> {
        unsupported()
    }

    fn update_pull(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &UpdatePullRequestBody,
    ) -> ForgeFuture<PullRequestDetail> {
        unsupported()
    }

    fn merge_pull(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _number: u64,
        _body: &MergePullRequestBody,
    ) -> ForgeFuture<MergePullRequestResult> {
        unsupported()
    }

    fn list_check_runs(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _sha: &str,
    ) -> ForgeFuture<Vec<CheckRun>> {
        unsupported()
    }

    fn get_check_run(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _check_run_id: u64,
    ) -> ForgeFuture<CheckRunDetail> {
        unsupported()
    }

    fn get_check_run_log(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _check_run_id: u64,
    ) -> ForgeFuture<CheckRunLog> {
        unsupported()
    }

    fn list_workflow_runs(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _sha: &str,
    ) -> ForgeFuture<Vec<WorkflowRun>> {
        unsupported()
    }

    fn list_commit_statuses(
        &self,
        _token: &str,
        _owner: &str,
        _repo: &str,
        _sha: &str,
    ) -> ForgeFuture<Vec<CommitStatus>> {
        unsupported()
    }
}
