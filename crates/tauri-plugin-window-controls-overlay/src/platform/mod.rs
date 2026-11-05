//! Per-platform implementations of the window controls overlay.
//!
//! Every platform module exposes the same set of `pub(crate)` free functions
//! (`attach`, `set_config`, `geometry`, `relayout`, `set_active`, `destroy`,
//! `resolved_colors`), re-exported here so the rest of the plugin has no `cfg`
//! noise.
//!
//! Everything here runs on the main thread: window backends are not
//! thread-safe, and all window messages and GTK signals arrive there.

use std::sync::Arc;

use tauri::{Runtime, Window};

use crate::{Result, color::Rgba, models::OverlayGeometry};

pub(crate) mod glyphs;
/// Electron's Windows caption button geometry and painting.
#[cfg(windows)]
pub(crate) mod win_style;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(crate) use macos::{
    attach, destroy, forget, geometry, relayout, resolved_colors, set_active, set_config,
};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::{
    attach, destroy, forget, geometry, relayout, resolved_colors, set_active, set_config,
};

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
mod linux;
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub(crate) use linux::{
    attach, destroy, forget, geometry, relayout, resolved_colors, set_active, set_config,
};

/// `None` means "use the platform default", exactly like Electron: Windows falls
/// back to the system window and caption text colors, Linux to the GTK window
/// color and the black/white that contrasts best with it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct OverlayConfig {
    pub color: Option<Rgba>,
    pub symbol_color: Option<Rgba>,
    pub height: Option<f64>,
    pub leading: bool,
}

/// Callbacks back into Tauri. The platform backends own native state and only
/// talk to Tauri through this interface.
pub(crate) trait OverlayHost: Send + Sync + 'static {
    fn minimize(&self);
    fn toggle_maximize(&self);
    fn close(&self);
    /// Activates the window, which is what a native non-client button press
    /// does before it acts.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn focus(&self);
    fn is_maximized(&self) -> bool;
    fn is_minimizable(&self) -> bool;
    fn is_maximizable(&self) -> bool;
    fn is_closable(&self) -> bool;
    fn is_fullscreen(&self) -> bool;
    fn emit_geometry(&self, geometry: OverlayGeometry);
}

pub(crate) type Host = Arc<dyn OverlayHost>;

#[cfg(windows)]
pub(crate) struct NativeHandles {
    // `::windows` disambiguates the crate from this module's `windows` submodule.
    pub hwnd: ::windows::Win32::Foundation::HWND,
}

#[cfg(target_os = "macos")]
pub(crate) struct NativeHandles {
    pub ns_window: *mut std::ffi::c_void,
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
pub(crate) struct NativeHandles {
    pub window: gtk::ApplicationWindow,
    /// The box Tauri adds as the window's sole child; the overlay is inserted
    /// above it so the web view keeps filling the window.
    pub vbox: gtk::Box,
}

/// Must be called on the main thread.
pub(crate) fn native_handles<R: Runtime>(window: &Window<R>) -> Result<NativeHandles> {
    #[cfg(windows)]
    {
        Ok(NativeHandles {
            hwnd: window.hwnd()?,
        })
    }
    #[cfg(target_os = "macos")]
    {
        Ok(NativeHandles {
            ns_window: window.ns_window()?,
        })
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    {
        Ok(NativeHandles {
            window: window.gtk_window()?,
            vbox: window.default_vbox()?,
        })
    }
    #[cfg(not(any(
        windows,
        target_os = "macos",
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    )))]
    {
        let _ = window;
        Err(crate::Error::UnsupportedPlatform)
    }
}
