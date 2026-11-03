use std::collections::HashMap;
use std::collections::HashSet;

use syntect::highlighting::{FontStyle, HighlightIterator, HighlightState, Highlighter};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference};

use crate::style::{font_bits, hex, packed, LineRef, Style, StyleInterner};
use crate::syntax::{detect_syntax, syntax_set};
use crate::themes::themes;
use crate::{MAX_HIGHLIGHT_BYTES, NO_NEWLINE_MARKER};

/// One side's resumable parser: grammar state and position persist across
/// calls so a section can be highlighted chunk-by-chunk, each call paying
/// only the lines that call needs (the parse-state checkpoint pattern from
/// syntect's docs).
pub(crate) struct SideStream {
    pub(crate) lines: Vec<String>,
    state: ParseState,
    next: usize,
    light_state: HighlightState,
    dark_state: HighlightState,
    // 'static: theme snapshots are leaked, never freed.
    light_hl: Highlighter<'static>,
    dark_hl: Highlighter<'static>,
    default_light: syntect::highlighting::Color,
    default_dark: syntect::highlighting::Color,
}

impl SideStream {
    pub(crate) fn new(text: &str, syntax: &'static SyntaxReference) -> SideStream {
        let themes = themes();
        let light_hl = Highlighter::new(&themes.light);
        let dark_hl = Highlighter::new(&themes.dark);
        let default_light = themes
            .light
            .settings
            .foreground
            .unwrap_or(syntect::highlighting::Color::WHITE);
        let default_dark = themes
            .dark
            .settings
            .foreground
            .unwrap_or(syntect::highlighting::Color::WHITE);
        SideStream {
            lines: text.split_inclusive('\n').map(str::to_string).collect(),
            state: ParseState::new(syntax),
            next: 0,
            light_state: HighlightState::new(&light_hl, ScopeStack::new()),
            dark_state: HighlightState::new(&dark_hl, ScopeStack::new()),
            light_hl,
            dark_hl,
            default_light,
            default_dark,
        }
    }

    /// Advances grammar state through `upto` (1-based inclusive) and returns
    /// styled spans for exactly the linenos in `wanted`. Lines in between are
    /// parsed for state but never materialized.
    pub(crate) fn advance(
        &mut self,
        upto: u32,
        wanted: &HashSet<u32>,
        interner: &mut StyleInterner,
    ) -> Vec<(u32, Vec<u32>)> {
        let mut produced = Vec::new();
        while self.next < self.lines.len() && self.next < upto as usize {
            let line = self.lines[self.next].clone();
            let content_len = line.trim_end_matches(['\r', '\n']).len();
            let lineno = (self.next + 1) as u32;
            if wanted.contains(&lineno) {
                let spans = self.style_line(&line, content_len, interner);
                if !spans.is_empty() {
                    produced.push((lineno, spans));
                }
            } else {
                self.feed_state(&line);
            }
            self.next += 1;
        }
        produced
    }

    fn feed_state(&mut self, line: &str) {
        let Ok(ops) = self.state.parse_line(line, syntax_set()) else {
            return;
        };
        // HighlightStates only advance when their iterator is drained; run
        // both to completion without emitting anything.
        let mut light = HighlightIterator::new(&mut self.light_state, &ops, line, &self.light_hl);
        let mut dark = HighlightIterator::new(&mut self.dark_state, &ops, line, &self.dark_hl);
        while light.next().is_some() {
            dark.next();
        }
    }

