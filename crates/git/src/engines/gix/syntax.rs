use crate::streaming::model::{DiffRow, DiffRowKind, WireStyle};

use highlight::{LineKind, LineRef, SectionHighlighter as Inner};

fn line_refs(rows: &[DiffRow]) -> Vec<LineRef<'_>> {
    rows.iter()
        .map(|row| LineRef {
            kind: match row.kind {
                DiffRowKind::Context => LineKind::Context,
                DiffRowKind::Addition => LineKind::Addition,
                DiffRowKind::Deletion => LineKind::Deletion,
                _ => LineKind::Other,
            },
            old_lineno: row.old_lineno,
            new_lineno: row.new_lineno,
            content: &row.content,
        })
        .collect()
}

pub struct SectionHighlighter {
    inner: Inner,
}

impl SectionHighlighter {
    pub fn new(path: &str, old: &[u8], new: &[u8]) -> Option<Self> {
        Inner::new(path, old, new).map(|inner| Self { inner })
    }

    pub fn highlight_rows(&mut self, rows: &mut [DiffRow]) {
        let lines = line_refs(rows);
        let spans = self.inner.spans(&lines);
        drop(lines);
        for (row, spans) in rows.iter_mut().zip(spans) {
            row.spans = spans;
        }
    }

    pub fn take_new_styles(&mut self) -> Vec<WireStyle> {
        self.inner.take_new_styles()
    }
}

pub fn attach(rows: &mut [DiffRow], path: &str, old: &[u8], new: &[u8]) -> Vec<WireStyle> {
    let Some(mut highlighter) = SectionHighlighter::new(path, old, new) else {
        return Vec::new();
    };
    highlighter.highlight_rows(rows);
    highlighter.take_new_styles()
}
