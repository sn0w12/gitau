use std::sync::OnceLock;

use syntect::parsing::{SyntaxReference, SyntaxSet};

const EXTENSION_LANGUAGES: &[&str] = &[
    "c", "cc", "clj", "cljs", "cljc", "cpp", "cs", "css", "cxx", "dart", "ex", "exs", "go", "gql",
    "h", "hh", "hpp", "hs", "htm", "html", "hxx", "java", "js", "json", "jsonc", "jsx", "kt",
    "kts", "less", "lua", "md", "mjs", "mts", "ph", "php", "ps1", "psm1", "py", "pyi", "r", "rb",
    "rs", "sass", "scala", "scss", "sh", "sql", "svg", "swift", "toml", "ts", "tsx", "xhtml",
    "yaml", "yml",
];

const FILENAME_LANGUAGES: &[&str] = &["CMakeLists.txt", "Dockerfile", "Makefile"];

/// Extension spellings that resolve to a different extension whose grammar
/// is the correct one for the language.
const EXTENSION_ALIASES: &[(&str, &str)] = &[("ph", "php"), ("jsonc", "json")];

pub(crate) fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(two_face::syntax::extra_newlines)
}

/// Mirrors the frontend's path-to-grammar mapping.
pub fn detect_syntax(path: &str) -> Option<&'static SyntaxReference> {
    fn probe(candidates: &[String]) -> Option<&'static SyntaxReference> {
        let ss = syntax_set();
        for candidate in candidates {
            if let Some(syntax) = ss.find_syntax_by_extension(candidate) {
                return Some(syntax);
            }
        }
        None
    }

    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let lowered = base.to_lowercase();

    if FILENAME_LANGUAGES.contains(&base) || base.starts_with("Dockerfile.") {
        if let Some(found) = probe(&[base.to_string(), lowered.clone()]) {
            return Some(found);
        }
    }

    let dot = base.rfind('.')?;
    if dot == 0 {
        return None;
    }
    let extension = lowered[dot + 1..].to_string();
    if !EXTENSION_LANGUAGES.contains(&extension.as_str()) {
        return None;
    }
    let aliased = EXTENSION_ALIASES
        .iter()
        .find(|(from, _)| *from == extension)
        .map(|(_, to)| (*to).to_string());
    let candidate = aliased.as_deref().unwrap_or(&extension);
    probe(&[candidate.to_string()])
}

/// Fence tags with no grammar: highlighting would only echo the text
/// back unstyled, so callers render them plain without a round trip.
const PLAIN_LANGUAGES: &[&str] = &[
    "console",
    "output",
    "plain",
    "plaintext",
    "shell-session",
    "terminal",
    "text",
    "txt",
];

/// Full language names resolving to a different extension lookup.
const LANGUAGE_ALIASES: &[(&str, &str)] = &[
    ("c#", "cs"),
    ("c++", "cpp"),
    ("dockerfile", "Dockerfile"),
    ("golang", "go"),
    ("javascript", "js"),
    ("jsonc", "json"),
    ("makefile", "Makefile"),
    ("markdown", "md"),
    ("powershell", "ps1"),
    ("python", "py"),
    ("ruby", "rb"),
    ("rust", "rs"),
    ("shell", "sh"),
    ("typescript", "ts"),
    ("zsh", "sh"),
];

/// Resolves a markdown fence tag to a grammar. Extension lookup covers
/// short tags (`rs`, `py`); a case-insensitive name match covers full
/// names and special files (`Dockerfile`, `Makefile`).
pub(crate) fn syntax_for_language(language: &str) -> Option<&'static SyntaxReference> {
    let lang = language.trim().to_lowercase();
    if lang.is_empty() || PLAIN_LANGUAGES.contains(&lang.as_str()) {
        return None;
    }
    let ss = syntax_set();
    let aliased = LANGUAGE_ALIASES
        .iter()
        .find(|(from, _)| *from == lang)
        .map(|(_, to)| (*to).to_string())
        .unwrap_or(lang.clone());
    if let Some(syntax) = ss.find_syntax_by_extension(&aliased) {
        return Some(syntax);
    }
    ss.syntaxes()
        .iter()
        .find(|syntax| syntax.name.to_lowercase() == lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonc_maps_to_the_json_grammar() {
        let syntax = detect_syntax("config.jsonc").expect("jsonc resolves");
        assert_eq!(syntax.name, "JSON");
    }

    #[test]
    fn jsonc_fence_resolves_to_json() {
        let syntax = syntax_for_language("jsonc").expect("jsonc fence resolves");
        assert_eq!(syntax.name, "JSON");
    }

    #[test]
    fn fence_tags_resolve_to_grammars() {
        for tag in [
            "rs",
            "py",
            "js",
            "ts",
            "typescript",
            "python",
            "shell",
            "Dockerfile",
        ] {
            assert!(
                syntax_for_language(tag).is_some(),
                "expected a grammar for fence tag {tag:?}"
            );
        }
        for tag in ["", "plaintext", "text", "console", "brainfuck-xyz"] {
            assert!(
                syntax_for_language(tag).is_none(),
                "expected no grammar for fence tag {tag:?}"
            );
        }
    }
}
