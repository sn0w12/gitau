//! Windows implementation of the window controls overlay.
//!
//! Electron keeps its caption buttons in the frame view of the window's single
//! `HWND`. Tauri hands the client area to WebView2, which owns child windows of
//! its own, so here the overlay is a `WS_CHILD` window of the host, kept above
//! the web view and clipped to the host's client area. Everything Electron
//! derives from its frame view (button geometry, hit testing, the hover
//! animation, maximize/restore state and geometry reporting) is reproduced.
//!
//! The window answers `WM_NCHITTEST` with `HTMINBUTTON` / `HTMAXBUTTON` /
//! `HTCLOSE`. That is what makes Windows 11 offer the snap layouts flyout over
//! the maximize button: no other part of Win32 lets an app say "this rectangle
//! is my maximize button". The click itself is handled here rather than by
//! `DefWindowProc`, which would paint its own pressed state over the overlay.
//!
//! Per-pixel alpha needs `WS_EX_LAYERED`, and a layered *child* window only
//! exists when the process manifest declares Windows 8 or later. That is a hard
//! requirement here: without layering the band is an ordinary child, and
//! WebView2's DirectComposition surface composites above ordinary children, so
//! the buttons would never appear. Tauri's default manifest does not declare
//! Windows 8, which is why [`Error::WindowsManifestRequired`] says how to fix it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, BeginPaint,
    COLOR_BTNFACE, COLOR_BTNTEXT, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, EndPaint, GetDC, GetSysColor, HGDIOBJ, PAINTSTRUCT, ReleaseDC, ScreenToClient,
    SelectObject,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWL_EXSTYLE, GWLP_USERDATA, GetClientRect,
    GetWindowLongPtrW, GetWindowRect, HTCLIENT, HTCLOSE, HTMAXBUTTON, HTMINBUTTON, IDC_ARROW,
    KillTimer, LoadCursorW, RegisterClassW, SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYCAPTION,
    SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_SHOWWINDOW, SYSTEM_METRICS_INDEX, SetCursor, SetTimer,
    SetWindowLongPtrW, SetWindowPos, ULW_ALPHA, UpdateLayeredWindow, WM_CAPTURECHANGED,
    WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCDESTROY,
    WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_PAINT,
    WM_SETCURSOR, WM_SIZE, WM_TIMER, WNDCLASS_STYLES, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
    WS_EX_LAYERED, WS_EX_LAYOUTRTL, WS_EX_NOINHERITLAYOUT, WS_VISIBLE,
};

/// Not exported by the `windows` crate's `WindowsAndMessaging` module.
const WM_MOUSELEAVE: u32 = 0x02A3;

const HOST_SUBCLASS_ID: usize = 0x7763_6f76;

use super::{
    Host, NativeHandles, OverlayConfig,
    glyphs::ButtonKind,
    win_style::{
        ButtonRect, CLOSE_HOVER_COLOR_WINDOWS_10, CLOSE_HOVER_COLOR_WINDOWS_11, WindowsInteraction,
        WindowsStyle, laid_out_width, layout_buttons, paint, visible_buttons,
    },
};
use crate::{
    Error, Result,
    color::Rgba,
    models::{OverlayGeometry, Rect},
    paint::Canvas,
};

const CLASS_NAME: windows::core::PCWSTR = windows::core::w!("TauriWindowControlsOverlay");

/// `views::Button` animates its hover tint over 150ms with `Tween::EASE_OUT`.
const HOVER_DURATION: Duration = Duration::from_millis(150);
/// One animation frame, matching Chromium's 60fps timer.
const FRAME_INTERVAL_MS: u32 = 15;
const ANIMATION_TIMER: usize = 1;

thread_local! {
  static OVERLAYS: RefCell<HashMap<String, Overlay>> = RefCell::new(HashMap::new());
  static CLASS_REGISTERED: RefCell<bool> = const { RefCell::new(false) };
}

fn with_overlay<R>(label: &str, f: impl FnOnce(&mut Overlay) -> R) -> Option<R> {
    OVERLAYS.with(|overlays| {
        let mut overlays = overlays.try_borrow_mut().ok()?;
        overlays.get_mut(label).map(f)
    })
}

