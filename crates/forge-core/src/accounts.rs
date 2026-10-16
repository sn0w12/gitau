use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::dto::AccountProfile;
use crate::error::ProviderError;
use crate::id::{ConnectionKey, ConnectionProfile};
use crate::provider::{CredentialSecret, ForgeProvider};
use crate::registry::ForgeRegistry;
use crate::token::TokenStore;

/// User-facing half of an in-progress device flow. The device code stays in
/// the backend; only the code the user must type crosses IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthChallenge {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in_secs: u64,
}

/// One tick of a device-flow poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthPoll {
    Authorized {
        token: String,
        scopes: Vec<String>,
    },
    Pending {
        /// Seconds to wait before the next poll, raised on a slow-down hint.
        retry_after_secs: u64,
    },
    Expired,
    Denied {
        message: String,
    },
}

#[derive(Clone, Default)]
struct CancelFlag(Arc<AtomicBool>);

impl CancelFlag {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    fn cancel(&self) -> bool {
        !self.0.swap(true, Ordering::AcqRel)
    }

    fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

struct PendingFlow {
    key: ConnectionKey,
    device_code: String,
    expires_at: Instant,
    cancel: CancelFlag,
    client_id: String,
}

/// Owns sign-in state and access tokens across every connected provider.
/// Profile caches live on disk beside settings; tokens live only in the OS
/// keychain and never cross IPC.
pub struct ForgeAccounts {
    registry: Arc<ForgeRegistry>,
    tokens: Box<dyn TokenStore>,
    cache_dir: Mutex<Option<PathBuf>>,
    connections: Mutex<HashMap<ConnectionKey, AccountProfile>>,
    pending: Mutex<Option<PendingFlow>>,
    client_ids: HashMap<String, String>,
}

impl ForgeAccounts {
    pub fn new(registry: Arc<ForgeRegistry>, tokens: Box<dyn TokenStore>) -> Self {
        let client_ids = registry
            .providers()
            .iter()
            .filter_map(|provider| {
                provider
                    .client_id()
                    .map(|client_id| (provider.id().to_owned(), client_id))
            })
            .collect();
        Self {
            registry,
            tokens,
            cache_dir: Mutex::new(None),
            connections: Mutex::new(HashMap::new()),
            pending: Mutex::new(None),
            client_ids,
        }
    }

    /// Overrides a provider's OAuth client id, for tests and alternative
    /// distributions.
    pub fn set_client_id(&mut self, provider: &str, client_id: impl Into<String>) {
        self.client_ids
            .insert(provider.to_owned(), client_id.into());
    }

    pub fn registry(&self) -> &Arc<ForgeRegistry> {
        &self.registry
    }

    /// Points the service at its profile cache directory and loads every
    /// saved connection. Missing files are normal (signed out).
    pub fn initialize(&self, dir: PathBuf) {
        let _ = std::fs::create_dir_all(&dir);
        let mut connections = HashMap::new();
        for provider in self.registry.providers() {
            for host in provider.hosts() {
                let key = ConnectionKey::new(provider.id(), *host);
                if let Some(profile) = load_profile(&dir.join(key.profile_file())) {
                    connections.insert(key, profile);
                }
            }
            for (host, path) in provider.legacy_profile_files(&dir) {
                let key = ConnectionKey::new(provider.id(), host);
                if connections.contains_key(&key) {
                    continue;
                }
                if let Some(profile) = load_profile(&path) {
                    // Rewritten under the connection's own name so later
                    // launches read the current path, leaving the legacy
                    // file as a fallback rather than the only copy.
                    let _ = persist_profile_to(&dir, &key, &profile);
                    connections.insert(key, profile);
                }
            }
        }
        *self.cache_dir.lock().unwrap() = Some(dir);
        *self.connections.lock().unwrap() = connections;
    }

    /// The connected account for a specific connection, only while a token
    /// still exists. A cached profile without a token reports signed out.
    pub fn account(&self, provider: &str, host: &str) -> Option<AccountProfile> {
        let key = ConnectionKey::new(provider, host);
        self.token(&key)?;
        self.connections.lock().unwrap().get(&key).cloned()
    }

    /// Every connection that currently holds a token.
    pub fn list_connections(&self) -> Vec<ConnectionProfile> {
        let connections = self.connections.lock().unwrap();
        let mut profiles: Vec<ConnectionProfile> = connections
            .iter()
            .filter(|(key, _)| self.token(key).is_some())
            .map(|(key, account)| ConnectionProfile {
                provider: key.provider.clone(),
                host: key.host.clone(),
                account: account.clone(),
            })
            .collect();
        profiles.sort_by(|a, b| {
            a.provider
                .cmp(&b.provider)
                .then_with(|| a.host.cmp(&b.host))
        });
        profiles
    }

