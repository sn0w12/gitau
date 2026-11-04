/// Supplies owner icon URLs from the forge layer. `None` means the host has
/// no provider or the provider serves no icon.
pub trait OwnerIconSource: Send + Sync {
    fn icon_url(&self, host: &str, owner: &str) -> Option<String>;
}
