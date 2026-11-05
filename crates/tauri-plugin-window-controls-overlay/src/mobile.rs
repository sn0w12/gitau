//! Mobile stub.
//!
//! The window controls overlay is a desktop concept — Electron's
//! `titleBarOverlay` is Windows/Linux only, and Tauri's mobile windows have no
//! title bar to overlay. The plugin still builds and registers on mobile so a
//! cross-platform app does not have to `#[cfg]` around it, but every operation
//! reports [`Error::UnsupportedPlatform`].

use tauri::{AppHandle, Runtime};

use crate::{
    Error, Result,
    models::{Config, OverlayGeometry, OverlayState, TitleBarOverlayInit, TitleBarOverlayOptions},
};

pub fn init<R: Runtime>(
    _app: &AppHandle<R>,
    _config: Config,
) -> crate::Result<WindowControlsOverlay<R>> {
    Ok(WindowControlsOverlay(std::marker::PhantomData))
}

/// Access to the window-controls-overlay APIs (unsupported on mobile).
pub struct WindowControlsOverlay<R: Runtime>(std::marker::PhantomData<R>);

impl<R: Runtime> WindowControlsOverlay<R> {
    pub fn enable(
        &self,
        _window: &tauri::Window<R>,
        _options: TitleBarOverlayInit,
    ) -> Result<OverlayState> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn disable(&self, _window: &tauri::Window<R>) -> Result<()> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn is_enabled(&self, _window: &tauri::Window<R>) -> bool {
        false
    }

    pub fn bootstrap(&self, _window: &tauri::Window<R>) -> Result<Option<OverlayState>> {
        Ok(None)
    }

    pub fn update(
        &self,
        _window: &tauri::Window<R>,
        _options: TitleBarOverlayOptions,
    ) -> Result<OverlayState> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn state(&self, _window: &tauri::Window<R>) -> Result<OverlayState> {
        Err(Error::UnsupportedPlatform)
    }

    pub fn geometry(&self, _window: &tauri::Window<R>) -> Result<OverlayGeometry> {
        Err(Error::UnsupportedPlatform)
    }
}
