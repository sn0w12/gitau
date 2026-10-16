use std::sync::Arc;

use crate::id::{ProviderDescriptor, RemoteCoords};
use crate::provider::ForgeProvider;

/// The ordered set of providers the app supports. Resolution is by id (for
/// commands) or by remote URL (for repository binding).
pub struct ForgeRegistry {
    providers: Vec<Arc<dyn ForgeProvider>>,
}

impl ForgeRegistry {
    pub fn new(providers: Vec<Arc<dyn ForgeProvider>>) -> Self {
        Self { providers }
    }

    pub fn providers(&self) -> &[Arc<dyn ForgeProvider>] {
        &self.providers
    }

    pub fn by_id(&self, id: &str) -> Option<&Arc<dyn ForgeProvider>> {
        self.providers
            .iter()
            .find(|provider| provider.id().eq_ignore_ascii_case(id))
    }

    /// The provider used when the caller has no explicit choice, the first
    /// registered. Registration order puts the most common provider first.
    pub fn default_provider(&self) -> Option<&Arc<dyn ForgeProvider>> {
        self.providers.first()
    }

    pub fn by_host(&self, host: &str) -> Option<&Arc<dyn ForgeProvider>> {
        self.providers
            .iter()
            .find(|provider| provider.matches_host(host))
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers
            .iter()
            .map(|provider| ProviderDescriptor {
                id: provider.id().to_owned(),
                label: provider.label().to_owned(),
                hosts: provider.hosts().iter().map(|h| (*h).to_owned()).collect(),
                capabilities: provider.capabilities(),
            })
            .collect()
    }

    /// Resolves `host/owner/repo` from a remote URL by asking each provider
    /// in turn. First match wins, so host-specific providers should be
    /// registered before catch-alls.
    pub fn resolve_remote(&self, url: &str) -> Option<RemoteCoords> {
        for provider in &self.providers {
            if let Some((host, owner, repo)) = provider.parse_remote(url) {
                return Some(RemoteCoords {
                    provider: provider.id().to_owned(),
                    host,
                    owner,
                    repo,
                });
            }
        }
        None
    }
}
