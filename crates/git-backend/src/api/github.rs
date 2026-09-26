use serde::{Deserialize, Serialize};

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

/// User-facing half of an in-progress device flow sign-in. The device code
/// stays in the backend; only the code the user must type crosses IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFlowStart {
    pub user_code: String,
    pub verification_uri: String,
    /// Seconds until [`DeviceFlowStart::user_code`] stops being accepted.
    pub expires_in_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubOrg {
    pub login: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PublishRepositoryRequest {
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
pub struct GithubNotification {
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
    pub notifications: Vec<GithubNotification>,
    pub page: u32,
    pub has_more: bool,
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
    pub labels: Vec<GithubLabel>,
    pub comment_count: u64,
    pub assignees: Vec<GithubUser>,
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

/// A GitHub user as shown on issues: login plus avatar, never the token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GithubUser {
    pub login: String,
    pub avatar_url: String,
}

/// A label on an issue. `color` is the hex string without `#`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GithubLabel {
    pub name: String,
    pub color: String,
}

/// One row of the issues list. Pull requests are filtered out before
/// reaching the UI, so every item here is a plain issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubIssueListItem {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub labels: Vec<GithubLabel>,
    pub comment_count: u64,
    pub assignees: Vec<GithubUser>,
    pub author: String,
    pub updated_at: String,
}

/// Full issue detail for the issue page header and sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubIssueDetail {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub body: String,
    pub author: GithubUser,
    pub labels: Vec<GithubLabel>,
    pub assignees: Vec<GithubUser>,
    pub participants: Vec<GithubUser>,
    pub created_at: String,
    pub updated_at: String,
    pub html_url: String,
}

/// One comment on the issue timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubIssueComment {
    pub id: u64,
    pub author: GithubUser,
    pub body: String,
    pub created_at: String,
    pub html_url: String,
}

/// One non-comment timeline entry (labeled, assigned, closed, ...).
/// `kind` is the raw GitHub event name; the UI maps known kinds to icons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubIssueEvent {
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

/// The signed-in user's access level on a repository. The UI gates
/// editing and deleting other people's comments on `push`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct GithubRepoPermissions {
    pub push: bool,
    pub admin: bool,
}