    /// Runs both theme resolutions over one line's scope operations. The two
    /// iterators are driven by identical ops, so segmentation always matches;
    /// each emitted span carries an interned style holding both colors.
    fn style_line(
        &mut self,
        line: &str,
        content_len: usize,
        interner: &mut StyleInterner,
    ) -> Vec<u32> {
        let Ok(ops) = self.state.parse_line(line, syntax_set()) else {
            return Vec::new();
        };
        let light_iter = HighlightIterator::new(&mut self.light_state, &ops, line, &self.light_hl);
        let mut dark_iter = HighlightIterator::new(&mut self.dark_state, &ops, line, &self.dark_hl);

        let mut spans: Vec<u32> = Vec::with_capacity(16);
        let mut offset = 0usize;
        for (light_style, light_text) in light_iter {
            let Some((dark_style, dark_text)) = dark_iter.next() else {
                break;
            };
            debug_assert_eq!(
                light_text.len(),
                dark_text.len(),
                "theme iterators diverged"
            );
            let len = light_text.len();

            let start = offset;
            offset += len;
            let end = (start + len).min(content_len);
            if start >= content_len || end <= start {
                continue;
            }

            let unstyled = font_bits(light_style.font_style) == 0
                && font_bits(dark_style.font_style) == 0
                && packed(light_style.foreground) == packed(self.default_light)
                && packed(dark_style.foreground) == packed(self.default_dark);
            if unstyled {
                continue;
            }

            let key = (
                packed(light_style.foreground),
                packed(dark_style.foreground),
                font_bits(light_style.font_style) | font_bits(dark_style.font_style),
            );
            let id = interner.intern(key, || Style {
                light: hex(light_style.foreground),
                dark: hex(dark_style.foreground),
                bold: light_style.font_style.contains(FontStyle::BOLD)
                    || dark_style.font_style.contains(FontStyle::BOLD),
                underline: light_style.font_style.contains(FontStyle::UNDERLINE)
                    || dark_style.font_style.contains(FontStyle::UNDERLINE),
                italic: light_style.font_style.contains(FontStyle::ITALIC)
                    || dark_style.font_style.contains(FontStyle::ITALIC),
            });
            spans.push(start as u32);
            spans.push((end - start) as u32);
            spans.push(u32::from(id));
        }
        spans
    }
}

/// Resumable highlighter for one diff section. Created during
/// materialization while both blobs are in hand; driven per chunk by the
/// streaming emitter so first-chunk latency covers only the first chunk's
/// rows.
pub struct SectionHighlighter {
    old: Option<SideStream>,
    new: Option<SideStream>,
    styles: StyleInterner,
}

impl SectionHighlighter {
    /// Returns None when the section cannot be highlighted: unknown grammar,
    /// non-UTF-8 blob, or a side over [`MAX_HIGHLIGHT_BYTES`].
    pub fn new(path: &str, old: &[u8], new: &[u8]) -> Option<SectionHighlighter> {
        crate::warm_up();
        if old.len() > MAX_HIGHLIGHT_BYTES || new.len() > MAX_HIGHLIGHT_BYTES {
            return None;
        }
        let syntax = detect_syntax(path)?;
        let old_text = std::str::from_utf8(old).ok()?;
        let new_text = std::str::from_utf8(new).ok()?;
        Some(SectionHighlighter {
            old: (!old_text.is_empty()).then(|| SideStream::new(old_text, syntax)),
            new: (!new_text.is_empty()).then(|| SideStream::new(new_text, syntax)),
            styles: StyleInterner::default(),
        })
    }

