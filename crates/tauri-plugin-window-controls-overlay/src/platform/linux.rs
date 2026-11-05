//! Linux implementation of the window controls overlay.
//!
//! Electron 44 draws its Linux window controls with
//! `FreedesktopNavButtonProvider` — theme-styled pills plus the desktop's own
//! `window-*-symbolic` icons — laid out by `ElectronFrameViewLayoutLinux`. Tauri
//! hands the whole GTK window to the web view, so here the overlay is a
//! [`gtk::Overlay`] child floating above it: the window's `GtkBox` is re-parented
//! under an overlay and small `GtkDrawingArea`s carrying the buttons are added as
//! overlay children.
//!
//! Which buttons exist, and which side of the title bar they go on, comes from
//! the desktop's `gtk-decoration-layout` setting — the same one Chromium reads —
//! so a desktop configured with the buttons on the left gets them on the left.
//!
//! Two button styles are implemented, selected by the GTK theme name exactly as
//! `DetectButtonStyle` does: Adwaita (the fallback) and Breeze (Plasma).

use std::cell::RefCell;
use std::collections::HashMap;

use gtk::prelude::*;

use super::{
    Host, NativeHandles, OverlayConfig,
    glyphs::{self, ButtonKind},
};
use crate::{
    Error, Result,
    color::Rgba,
    models::{OverlayGeometry, Rect},
    paint::{Canvas, Image, RectF},
};

/// `kDefaultBandHeight`: the caption height, and so the default overlay height.
const DEFAULT_BAND_HEIGHT: f64 = 32.0;
/// `kNominalIconSize`.
const NOMINAL_ICON_SIZE: f64 = 16.0;
/// `views::FrameCaptionButton::GetInactiveButtonColorAlphaRatio()`.
const INACTIVE_ALPHA_RATIO: f64 = 0.55;

/// Fallbacks for the title bar color when the GTK theme exposes no
/// `theme_bg_color`; the dark value is Electron's own fallback.
const FALLBACK_DARK_BACKGROUND: Rgba = Rgba::opaque(0x2E, 0x34, 0x36);
const FALLBACK_LIGHT_BACKGROUND: Rgba = Rgba::opaque(0xFA, 0xFA, 0xFA);

/// The Freedesktop symbolic icon for each button (`kIconNames`).
fn icon_name(kind: ButtonKind) -> &'static str {
    match kind {
        ButtonKind::Minimize => "window-minimize-symbolic",
        ButtonKind::Maximize => "window-maximize-symbolic",
        ButtonKind::Restore => "window-restore-symbolic",
        ButtonKind::Close => "window-close-symbolic",
    }
}

/// Index into [`ButtonStyle::states`]; the disabled state Electron also defines
/// is never reachable here, because the plugin never draws a disabled button.
const NORMAL: usize = 0;
const HOVERED: usize = 1;
const PRESSED: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PillColor {
    Base,
    /// [`StateStyle::accent`].
    Accent,
}

/// Upstream also has a `kContrastWithPill` variant, but neither bundled style
/// uses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GlyphColor {
    Base,
    /// The title bar background, i.e. the color the pill is "cut out" of.
    Inverse,
}

#[derive(Clone, Copy, Debug)]
struct StateStyle {
    pill: PillColor,
    /// `0.0..=1.0`; `0.0` means "no pill", which is how Electron expresses it.
    pill_alpha: f64,
    glyph: GlyphColor,
    /// Only meaningful together with [`PillColor::Accent`].
    accent: Rgba,
}

const fn state(pill: PillColor, pill_alpha: f64, glyph: GlyphColor) -> StateStyle {
    StateStyle {
        pill,
        pill_alpha,
        glyph,
        accent: Rgba::TRANSPARENT,
    }
}

#[derive(Clone, Copy, Debug)]
struct ButtonStyle {
    /// Diameter of the circular highlight; `0` means the full button size.
    pill_diameter: f64,
    /// Padding on each side of a button, inside its hit target.
    button_margin: f64,
    /// Gap between two buttons of the same group.
    inter_spacing: f64,
    /// Padding between the outermost button and the window edge.
    end_spacing: f64,
    states: [StateStyle; 3],
    close_states: [StateStyle; 3],
}

/// `AdwaitaButtonStyle()`: libadwaita's headerbar window buttons — a 24px circle
/// of the foreground color at 10% / 15% / 30% opacity.
const ADWAITA: ButtonStyle = ButtonStyle {
    pill_diameter: 24.0,
    button_margin: 5.0,
    inter_spacing: 3.0,
    end_spacing: 5.0,
    states: [
        state(PillColor::Base, 0.10, GlyphColor::Base),
        state(PillColor::Base, 0.15, GlyphColor::Base),
        state(PillColor::Base, 0.30, GlyphColor::Base),
    ],
    close_states: [
        state(PillColor::Base, 0.10, GlyphColor::Base),
        state(PillColor::Base, 0.15, GlyphColor::Base),
        state(PillColor::Base, 0.30, GlyphColor::Base),
    ],
};

/// `BreezeButtonStyle()`: Plasma's 18px circles — the glyph alone at rest,
/// inverted colors on hover, and the palette's negative color for close.
const BREEZE: ButtonStyle = {
    // KDE's small spacing: `max(2px, grid unit / 4)` for Plasma's 8px grid.
    const SMALL_SPACING: f64 = 4.0;
    const MARGIN: f64 = SMALL_SPACING / 2.0;
    const NEGATIVE_HOVER: Rgba = Rgba::opaque(0xFF, 0x98, 0xA2);
    const NEGATIVE_PRESSED: Rgba = Rgba::opaque(0x6D, 0x22, 0x29);
    // `{PillColor::kAccent, alpha, GlyphColor::kInverse, accent}`
    const fn accent(pill_alpha: f64, accent: Rgba) -> StateStyle {
        StateStyle {
            pill: PillColor::Accent,
            pill_alpha,
            glyph: GlyphColor::Inverse,
            accent,
        }
    }

    ButtonStyle {
        pill_diameter: 18.0,
        button_margin: MARGIN,
        // Plasma lays buttons out with `smallSpacing` between them, split into the
        // per-button margins; what is left over is the gap between two buttons.
        inter_spacing: SMALL_SPACING - 2.0 * MARGIN,
        // ...and twice that at the outer edge, minus the margin already contributed.
        end_spacing: 2.0 * SMALL_SPACING - MARGIN,
        states: [
            // normal: the glyph on its own
            state(PillColor::Base, 0.0, GlyphColor::Base),
            state(PillColor::Base, 1.0, GlyphColor::Inverse),
            state(PillColor::Base, 0.5, GlyphColor::Inverse),
        ],
        close_states: [
            state(PillColor::Base, 0.0, GlyphColor::Base),
            accent(1.0, NEGATIVE_HOVER),
            accent(1.0, NEGATIVE_PRESSED),
        ],
    }
};

