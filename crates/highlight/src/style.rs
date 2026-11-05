use std::collections::HashMap;

use serde::Serialize;
use syntect::highlighting::FontStyle;

/// One resolved syntax-highlighting style covering both app themes. Spans
/// reference entries by their 1-based position in the section's style table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    pub light: String,
    pub dark: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Addition,
    Deletion,
    Other,
}

impl LineKind {
    pub(crate) fn is_highlightable(self) -> bool {
        matches!(self, Self::Context | Self::Addition | Self::Deletion)
    }
}

/// One diff line handed to [`crate::SectionHighlighter::spans`]. Line numbers
/// are 1-based and describe the side the line belongs to.
pub struct LineRef<'a> {
    pub kind: LineKind,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: &'a str,
}

pub(crate) fn hex(color: syntect::highlighting::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

pub(crate) fn packed(color: syntect::highlighting::Color) -> u32 {
    (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)
}

pub(crate) fn font_bits(style: FontStyle) -> u8 {
    let mut bits = 0;
    if style.contains(FontStyle::BOLD) {
        bits |= 1;
    }
    if style.contains(FontStyle::UNDERLINE) {
        bits |= 2;
    }
    if style.contains(FontStyle::ITALIC) {
        bits |= 4;
    }
    bits
}

/// Interns resolved (light, dark, font) triples into stable wire IDs so
/// spans stay compact while carrying exact theme colors.
#[derive(Default)]
pub(crate) struct StyleInterner {
    map: HashMap<(u32, u32, u8), u16>,
    entries: Vec<Style>,
    flushed: usize,
}

impl StyleInterner {
    pub(crate) fn intern(&mut self, key: (u32, u32, u8), build: impl FnOnce() -> Style) -> u16 {
        if let Some(&id) = self.map.get(&key) {
            return id;
        }
        let id = self.entries.len() as u16 + 1;
        self.entries.push(build());
        self.map.insert(key, id);
        id
    }

    /// Styles appended since the previous call, for delta transport.
    pub(crate) fn take_new(&mut self) -> Vec<Style> {
        let new = self.entries[self.flushed..].to_vec();
        self.flushed = self.entries.len();
        new
    }
}