pub(crate) fn attach(
    label: &str,
    handles: NativeHandles,
    config: OverlayConfig,
    host: Host,
) -> Result<OverlayGeometry> {
    if OVERLAYS.with(|overlays| overlays.borrow().contains_key(label)) {
        return set_config(label, config);
    }

    let parent = handles.hwnd;
    if parent.0.is_null() {
        return Err(Error::Platform("the window has no HWND".into()));
    }
    register_class()?;

    let instance = unsafe { GetModuleHandleW(None) }.map_err(win_err)?;
    let hwnd = create_overlay(parent, instance)?;

    // The window procedure resolves its state through the label kept in the
    // window's user data, so it must be set before any message can arrive.
    let label_ptr = Box::into_raw(Box::new(label.to_owned()));
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, label_ptr as isize) };

    // The host's `WM_SIZE` runs before it paints, while Tauri's `Resized` event
    // is delivered later in the message loop. Repositioning from the subclass is
    // what keeps the band attached to the edge during a live resize.
    unsafe {
        let _ = SetWindowSubclass(parent, Some(host_subclass_proc), HOST_SUBCLASS_ID, 0);
    }

    let overlay = Overlay {
        parent,
        hwnd,
        host,
        config,
        canvas: Canvas::new(1, 1),
        pixels: Vec::new(),
        last_geometry: None,
        active: true,
        hovered: None,
        pressed: None,
        hover: std::array::from_fn(|_| HoverAnimation::default()),
        tracking_leave: false,
        timer: false,
    };

    let result = OVERLAYS.with(|overlays| {
        let mut overlays = overlays.borrow_mut();
        overlays.insert(label.to_owned(), overlay);
        overlays
            .get_mut(label)
            .expect("the overlay was just inserted")
            .relayout()
    });

    match result {
        Ok(geometry) => Ok(geometry),
        Err(error) => {
            destroy(label);
            Err(error)
        }
    }
}

fn create_overlay(parent: HWND, instance: windows::Win32::Foundation::HMODULE) -> Result<HWND> {
    // `WS_EX_NOINHERITLAYOUT` keeps an RTL host from mirroring the band; the
    // caller maps cursor positions back into this window's client coordinates.
    unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_NOINHERITLAYOUT,
            CLASS_NAME,
            windows::core::w!(""),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            Some(parent),
            None,
            Some(instance.into()),
            None,
        )
    }
    .map_err(|_| Error::WindowsManifestRequired)
}

fn register_class() -> Result<()> {
    if CLASS_REGISTERED.with(|flag| *flag.borrow()) {
        return Ok(());
    }
    let instance = unsafe { GetModuleHandleW(None) }.map_err(win_err)?;
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.map_err(win_err)?;
    let class = WNDCLASSW {
        style: WNDCLASS_STYLES(0),
        lpfnWndProc: Some(overlay_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance.into(),
        hIcon: Default::default(),
        hCursor: cursor,
        hbrBackground: Default::default(),
        lpszMenuName: windows::core::PCWSTR::null(),
        lpszClassName: CLASS_NAME,
    };
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        return Err(Error::Platform(
            "failed to register the overlay window class".into(),
        ));
    }
    CLASS_REGISTERED.with(|flag| *flag.borrow_mut() = true);
    Ok(())
}

pub(crate) fn set_config(label: &str, config: OverlayConfig) -> Result<OverlayGeometry> {
    with_overlay(label, |overlay| {
        overlay.config = config;
        overlay.relayout()
    })
    .ok_or(Error::NotEnabled)?
}

pub(crate) fn geometry(label: &str) -> Result<OverlayGeometry> {
    with_overlay(label, |overlay| Ok(overlay.geometry())).ok_or(Error::NotEnabled)?
}

pub(crate) fn relayout(label: &str) -> Result<OverlayGeometry> {
    with_overlay(label, |overlay| overlay.relayout()).ok_or(Error::NotEnabled)?
}

pub(crate) fn resolved_colors(label: &str) -> Result<(Rgba, Rgba)> {
    with_overlay(label, |overlay| (overlay.background(), overlay.symbol())).ok_or(Error::NotEnabled)
}

pub(crate) fn set_active(label: &str, active: bool) {
    with_overlay(label, |overlay| {
        if overlay.active == active {
            return;
        }
        overlay.active = active;
        overlay.present();
    });
}

pub(crate) fn destroy(label: &str) {
    release(label, true);
}

/// Drops the overlay of a window that is being destroyed: its `HWND` is already
/// gone, so there is nothing left to destroy.
pub(crate) fn forget(label: &str) {
    release(label, false);
}

