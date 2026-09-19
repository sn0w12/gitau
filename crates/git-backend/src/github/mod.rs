//! GitHub account integration: device-flow sign-in, OS-keychain token
//! storage, REST access, and automatic credential injection for
//! `https://github.com` remotes.
//!
//! The access token never leaves this crate: it is stored in the OS
//! keychain, read here to build libgit2 credentials, and never included in
//! any DTO sent to the UI. Only the non-secret [`AccountProfile`] crosses
//! IPC, cached on disk beside settings so the UI renders offline.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::api::remotes::{CredentialKind, CredentialRequest};
use crate::error::{GitError, Result};

pub mod api;
pub mod device_flow;
pub mod token_store;

pub use crate::api::github::{
    AccountProfile, DeviceFlowStart, GithubNotification, GithubOrg, NotificationPage,
    PublishRepositoryRequest, PublishResult,
};
pub use api::{CreateRepoBody, CreatedRepository, GithubApi, HttpGithubApi};
pub use token_store::{KeyringTokenStore, MemoryTokenStore, TokenStore};

pub const OAUTH_SCOPES: &str = "repo read:user workflow notifications";
const TOKEN_USERNAME: &str = "x-access-token";
const BUILTIN_CLIENT_ID: &str = "Ov23liIkWKR8TopkqSKP";

/// Development override so the app can be exercised against a personal
/// OAuth App without rebuilding.
fn resolve_client_id() -> String {
    std::env::var("GITAU_GITHUB_CLIENT_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| BUILTIN_CLIENT_ID.to_owned())
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    #[error("network failure: {message}")]
    Network { message: String },
    #[error("github authentication required")]
    Unauthorized,
    #[error("forbidden: {message}")]
    Forbidden { message: String },
    #[error("not found: {message}")]
    NotFound { message: String },
    #[error("github request failed ({status}): {message}")]
    Api { status: u16, message: String },
    #[error("{message}")]
    MalformedResponse { message: String },
    #[error("operation cancelled")]
    Cancelled,
    #[error("token storage failure: {message}")]
    Keyring { message: String },
    #[error("internal error: {message}")]
    Internal { message: String },
    #[error("io error: {message}")]
    Io { message: String },
}

impl GitHubError {
    pub fn code(&self) -> &'static str {
        match self {
            GitHubError::Network { .. } => "network",
            GitHubError::Unauthorized => "authenticationRequired",
            GitHubError::Forbidden { .. } => "forbidden",
            GitHubError::NotFound { .. } => "notFound",
            GitHubError::Api { .. } => "github",
            GitHubError::MalformedResponse { .. } => "github",
            GitHubError::Cancelled => "cancelled",
            GitHubError::Keyring { .. } => "internal",
            GitHubError::Internal { .. } => "internal",
            GitHubError::Io { .. } => "io",
        }
    }
}

impl From<reqwest::Error> for GitHubError {
    fn from(error: reqwest::Error) -> Self {
        GitHubError::Network {
            message: error.to_string(),
        }
    }
}

impl From<std::io::Error> for GitHubError {
    fn from(error: std::io::Error) -> Self {
        GitHubError::Io {
            message: error.to_string(),
        }
    }
}

impl From<&GitHubError> for GitError {
    fn from(error: &GitHubError) -> Self {
        match error {
            GitHubError::Network { message } => GitError::Network {
                message: message.clone(),
            },
            GitHubError::Unauthorized => GitError::AuthenticationRequired {
                remote: "github.com".into(),
            },
            GitHubError::Forbidden { message } => GitError::GitHub {
                status: 403,
                message: message.clone(),
            },
            GitHubError::NotFound { message } => GitError::GitHub {
                status: 404,
                message: message.clone(),
            },
            GitHubError::Api { status, message } => GitError::GitHub {
                status: *status,
                message: message.clone(),
            },
            GitHubError::MalformedResponse { message } => GitError::GitHub {
                status: 0,
                message: message.clone(),
            },
            GitHubError::Cancelled => GitError::Cancelled,
            GitHubError::Keyring { message } => {
                GitError::internal(format!("token storage failure: {message}"))
            }
            GitHubError::Internal { message } => GitError::internal(message.clone()),
            GitHubError::Io { message } => GitError::Io(std::io::Error::other(message.clone())),
        }
    }
}