    /// First connection for a provider that still holds a token, learned
    /// from its configured hosts.
    pub fn connected_key(&self, provider_id: &str) -> Option<ConnectionKey> {
        if let Some(key) = self
            .connections
            .lock()
            .unwrap()
            .keys()
            .find(|key| key.provider.eq_ignore_ascii_case(provider_id))
        {
            return Some(key.clone());
        }
        let provider = self.registry.by_id(provider_id)?;
        provider
            .hosts()
            .iter()
            .map(|host| ConnectionKey::new(provider.id(), *host))
            .find(|key| self.token(key).is_some())
    }

    /// The stored token for a connection, migrating a legacy entry on first
    /// read so an upgrade does not sign the user out.
    pub fn token(&self, key: &ConnectionKey) -> Option<String> {
        if let Ok(Some(token)) = self.tokens.get(&key.token_id()) {
            return Some(token);
        }
        let provider = self.registry.by_id(&key.provider)?;
        for legacy in provider.legacy_token_ids() {
            if let Ok(Some(token)) = self.tokens.get(legacy) {
                let _ = self.tokens.set(&key.token_id(), &token);
                let _ = self.tokens.delete(legacy);
                return Some(token);
            }
        }
        None
    }

    /// The token to use for a provider when the caller has no connection
    /// context, such as repository-independent inbox reads.
    pub fn require_token(&self, provider_id: &str) -> Result<String, ProviderError> {
        let key = self.connected_key(provider_id).ok_or_else(|| {
            let host = self
                .registry
                .by_id(provider_id)
                .and_then(|provider| provider.hosts().first().copied())
                .unwrap_or_default();
            ProviderError::Unauthorized {
                provider: provider_id.to_owned(),
                host: host.to_owned(),
            }
        })?;
        self.token(&key)
            .ok_or_else(|| ProviderError::unauthorized(&key))
    }

    /// Token credentials for a remote, but only when the URL is https and a
    /// registered provider matches its host and holds a token. Everything
    /// else falls back to whatever the request already carries.
    pub fn credential_for_url(&self, url: &str) -> Option<CredentialSecret> {
        let host = https_host(url)?;
        let provider = self.registry.by_host(&host)?;
        let key = ConnectionKey::new(provider.id(), host.to_lowercase());
        let token = self.token(&key)?;
        Some(CredentialSecret {
            username: provider.credential_username().to_owned(),
            password: token,
        })
    }

    pub async fn begin_sign_in(
        &self,
        provider_id: &str,
        host: &str,
    ) -> Result<AuthChallenge, ProviderError> {
        let provider = self.provider(provider_id)?;
        let client_id = self
            .client_ids
            .get(provider.id())
            .cloned()
            .or_else(|| provider.client_id())
            .unwrap_or_default();
        if client_id.is_empty() {
            return Err(ProviderError::Internal {
                message: "sign-in is not configured: no OAuth client id".into(),
            });
        }
        let (challenge, device_code) = provider
            .request_device_code(&client_id, provider.oauth_scopes())
            .await?;
        *self.pending.lock().unwrap() = Some(PendingFlow {
            key: ConnectionKey::new(provider.id(), host),
            device_code,
            expires_at: Instant::now() + Duration::from_secs(challenge.expires_in_secs),
            cancel: CancelFlag::new(),
            client_id,
        });
        Ok(challenge)
    }