fn release(label: &str, destroy_window: bool) {
    let overlay = OVERLAYS.with(|overlays| overlays.borrow_mut().remove(label));
    let Some(overlay) = overlay else {
        return;
    };
    unsafe {
        let ptr = GetWindowLongPtrW(overlay.hwnd, GWLP_USERDATA);
        if ptr != 0 {
            SetWindowLongPtrW(overlay.hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr as *mut String));
        }
        if destroy_window {
            let _ =
                RemoveWindowSubclass(overlay.parent, Some(host_subclass_proc), HOST_SUBCLASS_ID);
            let _ = DestroyWindow(overlay.hwnd);
        }
    }
}

fn win_err(error: windows::core::Error) -> Error {
    Error::Platform(error.to_string())
}

/// `views::SlideAnimation` with `Tween::EASE_OUT`, used for one button's hover
/// tint. The value is what `WinCaptionButton::OnPaintBackground` feeds into
/// `Tween::IntValueBetween`.
#[derive(Clone, Copy)]
struct HoverAnimation {
    value: f64,
    from: f64,
    to: f64,
    started: Option<Instant>,
    duration: Duration,
}

impl Default for HoverAnimation {
    fn default() -> Self {
        Self {
            value: 0.0,
            from: 0.0,
            to: 0.0,
            started: None,
            duration: HOVER_DURATION,
        }
    }
}

impl HoverAnimation {
    /// `Button::SetState` -> `HOVERED`: animate towards 1.
    fn show(&mut self) {
        self.run(1.0);
    }

    /// `Button::SetState` -> `NORMAL`: animate towards 0.
    fn hide(&mut self) {
        self.run(0.0);
    }

    /// `Button::SetState` -> `PRESSED` / `HOVERED`: jump, with no animation.
    fn reset(&mut self, value: f64) {
        self.value = value;
        self.from = value;
        self.to = value;
        self.started = None;
    }

    fn run(&mut self, target: f64) {
        if self.to == target && self.started.is_some() {
            return;
        }
        // `SlideAnimation::GetDuration` shortens the run so the value travels at
        // a constant rate when a direction change interrupts it.
        let travelled = if target > 0.0 {
            self.value
        } else {
            1.0 - self.value
        };
        let remaining = (1.0 - travelled).clamp(0.0, 1.0);
        self.from = self.value;
        self.to = target;
        if remaining <= 0.0 {
            self.reset(target);
            return;
        }
        self.duration = HOVER_DURATION.mul_f64(remaining);
        self.started = Some(Instant::now());
    }

    fn advance(&mut self, now: Instant) -> bool {
        let Some(started) = self.started else {
            return false;
        };
        let elapsed = now.saturating_duration_since(started);
        let progress = (elapsed.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - progress) * (1.0 - progress);
        self.value = self.from + (self.to - self.from) * eased;
        if progress >= 1.0 {
            self.value = self.to;
            self.started = None;
            return false;
        }
        true
    }
}

struct Overlay {
    parent: HWND,
    hwnd: HWND,
    host: Host,
    config: OverlayConfig,
    canvas: Canvas,
    pixels: Vec<u8>,
    last_geometry: Option<OverlayGeometry>,
    active: bool,
    hovered: Option<ButtonKind>,
    pressed: Option<ButtonKind>,
    hover: [HoverAnimation; 4],
    tracking_leave: bool,
    timer: bool,
}

impl Overlay {
    fn dpi(&self) -> u32 {
        let dpi = unsafe { GetDpiForWindow(self.parent) };
        if dpi == 0 { 96 } else { dpi }
    }

    fn scale(&self) -> f64 {
        f64::from(self.dpi()) / 96.0
    }

    fn rtl(&self) -> bool {
        let style = unsafe { GetWindowLongPtrW(self.parent, GWL_EXSTYLE) } as u32;
        style & WS_EX_LAYOUTRTL.0 != 0
    }

    fn leading(&self) -> bool {
        self.config.leading
    }

    fn metric(&self, index: SYSTEM_METRICS_INDEX) -> f64 {
        f64::from(unsafe { GetSystemMetricsForDpi(index, self.dpi()) }) / self.scale()
    }

    fn client_width(&self) -> f64 {
        let mut rect = RECT::default();
        if unsafe { GetClientRect(self.parent, &mut rect) }.is_ok() {
            f64::from(rect.right - rect.left) / self.scale()
        } else {
            0.0
        }
    }

    fn buttons(&self) -> Vec<ButtonRect> {
        let kinds = visible_buttons(
            self.host.is_maximized(),
            self.host.is_minimizable(),
            self.host.is_maximizable(),
        );
        layout_buttons(&kinds, self.rtl())
    }