/// `DetectButtonStyle`: Breeze themes get the Breeze treatment; everything else
/// — including an unset or unknown theme — falls back to Adwaita.
fn detect_style(theme_name: &str) -> ButtonStyle {
    let is_breeze = theme_name
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Breeze"));
    if is_breeze { BREEZE } else { ADWAITA }
}

fn theme_name() -> String {
    gtk::Settings::default()
        .and_then(|settings| settings.gtk_theme_name())
        .map(|name| name.to_string())
        .unwrap_or_default()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct DecorationLayout {
    leading: Vec<ButtonKind>,
    trailing: Vec<ButtonKind>,
}

impl DecorationLayout {
    /// What GTK and Chromium fall back to when the setting is unset or has no
    /// window buttons at all: the three standard buttons on the right.
    fn default_order() -> Self {
        Self {
            leading: Vec::new(),
            trailing: vec![
                ButtonKind::Minimize,
                ButtonKind::Maximize,
                ButtonKind::Close,
            ],
        }
    }

    fn is_empty(&self) -> bool {
        self.leading.is_empty() && self.trailing.is_empty()
    }
}

fn decoration_layout() -> DecorationLayout {
    let value = gtk::Settings::default()
        .and_then(|settings| settings.gtk_decoration_layout())
        .map(|value| value.to_string())
        .filter(|value| !value.is_empty());
    match value {
        Some(value) => {
            let layout = parse_decoration_layout(&value);
            if layout.is_empty() {
                DecorationLayout::default_order()
            } else {
                layout
            }
        }
        None => DecorationLayout::default_order(),
    }
}

/// Parses `gtk-decoration-layout`, e.g. `":minimize,maximize,close"` or
/// `"close,minimize,maximize:"` (buttons on the left).
///
/// The setting is `"leading:trailing"` with comma-separated button names.
/// Chromium reads the same string and only understands the window buttons, so
/// everything else the format allows (`menu`, `appmenu`, `icon`, `spacer`) is
/// ignored here too.
fn parse_decoration_layout(value: &str) -> DecorationLayout {
    let (leading, trailing) = match value.split_once(':') {
        Some((leading, trailing)) => (leading, trailing),
        // A value without the separator is malformed; treat it as the trailing side,
        // which is where window buttons conventionally live.
        None => ("", value),
    };
    DecorationLayout {
        leading: parse_buttons(leading),
        trailing: parse_buttons(trailing),
    }
}

fn parse_buttons(side: &str) -> Vec<ButtonKind> {
    side.split(',')
        .filter_map(|token| match token.trim() {
            "minimize" => Some(ButtonKind::Minimize),
            "maximize" => Some(ButtonKind::Maximize),
            "close" => Some(ButtonKind::Close),
            _ => None,
        })
        .collect()
}

/// Which edge of the title bar a group of buttons is anchored to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Leading,
    Trailing,
}

/// A laid-out navigation button, in logical pixels relative to its group.
#[derive(Clone, Copy, Debug, PartialEq)]
struct NavButton {
    kind: ButtonKind,
    /// The full hit target: the pill plus the theme's margins on both sides.
    /// Electron sizes the hit target for Fitts' law, not just the visible circle.
    hit: RectF,
    /// The pill's diameter, clamped to the band height.
    pill_diameter: f64,
}

fn layout_group(kinds: &[ButtonKind], height: f64, style: &ButtonStyle) -> (Vec<NavButton>, f64) {
    // `width = pill_diameter > 0 ? min(pill_diameter, height) : height`.
    let pill = if style.pill_diameter > 0.0 {
        style.pill_diameter.min(height)
    } else {
        height
    }
    .max(0.0);

    let mut x = style.end_spacing;
    let mut buttons = Vec::with_capacity(kinds.len());
    for (index, kind) in kinds.iter().copied().enumerate() {
        let hit = RectF::new(x, 0.0, pill + style.button_margin * 2.0, height);
        buttons.push(NavButton {
            kind,
            hit,
            pill_diameter: pill,
        });
        x = hit.right();
        if index + 1 < kinds.len() {
            x += style.inter_spacing;
        }
    }
    let width = if kinds.is_empty() {
        0.0
    } else {
        x + style.end_spacing
    };
    (buttons, width)
}

#[derive(Clone, Debug, PartialEq)]
struct Layout {
    leading: Vec<NavButton>,
    trailing: Vec<NavButton>,
    leading_width: f64,
    trailing_width: f64,
    /// Where the trailing group starts.
    trailing_x: f64,
}

impl Layout {
    /// `client_width` is the window's width in logical pixels.
    fn compute(
        layout: &DecorationLayout,
        height: f64,
        style: &ButtonStyle,
        client_width: f64,
    ) -> Self {
        let (leading, leading_width) = layout_group(&layout.leading, height, style);
        let (trailing, trailing_width) = layout_group(&layout.trailing, height, style);
        // The trailing group hugs the trailing edge; clamp so a pathologically
        // narrow window cannot make the two groups overlap.
        let trailing_x = (client_width - trailing_width).max(leading_width);
        let trailing = trailing
            .into_iter()
            .map(|mut button| {
                button.hit = button.hit.translated(trailing_x, 0.0);
                button
            })
            .collect();
        Self {
            leading,
            trailing,
            leading_width,
            trailing_width,
            trailing_x,
        }
    }

    fn buttons(&self, side: Side) -> &[NavButton] {
        match side {
            Side::Leading => &self.leading,
            Side::Trailing => &self.trailing,
        }
    }

    fn width(&self, side: Side) -> f64 {
        match side {
            Side::Leading => self.leading_width,
            Side::Trailing => self.trailing_width,
        }
    }

    /// The x coordinate the group's local origin maps to.
    fn origin(&self, side: Side) -> f64 {
        match side {
            Side::Leading => 0.0,
            Side::Trailing => self.trailing_x,
        }
    }

    /// `x`/`y` are group-local logical pixels.
    fn hit_test(&self, side: Side, x: f64, y: f64) -> Option<ButtonKind> {
        let x = x + self.origin(side);
        self.leading
            .iter()
            .chain(self.trailing.iter())
            .find(|button| button.hit.contains(x, y))
            .map(|button| button.kind)
    }
}

