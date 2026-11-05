//! A software rasterizer for the window controls.
//!
//! Used instead of GDI/Cairo so both backends draw identical pixels, the shapes
//! can be unit-tested, and `titleBarOverlay.color` keeps its alpha channel.

use crate::color::Rgba;

const SUPERSAMPLE: usize = 3;

/// A rectangle in device pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RectF {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl RectF {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn inset(&self, amount: f64) -> Self {
        Self::new(
            self.x + amount,
            self.y + amount,
            (self.width - amount * 2.0).max(0.0),
            (self.height - amount * 2.0).max(0.0),
        )
    }

    pub fn inset_edges(&self, left: f64, top: f64, right: f64, bottom: f64) -> Self {
        Self::new(
            self.x + left,
            self.y + top,
            (self.width - left - right).max(0.0),
            (self.height - top - bottom).max(0.0),
        )
    }

    pub fn translated(&self, dx: f64, dy: f64) -> Self {
        Self::new(self.x + dx, self.y + dy, self.width, self.height)
    }

    pub fn outset(&self, amount: f64) -> Self {
        self.inset(-amount)
    }

    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// An image in unpremultiplied RGBA, e.g. a symbolic icon from the icon theme.
///
/// Together with [`Canvas::draw_image`] and [`Canvas::fill_circle`] this is only
/// reachable from the Freedesktop backend, so outside `cfg(freedesktop)` it is
/// compiled for the tests alone, which exercise it on every platform.
#[cfg(any(freedesktop, test))]
pub(crate) struct Image<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels: &'a [u8],
}

/// A premultiplied-RGBA, supersampled drawing surface.
pub(crate) struct Canvas {
    width: u32,
    height: u32,
    /// `width * SUPERSAMPLE * height * SUPERSAMPLE` premultiplied RGBA pixels.
    buffer: Vec<u8>,
    /// Pixels inside this rect are painted; `None` means "everywhere".
    clip_include: Option<RectF>,
    /// Pixels inside this rounded rect are *not* painted.
    clip_exclude: Option<(RectF, f64)>,
    antialias: bool,
}

impl Canvas {
    pub fn new(width: u32, height: u32) -> Self {
        let mut canvas = Self {
            width: 0,
            height: 0,
            buffer: Vec::new(),
            clip_include: None,
            clip_exclude: None,
            antialias: true,
        };
        canvas.resize(width, height);
        canvas
    }