    /// `WinFrameView::FrameTopBorderThickness(false)`, in logical pixels.
    fn frame_top_border(&self) -> f64 {
        if self.host.is_maximized() || self.host.is_fullscreen() {
            // `needs_no_border`: a maximized window must inset its top border,
            // otherwise Windows would hand us back a standard titlebar.
            return 0.0;
        }
        (self.metric(SM_CXSIZEFRAME) + self.metric(SM_CXPADDEDBORDER)).floor()
    }

    /// `WinFrameView::TitlebarHeight(custom_height)`, in logical pixels.
    ///
    /// Tauri's client area starts at the top of the window, so unlike Electron
    /// there is no 1px non-client border above the band to leave uncovered.
    fn titlebar_height(&self, custom_height: f64) -> f64 {
        if self.host.is_fullscreen() && !self.host.is_maximized() {
            return 0.0;
        }
        let frame_top = self.frame_top_border();
        let maximized_visual = self.metric(SM_CYCAPTION) - frame_top;
        if custom_height > maximized_visual {
            custom_height
        } else {
            (maximized_visual + frame_top).max(0.0)
        }
    }

    fn container_height(&self) -> f64 {
        self.titlebar_height(self.config.height.unwrap_or(0.0))
    }

    /// `WinFrameView::LayoutWindowControlsOverlay()`'s reported overlay height.
    fn overlay_height(&self) -> f64 {
        let custom = self.config.height.unwrap_or(0.0);
        if custom > 0.0 {
            custom
        } else {
            self.container_height()
        }
    }

    fn background(&self) -> Rgba {
        self.config.color.unwrap_or_else(system_background)
    }

    fn symbol(&self) -> Rgba {
        self.config.symbol_color.unwrap_or_else(system_symbol)
    }

    /// Base color of the hover/press tint while the window is focused, i.e.
    /// Chromium's `kColorCaptionButtonForegroundActive`.
    ///
    /// Chromium derives this from the frame color, which is the system caption
    /// color unless `color` overrides it. Apps whose theme can differ from the
    /// system theme need the configured background to decide, or a light app
    /// on a dark system gets a white hover tint on a light band.
    fn foreground_active(&self) -> Rgba {
        self.background().with_alpha(255).contrasting()
    }

    /// Base color of the hover/press tint while the window is unfocused
    /// (`kColorCaptionButtonForegroundInactive`).
    fn foreground_inactive(&self) -> Rgba {
        self.background().with_alpha(255).contrasting()
    }

    fn close_background(&self) -> Rgba {
        if windows_11_or_greater() {
            CLOSE_HOVER_COLOR_WINDOWS_11
        } else {
            CLOSE_HOVER_COLOR_WINDOWS_10
        }
    }

    fn geometry(&self) -> OverlayGeometry {
        let client_width = self.client_width();
        let controls_width = laid_out_width(&self.buttons());
        let height = self.overlay_height();
        let free = (client_width - controls_width).max(0.0);

        let (titlebar_area, window_controls) = if self.rtl() || self.leading() {
            (
                Rect::new(controls_width, 0.0, free, height),
                Rect::new(0.0, 0.0, controls_width, height),
            )
        } else {
            (
                Rect::new(0.0, 0.0, free, height),
                Rect::new(free, 0.0, controls_width, height),
            )
        };

        OverlayGeometry {
            visible: height > 0.0 && free > 0.0,
            titlebar_area,
            window_controls,
            // Windows always keeps the caption buttons on the trailing edge.
            leading_window_controls: Rect::default(),
        }
    }

    fn relayout(&mut self) -> Result<OverlayGeometry> {
        let geometry = self.geometry();
        self.reposition()?;
        self.present();
        if self.last_geometry != Some(geometry) {
            self.last_geometry = Some(geometry);
            self.host.emit_geometry(geometry);
        }
        Ok(geometry)
    }