/// Keeps the buttons the desktop asked for that this window can actually use,
/// swapping maximize for restore when the window is maximized.
fn visible_buttons(
    kinds: &[ButtonKind],
    maximized: bool,
    minimizable: bool,
    maximizable: bool,
    closable: bool,
) -> Vec<ButtonKind> {
    kinds
        .iter()
        .filter_map(|kind| match kind {
            ButtonKind::Minimize if minimizable => Some(ButtonKind::Minimize),
            ButtonKind::Maximize | ButtonKind::Restore if maximizable => Some(if maximized {
                ButtonKind::Restore
            } else {
                ButtonKind::Maximize
            }),
            ButtonKind::Close if closable => Some(ButtonKind::Close),
            _ => None,
        })
        .collect()
}

/// An image loaded from the icon theme, in unpremultiplied RGBA.
#[derive(Clone, Debug)]
struct IconData {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// Supplies the themed glyph for a button, or `None` to fall back to the vector
/// shapes. Injected so [`paint_band`] stays testable without a display.
type IconProvider<'a> = dyn Fn(ButtonKind, Rgba, u32) -> Option<IconData> + 'a;

/// What a rasterized symbolic icon is keyed by: the button, the glyph color and
/// the device size. The color changes with the button state, so it is part of
/// the key.
type IconKey = (ButtonKind, [u8; 4], u32);

thread_local! {
  static ICON_CACHE: RefCell<HashMap<IconKey, Option<IconData>>> = RefCell::new(HashMap::new());
}

/// `GtkSymbolicIconProvider::LoadIcon`: resolves a `window-*-symbolic` icon from
/// the theme and recolors it to `color`.
///
/// `size` is in device pixels; the theme is asked for the matching logical size.
fn load_icon(kind: ButtonKind, color: Rgba, size: u32, scale: i32) -> Option<IconData> {
    let key = (kind, color.to_array(), size);
    if let Some(cached) = ICON_CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return cached;
    }
    let loaded = lookup_icon(kind, color, size, scale);
    ICON_CACHE.with(|cache| cache.borrow_mut().insert(key, loaded.clone()));
    loaded
}

fn lookup_icon(kind: ButtonKind, color: Rgba, size: u32, scale: i32) -> Option<IconData> {
    let scale = scale.max(1);
    let theme = gtk::IconTheme::default();
    let logical_size = (size as i32 / scale).max(1);
    let paintable = theme.lookup_icon(
        icon_name(kind),
        &[],
        logical_size,
        scale,
        gtk::TextDirection::None,
        gtk::IconLookupFlags::empty(),
    );
    // A full-color icon would lose its palette in the recoloring below, so only
    // symbolic icons go through this path.
    if !paintable.is_symbolic() {
        return None;
    }
    let path = paintable.file()?.path()?;
    let device_size = i32::try_from(size).ok()?;
    let pixbuf =
        gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(&path, device_size, device_size, true).ok()?;
    if pixbuf.n_channels() != 4 {
        return None;
    }

    let width = u32::try_from(pixbuf.width()).ok()?;
    let height = u32::try_from(pixbuf.height()).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let stride = usize::try_from(pixbuf.rowstride()).ok()?;
    let bytes = pixbuf.read_pixel_bytes();
    let data = bytes.as_ref();
    let row = width as usize * 4;

    // Pixbuf rows are padded to `rowstride`, and our canvas wants them tight.
    // GTK4 dropped `load_symbolic`, so the requested color replaces the RGB
    // channels while the icon's alpha keeps the shape.
    let mut pixels = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        let start = y * stride;
        let source = data.get(start..start + row)?;
        for pixel in source.as_chunks::<4>().0 {
            pixels.extend_from_slice(&[color.r, color.g, color.b, pixel[3]]);
        }
    }
    Some(IconData {
        width,
        height,
        pixels,
    })
}

#[derive(Clone, Copy, Debug)]
struct BandStyle {
    /// Fills the whole band, and doubles as `inverse` — the color a glyph is drawn
    /// in when it is "cut out" of the pill (Breeze's hovered and pressed states).
    background: Rgba,
    /// `base`: the glyph color.
    base: Rgba,
    active: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct BandInteraction {
    hovered: Option<ButtonKind>,
    pressed: Option<ButtonKind>,
}

#[derive(Clone, Copy)]
struct BandPaint<'a> {
    /// The target surface size in device pixels.
    size: (u32, u32),
    /// The band's x offset in window coordinates; `buttons` is laid out in window
    /// coordinates too.
    origin: f64,
    /// Device pixels per logical pixel.
    scale: f64,
    style: &'a ButtonStyle,
    colors: &'a BandStyle,
    interaction: &'a BandInteraction,
    icons: &'a IconProvider<'a>,
}

/// Paints one band. Deliberately free of GTK so the Freedesktop look can be
/// unit-tested without a display.
fn paint_band(canvas: &mut Canvas, buttons: &[NavButton], paint: &BandPaint<'_>) {
    let BandPaint {
        size,
        origin,
        scale,
        style,
        colors,
        interaction,
        icons,
    } = *paint;

    canvas.fill_rect(
        RectF::new(0.0, 0.0, f64::from(size.0), f64::from(size.1)),
        colors.background,
    );

    for button in buttons {
        let hit = RectF::new(
            (button.hit.x - origin) * scale,
            button.hit.y * scale,
            button.hit.width * scale,
            button.hit.height * scale,
        );

        let pressed = interaction.pressed == Some(button.kind);
        let hovered = interaction.hovered == Some(button.kind);
        let states = if button.kind == ButtonKind::Close {
            &style.close_states
        } else {
            &style.states
        };
        let state = if pressed {
            states[PRESSED]
        } else if hovered {
            states[HOVERED]
        } else {
            states[NORMAL]
        };

        // `pill` is the accent for the Breeze close button, and the glyph color
        // otherwise — even when no pill is painted, because the glyph color can
        // still be derived from it.
        let pill = if state.pill == PillColor::Accent {
            state.accent
        } else {
            colors.base
        };
        if state.pill_alpha > 0.0 {
            let (cx, cy) = hit.center();
            let diameter = (button.pill_diameter * scale)
                .min(hit.width)
                .min(hit.height);
            canvas.fill_circle(
                cx,
                cy,
                diameter / 2.0,
                pill.with_alpha(alpha_to_u8(state.pill_alpha)),
            );
        }

        let mut glyph = match state.glyph {
            GlyphColor::Base => colors.base,
            GlyphColor::Inverse => colors.background,
        };
        if !colors.active {
            glyph = glyph.with_alpha(alpha_to_u8(
                f64::from(glyph.a) / 255.0 * INACTIVE_ALPHA_RATIO,
            ));
        }

        // `icon_size = min(kNominalIconSize, min(width, height) - 2)`.
        let icon_size = NOMINAL_ICON_SIZE.min(button.pill_diameter - 2.0).max(1.0);
        let device_size = (icon_size * scale).round().max(1.0) as u32;
        let symbol = glyphs::symbol_rect(hit, icon_size, scale);
        match icons(button.kind, glyph, device_size) {
            Some(icon) => canvas.draw_image(
                &Image {
                    width: icon.width,
                    height: icon.height,
                    pixels: &icon.pixels,
                },
                symbol,
            ),
            // The icon theme had nothing usable: fall back to the vector shapes.
            None => glyphs::paint(
                canvas,
                button.kind,
                symbol,
                glyph,
                (1.5 * scale).round().max(1.0),
                scale,
            ),
        }
    }
}