impl From<GitHubError> for GitError {
    fn from(error: GitHubError) -> Self {
        GitError::from(&error)
    }
}

struct PendingFlow {
    device_code: String,
    expires_at: Instant,
    cancel: crate::runtime::cancellation::CancellationToken,
}

pub struct GitHubAuth {
    api: Arc<dyn GithubApi>,
    tokens: Box<dyn TokenStore>,
    client_id: String,
    pending: Mutex<Option<PendingFlow>>,
    cache_path: Mutex<Option<PathBuf>>,
    cached_profile: Mutex<Option<AccountProfile>>,
}

impl GitHubAuth {
    pub fn new(api: Arc<dyn GithubApi>, tokens: Box<dyn TokenStore>) -> Self {
        Self::with_client_id(api, tokens, resolve_client_id())
    }

    /// Explicit client id, for tests and alternative distributions.
    pub fn with_client_id(
        api: Arc<dyn GithubApi>,
        tokens: Box<dyn TokenStore>,
        client_id: String,
    ) -> Self {
        Self {
            api,
            tokens,
            client_id,
            pending: Mutex::new(None),
            cache_path: Mutex::new(None),
            cached_profile: Mutex::new(None),
        }
    }

    /// Points the service at its profile cache file and loads any saved
    /// connection. Missing files are normal (signed-out).
    pub fn initialize(&self, profile_cache_path: PathBuf) {
        let loaded = load_profile(&profile_cache_path);
        *self.cache_path.lock().unwrap() = Some(profile_cache_path);
        *self.cached_profile.lock().unwrap() = loaded;
    }

    /// True when an OAuth client id is available.
    pub fn configured(&self) -> bool {
        !self.client_id.is_empty()
    }

    /// The stored token, if any. Blocking OS call; keep off hot paths.
    pub fn token(&self) -> Option<String> {
        self.tokens.get().ok().flatten()
    }

    /// The connected account, only while a token still exists in the
    /// keychain. A cached profile without a token means the secret was
    /// removed out from under us and we report signed-out.
    pub fn account(&self) -> Option<AccountProfile> {
        self.token()?;
        self.cached_profile.lock().unwrap().clone()
    }

    pub async fn begin_sign_in(&self) -> Result<DeviceFlowStart> {
        if self.client_id.is_empty() {
            return Err(GitError::invalid_input(
                "GitHub sign-in is not configured: no OAuth client id",
            ));
        }
        let started = self
            .api
            .request_device_code(&self.client_id, OAUTH_SCOPES)
            .await?;
        let start = DeviceFlowStart {
            user_code: started.user_code,
            verification_uri: started.verification_uri,
            expires_in_secs: started.expires_in_secs,
        };
        *self.pending.lock().unwrap() = Some(PendingFlow {
            device_code: started.device_code,
            expires_at: Instant::now() + Duration::from_secs(started.expires_in_secs),
            cancel: crate::runtime::cancellation::CancellationToken::new(),
        });
        Ok(start)
    }

    /// Polls until the user finishes browser authorization, the code
    /// expires, or the flow is cancelled. Resolves with the fresh profile
    /// and persists the token + profile cache.
    pub async fn complete_sign_in(&self) -> Result<AccountProfile> {
        let (device_code, expires_at, cancel) = {
            let guard = self.pending.lock().unwrap();
            let Some(flow) = guard.as_ref() else {
                return Err(GitError::invalid_input("no sign-in in progress"));
            };
            (
                flow.device_code.clone(),
                flow.expires_at,
                flow.cancel.clone(),
            )
        };
        let id = self.client_id.clone();

        loop {
            if cancel.is_cancelled() {
                self.clear_pending();
                return Err(GitError::Cancelled);
            }
            if Instant::now() >= expires_at {
                self.clear_pending();
                return Err(GitError::GitHub {
                    status: 410,
                    message: "the sign-in code expired; start again".into(),
                });
            }

            match self.api.poll_token(&id, &device_code).await? {
                device_flow::TokenPoll::Authorized {
                    access_token,
                    scopes,
                } => {
                    self.tokens.set(&access_token)?;
                    let profile = self.fetch_profile(scopes).await;
                    self.clear_pending();
                    return profile;
                }
                device_flow::TokenPoll::Pending { retry_after_secs } => {
                    if !interruptible_sleep(&cancel, Duration::from_secs(retry_after_secs)).await {
                        self.clear_pending();
                        return Err(GitError::Cancelled);
                    }
                }
                device_flow::TokenPoll::Expired => {
                    self.clear_pending();
                    return Err(GitError::GitHub {
                        status: 410,
                        message: "the sign-in code expired; start again".into(),
                    });
                }
                device_flow::TokenPoll::Denied { message } => {
                    self.clear_pending();
                    return Err(GitError::GitHub {
                        status: 403,
                        message,
                    });
                }
            }
        }
    }