    /// Moves the band without repainting. Called from the host's `WM_SIZE`,
    /// where only the position has to keep up with the frame; the content and
    /// the reported geometry follow from Tauri's `Resized` event.
    fn reposition(&self) -> Result<()> {
        let scale = self.scale();
        let width = (laid_out_width(&self.buttons()) * scale).round().max(1.0) as i32;
        let height = (self.container_height() * scale).round().max(0.0) as i32;

        let mut client = RECT::default();
        unsafe { GetClientRect(self.parent, &mut client) }.map_err(win_err)?;

        let x = if self.leading() {
            0
        } else {
            (client.right - client.left - width).max(0)
        };
        // `HWND_TOP` keeps the band above the web view's own child windows;
        // they are created after ours and would otherwise cover it.
        let flags = SWP_NOACTIVATE
            | if height == 0 {
                SWP_HIDEWINDOW
            } else {
                SWP_SHOWWINDOW
            };

        unsafe {
            SetWindowPos(
                self.hwnd,
                Some(windows::Win32::UI::WindowsAndMessaging::HWND_TOP),
                x,
                0,
                width,
                height.max(1),
                flags,
            )
        }
        .map_err(win_err)
    }

    fn hit_test(&self, x: f64, y: f64) -> Option<ButtonKind> {
        let scale = self.scale();
        let (x, y) = (x / scale, y / scale);
        let height = self.container_height();
        self.buttons()
            .into_iter()
            .find(|button| {
                let rect = button.rect;
                x >= rect.x && x < rect.right() && y >= 0.0 && y < height
            })
            .map(|button| button.kind)
    }

    fn bounds(&self) -> (i32, i32, i32, i32) {
        let mut rect = RECT::default();
        let _ = unsafe { GetWindowRect(self.hwnd, &mut rect) };
        (
            rect.left,
            rect.top,
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
        )
    }

    fn render(&mut self) {
        let (_, _, width, height) = self.bounds();
        // Computed before `self.canvas` is borrowed mutably.
        let buttons = self.buttons();
        let container_height = self.container_height();
        let scale = self.scale();
        let style = WindowsStyle {
            background: self.background(),
            symbol: self.symbol(),
            foreground_active: self.foreground_active(),
            foreground_inactive: self.foreground_inactive(),
            close_background: self.close_background(),
            active: self.active,
        };
        let interaction = WindowsInteraction {
            pressed: self.pressed,
            hover: std::array::from_fn(|index| self.hover[index].value),
        };

        self.canvas.resize(width as u32, height as u32);
        paint(
            &mut self.canvas,
            (width as u32, height as u32),
            &buttons,
            container_height,
            &style,
            &interaction,
            scale,
        );
        self.canvas.resolve_into(&mut self.pixels);
    }

    fn present(&mut self) {
        let now = Instant::now();
        let mut animating = false;
        for animation in &mut self.hover {
            animating |= animation.advance(now);
        }
        self.render();
        let (_, _, width, height) = self.bounds();
        let _ = self.update_layered(width, height);
        self.set_timer(animating);
    }

    fn set_timer(&mut self, running: bool) {
        if running == self.timer {
            return;
        }
        self.timer = running;
        unsafe {
            if running {
                SetTimer(Some(self.hwnd), ANIMATION_TIMER, FRAME_INTERVAL_MS, None);
            } else {
                let _ = KillTimer(Some(self.hwnd), ANIMATION_TIMER);
            }
        }
    }

    fn update_layered(&self, width: i32, height: i32) -> Result<()> {
        let info = bitmap_info(width, height);

        unsafe {
            let screen = GetDC(None);
            let mem = CreateCompatibleDC(Some(screen));
            let mut bits: *mut c_void = std::ptr::null_mut();
            let dib = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .map_err(win_err)?;
            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(
                    self.pixels.as_ptr(),
                    bits as *mut u8,
                    self.pixels.len(),
                );
            }
            let previous = SelectObject(mem, HGDIOBJ(dib.0));

            let destination = POINT { x: 0, y: 0 };
            let size = SIZE {
                cx: width,
                cy: height,
            };
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let result = UpdateLayeredWindow(
                self.hwnd,
                None,
                None,
                Some(&size),
                Some(mem),
                Some(&destination),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )
            .map_err(win_err);

            SelectObject(mem, previous);
            let _ = DeleteObject(HGDIOBJ(dib.0));
            let _ = DeleteDC(mem);
            ReleaseDC(None, screen);

            result
        }
    }

    fn set_hovered(&mut self, hovered: Option<ButtonKind>) {
        if self.hovered == hovered {
            return;
        }
        self.hovered = hovered;
        for kind in ButtonKind::ALL {
            if self.pressed == Some(kind) {
                continue;
            }
            if self.hovered == Some(kind) {
                self.hover[kind.index()].show();
            } else {
                self.hover[kind.index()].hide();
            }
        }
    }

    fn press(&mut self, button: Option<ButtonKind>) {
        self.pressed = button;
        self.hovered = button;
        for kind in ButtonKind::ALL {
            if self.pressed == Some(kind) {
                // `Button::SetState` -> PRESSED resets the hover animation and
                // paints the pressed alpha instead.
                self.hover[kind.index()].reset(0.0);
            } else if self.hovered == Some(kind) {
                self.hover[kind.index()].show();
            } else {
                self.hover[kind.index()].hide();
            }
        }
    }

    fn release(&mut self, button: Option<ButtonKind>) -> Option<ButtonKind> {
        let pressed = self.pressed.take();
        self.hovered = button;
        if let Some(kind) = pressed {
            if button == Some(kind) {
                // PRESSED -> HOVERED jumps straight to fully hovered.
                self.hover[kind.index()].reset(1.0);
            } else {
                self.hover[kind.index()].reset(0.0);
            }
        }
        for kind in ButtonKind::ALL {
            if pressed == Some(kind) {
                continue;
            }
            if self.hovered == Some(kind) {
                self.hover[kind.index()].show();
            } else {
                self.hover[kind.index()].hide();
            }
        }
        pressed.filter(|kind| button == Some(*kind))
    }

    fn cancel_press(&mut self) {
        if self.pressed.take().is_none() {
            return;
        }
        self.hovered = None;
        for kind in ButtonKind::ALL {
            self.hover[kind.index()].hide();
        }
    }
}

