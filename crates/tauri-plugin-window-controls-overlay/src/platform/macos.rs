//! macOS has no equivalent of the window controls overlay.
//!
//! Electron only implements `titleBarOverlay` for Windows and Linux (see the
//! "Add native window controls (Windows Linux)" section of the custom title bar
//! tutorial). On macOS the traffic lights stay native and are managed by
//! `titleBarStyle` / `trafficLightPosition` instead, so there is nothing to
//! overlay: the window control buttons are already part of the frame.
//!
//! Every entry point therefore reports [`Error::UnsupportedPlatform`] rather
//! than silently doing nothing, which mirrors Electron's `process.platform !==
//! 'darwin'` guard.

use super::{Host, NativeHandles, OverlayConfig};
use crate::{Error, Result, models::OverlayGeometry};

pub(crate) fn attach(
    _label: &str,
    _handles: NativeHandles,
    _config: OverlayConfig,
    _host: Host,
) -> Result<OverlayGeometry> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn set_config(_label: &str, _config: OverlayConfig) -> Result<OverlayGeometry> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn geometry(_label: &str) -> Result<OverlayGeometry> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn relayout(_label: &str) -> Result<OverlayGeometry> {
    Err(Error::UnsupportedPlatform)
}

pub(crate) fn set_active(_label: &str, _active: bool) {}

pub(crate) fn destroy(_label: &str) {}

pub(crate) fn forget(_label: &str) {}

pub(crate) fn resolved_colors(_label: &str) -> Result<(crate::color::Rgba, crate::color::Rgba)> {
    Err(Error::UnsupportedPlatform)
}
