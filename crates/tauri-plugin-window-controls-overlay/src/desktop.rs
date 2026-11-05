use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, Manager, Runtime, WebviewWindow, Window, WindowEvent};

use crate::{
    Error, Result,
    color::Rgba,
    models::{Config, OverlayGeometry, OverlayState, TitleBarOverlayInit, TitleBarOverlayOptions},
    platform::{self, Host, OverlayConfig, OverlayHost},
};

/// Event emitted whenever the titlebar area changes (window resize, maximize,
/// fullscreen, overlay reconfiguration). The payload is an [`OverlayGeometry`].
pub const GEOMETRY_CHANGE_EVENT: &str = "window-controls-overlay://geometrychange";

pub fn init<R: Runtime>(
    app: &AppHandle<R>,
    config: Config,
) -> crate::Result<WindowControlsOverlay<R>> {
    Ok(WindowControlsOverlay {
        app: app.clone(),
        windows: Mutex::new(HashMap::new()),
        listening: Mutex::new(HashSet::new()),
        defaults: OverlayConfig {
            color: parse_color(config.color, "color")?,
            symbol_color: parse_color(config.symbol_color, "symbolColor")?,
            height: validate_height(config.height)?,
            leading: config.leading,
        },
        auto_enable: config.auto_enable,
    })
}

/// Implemented for both [`Window`] and [`WebviewWindow`], so the Rust API can be
/// called with whichever handle you already have - no need to turn on Tauri's
/// `unstable` feature just to reach [`Manager::get_window`].
pub trait OverlayWindow<R: Runtime>: Clone + Send + Sync + 'static {
    fn window(&self) -> Window<R>;
}

impl<R: Runtime> OverlayWindow<R> for Window<R> {
    fn window(&self) -> Window<R> {
        self.clone()
    }
}

impl<R: Runtime> OverlayWindow<R> for WebviewWindow<R> {
    fn window(&self) -> Window<R> {
        self.as_ref().window()
    }
}

/// Access to the window-controls-overlay APIs.
pub struct WindowControlsOverlay<R: Runtime> {
    app: AppHandle<R>,
    windows: Mutex<HashMap<String, OverlayConfig>>,
    listening: Mutex<HashSet<String>>,
    defaults: OverlayConfig,
    auto_enable: bool,
}

impl<R: Runtime> WindowControlsOverlay<R> {
    /// Enables the overlay, or updates its configuration if it is already on.
    ///
    /// Mirrors Electron's `titleBarOverlay` window option: pass `true` for the
    /// configured defaults, or `{ color, symbolColor, height }` to customise
    /// them. Unset fields fall back to the plugin configuration, and then to
    /// the platform defaults.
    ///
    /// The window must be frameless (`decorations: false`), which is Tauri's
    /// equivalent of Electron's `titleBarStyle: 'hidden'`.
    pub fn enable(
        &self,
        window: &impl OverlayWindow<R>,
        options: impl Into<TitleBarOverlayInit>,
    ) -> Result<OverlayState> {
        let window = window.window();
        let window = &window;
        let options = options.into();
        if let TitleBarOverlayInit::Enabled(false) = options {
            self.disable(window)?;
            return self.state(window);
        }

        let config = resolve_config(&options, self.defaults)?;
        if window.is_decorated()? {
            return Err(Error::WindowIsDecorated);
        }

        let label = window.label().to_owned();
        let existing = self
            .windows
            .lock()
            .expect("overlay registry poisoned")
            .contains_key(&label);

        // The registry lock is never held across a platform call: attaching can
        // synchronously deliver window events, which re-enter this type.
        if existing {
            platform::set_config(&label, config)?;
            self.windows
                .lock()
                .expect("overlay registry poisoned")
                .insert(label, config);
        } else {
            let host: Host = Arc::new(WindowHost(window.clone()));
            platform::attach(&label, platform::native_handles(window)?, config, host)?;
            // Register before listening so the handler sees the overlay as enabled.
            self.windows
                .lock()
                .expect("overlay registry poisoned")
                .insert(label, config);
            self.listen(window);
        }

        self.state(window)
    }

