const MAX_TEXT_CHARS: usize = 240;
#[cfg(feature = "enabled")]
const MAX_TYPE_CHARS: usize = 128;
#[cfg(feature = "enabled")]
const MAX_KEY_CHARS: usize = 48;

/// Redacts the shapes that carry personal data in practice: URLs (hosts and
/// tokens), absolute paths, email addresses, and long opaque identifiers.
/// A path swallows the rest of the input because paths may contain spaces.
/// Best effort by design; never pass user-authored text.
pub(crate) fn text(input: &str) -> String {
    text_with_limit(input, MAX_TEXT_CHARS)
}

fn text_with_limit(input: &str, max_chars: usize) -> String {
    let mut output = String::with_capacity(input.len().min(max_chars * 2));
    for raw in input.split_whitespace() {
        if !output.is_empty() {
            output.push(' ');
        }

        let candidate = raw.trim_matches(is_wrapping_punctuation);
        let candidate = if candidate.is_empty() { raw } else { candidate };

        if is_url(candidate) {
            output.push_str("<url>");
        } else if is_email(candidate) {
            output.push_str("<email>");
        } else if is_path(candidate) {
            output.push_str("<path>");
            break;
        } else if is_opaque_identifier(candidate) {
            output.push_str("<id>");
        } else {
            output.extend(raw.chars().filter(|ch| !ch.is_control()));
        }

        if output.chars().count() >= max_chars {
            break;
        }
    }

    truncate(output, max_chars)
}

#[cfg(feature = "enabled")]
pub(crate) fn key(input: &str) -> Option<String> {
    let mut output = String::with_capacity(input.len().min(MAX_KEY_CHARS));
    for ch in input.chars().take(MAX_KEY_CHARS) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            output.push(ch.to_ascii_lowercase());
        } else {
            output.push('_');
        }
    }
    let output = output.trim_matches('_').to_string();
    (!output.is_empty()).then_some(output)
}

#[cfg(feature = "enabled")]
pub(crate) fn key_or_default(input: &str) -> String {
    key(input).unwrap_or_else(|| "unknown".into())
}

#[cfg(feature = "enabled")]
pub(crate) fn type_name(input: &str) -> String {
    truncate(
        input.chars().filter(|ch| !ch.is_control()).collect(),
        MAX_TYPE_CHARS,
    )
}

fn is_wrapping_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '(' | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '<'
            | '>'
            | '"'
            | '\''
            | ','
            | ';'
            | ':'
            | '.'
            | '!'
            | '?'
    )
}

fn is_url(token: &str) -> bool {
    token.contains("://") || token.starts_with("www.")
}

fn is_email(token: &str) -> bool {
    let Some(at) = token.find('@') else {
        return false;
    };
    if token.contains('/') || token.contains('\\') {
        return false;
    }
    let (local, domain) = token.split_at(at);
    !local.is_empty() && domain.len() > 1
}

fn is_path(token: &str) -> bool {
    let bytes = token.as_bytes();
    let windows_drive = bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/');
    windows_drive
        || token.starts_with("\\\\")
        || token.starts_with("..\\")
        || token.starts_with("../")
        || token.starts_with(".\\")
        || token.starts_with("~/")
        || (token.starts_with('/') && token.len() > 1)
}

fn is_opaque_identifier(token: &str) -> bool {
    let meaningful: Vec<char> = token
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_'))
        .collect();
    if meaningful.len() < 32 {
        return false;
    }
    let alphanumeric = meaningful
        .iter()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .count();
    let dense = alphanumeric * 10 >= meaningful.len() * 9;
    dense && meaningful.iter().any(|ch| ch.is_ascii_digit())
}

fn truncate(mut input: String, max_chars: usize) -> String {
    if input.chars().count() <= max_chars {
        return input;
    }
    let cut = input
        .char_indices()
        .nth(max_chars)
        .map(|(index, _)| index)
        .unwrap_or(input.len());
    input.truncate(cut);
    input.push_str("...");
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_urls() {
        assert_eq!(
            text("request to https://api.example.com/repos?token=1 failed"),
            "request to <url> failed"
        );
    }

    #[test]
    fn redacts_absolute_paths_and_drops_the_tail() {
        assert_eq!(
            text(r"open C:\Users\bob\secret\file.txt failed"),
            "open <path>"
        );
        assert_eq!(text("open /home/bob/secret failed"), "open <path>");
        assert_eq!(text("open ~/secret failed"), "open <path>");
    }

    #[test]
    fn redacts_email_addresses() {
        assert_eq!(
            text("contact bob@example.com for access"),
            "contact <email> for access"
        );
        assert_eq!(text("contact bob@localhost"), "contact <email>");
    }

    #[test]
    fn redacts_long_opaque_identifiers() {
        assert_eq!(
            text("commit 0123456789abcdef0123456789abcdef01234567 is gone"),
            "commit <id> is gone"
        );
        assert_eq!(
            text("commit 550e8400-e29b-41d4-a716-446655440000 is gone"),
            "commit <id> is gone"
        );
    }

    #[test]
    fn keeps_ordinary_text() {
        assert_eq!(text("push rejected by remote"), "push rejected by remote");
        assert_eq!(text("branch main"), "branch main");
    }

    #[test]
    fn strips_control_characters() {
        assert_eq!(text("bad\u{0}value"), "badvalue");
    }

    #[test]
    fn truncates_long_text() {
        let long = "a".repeat(400);
        let output = text(&long);
        assert!(output.ends_with("..."));
        assert_eq!(output.chars().count(), MAX_TEXT_CHARS + 3);
    }

    #[cfg(feature = "enabled")]
    #[test]
    fn normalizes_keys() {
        assert_eq!(key("Files Changed"), Some("files_changed".into()));
        assert_eq!(key("repo.path"), Some("repo.path".into()));
        assert_eq!(key(""), None);
        assert_eq!(key("!!!"), None);
        assert!(key(&"x".repeat(100)).unwrap().len() <= MAX_KEY_CHARS);
    }

    #[cfg(feature = "enabled")]
    #[test]
    fn truncates_type_names() {
        let long = "a".repeat(200);
        assert_eq!(type_name(&long).chars().count(), MAX_TYPE_CHARS + 3);
    }
}
