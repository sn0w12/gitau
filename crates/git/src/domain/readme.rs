//! Extracts a human description from README content: the first real
//! prose paragraph, skipping the chrome READMEs lead with (centered HTML
//! logos, headers, badge rows, code fences, link definitions).

/// Cap for the extracted paragraph; the home card clamps to two lines, so
/// anything longer is cut here.
pub(crate) const README_DESCRIPTION_MAX_CHARS: usize = 300;

/// Returns the first paragraph of prose as plain text, or `None` when the
/// document holds none.
pub(crate) fn first_paragraph(content: &str, max_chars: usize) -> Option<String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let lines: Vec<&str> = content.lines().collect();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index].trim();
        if line.is_empty()
            || line.starts_with('#')
            || is_rule_like(line)
            || is_link_definition(line)
        {
            index += 1;
            continue;
        }
        if line.starts_with("```") || line.starts_with("~~~") {
            index = skip_fenced_block(&lines, index);
            continue;
        }
        // HTML blocks run until the first blank line.
        if line.starts_with('<') {
            index += 1;
            while index < lines.len() && !lines[index].trim().is_empty() {
                index += 1;
            }
            continue;
        }

        let start = index;
        while index < lines.len() && !lines[index].trim().is_empty() {
            index += 1;
        }
        // A rule as the block's final line means it was a setext heading.
        if index - start >= 2 && is_rule_like(lines[index - 1].trim()) {
            continue;
        }

        let paragraph = strip_markup(&lines[start..index].join(" "));
        if paragraph.is_empty() {
            continue;
        }
        return Some(truncate(&paragraph, max_chars));
    }
    None
}

fn skip_fenced_block(lines: &[&str], open: usize) -> usize {
    let fence = &lines[open].trim()[..3];
    let mut index = open + 1;
    while index < lines.len() && !lines[index].trim().starts_with(fence) {
        index += 1;
    }
    index + 1
}

/// Horizontal rules and setext underlines: only `=`, `-`, `_`, `*`, spaces.
fn is_rule_like(line: &str) -> bool {
    let has_marker = line.chars().any(|c| matches!(c, '=' | '-' | '_' | '*'));
    has_marker
        && line
            .chars()
            .all(|c| matches!(c, '=' | '-' | '_' | '*' | ' '))
}

fn is_link_definition(line: &str) -> bool {
    line.starts_with('[') && line.contains("]:")
}

/// Plain text for one paragraph: markdown images dropped, links reduced to
/// their text, HTML tags stripped, entities decoded, emphasis and inline
/// code markers removed, whitespace collapsed.
fn strip_markup(paragraph: &str) -> String {
    let without_links = markdown_links_to_text(paragraph);
    let without_html = strip_html_tags(&without_links);
    let without_markers = strip_emphasis(&without_html);
    let decoded = decode_entities(&without_markers);
    collapse_spaces(&decoded)
}

/// `(label_close, span_end)` for a `[label](target)` span whose label
/// opens at `open`: `label_close` indexes the depth-zero `]`, `span_end`
/// sits just past the closing `)`. `None` when the shape does not close.
fn markdown_link_span(bytes: &[u8], open: usize) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut cursor = open;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'[' => depth += 1,
            b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let mut target = cursor + 1;
                    if bytes.get(target) != Some(&b'(') {
                        return None;
                    }
                    let mut parens = 0usize;
                    while target < bytes.len() {
                        match bytes[target] {
                            b'(' => parens += 1,
                            b')' => {
                                parens -= 1;
                                if parens == 0 {
                                    return Some((cursor, target + 1));
                                }
                            }
                            _ => {}
                        }
                        target += 1;
                    }
                    return None;
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn markdown_links_to_text(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'!' && bytes.get(cursor + 1) == Some(&b'[') {
            if let Some((_, span_end)) = markdown_link_span(bytes, cursor + 1) {
                cursor = span_end;
                continue;
            }
        }
        if bytes[cursor] == b'[' {
            if let Some((label_close, span_end)) = markdown_link_span(bytes, cursor) {
                out.push_str(&markdown_links_to_text(&text[cursor + 1..label_close]));
                cursor = span_end;
                continue;
            }
        }
        let ch = text[cursor..].chars().next().unwrap();
        out.push(ch);
        cursor += ch.len_utf8();
    }
    out
}