    /// Reallocates the backing buffer if the size changed and clears it.
    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            let len = width as usize * SUPERSAMPLE * height as usize * SUPERSAMPLE * 4;
            self.buffer = vec![0; len];
        } else {
            self.buffer.fill(0);
        }
        self.clip_include = None;
        self.clip_exclude = None;
        self.antialias = true;
    }

    #[cfg(test)]
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn set_clip_include(&mut self, rect: RectF) {
        self.clip_include = Some(rect);
    }

    #[cfg(test)]
    pub fn set_clip_exclude(&mut self, rect: RectF) {
        self.clip_exclude = Some((rect, 0.0));
    }

    pub fn set_clip_exclude_rounded(&mut self, rect: RectF, radius: f64) {
        self.clip_exclude = Some((rect, radius.max(0.0)));
    }

    /// Turns supersampling off so shapes rasterize on pixel centres, the way
    /// Skia draws with `PaintFlags::setAntiAlias(false)`.
    #[cfg(any(windows, test))]
    pub fn set_antialias(&mut self, enabled: bool) {
        self.antialias = enabled;
    }

    pub fn reset_clips(&mut self) {
        self.clip_include = None;
        self.clip_exclude = None;
    }

    fn supersampled_size(&self) -> (usize, usize) {
        (
            self.width as usize * SUPERSAMPLE,
            self.height as usize * SUPERSAMPLE,
        )
    }

    fn blend(&mut self, sx: usize, sy: usize, color: Rgba) {
        let (sw, sh) = self.supersampled_size();
        if sx >= sw || sy >= sh {
            return;
        }
        let sa = u32::from(color.a);
        if sa == 0 {
            return;
        }
        let inv = 255 - sa;
        let index = (sy * sw + sx) * 4;
        let src = [
            u32::from(color.r) * sa / 255,
            u32::from(color.g) * sa / 255,
            u32::from(color.b) * sa / 255,
        ];
        for (channel, source) in src.iter().enumerate() {
            let dst = u32::from(self.buffer[index + channel]);
            self.buffer[index + channel] = ((source + dst * inv / 255).min(255)) as u8;
        }
        let dst_a = u32::from(self.buffer[index + 3]);
        self.buffer[index + 3] = ((sa + dst_a * inv / 255).min(255)) as u8;
    }

    fn paint(&mut self, shape: &dyn Fn(f64, f64) -> bool, color: Rgba) {
        // Copy the clips out first: the closures passed in borrow their own locals,
        // but `paint` itself needs `&mut self` throughout.
        let include = self.clip_include;
        let exclude = self.clip_exclude;

        if !self.antialias {
            for y in 0..self.height {
                for x in 0..self.width {
                    let px = f64::from(x) + 0.5;
                    let py = f64::from(y) + 0.5;
                    if !passes_clip(include, exclude, px, py) || !shape(px, py) {
                        continue;
                    }
                    for oy in 0..SUPERSAMPLE {
                        for ox in 0..SUPERSAMPLE {
                            let sx = x as usize * SUPERSAMPLE + ox;
                            let sy = y as usize * SUPERSAMPLE + oy;
                            self.blend(sx, sy, color);
                        }
                    }
                }
            }
            return;
        }

        let (sw, sh) = self.supersampled_size();
        let scale = SUPERSAMPLE as f64;
        for sy in 0..sh {
            let py = (sy as f64 + 0.5) / scale;
            for sx in 0..sw {
                let px = (sx as f64 + 0.5) / scale;
                if !passes_clip(include, exclude, px, py) {
                    continue;
                }
                if shape(px, py) {
                    self.blend(sx, sy, color);
                }
            }
        }
    }

    pub fn fill_rect(&mut self, rect: RectF, color: Rgba) {
        self.paint(&|px, py| rect.contains(px, py), color);
    }

    /// Strokes the outline of `rect`, centred on the rectangle's edge.
    pub fn stroke_rounded_rect(&mut self, rect: RectF, radius: f64, thickness: f64, color: Rgba) {
        self.stroke_rounded_rect_corners(rect, [radius; 4], thickness, color);
    }

    /// Strokes the outline of `rect` with a radius per corner, in
    /// top-left / top-right / bottom-right / bottom-left order.
    pub fn stroke_rounded_rect_corners(
        &mut self,
        rect: RectF,
        radii: [f64; 4],
        thickness: f64,
        color: Rgba,
    ) {
        let half = thickness / 2.0;
        let outer = RectF::new(
            rect.x - half,
            rect.y - half,
            rect.width + thickness,
            rect.height + thickness,
        );
        let inner = rect.inset(half);
        let radii = radii.map(|radius| radius.max(0.0));
        let outer_radii = radii.map(|radius| radius + half);
        let inner_radii = radii.map(|radius| (radius - half).max(0.0));
        self.paint(
            &|px, py| {
                inside_rounded_rect(px, py, outer, outer_radii)
                    && !inside_rounded_rect(px, py, inner, inner_radii)
            },
            color,
        );
    }

    /// Strokes a straight line segment with rounded caps.
    pub fn stroke_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, thickness: f64, color: Rgba) {
        let half = thickness / 2.0;
        self.paint(
            &|px, py| distance_to_segment(px, py, x0, y0, x1, y1) <= half,
            color,
        );
    }

    /// Draws `image` scaled to fill `rect`, sampled at supersampled resolution
    /// and box-filtered down once with the rest of the canvas.
    #[cfg(any(freedesktop, test))]
    pub fn draw_image(&mut self, image: &Image<'_>, rect: RectF) {
        if image.width == 0 || image.height == 0 || image.pixels.is_empty() {
            return;
        }
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }

        let include = self.clip_include;
        let exclude = self.clip_exclude;
        let (sw, sh) = self.supersampled_size();
        let scale = SUPERSAMPLE as f64;

        let first_x = (rect.x.max(0.0) * scale).floor().max(0.0) as usize;
        let first_y = (rect.y.max(0.0) * scale).floor().max(0.0) as usize;
        let last_x = (rect.right().max(0.0) * scale).ceil() as usize;
        let last_y = (rect.bottom().max(0.0) * scale).ceil() as usize;

        for sy in first_y..last_y.min(sh) {
            let py = (sy as f64 + 0.5) / scale;
            let v = (py - rect.y) / rect.height;
            if !(0.0..1.0).contains(&v) {
                continue;
            }
            for sx in first_x..last_x.min(sw) {
                let px = (sx as f64 + 0.5) / scale;
                let u = (px - rect.x) / rect.width;
                if !(0.0..1.0).contains(&u) {
                    continue;
                }
                if !passes_clip(include, exclude, px, py) {
                    continue;
                }
                self.blend(sx, sy, sample_bilinear(image, u, v));
            }
        }
    }

    #[cfg(any(freedesktop, test))]
    pub fn fill_circle(&mut self, cx: f64, cy: f64, radius: f64, color: Rgba) {
        let radius = radius.max(0.0);
        self.paint(
            &|px, py| {
                let dx = px - cx;
                let dy = py - cy;
                dx * dx + dy * dy <= radius * radius
            },
            color,
        );
    }

    /// Box-filters the supersampled buffer down into `out` as premultiplied BGRA,
    /// top-down, ready for `UpdateLayeredWindow` or a Cairo `ARGB32` surface.
    pub fn resolve_into(&self, out: &mut Vec<u8>) {
        let (sw, _) = self.supersampled_size();
        let samples = (SUPERSAMPLE * SUPERSAMPLE) as u32;
        out.clear();
        out.reserve(self.width as usize * self.height as usize * 4);
        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                let mut sums = [0u32; 4];
                for oy in 0..SUPERSAMPLE {
                    for ox in 0..SUPERSAMPLE {
                        let index = ((y * SUPERSAMPLE + oy) * sw + (x * SUPERSAMPLE + ox)) * 4;
                        for (channel, sum) in sums.iter_mut().enumerate() {
                            *sum += u32::from(self.buffer[index + channel]);
                        }
                    }
                }
                out.push((sums[2] / samples) as u8);
                out.push((sums[1] / samples) as u8);
                out.push((sums[0] / samples) as u8);
                out.push((sums[3] / samples) as u8);
            }
        }
    }
}