fn alpha_to_u8(alpha: f64) -> u8 {
    (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn rgba_from_gdk(rgba: gtk::gdk::RGBA) -> Rgba {
    Rgba::new(
        (rgba.red() * 255.0).round() as u8,
        (rgba.green() * 255.0).round() as u8,
        (rgba.blue() * 255.0).round() as u8,
        (rgba.alpha() * 255.0).round() as u8,
    )
}

thread_local! {
  static OVERLAYS: RefCell<HashMap<String, Overlay>> = RefCell::new(HashMap::new());
  static SETTINGS_WATCHED: RefCell<bool> = const { RefCell::new(false) };
}

fn with_overlay<R>(label: &str, f: impl FnOnce(&mut Overlay) -> R) -> Option<R> {
    OVERLAYS.with(|overlays| {
        let mut overlays = overlays.try_borrow_mut().ok()?;
        overlays.get_mut(label).map(f)
    })
}

struct Band {
    side: Side,
    /// A `GtkDrawingArea` owns a `GdkWindow` of its own, so the same widget is
    /// both the surface that gets painted and the event target the pointer lands
    /// on, and the pointer coordinates the handlers see are band-local.
    area: gtk::DrawingArea,
    canvas: Canvas,
    pixels: Vec<u8>,
}

struct Overlay {
    window: gtk::ApplicationWindow,
    vbox: gtk::Box,
    overlay: gtk::Overlay,
    bands: Vec<Band>,
    host: Host,
    config: OverlayConfig,
    active: bool,
    hovered: Option<ButtonKind>,
    pressed: Option<ButtonKind>,
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

    let NativeHandles { window, vbox } = handles;

    // Re-parent the window's content box under an overlay so the button bands can
    // float above the web view.
    let overlay = gtk::Overlay::new();
    if vbox.parent().is_some() {
        vbox.unparent();
    }
    overlay.set_child(Some(&vbox));

    let mut bands = Vec::with_capacity(2);
    for side in [Side::Leading, Side::Trailing] {
        let area = gtk::DrawingArea::new();
        area.set_widget_name(label);
        area.set_halign(match side {
            Side::Leading => gtk::Align::Start,
            Side::Trailing => gtk::Align::End,
        });
        area.set_valign(gtk::Align::Start);
        area.set_size_request(1, 1);
        overlay.add_overlay(&area);

        bands.push(Band {
            side,
            area,
            canvas: Canvas::new(1, 1),
            pixels: Vec::new(),
        });
    }

    window.set_child(Some(&overlay));

    // The surface layout handler below needs a window handle that outlives the
    // `Overlay`, which takes ownership of `window`.
    let resized_window = window.clone();
    OVERLAYS.with(|overlays| {
        overlays.borrow_mut().insert(
            label.to_owned(),
            Overlay {
                window,
                vbox,
                overlay: overlay.clone(),
                bands,
                host,
                config,
                active: true,
                hovered: None,
                pressed: None,
            },
        );
    });

    for side in [Side::Leading, Side::Trailing] {
        connect_band(label, side);
    }
    watch_settings();

    // Tauri dispatches `Resized` from the X surface event, which can fire before
    // GTK has applied the new allocation. `layout` runs after the surface has
    // been laid out, so the published geometry (and `--titlebar-area-width`)
    // follows the window while it is being resized.
    if let Some(surface) = resized_window.surface() {
        let resized_label = label.to_owned();
        surface.connect_layout(move |_, _, _| {
            let _ = relayout(&resized_label);
        });
    }

    relayout(label)
}

fn label_of(widget: &impl IsA<gtk::Widget>) -> Option<String> {
    let name = widget.widget_name().to_string();
    if OVERLAYS.with(|overlays| overlays.borrow().contains_key(&name)) {
        Some(name)
    } else {
        None
    }
}

fn update_hover(label: &str, side: Side, x: f64, y: f64) {
    let changed = with_overlay(label, |overlay| {
        let hovered = overlay.layout().hit_test(side, x, y);
        if overlay.hovered == hovered {
            false
        } else {
            overlay.hovered = hovered;
            true
        }
    });
    if changed == Some(true) {
        with_overlay(label, |overlay| overlay.queue_draw());
    }
}

fn connect_band(label: &str, side: Side) {
    let area = with_overlay(label, |overlay| {
        overlay
            .bands
            .iter()
            .find(|band| band.side == side)
            .map(|band| band.area.clone())
    })
    .flatten();
    let Some(area) = area else {
        return;
    };

    area.set_draw_func(move |area, cr, _, _| {
        if let Some(label) = label_of(area) {
            with_overlay(&label, |overlay| overlay.draw(side, cr));
        }
    });

    let motion = gtk::EventControllerMotion::new();
    {
        let area = area.clone();
        motion.connect_motion(move |_, x, y| {
            if let Some(label) = label_of(&area) {
                update_hover(&label, side, x, y);
            }
        });
    }
    // Moving the pointer straight into the band, when it appears under a
    // stationary pointer or when the pointer simply stops on a button, only
    // produces an enter event, not a motion one, so the hover has to be taken
    // from that too. Without this the first button the pointer reaches never
    // highlights.
    {
        let area = area.clone();
        motion.connect_enter(move |_, x, y| {
            if let Some(label) = label_of(&area) {
                update_hover(&label, side, x, y);
            }
        });
    }
    {
        let area = area.clone();
        motion.connect_leave(move |_| {
            if let Some(label) = label_of(&area) {
                let changed = with_overlay(&label, |overlay| overlay.hovered.take().is_some());
                if changed == Some(true) {
                    with_overlay(&label, |overlay| overlay.queue_draw());
                }
            }
        });
    }
    area.add_controller(motion);

    let click = gtk::GestureClick::new();
    click.set_button(gtk::gdk::BUTTON_PRIMARY);
    {
        let area = area.clone();
        click.connect_pressed(move |gesture, _, x, y| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            if let Some(label) = label_of(&area) {
                with_overlay(&label, |overlay| {
                    let button = overlay.layout().hit_test(side, x, y);
                    overlay.pressed = button;
                    overlay.hovered = button;
                    overlay.queue_draw();
                });
            }
        });
    }
    {
        let area = area.clone();
        click.connect_released(move |_, _, x, y| {
            let Some(label) = label_of(&area) else {
                return;
            };
            let clicked = with_overlay(&label, |overlay| {
                let pressed = overlay.pressed.take();
                let hovered = overlay.layout().hit_test(side, x, y);
                overlay.hovered = hovered;
                match pressed {
                    Some(kind) if Some(kind) == hovered => Some(kind),
                    _ => None,
                }
            })
            .flatten();
            if let Some(kind) = clicked {
                apply_button_action(&label, kind);
            }
            with_overlay(&label, |overlay| overlay.queue_draw());
        });
    }
    area.add_controller(click);
}

fn watch_settings() {
    let already_watched = SETTINGS_WATCHED.with(|watched| watched.replace(true));
    if already_watched {
        return;
    }
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    let relayout_all = |_settings: &gtk::Settings| {
        // `relayout` re-enters the registry, so collect the labels first.
        let labels =
            OVERLAYS.with(|overlays| overlays.borrow().keys().cloned().collect::<Vec<_>>());
        for label in labels {
            let _ = relayout(&label);
        }
    };
    settings.connect_gtk_decoration_layout_notify(relayout_all);
    settings.connect_gtk_theme_name_notify(relayout_all);
}

pub(crate) fn set_config(label: &str, config: OverlayConfig) -> Result<OverlayGeometry> {
    let found = with_overlay(label, |overlay| overlay.config = config);
    if found.is_none() {
        return Err(Error::NotEnabled);
    }
    relayout(label)
}

pub(crate) fn geometry(label: &str) -> Result<OverlayGeometry> {
    with_overlay(label, |overlay| overlay.geometry()).ok_or(Error::NotEnabled)
}

pub(crate) fn relayout(label: &str) -> Result<OverlayGeometry> {
    with_overlay(label, |overlay| {
        overlay.relayout();
        overlay.geometry()
    })
    .ok_or(Error::NotEnabled)
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
        overlay.queue_draw();
    });
}

