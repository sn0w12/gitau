//! Windows caption buttons: geometry and painting.
//!
//! Direct port of Electron's `shell/browser/ui/views/win_caption_button.cc`,
//! `win_caption_button_container.cc` and Chromium's `windows_icon_painter.cc`,
//! with the Chromium `views`/Skia drawing calls translated onto our software
//! rasterizer. Kept free of Win32 so it can be unit-tested on any host.

use super::glyphs::ButtonKind;
use crate::{
    color::Rgba,
    paint::{Canvas, RectF},
};

/// `WindowFrameUtil::kWindowsCaptionButtonWidth`.
pub(crate) const BUTTON_WIDTH: f64 = 45.0;
/// `WindowFrameUtil::kWindowsCaptionButtonVisualSpacing`.
pub(crate) const VISUAL_SPACING: f64 = 1.0;
/// `symbol_size_pixels = base::ClampRound(10 * scale)`.
pub(crate) const SYMBOL_SIZE: f64 = 10.0;

/// Close button base color, from `WinCaptionButton::OnPaintBackground`.
pub(crate) const CLOSE_HOVER_COLOR_WINDOWS_10: Rgba = Rgba::opaque(0xE8, 0x11, 0x23);
/// Windows 11's own caption buttons use a darker red than Chromium's constant.
pub(crate) const CLOSE_HOVER_COLOR_WINDOWS_11: Rgba = Rgba::opaque(0xC4, 0x2B, 0x1C);
const CLOSE_PRESSED_ALPHA: u8 = 0x98;
const HOVER_ALPHA: u8 = 0x1A;
const PRESSED_ALPHA: u8 = 0x33;
/// `kAlphaScale` applied when the overlay has a themed (opaque) background.
const THEME_ALPHA_SCALE: f64 = 1.3;

/// A laid-out caption button, in overlay-local logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ButtonRect {
    pub kind: ButtonKind,
    /// Hit target.
    pub rect: RectF,
    /// Painted background: `rect` inset by the inter-button spacing.
    pub visual: RectF,
}

/// Which buttons are shown, mirroring `WinCaptionButtonContainer::UpdateButtons`.
///
/// `minimizable` / `maximizable` map onto Tauri's `Window::is_minimizable()` &
/// co.; maximize and restore are mutually exclusive. Close stays visible even
/// when the window is not closable, exactly like Chromium, and the click is
/// ignored instead.
pub(crate) fn visible_buttons(
    maximized: bool,
    minimizable: bool,
    maximizable: bool,
) -> Vec<ButtonKind> {
    let mut buttons = Vec::with_capacity(3);
    if minimizable {
        buttons.push(ButtonKind::Minimize);
    }
    if maximizable {
        buttons.push(if maximized {
            ButtonKind::Restore
        } else {
            ButtonKind::Maximize
        });
    }
    buttons.push(ButtonKind::Close);
    buttons
}

/// Lays the buttons out from left to right (or right to left under RTL).
///
/// Every button is `BUTTON_WIDTH` wide; each button after the first in display
/// order grows by `VISUAL_SPACING` and insets its *background* by the same
/// amount. That reproduces Chromium's exact numbers: three buttons occupy
/// `3 * 45 + 2 * 1 = 137` logical pixels.
pub(crate) fn layout_buttons(kinds: &[ButtonKind], rtl: bool) -> Vec<ButtonRect> {
    let mut ordered: Vec<ButtonKind> = kinds.to_vec();
    if rtl {
        ordered.reverse();
    }

    let mut x = 0.0;
    let mut out = Vec::with_capacity(ordered.len());
    for (index, kind) in ordered.into_iter().enumerate() {
        let spacing = if index == 0 { 0.0 } else { VISUAL_SPACING };
        let rect = RectF::new(x, 0.0, BUTTON_WIDTH + spacing, 0.0);
        // `bounds.Inset(gfx::Insets::TLBR(0, GetBetweenButtonSpacing(), 0, 0))`:
        // the gap is reserved on the leading edge of each button.
        let visual = if rtl {
            rect.inset_edges(0.0, 0.0, spacing, 0.0)
        } else {
            rect.inset_edges(spacing, 0.0, 0.0, 0.0)
        };
        out.push(ButtonRect { kind, rect, visual });
        x += rect.width;
    }
    out
}

