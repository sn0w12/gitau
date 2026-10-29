use serde::{Deserialize, Serialize};

use crate::capabilities::Capabilities;
use crate::dto::AccountProfile;

/// One connected account identity: the provider it belongs to and the host
/// the token was issued for. GitHub serves one host, but self-hosted
/// providers serve many, so every key carries both.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionKey {
    pub provider: String,
    pub host: String,
}

impl ConnectionKey {
    pub fn new(provider: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            host: host.into(),
        }
    }

    /// The secret-store account name for this connection.
    pub fn token_id(&self) -> String {
        format!("{}:{}", self.provider, self.host)
    }

    /// File name inside the connections cache dir.
    pub fn profile_file(&self) -> String {
        format!("{}-{}.json", sanitize(&self.provider), sanitize(&self.host))
    }
}

/// Non-secret profile of a connected account, tagged with its connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfile {
    pub provider: String,
    pub host: String,
    pub account: AccountProfile,
}

/// A provider's advertised identity and capabilities, sent to the UI so it
/// can gate views without knowing provider names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderDescriptor {
    pub id: String,
    pub label: String,
    pub hosts: Vec<String>,
    pub image_hosts: Vec<String>,
    pub capabilities: Capabilities,
}

/// `host/owner/repo` coordinates resolved from a remote URL. Lowercased for
/// stable cache keys and comparisons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteCoords {
    pub provider: String,
    pub host: String,
    pub owner: String,
    pub repo: String,
}

impl RemoteCoords {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

/// The kind of page a web URL points at, so a provider can build it without
/// the caller knowing its URL shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteKind {
    Issue,
    PullRequest,
    Commit,
    Release,
    CheckSuite,
    Repository,
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