    /// Enables the overlay from the plugin configuration, if `autoEnable` is on,
    /// and returns the resulting state.
    ///
    /// The injected script calls this at page load, so an app that only wants
    /// the configured defaults never has to call [`Self::enable`]. Decorated
    /// windows are skipped, and `Ok(None)` means the overlay is not enabled.
    pub fn bootstrap(&self, window: &impl OverlayWindow<R>) -> Result<Option<OverlayState>> {
        let window = window.window();
        if self.auto_enable && !self.is_enabled(&window) && !window.is_decorated()? {
            // `Enabled(true)` resolves to the plugin defaults. A platform
            // without an overlay keeps `visible` false instead of warning on
            // every page load.
            match self.enable(&window, TitleBarOverlayInit::Enabled(true)) {
                Ok(_) | Err(Error::UnsupportedPlatform) | Err(Error::WindowIsDecorated) => {}
                Err(error) => return Err(error),
            }
        }
        if self.is_enabled(&window) {
            Ok(Some(self.state(&window)?))
        } else {
            Ok(None)
        }
    }

    pub fn disable(&self, window: &impl OverlayWindow<R>) -> Result<()> {
        let window = window.window();
        let label = window.label().to_owned();
        let existed = self
            .windows
            .lock()
            .expect("overlay registry poisoned")
            .remove(&label)
            .is_some();
        if !existed {
            return Ok(());
        }
        platform::destroy(&label);
        let _ = window.emit(GEOMETRY_CHANGE_EVENT, OverlayGeometry::default());
        Ok(())
    }

    pub fn is_enabled(&self, window: &impl OverlayWindow<R>) -> bool {
        let label = window.window().label().to_owned();
        self.windows
            .lock()
            .expect("overlay registry poisoned")
            .contains_key(&label)
    }

    /// Updates the colors and/or height of an already enabled overlay.
    ///
    /// Mirrors Electron's `win.setTitleBarOverlay(options)`, including throwing
    /// when the overlay was never enabled.
    pub fn update(
        &self,
        window: &impl OverlayWindow<R>,
        options: TitleBarOverlayOptions,
    ) -> Result<OverlayState> {
        let label = window.window().label().to_owned();
        let previous = self
            .windows
            .lock()
            .expect("overlay registry poisoned")
            .get(&label)
            .copied()
            .ok_or(Error::NotEnabled)?;

        // Unspecified fields keep their current value, as in Electron.
        let merged = OverlayConfig {
            color: parse_color(options.color, "color")?.or(previous.color),
            symbol_color: parse_color(options.symbol_color, "symbolColor")?
                .or(previous.symbol_color),
            height: validate_height(options.height)?.or(previous.height),
            leading: previous.leading,
        };
        platform::set_config(&label, merged)?;
        self.windows
            .lock()
            .expect("overlay registry poisoned")
            .insert(label, merged);

        self.state(window)
    }

    pub fn state(&self, window: &impl OverlayWindow<R>) -> Result<OverlayState> {
        let label = window.window().label().to_owned();
        let enabled = self
            .windows
            .lock()
            .expect("overlay registry poisoned")
            .contains_key(&label);
        if !enabled {
            return Err(Error::NotEnabled);
        }
        let geometry = platform::geometry(&label)?;
        let (color, symbol_color) = platform::resolved_colors(&label)?;
        Ok(OverlayState {
            enabled: true,
            visible: geometry.visible,
            color,
            symbol_color,
            height: geometry.titlebar_area.height,
            titlebar_area: geometry.titlebar_area,
            window_controls: geometry.window_controls,
            leading_window_controls: geometry.leading_window_controls,
        })
    }

    pub fn geometry(&self, window: &impl OverlayWindow<R>) -> Result<OverlayGeometry> {
        let label = window.window().label().to_owned();
        if !self
            .windows
            .lock()
            .expect("overlay registry poisoned")
            .contains_key(&label)
        {
            return Ok(OverlayGeometry::default());
        }
        platform::geometry(&label)
    }

