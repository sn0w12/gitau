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

    /// The owner icon URL from the provider that serves `host`. `None` when
    /// no provider recognizes the host or the provider serves no icon.
    pub fn owner_icon_url(&self, host: &str, owner: &str) -> Option<String> {
        self.by_host(host)?.owner_icon_url(host, owner)
    }

    pub fn descriptors(&self) -> Vec<ProviderDescriptor> {
        self.providers
            .iter()
            .map(|provider| ProviderDescriptor {
                id: provider.id().to_owned(),
                label: provider.label().to_owned(),
                hosts: provider.hosts().iter().map(|h| (*h).to_owned()).collect(),
                image_hosts: provider
                    .image_hosts()
                    .iter()
                    .map(|h| (*h).to_owned())
                    .collect(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::Capabilities;

    struct FakeProvider {
        id: &'static str,
        hosts: &'static [&'static str],
    }

    impl ForgeProvider for FakeProvider {
        fn id(&self) -> &'static str {
            self.id
        }

        fn label(&self) -> &'static str {
            self.id
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities::default()
        }

        fn hosts(&self) -> &'static [&'static str] {
            self.hosts
        }

        fn owner_icon_url(&self, host: &str, owner: &str) -> Option<String> {
            Some(format!("https://{host}/{owner}.png"))
        }

        fn parse_remote(&self, _url: &str) -> Option<(String, String, String)> {
            None
        }
    }

    fn registry() -> ForgeRegistry {
        ForgeRegistry::new(vec![Arc::new(FakeProvider {
            id: "fake",
            hosts: &["example.com"],
        })])
    }

    #[test]
    fn delegates_owner_icons_to_the_matching_provider() {
        assert_eq!(
            registry().owner_icon_url("example.com", "owner"),
            Some("https://example.com/owner.png".to_owned())
        );
    }

    #[test]
    fn unregistered_hosts_have_no_owner_icon() {
        assert_eq!(registry().owner_icon_url("other.com", "owner"), None);
    }
}