fn strip_html_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            for skipped in chars.by_ref() {
                if skipped == '>' {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

/// Removes emphasis and inline-code markers. Underscores only count as
/// emphasis at word boundaries so snake_case survives.
fn strip_emphasis(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (index, &ch) in chars.iter().enumerate() {
        if ch == '*' || ch == '`' {
            continue;
        }
        if ch == '_' {
            let prev_space = index == 0 || chars[index - 1].is_whitespace();
            let next_space = index + 1 == chars.len() || chars[index + 1].is_whitespace();
            let opening = prev_space && !next_space;
            let closing = !prev_space && next_space;
            if opening || closing {
                continue;
            }
        }
        out.push(ch);
    }
    out
}

fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let cut: String = text.chars().take(max_chars).collect();
    match cut.rfind(' ') {
        Some(pos) if pos > max_chars / 2 => format!("{}...", &cut[..pos]),
        _ => format!("{cut}..."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paragraph(content: &str) -> String {
        first_paragraph(content, README_DESCRIPTION_MAX_CHARS).expect("paragraph expected")
    }

    #[test]
    fn plain_markdown_yields_first_paragraph() {
        assert_eq!(
            paragraph("# My Project\n\nFast, cross-platform builds.\n\nMore."),
            "Fast, cross-platform builds."
        );
    }

    #[test]
    fn centered_html_logo_and_badges_are_skipped() {
        let content = "<div align=\"center\">\n  <img src=\"logo.png\" alt=\"logo\"/>\n\n\
                       [![build](https://img.shields.io/ci.svg)](https://ci)\n\
                       [![crates.io](https://img.shields.io/crates/v/x.svg)](https://crates.io)\n\n\
                       </div>\n\n\
                       # Project\n\n\
                       A **fast** build tool with [docs](https://example.com).\n";
        assert_eq!(paragraph(content), "A fast build tool with docs.");
    }

    #[test]
    fn inline_markdown_is_stripped() {
        assert_eq!(
            paragraph("A `tool` for *editing* and [viewing](https://x.y) files."),
            "A tool for editing and viewing files."
        );
    }

    #[test]
    fn inline_html_in_paragraph_is_stripped() {
        assert_eq!(
            paragraph("Runs on <em>every</em> platform &amp; shell."),
            "Runs on every platform & shell."
        );
    }

    #[test]
    fn setext_title_is_skipped() {
        let content = "My Project\n==========\n\nReal description here.\n";
        assert_eq!(paragraph(content), "Real description here.");
    }

    #[test]
    fn badge_only_paragraphs_are_skipped() {
        let content = "[![a](https://img.shields.io/a.svg)](https://a)\n\n\
                       [ref]: https://example.com\n\n\
                       The actual description.\n";
        assert_eq!(paragraph(content), "The actual description.");
    }

    #[test]
    fn fenced_intro_blocks_are_skipped() {
        let content = "```\ncargo install demo\n```\n\nInstalls the tool.\n";
        assert_eq!(paragraph(content), "Installs the tool.");
    }

    #[test]
    fn long_paragraphs_truncate_on_word_boundaries() {
        let word = "word ";
        let content = format!("{}\n", word.repeat(120));
        let result = paragraph(&content);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= README_DESCRIPTION_MAX_CHARS + 3);
        assert!(result.chars().count() < content.trim().chars().count());
    }

    #[test]
    fn documents_without_prose_resolve_to_none() {
        let content = "# Title\n\n[![a](https://img/a.svg)](https://a)\n";
        assert!(first_paragraph(content, 300).is_none());
        assert!(first_paragraph("", 300).is_none());
    }

    #[test]
    fn snake_case_survives_underscore_stripping() {
        assert_eq!(
            paragraph("Uses `cargo_build` and cargo_next.\n"),
            "Uses cargo_build and cargo_next."
        );
    }
}