    /// Returns spans aligned with `lines`; `None` keeps the line plain.
    /// Context lines prefer the new side, matching byte-identical content on
    /// both sides.
    pub fn spans(&mut self, lines: &[LineRef<'_>]) -> Vec<Option<Vec<u32>>> {
        let mut need_old: HashSet<u32> = HashSet::new();
        let mut need_new: HashSet<u32> = HashSet::new();
        for line in lines {
            if !line.kind.is_highlightable()
                || line.content.is_empty()
                || line.content == NO_NEWLINE_MARKER
            {
                continue;
            }
            if let Some(lineno) = line.old_lineno {
                need_old.insert(lineno);
            }
            if let Some(lineno) = line.new_lineno {
                need_new.insert(lineno);
            }
        }

        let mut old_spans: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut new_spans: HashMap<u32, Vec<u32>> = HashMap::new();
        if !need_old.is_empty() {
            if let Some(side) = self.old.as_mut() {
                old_spans.extend(side.advance(max_of(&need_old), &need_old, &mut self.styles));
            }
        }
        if !need_new.is_empty() {
            if let Some(side) = self.new.as_mut() {
                new_spans.extend(side.advance(max_of(&need_new), &need_new, &mut self.styles));
            }
        }

        lines
            .iter()
            .map(|line| {
                if !line.kind.is_highlightable()
                    || line.content.is_empty()
                    || line.content == NO_NEWLINE_MARKER
                {
                    return None;
                }
                let spans = line
                    .new_lineno
                    .and_then(|lineno| new_spans.get(&lineno))
                    .or_else(|| line.old_lineno.and_then(|lineno| old_spans.get(&lineno)))?;
                let limit = line.content.len() as u32;
                spans
                    .chunks_exact(3)
                    .all(|triple| triple[0] + triple[1] <= limit)
                    .then(|| spans.clone())
            })
            .collect()
    }

    /// Style entries appended since the previous call, for delta transport.
    pub fn take_new_styles(&mut self) -> Vec<Style> {
        self.styles.take_new()
    }
}

fn max_of(wanted: &HashSet<u32>) -> u32 {
    wanted.iter().copied().max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::LineKind;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct DiffRow {
        kind: DiffRowKind,
        old_lineno: Option<u32>,
        new_lineno: Option<u32>,
        content: String,
        raw_hex: Option<String>,
        spans: Option<Vec<u32>>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum DiffRowKind {
        Context,
        Addition,
    }

    fn lines_for<'a>(rows: &'a [DiffRow]) -> Vec<LineRef<'a>> {
        rows.iter()
            .map(|row| LineRef {
                kind: match row.kind {
                    DiffRowKind::Context => LineKind::Context,
                    DiffRowKind::Addition => LineKind::Addition,
                },
                old_lineno: row.old_lineno,
                new_lineno: row.new_lineno,
                content: &row.content,
            })
            .collect()
    }

    fn attach(rows: &mut [DiffRow], path: &str, old: &[u8], new: &[u8]) -> Vec<Style> {
        let Some(mut highlighter) = SectionHighlighter::new(path, old, new) else {
            return Vec::new();
        };
        let lines = lines_for(rows);
        let spans = highlighter.spans(&lines);
        drop(lines);
        for (row, spans) in rows.iter_mut().zip(spans) {
            row.spans = spans;
        }
        highlighter.take_new_styles()
    }

    trait HighlightRows {
        fn highlight_rows(&mut self, rows: &mut [DiffRow]);
    }

    impl HighlightRows for SectionHighlighter {
        fn highlight_rows(&mut self, rows: &mut [DiffRow]) {
            let lines = lines_for(rows);
            let spans = self.spans(&lines);
            drop(lines);
            for (row, spans) in rows.iter_mut().zip(spans) {
                row.spans = spans;
            }
        }
    }

    const SOURCE: &str = "\
fn main() {
    // greet the world
    let name = \"world\";
    println!(\"hello {name}\");
}
";

    fn addition_rows(text: &str) -> Vec<DiffRow> {
        text.lines()
            .enumerate()
            .map(|(idx, line)| DiffRow {
                kind: DiffRowKind::Addition,
                old_lineno: None,
                new_lineno: Some(idx as u32 + 1),
                content: line.to_string(),
                raw_hex: None,
                spans: None,
            })
            .collect()
    }

    fn reconstruct(row: &DiffRow) -> String {
        let spans = row.spans.as_ref().unwrap();
        let mut out = String::new();
        let mut cursor = 0usize;
        for t in spans.chunks_exact(3) {
            let (start, len) = (t[0] as usize, t[1] as usize);
            out.push_str(&row.content[cursor..start]);
            out.push_str(&row.content[start..start + len]);
            cursor = start + len;
        }
        out.push_str(&row.content[cursor..]);
        out
    }

