use std::sync::Arc;

use forge_core::ForgeRegistry;
use icons::OwnerIconSource;

pub struct ForgeIconSource {
    registry: Arc<ForgeRegistry>,
}

impl ForgeIconSource {
    pub fn new(registry: Arc<ForgeRegistry>) -> Self {
        Self { registry }
    }
}

impl OwnerIconSource for ForgeIconSource {
    fn icon_url(&self, host: &str, owner: &str) -> Option<String> {
        self.registry.owner_icon_url(host, owner)
    }
}
