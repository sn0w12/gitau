use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Error, Result};

/// A straight (non-premultiplied) RGBA color.
///
/// Parsed with the same CSS Color Level 4 rules as Chromium's
/// `content::ParseCssColorString`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self::new(0, 0, 0, 0);
    pub const WHITE: Self = Self::new(255, 255, 255, 255);
    pub const BLACK: Self = Self::new(0, 0, 0, 255);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Includes `field` in the error.
    pub fn parse(field: &'static str, value: &str) -> Result<Self> {
        let color = csscolorparser::Color::from_html(value).map_err(|_| Error::InvalidColor {
            field,
            value: value.to_owned(),
        })?;
        let [r, g, b, a] = color.to_rgba8();
        Ok(Self::new(r, g, b, a))
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        Self::new(self.r, self.g, self.b, a)
    }

    pub const fn to_array(self) -> [u8; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// Matches `gfx::Tween::ColorValueBetween`: channels interpolate in
    /// premultiplied space, so the alpha of either endpoint is respected.
    #[cfg(any(windows, test))]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        let t = t.clamp(0.0, 1.0);
        let start_alpha = f64::from(self.a) / 255.0;
        let target_alpha = f64::from(other.a) / 255.0;
        let alpha = start_alpha + (target_alpha - start_alpha) * t;
        if alpha <= 0.0 {
            return Self::TRANSPARENT;
        }
        let alpha = alpha.min(1.0);
        let mix = |start: u8, target: u8| {
            let start = f64::from(start) / 255.0 * start_alpha;
            let target = f64::from(target) / 255.0 * target_alpha;
            let blended = start + (target - start) * t;
            ((blended / alpha) * 255.0).round().clamp(0.0, 255.0) as u8
        };
        Self::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
            (alpha * 255.0).round() as u8,
        )
    }

    /// `#RRGGBBAA`, mirroring how Electron serialises the `titleBarOverlay` getter.
    pub fn to_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
    }

    pub fn luminance(self) -> f64 {
        fn channel(value: u8) -> f64 {
            let value = f64::from(value) / 255.0;
            if value <= 0.040_45 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// Black or white, whichever contrasts more; this is the rule Electron uses
    /// for the default glyph color (`color_utils::GetColorWithMaxContrast`).
    pub fn contrasting(self) -> Self {
        if self.luminance() > 0.179 {
            Self::BLACK
        } else {
            Self::WHITE
        }
    }
}

impl FromStr for Rgba {
    type Err = csscolorparser::ParseColorError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let color = csscolorparser::Color::from_html(s)?;
        let [r, g, b, a] = color.to_rgba8();
        Ok(Self::new(r, g, b, a))
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Rgba {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgba {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::Rgba;

    #[test]
    fn parses_everything_electron_accepts() {
        assert_eq!(
            Rgba::parse("color", "#fff").unwrap(),
            Rgba::opaque(255, 255, 255)
        );
        assert_eq!(
            Rgba::parse("color", "#2f3241").unwrap(),
            Rgba::opaque(0x2f, 0x32, 0x41)
        );
        assert_eq!(
            Rgba::parse("color", "#2f324180").unwrap(),
            Rgba::new(0x2f, 0x32, 0x41, 0x80)
        );
        assert_eq!(
            Rgba::parse("color", "rgba(47, 50, 65, 0.5)").unwrap(),
            Rgba::new(47, 50, 65, 128)
        );
        assert_eq!(
            Rgba::parse("color", "hsl(210, 16%, 22%)").unwrap(),
            Rgba::opaque(47, 56, 65)
        );
        assert_eq!(
            Rgba::parse("color", "rebeccapurple").unwrap(),
            Rgba::opaque(102, 51, 153)
        );
        assert_eq!(
            Rgba::parse("color", "transparent").unwrap(),
            Rgba::TRANSPARENT
        );
        assert!(Rgba::parse("color", "not-a-color").is_err());
    }

    #[test]
    fn serialises_as_hex_with_alpha() {
        assert_eq!(Rgba::new(0x2f, 0x32, 0x41, 0xff).to_hex(), "#2F3241FF");
        assert_eq!(Rgba::new(0, 0, 0, 0).to_hex(), "#00000000");
    }

    #[test]
    fn contrast_picks_black_on_light_backgrounds() {
        assert_eq!(Rgba::WHITE.contrasting(), Rgba::BLACK);
        assert_eq!(Rgba::BLACK.contrasting(), Rgba::WHITE);
    }

    #[test]
    fn tweening_handles_opaque_and_translucent_endpoints() {
        assert_eq!(
            Rgba::WHITE.lerp(Rgba::BLACK, 0.5),
            Rgba::opaque(128, 128, 128)
        );
        // premultiplied interpolation keeps the colour when only alpha changes
        assert_eq!(
            Rgba::TRANSPARENT.lerp(Rgba::new(10, 20, 30, 255), 0.5),
            Rgba::new(10, 20, 30, 128)
        );
    }
}
