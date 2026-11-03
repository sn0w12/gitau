use std::collections::HashMap;
use std::collections::HashSet;

use crate::section::SideStream;
use crate::style::{Style, StyleInterner};
use crate::syntax::syntax_for_language;
use crate::{warm_up, MAX_HIGHLIGHT_BYTES};

/// Highlighted spans for one standalone snippet (a markdown code fence),
/// aligned with `text.split('\n')`: empty entries render plain.
pub struct SnippetHighlight {
    pub spans_by_line: Vec<Vec<u32>>,
    pub styles: Vec<Style>,
}

/// Highlights `text` with full-text grammar context. Returns `None` for
/// unknown languages, empty text, or oversized input; callers render
/// those plain.
pub fn highlight_snippet(language: &str, text: &str) -> Option<SnippetHighlight> {
    warm_up();
    if text.is_empty() || text.len() > MAX_HIGHLIGHT_BYTES {
        return None;
    }
    let syntax = syntax_for_language(language)?;
    let mut stream = SideStream::new(text, syntax);
    let mut interner = StyleInterner::default();
    let line_count = stream.lines.len() as u32;
    let wanted: HashSet<u32> = (1..=line_count).collect();
    let mut by_line: HashMap<u32, Vec<u32>> = stream
        .advance(line_count, &wanted, &mut interner)
        .into_iter()
        .collect();
    let spans_by_line = (1..=line_count)
        .map(|lineno| by_line.remove(&lineno).unwrap_or_default())
        .collect();
    Some(SnippetHighlight {
        spans_by_line,
        styles: interner.take_new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "\
fn main() {
    // greet the world
    let name = \"world\";
    println!(\"hello {name}\");
}
";

    #[test]
    fn snippet_highlight_covers_code_and_skips_plain() {
        let highlighted = highlight_snippet("rs", SOURCE).expect("rust snippet highlights");
        assert_eq!(
            highlighted.spans_by_line.len(),
            SOURCE.split_inclusive('\n').count()
        );
        assert!(!highlighted.styles.is_empty());
        assert!(
            !highlighted.spans_by_line[1].is_empty(),
            "comment line carries spans"
        );

        assert!(highlight_snippet("plaintext", SOURCE).is_none());
        assert!(highlight_snippet("rs", "").is_none());
    }

    #[test]
    fn justfile_snippet_highlights() {
        let source = "\
set shell := [\"bash\", \"-cu\"]

build target=\"debug\":
    @echo \"building {{target}}\"
    cargo build {{target}}
";
        let highlighted = highlight_snippet("justfile", source).expect("justfile highlights");
        assert_eq!(
            highlighted.spans_by_line.len(),
            source.split_inclusive('\n').count()
        );
        assert!(!highlighted.styles.is_empty());
        assert!(
            !highlighted.spans_by_line[3].is_empty(),
            "recipe body carries spans"
        );
    }
}