pub(crate) fn laid_out_width(buttons: &[ButtonRect]) -> f64 {
    buttons
        .last()
        .map(|button| button.rect.right())
        .unwrap_or(0.0)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WindowsStyle {
    pub background: Rgba,
    /// `WinCaptionButton::PaintSymbol` paints this verbatim; unfocused glyphs
    /// are not faded, unlike upstream Chromium.
    pub symbol: Rgba,
    /// Chromium's `kColorCaptionButtonForegroundActive`.
    pub foreground_active: Rgba,
    /// Chromium's `kColorCaptionButtonForegroundInactive`.
    pub foreground_inactive: Rgba,
    /// Chromium hardcodes `#E81123`; Windows 11 uses `#C42B1C`.
    pub close_background: Rgba,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct WindowsInteraction {
    pub pressed: Option<ButtonKind>,
    /// `0.0..=1.0`, indexed by [`ButtonKind::index`].
    pub hover: [f64; 4],
}

/// `size` is in device pixels; `height` is the container height in logical
/// pixels and `scale` is device pixels per logical pixel.
pub(crate) fn paint(
    canvas: &mut Canvas,
    size: (u32, u32),
    buttons: &[ButtonRect],
    height: f64,
    style: &WindowsStyle,
    interaction: &WindowsInteraction,
    scale: f64,
) {
    canvas.reset_clips();
    let full = RectF::new(0.0, 0.0, f64::from(size.0), f64::from(size.1));
    canvas.fill_rect(full, style.background);

    // `theme_alpha > 0` means the overlay has a themed background, in which case
    // Electron bumps the hover/press alphas so the states stay visible.
    let themed = style.background.a > 0;
    let (hover_alpha, pressed_alpha) = if themed {
        (
            (f64::from(HOVER_ALPHA) * THEME_ALPHA_SCALE).round() as u8,
            (f64::from(PRESSED_ALPHA) * THEME_ALPHA_SCALE).round() as u8,
        )
    } else {
        (HOVER_ALPHA, PRESSED_ALPHA)
    };

    for button in buttons {
        let rect = scale_rect(button.rect, scale, height);
        let visual = scale_rect(button.visual, scale, height);

        let is_close = button.kind == ButtonKind::Close;
        let pressed = interaction.pressed == Some(button.kind);
        let hover = interaction.hover[button.kind.index()].clamp(0.0, 1.0);

        let (base, alpha) = if is_close {
            (
                style.close_background,
                if pressed {
                    CLOSE_PRESSED_ALPHA
                } else {
                    (hover * 255.0).round() as u8
                },
            )
        } else {
            let foreground = if style.active {
                style.foreground_active
            } else {
                style.foreground_inactive
            };
            (
                foreground,
                if pressed {
                    pressed_alpha
                } else {
                    (hover * f64::from(hover_alpha)).round() as u8
                },
            )
        };
        if alpha > 0 {
            canvas.fill_rect(visual, base.with_alpha(alpha));
        }

        let symbol = if is_close {
            style
                .symbol
                .lerp(Rgba::WHITE, if pressed { 1.0 } else { hover })
        } else {
            style.symbol
        };
        paint_symbol(canvas, button.kind, rect, symbol, scale);
    }
    canvas.reset_clips();
}

fn scale_rect(rect: RectF, scale: f64, height: f64) -> RectF {
    RectF::new(
        rect.x * scale,
        rect.y * scale,
        rect.width * scale,
        height * scale,
    )
}

/// Ports `WinCaptionButton::PaintSymbol`.
///
/// `stroke_width = base::ClampRound(scale)` and
/// `symbol_size_pixels = base::ClampRound(10 * scale)`.
fn paint_symbol(canvas: &mut Canvas, kind: ButtonKind, button: RectF, color: Rgba, scale: f64) {
    let thickness = scale.round().max(1.0);
    let symbol = super::glyphs::symbol_rect(button, SYMBOL_SIZE, scale);
    // `PaintSymbol` turns antialiasing off for the flags; `PaintCloseIcon`
    // turns it back on for the X.
    canvas.set_antialias(kind == ButtonKind::Close);
    super::glyphs::paint(canvas, kind, symbol, color, thickness, scale);
    canvas.set_antialias(true);
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

    #[test]
    fn three_buttons_occupy_137_logical_pixels() {
        let laid_out = layout_buttons(&kinds(), false);
        assert_eq!(laid_out_width(&laid_out), 137.0);
        assert_eq!(laid_out.len(), 3);
        assert_eq!(laid_out[0].rect.x, 0.0);
        assert_eq!(laid_out[0].rect.width, 45.0);
        assert_eq!(laid_out[1].rect.x, 45.0);
        assert_eq!(laid_out[1].rect.width, 46.0);
        assert_eq!(laid_out[2].rect.x, 91.0);
        assert_eq!(laid_out[2].rect.width, 46.0);
        assert_eq!(laid_out[2].rect.right(), 137.0);
    }

    #[test]
    fn first_button_has_no_reserved_gap() {
        let laid_out = layout_buttons(&kinds(), false);
        assert_eq!(laid_out[0].visual, laid_out[0].rect);
        assert_eq!(laid_out[1].visual.x, 46.0);
        assert_eq!(laid_out[1].visual.width, 45.0);
    }

    #[test]
    fn rtl_mirrors_the_button_order() {
        let laid_out = layout_buttons(&kinds(), true);
        assert_eq!(laid_out[0].kind, ButtonKind::Close);
        assert_eq!(laid_out[2].kind, ButtonKind::Minimize);
        // the reserved gap moves to the trailing edge
        assert_eq!(laid_out[0].visual.x, 0.0);
        assert_eq!(laid_out[0].visual.width, 45.0);
        assert_eq!(laid_out[1].visual.width, 45.0);
    }

    #[test]
    fn maximize_and_restore_are_mutually_exclusive() {
        assert_eq!(
            visible_buttons(false, true, true),
            vec![
                ButtonKind::Minimize,
                ButtonKind::Maximize,
                ButtonKind::Close
            ]
        );
        assert_eq!(
            visible_buttons(true, true, true),
            vec![ButtonKind::Minimize, ButtonKind::Restore, ButtonKind::Close]
        );
        assert_eq!(
            visible_buttons(false, false, false),
            vec![ButtonKind::Close]
        );
    }

    /// Stand-ins for `kColorCaptionButtonForeground{Active,Inactive}`.
    const ACTIVE_FOREGROUND: Rgba = Rgba::opaque(0x00, 0x00, 0x00);
    const INACTIVE_FOREGROUND: Rgba = Rgba::opaque(0x80, 0x80, 0x80);

    fn hovered(kind: ButtonKind) -> [f64; 4] {
        let mut hover = [0.0; 4];
        hover[kind.index()] = 1.0;
        hover
    }

    fn paint_one(interaction: WindowsInteraction, background: Rgba) -> Canvas {
        paint_one_with(interaction, background, true)
    }

    fn paint_one_with(interaction: WindowsInteraction, background: Rgba, active: bool) -> Canvas {
        let kinds = kinds();
        let buttons = layout_buttons(&kinds, false);
        let mut canvas = Canvas::new(137, 30);
        paint(
            &mut canvas,
            (137, 30),
            &buttons,
            30.0,
            &WindowsStyle {
                background,
                symbol: Rgba::WHITE,
                foreground_active: ACTIVE_FOREGROUND,
                foreground_inactive: INACTIVE_FOREGROUND,
                close_background: CLOSE_HOVER_COLOR_WINDOWS_10,
                active,
            },
            &interaction,
            1.0,
        );
        canvas
    }

    #[test]
    fn background_covers_the_whole_overlay() {
        let canvas = paint_one(
            WindowsInteraction::default(),
            Rgba::opaque(0x2f, 0x32, 0x41),
        );
        let mut out = Vec::new();
        canvas.resolve_into(&mut out);
        assert_eq!(&out[0..4], &[0x41, 0x32, 0x2f, 0xff]);
        let i = ((15 * 137) + 110) * 4;
        assert_eq!(&out[i..i + 4], &[0x41, 0x32, 0x2f, 0xff]);
    }

    /// A pixel away from the glyphs, so only the background and hover tint show.
    fn sample(out: &[u8], x: usize, y: usize) -> [u8; 4] {
        let i = (y * 137 + x) * 4;
        [out[i], out[i + 1], out[i + 2], out[i + 3]]
    }

    #[test]
    fn hover_tints_the_button_but_not_its_neighbours() {
        let canvas = paint_one(
            WindowsInteraction {
                pressed: None,
                hover: hovered(ButtonKind::Maximize),
            },
            Rgba::opaque(0x2f, 0x32, 0x41),
        );
        let mut out = Vec::new();
        canvas.resolve_into(&mut out);
        // y = 4 is above the 10px glyph box centred in the button.
        assert_ne!(sample(&out, 68, 4), sample(&out, 22, 4));
        assert_eq!(sample(&out, 22, 4), [0x41, 0x32, 0x2f, 0xff]);
    }

    #[test]
    fn a_partially_complete_hover_animation_paints_a_weaker_tint() {
        let background = Rgba::opaque(0x2f, 0x32, 0x41);
        let mut half = [0.0; 4];
        half[ButtonKind::Maximize.index()] = 0.5;
        let mut full = Vec::new();
        paint_one(
            WindowsInteraction {
                pressed: None,
                hover: hovered(ButtonKind::Maximize),
            },
            background,
        )
        .resolve_into(&mut full);
        let mut halfway = Vec::new();
        paint_one(
            WindowsInteraction {
                pressed: None,
                hover: half,
            },
            background,
        )
        .resolve_into(&mut halfway);
        let distance = |a: [u8; 4], b: [u8; 4]| {
            (0..3)
                .map(|i| i32::from(a[i]).abs_diff(i32::from(b[i])) as i32)
                .sum::<i32>()
        };
        // halfway sits between the plain background and the fully hovered tint
        assert!(distance(sample(&halfway, 68, 4), sample(&full, 68, 4)) > 0);
        assert!(distance(sample(&halfway, 68, 4), [0x41, 0x32, 0x2f, 0xff]) > 0);
    }

    #[test]
    fn an_unfocused_window_paints_the_same_glyphs_as_a_focused_one() {
        // `WinCaptionButton::PaintSymbol` paints `overlay_symbol_color_` as-is, so
        // only the hover tint — not the glyphs — depends on focus.
        let background = Rgba::opaque(0x2f, 0x32, 0x41);
        let mut focused = Vec::new();
        paint_one_with(WindowsInteraction::default(), background, true).resolve_into(&mut focused);
        let mut unfocused = Vec::new();
        paint_one_with(WindowsInteraction::default(), background, false)
            .resolve_into(&mut unfocused);
        assert_eq!(focused, unfocused);
    }

    #[test]
    fn an_unfocused_window_tints_with_the_inactive_foreground() {
        let background = Rgba::opaque(0x2f, 0x32, 0x41);
        let interaction = WindowsInteraction {
            pressed: None,
            hover: hovered(ButtonKind::Maximize),
        };
        let mut focused = Vec::new();
        paint_one_with(interaction, background, true).resolve_into(&mut focused);
        let mut unfocused = Vec::new();
        paint_one_with(interaction, background, false).resolve_into(&mut unfocused);
        // `kColorCaptionButtonForegroundInactive` is a different color, not the
        // active one at a reduced alpha.
        assert_ne!(sample(&focused, 68, 4), sample(&unfocused, 68, 4));
        assert_ne!(sample(&unfocused, 68, 4), [0x41, 0x32, 0x2f, 0xff]);
    }

    #[test]
    fn close_button_turns_red_on_hover() {
        let canvas = paint_one(
            WindowsInteraction {
                pressed: None,
                hover: hovered(ButtonKind::Close),
            },
            Rgba::opaque(0x2f, 0x32, 0x41),
        );
        let mut out = Vec::new();
        canvas.resolve_into(&mut out);
        let i = ((15 * 137) + 92) * 4;
        assert_eq!(&out[i..i + 4], &[0x23, 0x11, 0xe8, 0xff]);
    }

    #[test]
    fn translucent_background_keeps_the_alpha_channel() {
        let canvas = paint_one(
            WindowsInteraction::default(),
            Rgba::new(0x2f, 0x32, 0x41, 0x80),
        );
        let mut out = Vec::new();
        canvas.resolve_into(&mut out);
        assert_eq!(out[3], 0x80);
    }

    #[test]
    fn the_minimize_glyph_is_a_ten_by_one_bar() {
        let buttons = layout_buttons(&[ButtonKind::Minimize], false);
        let mut canvas = Canvas::new(45, 30);
        paint(
            &mut canvas,
            (45, 30),
            &buttons,
            30.0,
            &WindowsStyle {
                background: Rgba::TRANSPARENT,
                symbol: Rgba::WHITE,
                foreground_active: ACTIVE_FOREGROUND,
                foreground_inactive: INACTIVE_FOREGROUND,
                close_background: CLOSE_HOVER_COLOR_WINDOWS_10,
                active: true,
            },
            &WindowsInteraction::default(),
            1.0,
        );
        let mut out = Vec::new();
        canvas.resolve_into(&mut out);
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for y in 0..30 {
            for x in 0..45 {
                if out[(y * 45 + x) * 4 + 3] > 0 {
                    xs.push(x);
                    ys.push(y);
                }
            }
        }
        let width = xs.iter().max().unwrap() - xs.iter().min().unwrap() + 1;
        let height = ys.iter().max().unwrap() - ys.iter().min().unwrap() + 1;
        assert_eq!(width, 10, "minimize bar is {width}px wide");
        assert_eq!(height, 1, "minimize bar is {height}px tall");
    }

    #[test]
    fn glyphs_are_actually_drawn() {
        for kind in [
            ButtonKind::Minimize,
            ButtonKind::Maximize,
            ButtonKind::Restore,
            ButtonKind::Close,
        ] {
            let buttons = layout_buttons(&[kind], false);
            let mut canvas = Canvas::new(45, 30);
            paint(
                &mut canvas,
                (45, 30),
                &buttons,
                30.0,
                &WindowsStyle {
                    background: Rgba::TRANSPARENT,
                    symbol: Rgba::WHITE,
                    foreground_active: ACTIVE_FOREGROUND,
                    foreground_inactive: INACTIVE_FOREGROUND,
                    close_background: CLOSE_HOVER_COLOR_WINDOWS_10,
                    active: true,
                },
                &WindowsInteraction::default(),
                1.0,
            );
            let mut out = Vec::new();
            canvas.resolve_into(&mut out);
            let painted = (0..out.len() / 4).filter(|i| out[i * 4 + 3] > 0).count();
            assert!(painted > 5, "{kind:?} painted only {painted} pixels");
        }
    }
}