    pub fn cancel_sign_in(&self) {
        if let Some(flow) = self.pending.lock().unwrap().as_ref() {
            flow.cancel.cancel();
        }
    }

    fn clear_pending(&self) {
        if let Some(flow) = self.pending.lock().unwrap().take() {
            flow.cancel.cancel();
        }
    }

    /// Loads the authenticated user, stamps connection metadata, and
    /// persists the profile cache. On failure the token is rolled back so a
    /// half-finished sign-in never reports success.
    async fn fetch_profile(&self, scopes: Vec<String>) -> Result<AccountProfile> {
        let token = match self.token() {
            Some(token) => token,
            None => {
                return Err(GitError::internal("token vanished during sign-in"));
            }
        };
        let mut user = self.api.authenticated_user(&token).await?;
        if user.scopes.is_empty() {
            user.scopes = scopes;
        }
        let profile = AccountProfile {
            login: user.login,
            name: user.name,
            avatar_url: user.avatar_url,
            html_url: user.html_url,
            scopes: user.scopes,
            connected_at_ms: now_ms(),
        };
        if let Err(error) = self.persist_profile(&profile) {
            let _ = self.tokens.delete();
            return Err(error);
        }
        *self.cached_profile.lock().unwrap() = Some(profile.clone());
        Ok(profile)
    }