fn bitmap_info(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            // A negative height requests a top-down DIB, matching our pixel buffer.
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [Default::default(); 1],
    }
}

/// Windows 11's own caption buttons use a darker close tint than Chromium's
/// constant. `RtlGetVersion` is the only reliable version query: `GetVersionEx`
/// lies unless the manifest opts in, and the whole point here is that it may not.
fn windows_11_or_greater() -> bool {
    static WINDOWS_11: OnceLock<bool> = OnceLock::new();
    *WINDOWS_11.get_or_init(|| unsafe {
        let Ok(ntdll) = GetModuleHandleW(windows::core::w!("ntdll.dll")) else {
            return false;
        };
        let Some(proc) =
            GetProcAddress(ntdll, windows::core::PCSTR(c"RtlGetVersion".as_ptr() as _))
        else {
            return false;
        };
        let rtl_get_version: unsafe extern "system" fn(*mut OsVersionInfoW) -> i32 =
            std::mem::transmute(proc);
        let mut info = OsVersionInfoW {
            size: std::mem::size_of::<OsVersionInfoW>() as u32,
            ..Default::default()
        };
        rtl_get_version(&mut info) >= 0 && info.build >= 22000
    })
}

#[repr(C)]
struct OsVersionInfoW {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform: u32,
    service_pack: [u16; 128],
}

impl Default for OsVersionInfoW {
    fn default() -> Self {
        Self {
            size: 0,
            major: 0,
            minor: 0,
            build: 0,
            platform: 0,
            service_pack: [0; 128],
        }
    }
}

/// `GetSysColor(COLOR_BTNFACE)` — Electron's default `titleBarOverlay.color`.
fn system_background() -> Rgba {
    from_colorref(unsafe { GetSysColor(COLOR_BTNFACE) })
}

/// `GetSysColor(COLOR_BTNTEXT)` — Electron's default `titleBarOverlay.symbolColor`.
fn system_symbol() -> Rgba {
    from_colorref(unsafe { GetSysColor(COLOR_BTNTEXT) })
}

/// `GetSysColor` hands back a `COLORREF` as a plain `0x00BBGGRR` integer.
fn from_colorref(value: u32) -> Rgba {
    Rgba::new(
        (value & 0xFF) as u8,
        ((value >> 8) & 0xFF) as u8,
        ((value >> 16) & 0xFF) as u8,
        255,
    )
}

fn hit_test_code(kind: Option<ButtonKind>) -> LRESULT {
    let code = match kind {
        Some(ButtonKind::Minimize) => HTMINBUTTON,
        Some(ButtonKind::Maximize | ButtonKind::Restore) => HTMAXBUTTON,
        Some(ButtonKind::Close) => HTCLOSE,
        None => HTCLIENT,
    };
    LRESULT(code as isize)
}

fn screen_position(lparam: LPARAM) -> (f64, f64) {
    let x = (lparam.0 & 0xFFFF) as u16 as i16;
    let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16;
    (f64::from(x), f64::from(y))
}

