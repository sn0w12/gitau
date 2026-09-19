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