    pub async fn sign_out(&self) -> Result<()> {
        self.clear_pending();
        self.tokens.delete()?;
        if let Some(path) = self.cache_path.lock().unwrap().clone() {
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        *self.cached_profile.lock().unwrap() = None;
        Ok(())
    }

    /// Organizations the signed-in user belongs to, for the publish
    /// dialog's owner picker.
    pub async fn list_orgs(&self) -> Result<Vec<GithubOrg>> {
        let token = self.token().ok_or(GitError::AuthenticationRequired {
            remote: "github.com".into(),
        })?;
        Ok(self.api.list_orgs(&token).await?)
    }

    pub async fn create_repository(
        &self,
        owner: Option<&str>,
        body: &CreateRepoBody,
    ) -> Result<CreatedRepository> {
        let token = self.token().ok_or(GitError::AuthenticationRequired {
            remote: "github.com".into(),
        })?;
        Ok(self.api.create_repository(&token, owner, body).await?)
    }

    /// One page of all notifications (read + unread), newest first.
    /// Unread counts and filtering happen on the frontend from these pages.
    pub async fn list_notifications(&self, page: u32) -> Result<NotificationPage> {
        let token = self.token().ok_or(GitError::AuthenticationRequired {
            remote: "github.com".into(),
        })?;
        Ok(self.api.list_notifications(&token, page).await?)
    }

    pub async fn mark_notification_read(&self, thread_id: &str) -> Result<()> {
        let token = self.token().ok_or(GitError::AuthenticationRequired {
            remote: "github.com".into(),
        })?;
        Ok(self.api.mark_notification_read(&token, thread_id).await?)
    }

    pub async fn mark_all_notifications_read(&self) -> Result<()> {
        let token = self.token().ok_or(GitError::AuthenticationRequired {
            remote: "github.com".into(),
        })?;
        Ok(self.api.mark_all_notifications_read(&token).await?)
    }

    /// Resolves a subject API URL to its web URL, for notification types
    /// without a static mapping. Returns `None` when the subject is gone.
    pub async fn resolve_subject_url(&self, subject_url: &str) -> Result<Option<String>> {
        let token = self.token().ok_or(GitError::AuthenticationRequired {
            remote: "github.com".into(),
        })?;
        Ok(self.api.fetch_subject_html_url(&token, subject_url).await?)
    }

    /// Token credentials for a configured remote, but only when the remote
    /// is an https github.com URL and a token exists. Everything else falls
    /// back to whatever the request already carries.
    pub fn credential_for_remote(
        &self,
        repo: &git2::Repository,
        remote_name: &str,
    ) -> Option<CredentialRequest> {
        let remote = repo.find_remote(remote_name).ok()?;
        let url = remote.url().ok()?.to_owned();
        self.credential_for_url(&url)
    }

    pub fn credential_for_url(&self, url: &str) -> Option<CredentialRequest> {
        if !is_github_https(url) {
            return None;
        }
        let token = self.token()?;
        Some(CredentialRequest {
            kind: CredentialKind::UsernamePassword {
                username: TOKEN_USERNAME.to_owned(),
                password: token,
            },
        })
    }

    fn persist_profile(&self, profile: &AccountProfile) -> Result<()> {
        let Some(path) = self.cache_path.lock().unwrap().clone() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(profile)
            .map_err(|error| GitError::internal(error.to_string()))?;
        write_atomic(&path, &json)
    }
}

/// Matches https URLs pointed at github.com. SSH remotes cannot consume an
/// OAuth token and are deliberately excluded.
fn is_github_https(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host_end = rest.find(['/', ':']).unwrap_or(rest.len());
    let host = &rest[..host_end];
    host.eq_ignore_ascii_case("github.com")
}

async fn interruptible_sleep(
    cancel: &crate::runtime::cancellation::CancellationToken,
    total: Duration,
) -> bool {
    let deadline = Instant::now() + total;
    loop {
        if cancel.is_cancelled() {
            return false;
        }
        let now = Instant::now();
        if now >= deadline {
            return true;
        }
        let step = Duration::from_millis(200).min(deadline - now);
        tokio::time::sleep(step).await;
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn load_profile(path: &std::path::Path) -> Option<AccountProfile> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    let directory = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let temp = tempfile::Builder::new()
        .prefix(".github-account")
        .suffix(".tmp")
        .tempfile_in(directory)?;
    std::io::Write::write_all(&mut temp.as_file(), bytes)?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_only_github_https_urls() {
        assert!(is_github_https("https://github.com/octocat/repo.git"));
        assert!(is_github_https("https://GITHUB.com/octocat/repo.git"));
        assert!(is_github_https("https://github.com:443/octo/repo.git"));
        assert!(!is_github_https("http://github.com/octocat/repo.git"));
        assert!(!is_github_https("https://github.evil.dev/octocat/repo.git"));
        assert!(!is_github_https("git@github.com:octocat/repo.git"));
        assert!(!is_github_https("ssh://git@github.com/octocat/repo.git"));
        assert!(!is_github_https("https://gitlab.com/a/b.git"));
    }

    #[test]
    fn credential_resolution_requires_token_and_github_host() {
        let auth = GitHubAuth::new(
            Arc::new(HttpGithubApi::default()),
            Box::new(MemoryTokenStore::default()),
        );
        assert!(
            auth.credential_for_url("https://github.com/o/r.git")
                .is_none()
        );
        auth.tokens.set("tok").unwrap();
        let cred = auth
            .credential_for_url("https://github.com/o/r.git")
            .expect("credential for github https");
        match cred.kind {
            CredentialKind::UsernamePassword { username, password } => {
                assert_eq!(username, "x-access-token");
                assert_eq!(password, "tok");
            }
            other => panic!("expected username/password, got {other:?}"),
        }
        assert!(auth.credential_for_url("git@github.com:o/r.git").is_none());
        assert!(
            auth.credential_for_url("https://gitlab.com/a/b.git")
                .is_none()
        );
    }

    #[test]
    fn account_is_signed_out_without_a_token_even_when_cached() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("github-account.json");
        std::fs::write(
            &path,
            br#"{"login":"octo","name":null,"avatarUrl":"a","htmlUrl":"h","scopes":["repo"],"connectedAtMs":1}"#,
        )
        .unwrap();

        let auth = GitHubAuth::new(
            Arc::new(HttpGithubApi::default()),
            Box::new(MemoryTokenStore::default()),
        );
        auth.initialize(path);
        assert_eq!(auth.account(), None);

        auth.tokens.set("tok").unwrap();
        let account = auth.account().unwrap();
        assert_eq!(account.login, "octo");
        assert_eq!(account.scopes, vec!["repo".to_owned()]);
    }

    #[test]
    fn error_mapping_preserves_semantics() {
        let unauthorized: GitError = GitHubError::Unauthorized.into();
        assert_eq!(unauthorized.code(), "authenticationRequired");

        let api_err: GitError = GitHubError::Api {
            status: 422,
            message: "name already exists on this account".into(),
        }
        .into();
        assert_eq!(api_err.code(), "github");
        assert!(!api_err.retryable());

        let network: GitError = GitHubError::Network {
            message: "offline".into(),
        }
        .into();
        assert_eq!(network.code(), "network");
        assert!(network.retryable());

        let server: GitError = GitHubError::Api {
            status: 502,
            message: "bad gateway".into(),
        }
        .into();
        assert!(server.retryable());
    }

    #[tokio::test]
    async fn complete_without_begin_fails_cleanly() {
        let auth = GitHubAuth::new(
            Arc::new(HttpGithubApi::default()),
            Box::new(MemoryTokenStore::default()),
        );
        let result = auth.complete_sign_in().await.unwrap_err();
        assert_eq!(result.code(), "invalidInput");
    }

    #[tokio::test]
    async fn cancel_sign_in_aborts_polling() {
        struct SlowApi;

        impl GithubApi for SlowApi {
            fn request_device_code(
                &self,
                _client_id: &str,
                _scopes: &str,
            ) -> api::GithubFuture<device_flow::DeviceCodeResponse> {
                Box::pin(async {
                    Ok(device_flow::DeviceCodeResponse {
                        device_code: "d".into(),
                        user_code: "ABCD-1234".into(),
                        verification_uri: "https://github.com/login/device".into(),
                        expires_in_secs: 900,
                        interval_secs: 5,
                    })
                })
            }

            fn poll_token(
                &self,
                _client_id: &str,
                _device_code: &str,
            ) -> api::GithubFuture<device_flow::TokenPoll> {
                Box::pin(async {
                    Ok(device_flow::TokenPoll::Pending {
                        retry_after_secs: 30,
                    })
                })
            }

            fn authenticated_user(&self, _token: &str) -> api::GithubFuture<AccountProfile> {
                unreachable!()
            }

            fn list_orgs(&self, _token: &str) -> api::GithubFuture<Vec<GithubOrg>> {
                unreachable!()
            }

            fn create_repository(
                &self,
                _token: &str,
                _owner: Option<&str>,
                _body: &CreateRepoBody,
            ) -> api::GithubFuture<CreatedRepository> {
                unreachable!()
            }

            fn list_notifications(
                &self,
                _token: &str,
                _page: u32,
            ) -> api::GithubFuture<NotificationPage> {
                unreachable!()
            }

            fn mark_notification_read(
                &self,
                _token: &str,
                _thread_id: &str,
            ) -> api::GithubFuture<()> {
                unreachable!()
            }

            fn mark_all_notifications_read(&self, _token: &str) -> api::GithubFuture<()> {
                unreachable!()
            }

            fn fetch_subject_html_url(
                &self,
                _token: &str,
                _subject_url: &str,
            ) -> api::GithubFuture<Option<String>> {
                unreachable!()
            }
        }

        let auth = GitHubAuth::with_client_id(
            Arc::new(SlowApi),
            Box::new(MemoryTokenStore::default()),
            "test-client-id".into(),
        );
        auth.begin_sign_in().await.unwrap();
        auth.cancel_sign_in();
        let result = auth.complete_sign_in().await.unwrap_err();
        assert_eq!(result.code(), "cancelled");
    }
}
