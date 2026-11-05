//! The minimize / maximize / restore / close glyphs.
//!
//! Ported from Chromium's `windows_icon_painter.cc` (used by Electron) so the
//! Windows overlay matches pixel-for-pixel, and reused by the Linux overlay,
//! which draws the same shapes in the Freedesktop/Adwaita arrangement.

use crate::{
    color::Rgba,
    paint::{Canvas, RectF},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum ButtonKind {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl ButtonKind {
    #[cfg(windows)]
    pub(crate) const ALL: [Self; 4] = [Self::Minimize, Self::Maximize, Self::Restore, Self::Close];

    /// Index into the per-button arrays the Windows backend keeps.
    #[cfg(windows)]
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Minimize => 0,
            Self::Maximize => 1,
            Self::Restore => 2,
            Self::Close => 3,
        }
    }
}

pub(crate) fn symbol_rect(bounds: RectF, size: f64, scale: f64) -> RectF {
    let size = (size * scale).round();
    let (cx, cy) = bounds.center();
    RectF::new(
        (cx - size / 2.0).round(),
        (cy - size / 2.0).round(),
        size,
        size,
    )
}

/// Draws `kind` inside `symbol`, in device pixels.
///
/// * `thickness` — stroke width in device pixels.
/// * `scale` — device pixels per logical pixel, used for the Windows 11 corner
///   radius and the restore icon's separation.
pub(crate) fn paint(
    canvas: &mut Canvas,
    kind: ButtonKind,
    symbol: RectF,
    color: Rgba,
    thickness: f64,
    scale: f64,
) {
    let stroke_half = thickness / 2.0;
    let radius = (1.5 * scale).floor();

    match kind {
        // `Windows10IconPainter::PaintMinimizeIcon`: `DrawLine` with the default
        // butt caps, i.e. a 10 x stroke-width bar, not a rounded stroke.
        ButtonKind::Minimize => {
            let y = symbol.y + (symbol.height - thickness) / 2.0;
            canvas.fill_rect(RectF::new(symbol.x, y, symbol.width, thickness), color);
        }
        // `Windows11IconPainter::PaintMaximizeIcon`.
        ButtonKind::Maximize => {
            canvas.stroke_rounded_rect(symbol.inset(stroke_half), radius, thickness, color);
        }
        // `Windows11IconPainter::PaintRestoreIcon`: a front square at the
        // bottom-left and a back square at the top-right, the back one clipped so
        // that only its top and right edges survive.
        ButtonKind::Restore => {
            let separation = 2.0 * scale;
            let base = symbol
                .inset(stroke_half)
                .inset_edges(separation, 0.0, 0.0, separation);
            canvas.stroke_rounded_rect(base, radius, thickness, color);

            let growth = separation - stroke_half;
            canvas.set_clip_exclude_rounded(base.outset(growth), radius + growth);
            let top = base.translated(separation, -separation);
            canvas.stroke_rounded_rect_corners(
                top,
                [radius, radius * 2.0, radius, radius],
                thickness,
                color,
            );
            canvas.reset_clips();
        }
        // `Windows10IconPainter::PaintCloseIcon`.
        ButtonKind::Close => {
            // The close glyph's X is surrounded by a "halo" of transparent pixels;
            // when the X is white the halo is a touch brighter so it stays visible.
            let halo = thickness * if color == Rgba::WHITE { 0.1 } else { 0.05 };
            canvas.set_clip_include(symbol);
            canvas.stroke_line(
                symbol.x,
                symbol.y,
                symbol.right(),
                symbol.bottom(),
                thickness + halo,
                color,
            );
            canvas.stroke_line(
                symbol.right(),
                symbol.y,
                symbol.x,
                symbol.bottom(),
                thickness + halo,
                color,
            );
            canvas.reset_clips();
        }
    }
}