    #[test]
    fn attach_resolves_theme_styles_and_preserves_content() {
        let mut rows = addition_rows(SOURCE);
        let styles = attach(rows.as_mut_slice(), "main.rs", b"", SOURCE.as_bytes());

        assert!(!styles.is_empty(), "expected resolved theme styles");
        for style in &styles {
            assert!(style.light.starts_with('#') && style.light.len() == 7);
            assert!(style.dark.starts_with('#') && style.dark.len() == 7);
        }

        // The keyword/string/comment lines carry styled spans that
        // reconstruct their content verbatim.
        let commented = &rows[1];
        assert!(commented.spans.as_ref().is_some_and(|s| !s.is_empty()));
        assert_eq!(reconstruct(commented), commented.content);

        let assignment = &rows[2];
        let spans = assignment.spans.as_ref().unwrap();
        // The literal may be split into quote-punctuation + inner-text
        // pieces by the grammar; assert the region is covered.
        let region_start = assignment.content.find("\"world\"").unwrap();
        let region_end = region_start + "\"world\"".len();
        let covering = spans
            .chunks_exact(3)
            .filter(|t| {
                (t[0] as usize) < region_end && (t[0] as usize + t[1] as usize) > region_start
            })
            .count();
        assert!(covering >= 1, "string literal region must be styled");
        let entry = &styles[spans[2] as usize - 1];
        assert_ne!(entry.light, entry.dark, "themes should disagree somewhere");

        // Plain-text line (none here beyond braces) would simply be sparse;
        // every row that got spans must reconstruct byte-for-byte.
        for row in rows.iter().filter(|r| r.spans.is_some()) {
            assert_eq!(reconstruct(row), row.content);
        }
    }

    #[test]
    fn incremental_chunking_matches_batch_exactly() {
        let expected = {
            let mut rows = addition_rows(SOURCE);
            let styles = attach(rows.as_mut_slice(), "main.rs", b"", SOURCE.as_bytes());
            (rows, styles)
        };

        for split_at in [1usize, 2, 3, 5] {
            let mut hl = SectionHighlighter::new("main.rs", b"", SOURCE.as_bytes()).unwrap();
            let mut rows = addition_rows(SOURCE);
            let mut styles = Vec::new();
            hl.highlight_rows(&mut rows[..split_at]);
            styles.extend(hl.take_new_styles());
            hl.highlight_rows(&mut rows[split_at..]);
            styles.extend(hl.take_new_styles());

            assert_eq!(rows, expected.0, "chunked at {split_at}");
            assert_eq!(styles, expected.1, "style table diverged at {split_at}");
        }
    }

    #[test]
    fn marker_rows_and_oversized_files_stay_plain() {
        let old_text = "a\nb\n";
        let new_text = "a\nB\nc\n";
        let mut rows = vec![
            DiffRow {
                kind: DiffRowKind::Context,
                old_lineno: Some(1),
                new_lineno: Some(1),
                content: "a".into(),
                raw_hex: None,
                spans: None,
            },
            DiffRow {
                kind: DiffRowKind::Addition,
                old_lineno: None,
                new_lineno: Some(3),
                content: NO_NEWLINE_MARKER.into(),
                raw_hex: None,
                spans: None,
            },
        ];
        let styles = attach(
            rows.as_mut_slice(),
            "notes.txt",
            old_text.as_bytes(),
            new_text.as_bytes(),
        );
        // notes.txt has no grammar; nothing highlighted anywhere.
        assert!(styles.is_empty());
        assert!(rows.iter().all(|r| r.spans.is_none()));

        let big = "x".repeat(MAX_HIGHLIGHT_BYTES + 1);
        let mut rows = addition_rows("x\n");
        let styles = attach(
            rows.as_mut_slice(),
            "big.rs",
            big.as_bytes(),
            big.as_bytes(),
        );
        assert!(styles.is_empty());
        assert!(rows[0].spans.is_none());
    }
}