fn client_position(hwnd: HWND, lparam: LPARAM) -> (f64, f64) {
    let (x, y) = screen_position(lparam);
    let mut point = POINT {
        x: x as i32,
        y: y as i32,
    };
    let _ = unsafe { ScreenToClient(hwnd, &mut point) };
    (f64::from(point.x), f64::from(point.y))
}

fn mouse_position(lparam: LPARAM) -> (f64, f64) {
    let x = (lparam.0 & 0xFFFF) as u16 as i16;
    let y = ((lparam.0 >> 16) & 0xFFFF) as u16 as i16;
    (f64::from(x), f64::from(y))
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_ERASEBKGND => return LRESULT(1),
        WM_NCHITTEST => {
            let Some(label) = overlay_label(hwnd) else {
                return LRESULT(HTCLIENT as isize);
            };
            let (x, y) = client_position(hwnd, lparam);
            let kind = with_overlay(&label, |overlay| overlay.hit_test(x, y)).flatten();
            return hit_test_code(kind);
        }
        WM_SETCURSOR => {
            if let Ok(cursor) = unsafe { LoadCursorW(None, IDC_ARROW) } {
                unsafe {
                    SetCursor(Some(cursor));
                }
                return LRESULT(1);
            }
        }
        WM_NCMOUSEMOVE | WM_MOUSEMOVE => {
            let Some(label) = overlay_label(hwnd) else {
                return LRESULT(0);
            };
            let (x, y) = if message == WM_NCMOUSEMOVE {
                client_position(hwnd, lparam)
            } else {
                mouse_position(lparam)
            };
            let mut repaint = false;
            let mut track = false;
            with_overlay(&label, |overlay| {
                let hovered = overlay.hit_test(x, y);
                if overlay.hovered != hovered {
                    overlay.set_hovered(hovered);
                    repaint = true;
                }
                if !overlay.tracking_leave {
                    overlay.tracking_leave = true;
                    track = true;
                }
            });
            if track {
                register_leave_tracking(hwnd, message == WM_NCMOUSEMOVE);
            }
            if repaint {
                with_overlay(&label, |overlay| overlay.present());
            }
            return LRESULT(0);
        }
        WM_NCMOUSELEAVE | WM_MOUSELEAVE => {
            if let Some(label) = overlay_label(hwnd) {
                let mut repaint = false;
                with_overlay(&label, |overlay| {
                    overlay.tracking_leave = false;
                    if overlay.pressed.is_none() && overlay.hovered.is_some() {
                        overlay.set_hovered(None);
                        repaint = true;
                    }
                });
                if repaint {
                    with_overlay(&label, |overlay| overlay.present());
                }
            }
            return LRESULT(0);
        }
        WM_NCLBUTTONDOWN | WM_LBUTTONDOWN => {
            let Some(label) = overlay_label(hwnd) else {
                return LRESULT(0);
            };
            let (x, y) = if message == WM_NCLBUTTONDOWN {
                client_position(hwnd, lparam)
            } else {
                mouse_position(lparam)
            };
            let hit = with_overlay(&label, |overlay| overlay.hit_test(x, y)).flatten();
            let host = with_overlay(&label, |overlay| overlay.host.clone());
            if hit.is_some() {
                // Outside the borrow above: `focus` can deliver Tauri's focus
                // event synchronously, which re-enters the overlay registry.
                if let Some(host) = host {
                    host.focus();
                }
            }
            with_overlay(&label, |overlay| overlay.press(hit));
            unsafe {
                SetCapture(hwnd);
            }
            with_overlay(&label, |overlay| overlay.present());
            return LRESULT(0);
        }
        WM_NCLBUTTONUP | WM_LBUTTONUP => {
            let Some(label) = overlay_label(hwnd) else {
                return LRESULT(0);
            };
            let (x, y) = if message == WM_NCLBUTTONUP {
                client_position(hwnd, lparam)
            } else {
                mouse_position(lparam)
            };
            let clicked = with_overlay(&label, |overlay| {
                let button = overlay.hit_test(x, y);
                overlay.release(button)
            })
            .flatten();
            unsafe {
                let _ = ReleaseCapture();
            }
            if let Some(kind) = clicked {
                apply_button_action(&label, kind);
            } else {
                with_overlay(&label, |overlay| overlay.present());
            }
            return LRESULT(0);
        }
        WM_CAPTURECHANGED => {
            if let Some(label) = overlay_label(hwnd) {
                with_overlay(&label, |overlay| {
                    if overlay.pressed.is_some() {
                        overlay.cancel_press();
                        overlay.present();
                    }
                });
            }
            return LRESULT(0);
        }
        WM_TIMER => {
            if wparam.0 == ANIMATION_TIMER {
                if let Some(label) = overlay_label(hwnd) {
                    with_overlay(&label, |overlay| overlay.present());
                }
                return LRESULT(0);
            }
        }
        WM_PAINT => {
            // A layered window is only ever painted through
            // `UpdateLayeredWindow`, but it still has to validate its region.
            let mut paint_struct = PAINTSTRUCT::default();
            unsafe {
                let _ = BeginPaint(hwnd, &mut paint_struct);
                let _ = EndPaint(hwnd, &paint_struct);
            }
            return LRESULT(0);
        }
        WM_NCDESTROY => {}
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}