    fn listen(&self, window: &Window<R>) {
        let label = window.label().to_owned();
        {
            let mut listening = self.listening.lock().expect("listener registry poisoned");
            if !listening.insert(label.clone()) {
                return;
            }
        }
        let app = self.app.clone();
        window.on_window_event(move |event| {
            let Some(state) = app.try_state::<WindowControlsOverlay<R>>() else {
                return;
            };
            if !state.is_enabled_label(&label) {
                return;
            }
            match event {
                // Tauri has no dedicated maximize/restore event: both arrive as a
                // resize, which is also when the button set has to be recomputed.
                WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                    let _ = platform::relayout(&label);
                }
                WindowEvent::Focused(focused) => platform::set_active(&label, *focused),
                WindowEvent::Destroyed => {
                    state.forget_label(&label);
                    // The window is already being torn down by GTK/Win32, so the overlay
                    // has to be dropped without touching its widgets.
                    platform::forget(&label);
                }
                _ => {}
            }
        });
    }

    fn is_enabled_label(&self, label: &str) -> bool {
        self.windows
            .lock()
            .expect("overlay registry poisoned")
            .contains_key(label)
    }

    fn forget_label(&self, label: &str) {
        self.windows
            .lock()
            .expect("overlay registry poisoned")
            .remove(label);
        self.listening
            .lock()
            .expect("listener registry poisoned")
            .remove(label);
    }
}

struct WindowHost<R: Runtime>(Window<R>);

impl<R: Runtime> OverlayHost for WindowHost<R> {
    fn minimize(&self) {
        let _ = self.0.minimize();
    }

    fn toggle_maximize(&self) {
        // Tauri has no `toggle_maximize`, so mirror what Electron's maximize /
        // restore buttons do and pick the right one from the current state.
        if self.0.is_maximized().unwrap_or(false) {
            let _ = self.0.unmaximize();
        } else {
            let _ = self.0.maximize();
        }
    }

    fn close(&self) {
        let _ = self.0.close();
    }

    fn focus(&self) {
        let _ = self.0.set_focus();
    }

    fn is_maximized(&self) -> bool {
        self.0.is_maximized().unwrap_or(false)
    }

    fn is_minimizable(&self) -> bool {
        self.0.is_minimizable().unwrap_or(false)
    }

    fn is_maximizable(&self) -> bool {
        self.0.is_maximizable().unwrap_or(false)
    }

    fn is_closable(&self) -> bool {
        self.0.is_closable().unwrap_or(true)
    }

    fn is_fullscreen(&self) -> bool {
        self.0.is_fullscreen().unwrap_or(false)
    }

    fn emit_geometry(&self, geometry: OverlayGeometry) {
        let _ = self.0.emit(GEOMETRY_CHANGE_EVENT, geometry);
    }
}

fn resolve_config(options: &TitleBarOverlayInit, defaults: OverlayConfig) -> Result<OverlayConfig> {
    let options = match options {
        TitleBarOverlayInit::Enabled(_) => &TitleBarOverlayOptions::default(),
        TitleBarOverlayInit::Options(options) => options,
    };
    Ok(OverlayConfig {
        color: parse_color(options.color.clone(), "color")?.or(defaults.color),
        symbol_color: parse_color(options.symbol_color.clone(), "symbolColor")?
            .or(defaults.symbol_color),
        height: validate_height(options.height)?.or(defaults.height),
        leading: defaults.leading,
    })
}

fn parse_color(value: Option<String>, field: &'static str) -> Result<Option<Rgba>> {
    match value {
        Some(value) => Ok(Some(Rgba::parse(field, &value)?)),
        None => Ok(None),
    }
}

/// Electron stores the overlay height as an integer; anything non-finite,
/// negative or absurdly large is rejected so the layout maths stay sane.
fn validate_height(height: Option<f64>) -> Result<Option<f64>> {
    match height {
        None => Ok(None),
        Some(height) if height.is_finite() && height > 0.0 && height <= 10_000.0 => {
            Ok(Some(height))
        }
        Some(height) => Err(Error::InvalidHeight(height)),
    }
}
