use serde::{Deserialize, Serialize};

use crate::color::Rgba;

/// The object form of Electron's `titleBarOverlay` window option.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleBarOverlayOptions {
    /// CSS color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// CSS color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_color: Option<String>,
    /// Logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
}

impl TitleBarOverlayOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn symbol_color(mut self, symbol_color: impl Into<String>) -> Self {
        self.symbol_color = Some(symbol_color.into());
        self
    }

    pub fn height(mut self, height: f64) -> Self {
        self.height = Some(height);
        self
    }
}

/// Electron accepts either `titleBarOverlay: true` or an options object; both
/// work here from JS and Rust.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum TitleBarOverlayInit {
    Enabled(bool),
    Options(TitleBarOverlayOptions),
}

impl Default for TitleBarOverlayInit {
    fn default() -> Self {
        Self::Options(TitleBarOverlayOptions::default())
    }
}

impl From<bool> for TitleBarOverlayInit {
    fn from(enabled: bool) -> Self {
        Self::Enabled(enabled)
    }
}

impl From<TitleBarOverlayOptions> for TitleBarOverlayInit {
    fn from(options: TitleBarOverlayOptions) -> Self {
        Self::Options(options)
    }
}

/// The configuration passed to [`init_with_config`](crate::init_with_config).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Defaults to `true`.
    pub auto_enable: bool,
    pub color: Option<String>,
    pub symbol_color: Option<String>,
    pub height: Option<f64>,
    /// Draws the band on the leading edge. Lets an app preview the leading
    /// layout on Windows without switching the window to right-to-left.
    pub leading: bool,
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn auto_enable(mut self, auto_enable: bool) -> Self {
        self.auto_enable = auto_enable;
        self
    }

    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn symbol_color(mut self, symbol_color: impl Into<String>) -> Self {
        self.symbol_color = Some(symbol_color.into());
        self
    }

    pub fn height(mut self, height: f64) -> Self {
        self.height = Some(height);
        self
    }

    pub fn leading(mut self, leading: bool) -> Self {
        self.leading = leading;
        self
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_enable: true,
            color: None,
            symbol_color: None,
            height: None,
            leading: false,
        }
    }
}

/// A rectangle in CSS pixels, relative to the top-left of the window's client area.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// Mirrors the Web `Window Controls Overlay` API Electron exposes once
/// `titleBarOverlay` is enabled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayGeometry {
    pub visible: bool,
    /// The part of the title bar not covered by the native controls. Also
    /// published as the `--titlebar-area-*` CSS variables.
    pub titlebar_area: Rect,
    /// DOM elements in this region receive no pointer events. On Linux this is
    /// the trailing group; left-hand buttons are in
    /// [`Self::leading_window_controls`].
    pub window_controls: Rect,
    /// Only Linux desktops with `gtk-decoration-layout` buttons on the left use
    /// this.
    pub leading_window_controls: Rect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayState {
    pub enabled: bool,
    pub visible: bool,
    /// Defaults to the platform's system color, exactly like Electron.
    pub color: Rgba,
    /// Defaults to the platform's caption text color on Windows and the
    /// best-contrasting color on Linux, exactly like Electron.
    pub symbol_color: Rgba,
    /// Logical pixels.
    pub height: f64,
    pub titlebar_area: Rect,
    pub window_controls: Rect,
    pub leading_window_controls: Rect,
}