pub(crate) fn destroy(label: &str) {
    let overlay = OVERLAYS.with(|overlays| overlays.borrow_mut().remove(label));
    let Some(overlay) = overlay else {
        return;
    };
    for band in &overlay.bands {
        band.area.unparent();
    }
    overlay.vbox.unparent();
    overlay.overlay.unparent();
    if overlay.window.is_realized() {
        overlay.window.set_child(Some(&overlay.vbox));
    }
}

/// Drops the overlay of a window that is being destroyed. GTK owns the widget
/// tree at that point, so nothing is re-parented back.
pub(crate) fn forget(label: &str) {
    OVERLAYS.with(|overlays| {
        overlays.borrow_mut().remove(label);
    });
}

impl Overlay {
    fn style(&self) -> ButtonStyle {
        detect_style(&theme_name())
    }

    /// `ElectronFrameViewLayoutLinux::GetWCOContentHeight()`.
    fn container_height(&self) -> f64 {
        self.config.height.unwrap_or(DEFAULT_BAND_HEIGHT)
    }

    fn client_width(&self) -> f64 {
        f64::from(self.window.width())
    }

    fn layout(&self) -> Layout {
        let requested = decoration_layout();
        let maximized = self.host.is_maximized();
        let (minimizable, maximizable, closable) = (
            self.host.is_minimizable(),
            self.host.is_maximizable(),
            self.host.is_closable(),
        );
        let layout = DecorationLayout {
            leading: visible_buttons(
                &requested.leading,
                maximized,
                minimizable,
                maximizable,
                closable,
            ),
            trailing: visible_buttons(
                &requested.trailing,
                maximized,
                minimizable,
                maximizable,
                closable,
            ),
        };
        Layout::compute(
            &layout,
            self.container_height(),
            &self.style(),
            self.client_width(),
        )
    }

    /// `titleBarOverlay.color`, else the GTK theme's window color — Electron's
    /// `ui::kColorFrameActive` fallback.
    #[allow(deprecated)]
    fn background(&self) -> Rgba {
        if let Some(color) = self.config.color {
            return color;
        }
        if let Some(rgba) = self.window.style_context().lookup_color("theme_bg_color") {
            return rgba_from_gdk(rgba);
        }
        let prefers_dark = gtk::Settings::default()
            .map(|settings| settings.is_gtk_application_prefer_dark_theme())
            .unwrap_or(true);
        if prefers_dark {
            FALLBACK_DARK_BACKGROUND
        } else {
            FALLBACK_LIGHT_BACKGROUND
        }
    }

    /// `titleBarOverlay.symbolColor`, else the color with the best contrast
    /// against the title bar (`views::FrameCaptionButton::GetButtonColor`).
    fn symbol(&self) -> Rgba {
        self.config
            .symbol_color
            .unwrap_or_else(|| self.background().with_alpha(255).contrasting())
    }

    /// The geometry published to the web view. `titlebar_area` is the free area
    /// *between* the button groups, matching Chromium's
    /// `GetWindowControlsOverlayRect`.
    fn geometry(&self) -> OverlayGeometry {
        let width = self.client_width();
        let height = self.container_height();
        let layout = self.layout();
        let free = (width - layout.leading_width - layout.trailing_width).max(0.0);

        let window_controls = if layout.trailing.is_empty() {
            Rect::default()
        } else {
            Rect::new(
                layout.trailing_x,
                0.0,
                layout.trailing_width.min(width),
                height,
            )
        };
        let leading_window_controls = if layout.leading.is_empty() {
            Rect::default()
        } else {
            Rect::new(0.0, 0.0, layout.leading_width.min(width), height)
        };

        OverlayGeometry {
            visible: !self.host.is_fullscreen() && free > 0.0,
            titlebar_area: Rect::new(layout.leading_width, 0.0, free, height),
            window_controls,
            leading_window_controls,
        }
    }

    fn relayout(&mut self) {
        let fullscreen = self.host.is_fullscreen();
        let height = self.container_height();
        let layout = self.layout();
        for band in &mut self.bands {
            let width = layout.width(band.side);
            let empty = layout.buttons(band.side).is_empty();
            band.area
                .set_size_request(width.ceil().max(1.0) as i32, height.ceil().max(1.0) as i32);
            band.area.set_visible(!fullscreen && !empty);
        }
        self.queue_draw();
        let geometry = self.geometry();
        self.host.emit_geometry(geometry);
    }