    /// Polls until the user finishes browser authorization, the code
    /// expires, or the flow is cancelled. Persists the token and profile.
    pub async fn complete_sign_in(&self) -> Result<ConnectionProfile, ProviderError> {
        let (key, device_code, expires_at, cancel, client_id) = {
            let guard = self.pending.lock().unwrap();
            let Some(flow) = guard.as_ref() else {
                return Err(ProviderError::Internal {
                    message: "no sign-in in progress".into(),
                });
            };
            (
                flow.key.clone(),
                flow.device_code.clone(),
                flow.expires_at,
                flow.cancel.clone(),
                flow.client_id.clone(),
            )
        };
        let provider = self.registry.by_id(&key.provider).cloned().ok_or_else(|| {
            ProviderError::UnknownProvider {
                provider: key.provider.clone(),
            }
        })?;

        loop {
            if cancel.is_cancelled() {
                self.clear_pending();
                return Err(ProviderError::Cancelled);
            }
            if Instant::now() >= expires_at {
                self.clear_pending();
                return Err(sign_in_expired(&key.provider));
            }
            match provider.poll_token(&client_id, &device_code).await? {
                AuthPoll::Authorized { token, scopes } => {
                    self.tokens.set(&key.token_id(), &token)?;
                    let account = self.finish_profile(&provider, &key, token, scopes).await?;
                    self.clear_pending();
                    return Ok(ConnectionProfile {
                        provider: key.provider,
                        host: key.host,
                        account,
                    });
                }
                AuthPoll::Pending { retry_after_secs } => {
                    if !interruptible_sleep(&cancel, Duration::from_secs(retry_after_secs)).await {
                        self.clear_pending();
                        return Err(ProviderError::Cancelled);
                    }
                }
                AuthPoll::Expired => {
                    self.clear_pending();
                    return Err(sign_in_expired(&key.provider));
                }
                AuthPoll::Denied { message } => {
                    self.clear_pending();
                    return Err(ProviderError::Forbidden {
                        provider: key.provider.clone(),
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

    pub fn sign_out(&self, provider: &str, host: &str) -> Result<(), ProviderError> {
        self.clear_pending();
        let key = ConnectionKey::new(provider, host);
        self.tokens.delete(&key.token_id())?;
        if let Some(dir) = self.cache_dir.lock().unwrap().clone() {
            match std::fs::remove_file(dir.join(key.profile_file())) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.connections.lock().unwrap().remove(&key);
        Ok(())
    }

    fn provider(&self, id: &str) -> Result<Arc<dyn ForgeProvider>, ProviderError> {
        self.registry
            .by_id(id)
            .cloned()
            .ok_or_else(|| ProviderError::UnknownProvider {
                provider: id.to_owned(),
            })
    }

    async fn finish_profile(
        &self,
        provider: &Arc<dyn ForgeProvider>,
        key: &ConnectionKey,
        token: String,
        scopes: Vec<String>,
    ) -> Result<AccountProfile, ProviderError> {
        let mut account = provider.authenticated_user(&token).await?;
        if account.scopes.is_empty() {
            account.scopes = scopes;
        }
        account.connected_at_ms = now_ms();
        if let Err(error) = self.persist_profile(key, &account) {
            let _ = self.tokens.delete(&key.token_id());
            return Err(error);
        }
        self.connections
            .lock()
            .unwrap()
            .insert(key.clone(), account.clone());
        Ok(account)
    }

    fn persist_profile(
        &self,
        key: &ConnectionKey,
        account: &AccountProfile,
    ) -> Result<(), ProviderError> {
        let Some(dir) = self.cache_dir.lock().unwrap().clone() else {
            return Ok(());
        };
        persist_profile_to(&dir, key, account)
    }

    fn clear_pending(&self) {
        if let Some(flow) = self.pending.lock().unwrap().take() {
            flow.cancel.cancel();
        }
    }
}

fn sign_in_expired(provider: &str) -> ProviderError {
    ProviderError::Api {
        provider: provider.to_owned(),
        status: 410,
        message: "the sign-in code expired; start again".into(),
    }
}

/// The lowercase host of an https URL, or `None` for anything else. SSH
/// remotes cannot consume an OAuth token and are deliberately excluded.
fn https_host(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://")?;
    let host_end = rest.find(['/', ':']).unwrap_or(rest.len());
    let host = &rest[..host_end];
    if host.is_empty() {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

async fn interruptible_sleep(cancel: &CancelFlag, total: Duration) -> bool {
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
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn load_profile(path: &Path) -> Option<AccountProfile> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn persist_profile_to(
    dir: &Path,
    key: &ConnectionKey,
    account: &AccountProfile,
) -> Result<(), ProviderError> {
    let path = dir.join(key.profile_file());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_vec_pretty(account).map_err(|error| ProviderError::Internal {
        message: error.to_string(),
    })?;
    write_atomic(&path, &json)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ProviderError> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let temp = tempfile::Builder::new()
        .prefix(".forge-account")
        .suffix(".tmp")
        .tempfile_in(directory)?;
    std::io::Write::write_all(&mut temp.as_file(), bytes)?;
    temp.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::Capabilities;

    /// Token store with a map the test can seed and read back.
    #[derive(Clone, Default)]
    struct SharedStore(Arc<Mutex<HashMap<String, String>>>);

    impl SharedStore {
        fn seed(&self, id: &str, token: &str) {
            self.0
                .lock()
                .unwrap()
                .insert(id.to_owned(), token.to_owned());
        }

        fn peek(&self, id: &str) -> Option<String> {
            self.0.lock().unwrap().get(id).cloned()
        }
    }

    impl TokenStore for SharedStore {
        fn get(&self, id: &str) -> Result<Option<String>, ProviderError> {
            Ok(self.peek(id))
        }

        fn set(&self, id: &str, token: &str) -> Result<(), ProviderError> {
            self.seed(id, token);
            Ok(())
        }

        fn delete(&self, id: &str) -> Result<(), ProviderError> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }

    /// Serves one host, and declares the ids and file name an install from
    /// before connections were keyed by host would have written.
    struct LegacyProvider;

    impl ForgeProvider for LegacyProvider {
        fn id(&self) -> &'static str {
            "github"
        }

        fn label(&self) -> &'static str {
            "GitHub"
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::all()
        }

        fn hosts(&self) -> &'static [&'static str] {
            &["github.com"]
        }

        fn parse_remote(&self, _url: &str) -> Option<(String, String, String)> {
            None
        }

        fn legacy_token_ids(&self) -> &'static [&'static str] {
            &["github"]
        }

        fn legacy_profile_files(&self, dir: &Path) -> Vec<(String, PathBuf)> {
            vec![("github.com".to_owned(), dir.join("github-account.json"))]
        }
    }

    fn accounts(store: SharedStore) -> ForgeAccounts {
        let registry = Arc::new(ForgeRegistry::new(vec![Arc::new(LegacyProvider)]));
        ForgeAccounts::new(registry, Box::new(store))
    }

    fn profile(login: &str) -> AccountProfile {
        AccountProfile {
            login: login.to_owned(),
            name: None,
            avatar_url: String::new(),
            html_url: String::new(),
            scopes: Vec::new(),
            connected_at_ms: 7,
        }
    }

    #[test]
    fn token_migrates_a_legacy_keychain_entry() {
        let store = SharedStore::default();
        store.seed("github", "tok-legacy");
        let accounts = accounts(store.clone());
        let key = ConnectionKey::new("github", "github.com");

        assert_eq!(accounts.token(&key).as_deref(), Some("tok-legacy"));
        assert_eq!(
            store.peek("github:github.com").as_deref(),
            Some("tok-legacy")
        );
        assert_eq!(store.peek("github"), None);
        assert_eq!(accounts.token(&key).as_deref(), Some("tok-legacy"));
    }

    #[test]
    fn require_token_migrates_and_reports_unauthorized() {
        let store = SharedStore::default();
        let accounts = accounts(store.clone());

        assert!(matches!(
            accounts.require_token("github"),
            Err(ProviderError::Unauthorized { .. })
        ));

        store.seed("github", "tok-legacy");
        assert_eq!(accounts.require_token("github").unwrap(), "tok-legacy");
        assert_eq!(
            store.peek("github:github.com").as_deref(),
            Some("tok-legacy")
        );
    }

    #[test]
    fn initialize_migrates_a_legacy_profile_file() {
        let dir = tempfile::tempdir().unwrap();
        let legacy = dir.path().join("github-account.json");
        std::fs::write(&legacy, serde_json::to_vec(&profile("octocat")).unwrap()).unwrap();

        let store = SharedStore::default();
        store.seed("github:github.com", "tok-1");
        let accounts = accounts(store);
        accounts.initialize(dir.path().to_path_buf());

        assert_eq!(
            accounts.account("github", "github.com").unwrap().login,
            "octocat"
        );
        assert!(dir.path().join("github-github.com.json").exists());
        assert_eq!(accounts.list_connections().len(), 1);
    }

    #[test]
    fn a_profile_without_a_token_reports_signed_out() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = accounts(SharedStore::default());
        accounts.initialize(dir.path().to_path_buf());

        accounts
            .connections
            .lock()
            .unwrap()
            .insert(ConnectionKey::new("github", "github.com"), profile("ghost"));

        assert!(accounts.account("github", "github.com").is_none());
        assert!(accounts.list_connections().is_empty());
    }

    #[test]
    fn credential_for_url_only_matches_known_https_hosts() {
        let store = SharedStore::default();
        store.seed("github", "tok-legacy");
        let accounts = accounts(store);

        let credential = accounts
            .credential_for_url("https://github.com/octo/repo.git")
            .unwrap();
        assert_eq!(credential.username, "x-access-token");
        assert_eq!(credential.password, "tok-legacy");
        assert!(accounts
            .credential_for_url("git@github.com:octo/repo.git")
            .is_none());
        assert!(accounts
            .credential_for_url("https://example.com/octo/repo.git")
            .is_none());
    }
}
