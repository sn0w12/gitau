use serde::Serialize;

use crate::engines::gix::highlight;

/// Oversized fences render plain rather than stalling the UI thread.
const MAX_SNIPPET_BYTES: usize = 128 * 1024;

/// One highlighted markdown code fence. `spans_by_line` aligns with
/// `text.split('\n')` (a trailing empty line simply has no entry);
/// `styles` is the 1-based table the span triples index into, shaped
/// exactly like the diff `SyntaxStyle` the UI already renders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HighlightedSnippet {
    pub highlighted: bool,
    pub spans_by_line: Vec<Vec<u32>>,
    pub styles: Vec<SnippetStyle>,
}

/// One resolved style covering both app themes. Field names match the
/// frontend `SyntaxStyle` (`b`/`i`/`u`), not the diff `WireStyle`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnippetStyle {
    pub light: String,
    pub dark: String,
    #[serde(skip_serializing_if = "std::ops::Not::not", rename = "b")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not", rename = "i")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not", rename = "u")]
    pub underline: bool,
}

/// Highlights one code fence, or reports it plain. Pure and fast for
/// snippet sizes; unknown languages and oversized input stay plain.
pub fn highlight_code(language: &str, text: &str) -> HighlightedSnippet {
    if text.len() > MAX_SNIPPET_BYTES {
        return plain();
    }
    let Some(highlighted) = highlight::highlight_snippet(language, text) else {
        return plain();
    };
    HighlightedSnippet {
        highlighted: true,
        spans_by_line: highlighted.spans_by_line,
        styles: highlighted
            .styles
            .into_iter()
            .map(|style| SnippetStyle {
                light: style.light,
                dark: style.dark,
                bold: style.bold,
                italic: style.italic,
                underline: style.underline,
            })
            .collect(),
    }
}

fn plain() -> HighlightedSnippet {
    HighlightedSnippet {
        highlighted: false,
        spans_by_line: Vec::new(),
        styles: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_language_stays_plain() {
        let snippet = highlight_code("brainfuck-xyz", "let x = 1;\n");
        assert!(!snippet.highlighted);
        assert!(snippet.spans_by_line.is_empty());
        assert!(snippet.styles.is_empty());
    }

    #[test]
    fn rust_fence_highlights_with_style_table() {
        let snippet = highlight_code("rust", "fn main() {}\n");
        assert!(snippet.highlighted);
        assert_eq!(snippet.spans_by_line.len(), 1);
        assert!(!snippet.styles.is_empty());
        for style in &snippet.styles {
            assert!(style.light.starts_with('#') && style.light.len() == 7);
        }
    }

    #[test]
    fn oversized_fence_stays_plain() {
        let text = "x\n".repeat(MAX_SNIPPET_BYTES);
        assert!(!highlight_code("rust", &text).highlighted);
    }
}