    fn queue_draw(&self) {
        for band in &self.bands {
            band.area.queue_draw();
        }
    }

    fn band(&mut self, side: Side) -> Option<&mut Band> {
        self.bands.iter_mut().find(|band| band.side == side)
    }

    fn draw(&mut self, side: Side, cr: &gtk::cairo::Context) {
        // Computed before the band is borrowed mutably.
        let layout = self.layout();
        let buttons = layout.buttons(side).to_vec();
        let origin = layout.origin(side);
        let style = self.style();
        let colors = BandStyle {
            background: self.background(),
            base: self.symbol(),
            active: self.active,
        };
        let interaction = BandInteraction {
            hovered: self.hovered,
            pressed: self.pressed,
        };

        let Some(band) = self.band(side) else {
            return;
        };
        let device_scale = band.area.scale_factor().max(1);
        let scale = f64::from(device_scale);
        let width = (f64::from(band.area.width()) * scale).round().max(1.0) as u32;
        let height = (f64::from(band.area.height()) * scale).round().max(1.0) as u32;

        band.canvas.resize(width, height);
        let icons =
            |kind: ButtonKind, color: Rgba, size: u32| load_icon(kind, color, size, device_scale);
        paint_band(
            &mut band.canvas,
            &buttons,
            &BandPaint {
                size: (width, height),
                origin,
                scale,
                style: &style,
                colors: &colors,
                interaction: &interaction,
                icons: &icons,
            },
        );
        band.canvas.resolve_into(&mut band.pixels);

        let Ok(surface) = gtk::cairo::ImageSurface::create_for_data(
            band.pixels.clone(),
            gtk::cairo::Format::ARgb32,
            width as i32,
            height as i32,
            (width * 4) as i32,
        ) else {
            return;
        };
        // GTK hands the draw vfunc a context already scaled so that user space is
        // logical pixels; undo that so the device-resolution surface maps 1:1.
        let _ = cr.save();
        cr.scale(1.0 / scale, 1.0 / scale);
        let _ = cr.set_source_surface(&surface, 0.0, 0.0);
        let _ = cr.paint();
        let _ = cr.restore();
    }
}

