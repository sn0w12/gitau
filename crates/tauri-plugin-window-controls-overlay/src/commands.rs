use tauri::{AppHandle, Runtime, Window, command};

use crate::{
    Result, WindowControlsOverlayExt,
    models::{OverlayGeometry, OverlayState, TitleBarOverlayInit, TitleBarOverlayOptions},
};

/// These commands are intentionally *synchronous*: Tauri runs non-`async`
/// commands on the main thread, which is required because the platform backends
/// touch window handles (HWND / GTK widgets) that may only be used there.

#[command]
pub(crate) fn enable<R: Runtime>(
    app: AppHandle<R>,
    window: Window<R>,
    options: Option<TitleBarOverlayInit>,
) -> Result<OverlayState> {
    app.window_controls_overlay()
        .enable(&window, options.unwrap_or_default())
}

#[command]
pub(crate) fn disable<R: Runtime>(app: AppHandle<R>, window: Window<R>) -> Result<()> {
    app.window_controls_overlay().disable(&window)
}

#[command]
pub(crate) fn is_enabled<R: Runtime>(app: AppHandle<R>, window: Window<R>) -> Result<bool> {
    Ok(app.window_controls_overlay().is_enabled(&window))
}

#[command]
pub(crate) fn set<R: Runtime>(
    app: AppHandle<R>,
    window: Window<R>,
    options: TitleBarOverlayOptions,
) -> Result<OverlayState> {
    app.window_controls_overlay().update(&window, options)
}

#[command]
pub(crate) fn get_state<R: Runtime>(app: AppHandle<R>, window: Window<R>) -> Result<OverlayState> {
    app.window_controls_overlay().state(&window)
}

#[command]
pub(crate) fn get_geometry<R: Runtime>(
    app: AppHandle<R>,
    window: Window<R>,
) -> Result<OverlayGeometry> {
    app.window_controls_overlay().geometry(&window)
}

#[command]
pub(crate) fn bootstrap<R: Runtime>(
    app: AppHandle<R>,
    window: Window<R>,
) -> Result<Option<OverlayState>> {
    app.window_controls_overlay().bootstrap(&window)
}
