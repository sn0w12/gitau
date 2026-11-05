use serde::{Serialize, ser::Serializer};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Tauri(#[from] tauri::Error),

    /// Electron only exposes the window controls overlay for frameless windows
    /// (`titleBarStyle: 'hidden'`). The Tauri equivalent is `decorations: false`.
    #[error(
        "the window controls overlay requires a frameless window; create the window with `decorations: false` (Electron: `titleBarStyle: 'hidden'`)"
    )]
    WindowIsDecorated,

    #[error("the window controls overlay is not enabled for this window")]
    NotEnabled,

    #[error("the window controls overlay is only available on Windows and Linux")]
    UnsupportedPlatform,

    #[error("`{field}` is not a valid CSS color: `{value}`")]
    InvalidColor { field: &'static str, value: String },

    #[error("the window controls overlay height must be a positive, finite number, got {0}")]
    InvalidHeight(f64),

    #[error("{0}")]
    Platform(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[cfg(mobile)]
    #[error(transparent)]
    PluginInvoke(#[from] tauri::plugin::mobile::PluginInvokeError),

    /// A layered child window needs the process manifest to declare Windows 8
    /// or later. Without layering the overlay cannot composite above WebView2's
    /// DirectComposition surface at all.
    #[error(
        "the window controls overlay needs a layered child window, and Windows only allows that when the application manifest declares Windows 8 or later; add `<supportedOS Id=\"{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}\"/>` to the compatibility section (see the Windows build.rs example in the README)"
    )]
    WindowsManifestRequired,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}