/// Bilinearly samples `image` at `(u, v)`, both in `0..=1`.
///
/// The interpolation happens in premultiplied space so that filtering across
/// the edges of an icon does not pull the transparent color into the glyph.
#[cfg(any(freedesktop, test))]
fn sample_bilinear(image: &Image<'_>, u: f64, v: f64) -> Rgba {
    let fx = u * image.width as f64 - 0.5;
    let fy = v * image.height as f64 - 0.5;
    let x0 = fx.floor();
    let y0 = fy.floor();
    let tx = fx - x0;
    let ty = fy - y0;
    let max_x = image.width as i64 - 1;
    let max_y = image.height as i64 - 1;

    let mut sums = [0.0f64; 4];
    for (dx, wx) in [(0i64, 1.0 - tx), (1i64, tx)] {
        for (dy, wy) in [(0i64, 1.0 - ty), (1i64, ty)] {
            let weight = wx * wy;
            if weight == 0.0 {
                continue;
            }
            let x = (x0 as i64 + dx).clamp(0, max_x) as usize;
            let y = (y0 as i64 + dy).clamp(0, max_y) as usize;
            let index = (y * image.width as usize + x) * 4;
            let alpha = f64::from(image.pixels[index + 3]) / 255.0;
            for (channel, sum) in sums.iter_mut().enumerate().take(3) {
                *sum += f64::from(image.pixels[index + channel]) * alpha * weight;
            }
            sums[3] += alpha * weight;
        }
    }

    if sums[3] <= 0.0 {
        return Rgba::TRANSPARENT;
    }
    let unpremultiply = |value: f64| (value / sums[3]).clamp(0.0, 255.0).round() as u8;
    Rgba::new(
        unpremultiply(sums[0]),
        unpremultiply(sums[1]),
        unpremultiply(sums[2]),
        (sums[3] * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn passes_clip(include: Option<RectF>, exclude: Option<(RectF, f64)>, px: f64, py: f64) -> bool {
    if include.is_some_and(|rect| !rect.contains(px, py)) {
        return false;
    }
    if exclude.is_some_and(|(rect, radius)| inside_rounded_rect(px, py, rect, [radius; 4])) {
        return false;
    }
    true
}

fn inside_rounded_rect(px: f64, py: f64, rect: RectF, radii: [f64; 4]) -> bool {
    if !rect.contains(px, py) {
        return false;
    }
    let max_radius = (rect.width / 2.0).min(rect.height / 2.0);
    let left = rect.x;
    let top = rect.y;
    let right = rect.right();
    let bottom = rect.bottom();
    let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];
    for (index, (corner_x, corner_y)) in corners.into_iter().enumerate() {
        let radius = radii[index].min(max_radius).max(0.0);
        if radius == 0.0 {
            continue;
        }
        let is_left = corner_x == left;
        let is_top = corner_y == top;
        let cx = if is_left {
            left + radius
        } else {
            right - radius
        };
        let cy = if is_top {
            top + radius
        } else {
            bottom - radius
        };
        let within_x = if is_left { px < cx } else { px > cx };
        let within_y = if is_top { py < cy } else { py > cy };
        if within_x && within_y {
            let dx = px - cx;
            let dy = py - cy;
            return dx * dx + dy * dy <= radius * radius;
        }
    }
    true
}

fn distance_to_segment(px: f64, py: f64, x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq == 0.0 {
        0.0
    } else {
        (((px - x0) * dx + (py - y0) * dy) / len_sq).clamp(0.0, 1.0)
    };
    let nx = x0 + t * dx;
    let ny = y0 + t * dy;
    ((px - nx).powi(2) + (py - ny).powi(2)).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(canvas: &Canvas, x: u32, y: u32) -> [u8; 4] {
        let mut out = Vec::new();
        canvas.resolve_into(&mut out);
        let i = ((y * canvas.width() + x) * 4) as usize;
        [out[i], out[i + 1], out[i + 2], out[i + 3]]
    }

    #[test]
    fn opaque_fill_covers_the_whole_surface() {
        let mut canvas = Canvas::new(4, 4);
        canvas.fill_rect(
            RectF::new(0.0, 0.0, 4.0, 4.0),
            Rgba::opaque(0x2f, 0x32, 0x41),
        );
        assert_eq!(pixel(&canvas, 0, 0), [0x41, 0x32, 0x2f, 0xff]);
        assert_eq!(pixel(&canvas, 3, 3), [0x41, 0x32, 0x2f, 0xff]);
    }

    #[test]
    fn translucent_fill_produces_premultiplied_pixels() {
        let mut canvas = Canvas::new(2, 2);
        canvas.fill_rect(
            RectF::new(0.0, 0.0, 2.0, 2.0),
            Rgba::new(255, 255, 255, 128),
        );
        // premultiplied BGRA of white @ 50%
        assert_eq!(pixel(&canvas, 0, 0), [128, 128, 128, 128]);
    }

    #[test]
    fn translucent_fill_over_opaque_layer_keeps_full_alpha() {
        let mut canvas = Canvas::new(2, 2);
        canvas.fill_rect(RectF::new(0.0, 0.0, 2.0, 2.0), Rgba::opaque(0, 0, 0));
        canvas.fill_rect(
            RectF::new(0.0, 0.0, 2.0, 2.0),
            Rgba::new(255, 255, 255, 128),
        );
        let [b, g, r, a] = pixel(&canvas, 0, 0);
        assert_eq!(a, 255);
        assert!((127..=129).contains(&b));
        assert_eq!([b, g, r], [b, b, b]);
    }

    #[test]
    fn empty_regions_stay_transparent() {
        let canvas = Canvas::new(4, 4);
        assert_eq!(pixel(&canvas, 1, 1), [0, 0, 0, 0]);
    }

    #[test]
    fn clip_exclude_removes_pixels() {
        let mut canvas = Canvas::new(4, 4);
        canvas.set_clip_exclude(RectF::new(2.0, 0.0, 2.0, 4.0));
        canvas.fill_rect(RectF::new(0.0, 0.0, 4.0, 4.0), Rgba::WHITE);
        assert_eq!(pixel(&canvas, 0, 0)[3], 255);
        assert_eq!(pixel(&canvas, 3, 0)[3], 0);
    }

    #[test]
    fn clip_include_limits_painting() {
        let mut canvas = Canvas::new(4, 4);
        canvas.set_clip_include(RectF::new(0.0, 0.0, 2.0, 4.0));
        canvas.fill_rect(RectF::new(0.0, 0.0, 4.0, 4.0), Rgba::WHITE);
        assert_eq!(pixel(&canvas, 1, 1)[3], 255);
        assert_eq!(pixel(&canvas, 2, 1)[3], 0);
    }

    #[test]
    fn stroke_is_centred_on_the_edge() {
        let mut canvas = Canvas::new(8, 8);
        canvas.stroke_rounded_rect(RectF::new(2.0, 2.0, 4.0, 4.0), 0.0, 2.0, Rgba::WHITE);
        // the stroke spans x = 1..3, so x = 1 is painted and x = 0 is not
        assert!(pixel(&canvas, 1, 4)[3] > 0);
        assert_eq!(pixel(&canvas, 0, 4)[3], 0);
        assert_eq!(pixel(&canvas, 4, 4)[3], 0);
    }

    #[test]
    fn line_has_a_cap_at_both_ends() {
        let mut canvas = Canvas::new(16, 4);
        canvas.stroke_line(4.0, 2.0, 12.0, 2.0, 2.0, Rgba::WHITE);
        assert_eq!(pixel(&canvas, 0, 2)[3], 0);
        assert!(pixel(&canvas, 4, 2)[3] > 0);
        assert!(pixel(&canvas, 12, 2)[3] > 0);
        assert_eq!(pixel(&canvas, 15, 2)[3], 0);
    }

    #[test]
    fn an_image_fills_the_rect_it_is_drawn_into() {
        let mut canvas = Canvas::new(4, 4);
        let red = [255u8, 0, 0, 255];
        canvas.draw_image(
            &Image {
                width: 1,
                height: 1,
                pixels: &red,
            },
            RectF::new(0.0, 0.0, 4.0, 4.0),
        );
        assert_eq!(pixel(&canvas, 0, 0), [0, 0, 255, 255]);
        assert_eq!(pixel(&canvas, 3, 3), [0, 0, 255, 255]);
    }

    #[test]
    fn an_image_leaves_the_rest_of_the_surface_alone() {
        let mut canvas = Canvas::new(8, 2);
        let white = [255u8, 255, 255, 255];
        canvas.draw_image(
            &Image {
                width: 1,
                height: 1,
                pixels: &white,
            },
            RectF::new(2.0, 0.0, 4.0, 2.0),
        );
        assert_eq!(pixel(&canvas, 1, 0)[3], 0);
        assert_eq!(pixel(&canvas, 4, 0)[3], 255);
        assert_eq!(pixel(&canvas, 6, 1)[3], 0);
    }

    #[test]
    fn a_translucent_image_keeps_its_alpha() {
        let mut canvas = Canvas::new(2, 2);
        let half = [255u8, 255, 255, 128];
        canvas.draw_image(
            &Image {
                width: 1,
                height: 1,
                pixels: &half,
            },
            RectF::new(0.0, 0.0, 2.0, 2.0),
        );
        let [b, g, r, a] = pixel(&canvas, 0, 0);
        assert_eq!([b, g, r], [128, 128, 128]);
        assert!((127..=129).contains(&a));
    }

    #[test]
    fn a_circle_stays_inside_its_radius() {
        let mut canvas = Canvas::new(9, 9);
        canvas.fill_circle(4.5, 4.5, 3.0, Rgba::WHITE);
        assert_eq!(pixel(&canvas, 4, 4)[3], 255);
        // The centre row spans x = 1.5..7.5, so x = 1 is outside and x = 2 is in.
        assert_eq!(pixel(&canvas, 0, 4)[3], 0);
        assert!(pixel(&canvas, 2, 4)[3] > 0);
        // The corners are outside the circle even though they are inside the box.
        assert_eq!(pixel(&canvas, 0, 0)[3], 0);
        assert_eq!(pixel(&canvas, 8, 8)[3], 0);
    }

    #[test]
    fn clipping_combines_include_and_exclude() {
        let mut canvas = Canvas::new(6, 2);
        canvas.set_clip_include(RectF::new(0.0, 0.0, 4.0, 2.0));
        canvas.set_clip_exclude(RectF::new(1.0, 0.0, 2.0, 2.0));
        canvas.fill_rect(RectF::new(0.0, 0.0, 6.0, 2.0), Rgba::WHITE);
        assert_eq!(pixel(&canvas, 0, 0)[3], 255);
        assert_eq!(pixel(&canvas, 2, 0)[3], 0);
        assert_eq!(pixel(&canvas, 4, 0)[3], 0);
    }

    #[test]
    fn a_rounded_exclusion_clears_the_shape_but_not_its_corners() {
        let mut canvas = Canvas::new(8, 8);
        canvas.set_clip_exclude_rounded(RectF::new(0.0, 0.0, 8.0, 8.0), 4.0);
        canvas.fill_rect(RectF::new(0.0, 0.0, 8.0, 8.0), Rgba::WHITE);
        // the rounded rect is cut out; its own corners lie outside it and stay
        assert_eq!(pixel(&canvas, 4, 4)[3], 0);
        assert_eq!(pixel(&canvas, 0, 0)[3], 255);
    }

    #[test]
    fn corner_radii_apply_per_corner() {
        let rect = RectF::new(0.0, 0.0, 10.0, 10.0);
        assert!(!inside_rounded_rect(1.0, 1.0, rect, [4.0, 0.0, 0.0, 0.0]));
        assert!(inside_rounded_rect(9.0, 9.0, rect, [4.0, 0.0, 0.0, 0.0]));
        assert!(!inside_rounded_rect(9.0, 9.0, rect, [0.0, 4.0, 4.0, 0.0]));
    }

    #[test]
    fn crisp_painting_has_no_partial_alpha() {
        let mut canvas = Canvas::new(6, 6);
        canvas.set_antialias(false);
        canvas.fill_circle(3.0, 3.0, 2.5, Rgba::WHITE);
        let mut out = Vec::new();
        canvas.resolve_into(&mut out);
        // every pixel is either untouched or fully opaque, never a soft edge
        for alpha in out.iter().skip(3).step_by(4) {
            assert!(*alpha == 0 || *alpha == 255, "alpha {alpha} is partial");
        }
    }
}