fn apply_button_action(label: &str, kind: ButtonKind) {
    let host = with_overlay(label, |overlay| overlay.host.clone());
    let Some(host) = host else {
        return;
    };
    match kind {
        ButtonKind::Minimize => host.minimize(),
        ButtonKind::Maximize | ButtonKind::Restore => host.toggle_maximize(),
        ButtonKind::Close => host.close(),
    }
    let _ = relayout(label);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds() -> Vec<ButtonKind> {
        vec![
            ButtonKind::Minimize,
            ButtonKind::Maximize,
            ButtonKind::Close,
        ]
    }

    fn no_icons(_kind: ButtonKind, _color: Rgba, _size: u32) -> Option<IconData> {
        None
    }

    #[test]
    fn adwaita_puts_three_buttons_in_118_logical_pixels() {
        let (buttons, width) = layout_group(&kinds(), DEFAULT_BAND_HEIGHT, &ADWAITA);
        assert_eq!(width, 118.0);
        assert_eq!(buttons.len(), 3);
        // 5px end spacing, then a button whose hit target is the 24px pill plus 5px
        // of margin on each side.
        assert_eq!(buttons[0].hit.x, 5.0);
        assert_eq!(buttons[0].hit.width, 34.0);
        assert_eq!(buttons[0].pill_diameter, 24.0);
        assert_eq!(buttons[1].hit.x, 42.0);
        assert_eq!(buttons[2].hit.x, 79.0);
        assert_eq!(buttons[2].hit.right() + 5.0, width);
    }

    #[test]
    fn breeze_puts_three_buttons_in_78_logical_pixels() {
        let (buttons, width) = layout_group(&kinds(), DEFAULT_BAND_HEIGHT, &BREEZE);
        assert_eq!(width, 78.0);
        assert_eq!(buttons[0].hit.x, 6.0);
        assert_eq!(buttons[0].hit.width, 22.0);
        assert_eq!(buttons[0].pill_diameter, 18.0);
        assert_eq!(buttons[1].hit.x, 28.0);
        assert_eq!(buttons[2].hit.x, 50.0);
        // Plasma spaces buttons by 4px: 2px of margin on each side, no gap left.
        assert_eq!(buttons[1].hit.x - buttons[0].hit.right(), 0.0);
    }

    #[test]
    fn a_short_band_shrinks_the_pill() {
        let (buttons, _) = layout_group(&kinds(), 18.0, &ADWAITA);
        assert_eq!(buttons[0].pill_diameter, 18.0);
        assert_eq!(buttons[0].hit.width, 18.0 + 5.0 * 2.0);
    }

    #[test]
    fn breeze_gets_its_own_style() {
        assert_eq!(detect_style("Breeze").pill_diameter, BREEZE.pill_diameter);
        assert_eq!(
            detect_style("Breeze-Dark").pill_diameter,
            BREEZE.pill_diameter
        );
        assert_eq!(detect_style("breeze").pill_diameter, BREEZE.pill_diameter);
        assert_eq!(detect_style("Adwaita").pill_diameter, ADWAITA.pill_diameter);
        assert_eq!(detect_style("").pill_diameter, ADWAITA.pill_diameter);
    }

    #[test]
    fn breeze_dimming_uses_the_negative_colors_for_close() {
        assert_eq!(BREEZE.close_states[HOVERED].pill, PillColor::Accent);
        assert_eq!(
            BREEZE.close_states[HOVERED].accent,
            Rgba::opaque(0xFF, 0x98, 0xA2)
        );
        assert_eq!(
            BREEZE.close_states[PRESSED].accent,
            Rgba::opaque(0x6D, 0x22, 0x29)
        );
        // ...and the other buttons invert instead.
        assert_eq!(BREEZE.states[HOVERED].pill, PillColor::Base);
        assert_eq!(BREEZE.states[HOVERED].glyph, GlyphColor::Inverse);
        // at rest the glyph stands alone
        assert_eq!(BREEZE.states[NORMAL].pill_alpha, 0.0);
    }

    #[test]
    fn decoration_layout_parses_both_sides() {
        let layout = parse_decoration_layout("appmenu:minimize,maximize,close");
        assert_eq!(layout.leading, Vec::new());
        assert_eq!(layout.trailing, kinds());

        let layout = parse_decoration_layout("close,minimize,maximize:");
        assert_eq!(
            layout.leading,
            vec![
                ButtonKind::Close,
                ButtonKind::Minimize,
                ButtonKind::Maximize
            ]
        );
        assert_eq!(layout.trailing, Vec::new());

        let layout = parse_decoration_layout("close:minimize,maximize");
        assert_eq!(layout.leading, vec![ButtonKind::Close]);
        assert_eq!(
            layout.trailing,
            vec![ButtonKind::Minimize, ButtonKind::Maximize]
        );
    }

    #[test]
    fn decoration_layout_ignores_buttons_it_cannot_draw() {
        let layout = parse_decoration_layout("icon,appmenu:minimize,spacer,close");
        assert_eq!(layout.leading, Vec::new());
        assert_eq!(
            layout.trailing,
            vec![ButtonKind::Minimize, ButtonKind::Close]
        );
    }

    #[test]
    fn a_separator_less_layout_falls_back_to_the_trailing_side() {
        let layout = parse_decoration_layout("minimize,maximize,close");
        assert_eq!(layout.leading, Vec::new());
        assert_eq!(layout.trailing, kinds());
    }

    #[test]
    fn visible_buttons_drops_what_the_window_disables() {
        assert_eq!(
            visible_buttons(&kinds(), false, true, true, true),
            vec![
                ButtonKind::Minimize,
                ButtonKind::Maximize,
                ButtonKind::Close
            ]
        );
        assert_eq!(
            visible_buttons(&kinds(), true, true, true, true),
            vec![ButtonKind::Minimize, ButtonKind::Restore, ButtonKind::Close]
        );
        // `kinds()` has no close button when the window is not closable.
        assert_eq!(
            visible_buttons(&kinds(), false, false, false, true),
            vec![ButtonKind::Close]
        );
        assert_eq!(
            visible_buttons(&kinds(), false, true, true, false),
            vec![ButtonKind::Minimize, ButtonKind::Maximize]
        );
    }

    #[test]
    fn a_leading_group_moves_the_titlebar_area() {
        let requested = DecorationLayout {
            leading: vec![ButtonKind::Close],
            trailing: vec![ButtonKind::Minimize, ButtonKind::Maximize],
        };
        let layout = Layout::compute(&requested, DEFAULT_BAND_HEIGHT, &ADWAITA, 800.0);
        let (_, leading_width) = layout_group(&[ButtonKind::Close], DEFAULT_BAND_HEIGHT, &ADWAITA);
        assert_eq!(layout.leading_width, leading_width);
        assert_eq!(layout.trailing_x, 800.0 - layout.trailing_width);
        assert_eq!(layout.leading[0].hit.x, 5.0);
        assert_eq!(layout.trailing[0].hit.x, layout.trailing_x + 5.0);
    }

    #[test]
    fn groups_never_overlap_in_a_narrow_window() {
        let requested = DecorationLayout {
            leading: kinds(),
            trailing: kinds(),
        };
        let layout = Layout::compute(&requested, DEFAULT_BAND_HEIGHT, &ADWAITA, 100.0);
        assert_eq!(layout.trailing_x, layout.leading_width);
    }

    #[test]
    fn hit_testing_is_local_to_the_group() {
        let requested = DecorationLayout {
            leading: Vec::new(),
            trailing: kinds(),
        };
        let layout = Layout::compute(&requested, DEFAULT_BAND_HEIGHT, &ADWAITA, 800.0);
        // The trailing group starts at 800 - 118 = 682, so local x = 59 is the
        // middle of the maximize button.
        assert_eq!(
            layout.hit_test(Side::Trailing, 59.0, 24.0),
            Some(ButtonKind::Maximize)
        );
        assert_eq!(layout.hit_test(Side::Trailing, 1.0, 24.0), None);
        assert_eq!(
            layout.hit_test(Side::Trailing, 96.0, 24.0),
            Some(ButtonKind::Close)
        );
    }

    #[test]
    fn hit_testing_reaches_into_the_margins() {
        let requested = DecorationLayout {
            leading: Vec::new(),
            trailing: vec![ButtonKind::Minimize],
        };
        let layout = Layout::compute(&requested, DEFAULT_BAND_HEIGHT, &ADWAITA, 800.0);
        // The pill is 24px wide but the hit target is 34px, so 2px left of the pill
        // still hits it.
        assert_eq!(
            layout.hit_test(Side::Trailing, 7.0, 24.0),
            Some(ButtonKind::Minimize)
        );
        assert_eq!(layout.hit_test(Side::Trailing, 3.0, 24.0), None);
    }

    fn paint_band_with(
        interaction: BandInteraction,
        active: bool,
        background: Rgba,
        icons: &IconProvider<'_>,
    ) -> (Vec<u8>, u32) {
        paint_band_with_colors(interaction, active, background, Rgba::WHITE, icons)
    }

    fn paint_band_with_colors(
        interaction: BandInteraction,
        active: bool,
        background: Rgba,
        base: Rgba,
        icons: &IconProvider<'_>,
    ) -> (Vec<u8>, u32) {
        let requested = DecorationLayout {
            leading: Vec::new(),
            trailing: kinds(),
        };
        let layout = Layout::compute(&requested, DEFAULT_BAND_HEIGHT, &ADWAITA, 118.0);
        let width = layout.trailing_width as u32;
        let height = DEFAULT_BAND_HEIGHT as u32;
        let mut canvas = Canvas::new(width, height);
        paint_band(
            &mut canvas,
            &layout.trailing,
            &BandPaint {
                size: (width, height),
                origin: layout.origin(Side::Trailing),
                scale: 1.0,
                style: &ADWAITA,
                colors: &BandStyle {
                    background,
                    base,
                    active,
                },
                interaction: &interaction,
                icons,
            },
        );
        let mut out = Vec::new();
        canvas.resolve_into(&mut out);
        (out, width)
    }

    fn pixel(out: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [out[i], out[i + 1], out[i + 2], out[i + 3]]
    }

    /// The exact pixel a translucent color produces over an opaque background,
    /// mirroring `Canvas::blend` and `Canvas::resolve_into`.
    fn composited(foreground: Rgba, background: Rgba) -> [u8; 4] {
        let sa = u32::from(foreground.a);
        let inv = 255 - sa;
        let channel = |front: u8, back: u8| {
            ((u32::from(front) * sa / 255) + (u32::from(back) * inv / 255)) as u8
        };
        [
            channel(foreground.b, background.b),
            channel(foreground.g, background.g),
            channel(foreground.r, background.r),
            (sa + u32::from(background.a) * inv / 255) as u8,
        ]
    }

    /// The button centres, at a y inside the pill but clear of the glyph: the band
    /// is 32px tall, the 24px pill spans y = 4..28 and the 16px glyph box y =
    /// 8..24, so row 26 crosses the pill only.
    const PILL_ROW: u32 = 26;
    const MINIMIZE_X: u32 = 22;
    const MAXIMIZE_X: u32 = 59;
    const CLOSE_X: u32 = 96;

    #[test]
    fn adwaita_pills_use_ten_percent() {
        let background = Rgba::opaque(0x2f, 0x32, 0x41);
        let (out, width) = paint_band_with(BandInteraction::default(), true, background, &no_icons);
        assert_eq!(
            pixel(&out, width, MINIMIZE_X, PILL_ROW),
            composited(Rgba::WHITE.with_alpha(alpha_to_u8(0.10)), background)
        );
    }

    #[test]
    fn hovering_thickens_only_the_hovered_pill() {
        let background = Rgba::opaque(0x20, 0x20, 0x20);
        let (idle, width) =
            paint_band_with(BandInteraction::default(), true, background, &no_icons);
        let (hovered, _) = paint_band_with(
            BandInteraction {
                hovered: Some(ButtonKind::Maximize),
                pressed: None,
            },
            true,
            background,
            &no_icons,
        );
        assert_ne!(
            pixel(&hovered, width, MAXIMIZE_X, PILL_ROW),
            pixel(&idle, width, MAXIMIZE_X, PILL_ROW)
        );
        assert_eq!(
            pixel(&hovered, width, MINIMIZE_X, PILL_ROW),
            pixel(&idle, width, MINIMIZE_X, PILL_ROW)
        );
        // ...and the hovered pill is the 15% one, not the 10% one.
        assert_eq!(
            pixel(&hovered, width, MAXIMIZE_X, PILL_ROW),
            composited(Rgba::WHITE.with_alpha(alpha_to_u8(0.15)), background)
        );
    }

    #[test]
    fn the_close_button_uses_the_same_pill_as_the_others() {
        // Adwaita has no red close button: `style.close_states = style.states`.
        let background = Rgba::opaque(0x20, 0x20, 0x20);
        let (close, width) = paint_band_with(
            BandInteraction {
                hovered: Some(ButtonKind::Close),
                pressed: None,
            },
            true,
            background,
            &no_icons,
        );
        let (maximize, _) = paint_band_with(
            BandInteraction {
                hovered: Some(ButtonKind::Maximize),
                pressed: None,
            },
            true,
            background,
            &no_icons,
        );
        assert_eq!(
            pixel(&close, width, CLOSE_X, PILL_ROW),
            pixel(&maximize, width, MAXIMIZE_X, PILL_ROW)
        );
    }

    #[test]
    fn opaque_pixels_of_the_band_are_painted_even_without_icons() {
        let (out, _) = paint_band_with(
            BandInteraction::default(),
            true,
            Rgba::TRANSPARENT,
            &no_icons,
        );
        let painted = (0..out.len() / 4).filter(|i| out[i * 4 + 3] > 0).count();
        assert!(painted > 60, "only {painted} pixels were painted");
    }

    #[test]
    fn inactive_windows_dim_the_glyphs() {
        // Comparing against an explicitly dimmed symbol color sidesteps the parts of
        // the band that blend twice (the close button's crossing diagonals): if the
        // inactive band draws its glyphs at `GetInactiveButtonColorAlphaRatio()` of
        // the symbol's alpha, the two renders are byte-for-byte identical.
        let dimmed = Rgba::WHITE.with_alpha(alpha_to_u8(INACTIVE_ALPHA_RATIO));
        let (explicit, _) = paint_band_with_colors(
            BandInteraction::default(),
            true,
            Rgba::TRANSPARENT,
            dimmed,
            &no_icons,
        );
        let (inactive, _) = paint_band_with_colors(
            BandInteraction::default(),
            false,
            Rgba::TRANSPARENT,
            Rgba::WHITE,
            &no_icons,
        );
        assert_eq!(explicit, inactive);

        // The focused band is brighter than the unfocused one, i.e. the glyphs really
        // are dimmed and not merely recolored.
        let (active, _) = paint_band_with_colors(
            BandInteraction::default(),
            true,
            Rgba::TRANSPARENT,
            Rgba::WHITE,
            &no_icons,
        );
        let brightest = |out: &[u8]| {
            (0..out.len() / 4)
                .map(|i| out[i * 4 + 3])
                .max()
                .unwrap_or(0)
        };
        assert_eq!(brightest(&active), 255);
        assert!(brightest(&inactive) < brightest(&active));
    }

    #[test]
    fn a_themed_icon_replaces_the_vector_glyph() {
        let icon = |_kind: ButtonKind, _color: Rgba, _size: u32| {
            Some(IconData {
                width: 1,
                height: 1,
                // The icon is drawn over the glyph box, so a 1x1 red icon makes the
                // whole box red where the vector glyph would only stroke an outline.
                pixels: vec![255, 0, 0, 255],
            })
        };
        let with_icon = paint_band_with(
            BandInteraction::default(),
            true,
            Rgba::opaque(0x20, 0x20, 0x20),
            &icon,
        );
        let vectors = paint_band_with(
            BandInteraction::default(),
            true,
            Rgba::opaque(0x20, 0x20, 0x20),
            &no_icons,
        );
        let (_, width) = vectors;
        // The top row of the minimize glyph box, which a centred 10px stroke of the
        // vector glyph would leave empty.
        let box_top = pixel(&with_icon.0, width, MINIMIZE_X, 17);
        assert_eq!(box_top, [0, 0, 255, 255]);
        assert_ne!(pixel(&vectors.0, width, MINIMIZE_X, 17), box_top);
    }

    #[test]
    fn icon_names_match_the_freedesktop_spec() {
        assert_eq!(icon_name(ButtonKind::Minimize), "window-minimize-symbolic");
        assert_eq!(icon_name(ButtonKind::Maximize), "window-maximize-symbolic");
        assert_eq!(icon_name(ButtonKind::Restore), "window-restore-symbolic");
        assert_eq!(icon_name(ButtonKind::Close), "window-close-symbolic");
    }

    #[test]
    fn a_translucent_background_keeps_its_alpha() {
        let (out, width) = paint_band_with(
            BandInteraction::default(),
            true,
            Rgba::new(0x2f, 0x32, 0x41, 0x40),
            &no_icons,
        );
        assert!(pixel(&out, width, 0, 0)[3] <= 0x40);
    }
}