fn register_leave_tracking(hwnd: HWND, non_client: bool) {
    let extra = if non_client {
        TME_NONCLIENT
    } else {
        windows::Win32::UI::Input::KeyboardAndMouse::TRACKMOUSEEVENT_FLAGS(0)
    };
    let mut track = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE | extra,
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut track);
    }
}

fn overlay_label(hwnd: HWND) -> Option<String> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const String;
    if ptr.is_null() {
        None
    } else {
        Some(unsafe { (*ptr).clone() })
    }
}

fn label_for_parent(parent: HWND) -> Option<String> {
    OVERLAYS.with(|overlays| {
        let overlays = overlays.try_borrow().ok()?;
        overlays
            .iter()
            .find(|(_, overlay)| overlay.parent == parent)
            .map(|(label, _)| label.clone())
    })
}

/// Watches the host for size changes. Tauri's `WindowEvent::Resized` is queued
/// on the event loop, so relying on it leaves the band behind the new edge for
/// a frame; `WM_SIZE` is delivered before the host paints.
unsafe extern "system" fn host_subclass_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _ref_data: usize,
) -> LRESULT {
    if (message == WM_SIZE || message == WM_DPICHANGED)
        && let Some(label) = label_for_parent(hwnd)
    {
        let _ = with_overlay(&label, |overlay| overlay.reposition());
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn apply_button_action(label: &str, kind: ButtonKind) {
    let host = with_overlay(label, |overlay| overlay.host.clone());
    let Some(host) = host else {
        return;
    };
    match kind {
        ButtonKind::Minimize => host.minimize(),
        ButtonKind::Maximize | ButtonKind::Restore => host.toggle_maximize(),
        ButtonKind::Close if host.is_closable() => host.close(),
        ButtonKind::Close => {}
    }
    with_overlay(label, |overlay| overlay.relayout());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn after(duration: Duration) -> Instant {
        Instant::now() + duration
    }

    #[test]
    fn hover_fades_in_over_150ms_with_ease_out() {
        let mut animation = HoverAnimation::default();
        animation.show();
        animation.advance(after(Duration::from_millis(75)));
        // Tween::EASE_OUT(0.5) = 0.75, not 0.5
        assert!((animation.value - 0.75).abs() < 0.05, "{}", animation.value);
        animation.advance(after(HOVER_DURATION + Duration::from_millis(5)));
        assert_eq!(animation.value, 1.0);
    }

    #[test]
    fn hover_fades_out_over_150ms() {
        let mut animation = HoverAnimation::default();
        animation.reset(1.0);
        animation.hide();
        animation.advance(after(Duration::from_millis(75)));
        assert!((animation.value - 0.25).abs() < 0.05, "{}", animation.value);
        animation.advance(after(HOVER_DURATION + Duration::from_millis(5)));
        assert_eq!(animation.value, 0.0);
    }

    #[test]
    fn a_press_jumps_instead_of_animating() {
        let mut animation = HoverAnimation::default();
        animation.reset(1.0);
        animation.show();
        animation.reset(0.0);
        assert_eq!(animation.value, 0.0);
        assert!(!animation.advance(after(Duration::from_millis(16))));
    }

    #[test]
    fn reversing_midway_shortens_the_remaining_run() {
        let mut animation = HoverAnimation::default();
        animation.show();
        animation.advance(after(Duration::from_millis(75)));
        let midway = animation.value;
        animation.hide();
        // the run back down is scaled by how far it had come
        assert!(animation.duration < HOVER_DURATION);
        animation.advance(after(Duration::from_millis(5)));
        assert!(animation.value < midway);
    }
}
