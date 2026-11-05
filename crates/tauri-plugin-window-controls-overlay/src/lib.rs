use tauri::{
    Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

pub use color::Rgba;
pub use error::{Error, Result};
pub use models::{
    Config, OverlayGeometry, OverlayState, Rect, TitleBarOverlayInit, TitleBarOverlayOptions,
};

mod color;
mod commands;
mod error;
mod models;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

#[cfg(desktop)]
mod paint;
#[cfg(desktop)]
mod platform;

#[cfg(desktop)]
pub use desktop::{GEOMETRY_CHANGE_EVENT, OverlayWindow, WindowControlsOverlay};
#[cfg(mobile)]
pub use mobile::WindowControlsOverlay;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to
/// access the window-controls-overlay APIs.
pub trait WindowControlsOverlayExt<R: Runtime> {
    fn window_controls_overlay(&self) -> &WindowControlsOverlay<R>;
}

impl<R: Runtime, T: Manager<R>> WindowControlsOverlayExt<R> for T {
    fn window_controls_overlay(&self) -> &WindowControlsOverlay<R> {
        self.state::<WindowControlsOverlay<R>>().inner()
    }
}

/// Initializes the plugin with platform defaults.
///
/// Equivalent to [`init_with_config`] with [`Config::default`]. The plugin does
/// not read `plugins.window-controls-overlay` from `tauri.conf.json`.
pub fn init<R: Runtime>() -> TauriPlugin<R, Option<Config>> {
    init_with_config(Config::default())
}

/// Initializes the plugin with the supplied configuration.
///
/// The overlay enables itself on every undecorated window at page load. Call
/// [`WindowControlsOverlay::enable`] from Rust, or
/// `navigator.windowControlsOverlay.enable()` from JavaScript, to opt out of
/// that and configure individual windows.
///
/// Configuration comes only from this argument. The plugin does not read
/// `plugins.window-controls-overlay` from `tauri.conf.json`.
///
/// # Example
///
/// ```ignore
/// use tauri_plugin_window_controls_overlay::{Config, init_with_config};
///
/// tauri::Builder::default()
///   .plugin(init_with_config(
///       Config::new()
///           .color("#2f3241")
///           .symbol_color("#74b1be")
///           .height(48.0),
///   ))
///   .run(tauri::generate_context!())
///   .expect("error while running tauri application");
/// ```
pub fn init_with_config<R: Runtime>(config: Config) -> TauriPlugin<R, Option<Config>> {
    // Same names as Tauri's `platform()`; substituting here avoids an OS-plugin
    // dependency just to style the macOS title bar.
    let script = include_str!("init.js").replace("__PLATFORM__", std::env::consts::OS);
    Builder::<R, Option<Config>>::new("window-controls-overlay")
        .initialization_script(script)
        .invoke_handler(tauri::generate_handler![
            commands::enable,
            commands::disable,
            commands::is_enabled,
            commands::set,
            commands::get_state,
            commands::get_geometry,
            commands::bootstrap,
        ])
        .setup(move |app, _api| {
            #[cfg(mobile)]
            let window_controls_overlay = mobile::init(app, config)?;
            #[cfg(desktop)]
            let window_controls_overlay = desktop::init(app, config)?;
            app.manage(window_controls_overlay);
            Ok(())
        })
        .build()
}
