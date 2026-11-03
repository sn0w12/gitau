use std::sync::OnceLock;

use syntect::parsing::{SyntaxDefinition, SyntaxReference, SyntaxSet, SyntaxSetBuilder};

/// Extension spellings that resolve to a different extension whose grammar
/// is the correct one for the language.
const EXTENSION_ALIASES: &[(&str, &str)] = &[("ph", "php"), ("jsonc", "json")];

/// Carries the owning syntax set: `parse_line` requires the set the grammar
/// was loaded from.
#[derive(Clone, Copy)]
pub(crate) struct Grammar {
    pub(crate) syntax: &'static SyntaxReference,
    pub(crate) set: &'static SyntaxSet,
}

pub(crate) fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(two_face::syntax::extra_newlines)
}

/// two-face and syntect ship no Just grammar, so the vendored one lives in
/// its own tiny set rather than rebuilding the 220-grammar set at startup.
fn just_set() -> &'static SyntaxSet {
    static JUST_SET: OnceLock<SyntaxSet> = OnceLock::new();
    JUST_SET.get_or_init(|| {
        let definition = SyntaxDefinition::load_from_str(
            include_str!("../syntaxes/just.sublime-syntax"),
            true,
            Some("Just"),
        )
        .expect("vendored Just grammar parses");
        let mut builder = SyntaxSetBuilder::new();
        builder.add(definition);
        builder.build()
    })
}

fn grammar_sets() -> [&'static SyntaxSet; 2] {
    [just_set(), syntax_set()]
}

pub(crate) fn warm_up() {
    let _ = grammar_sets();
}

fn is_plain_text(syntax: &SyntaxReference) -> bool {
    syntax.name == "Plain Text"
}

/// Longest suffix first, so `app.html.erb` resolves through `html.erb`
/// before falling back to `erb`.
fn find_by_path(set: &'static SyntaxSet, base: &str) -> Option<&'static SyntaxReference> {
    let mut candidate = base;
    loop {
        let lookup = EXTENSION_ALIASES
            .iter()
            .find(|(from, _)| *from == candidate)
            .map(|(_, to)| *to)
            .unwrap_or(candidate);
        if let Some(syntax) = set.find_syntax_by_extension(lookup) {
            return (!is_plain_text(syntax)).then_some(syntax);
        }
        let dot = candidate.find('.')?;
        candidate = &candidate[dot + 1..];
    }
}

pub(crate) fn detect_syntax(path: &str) -> Option<Grammar> {
    let base = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .to_lowercase();
    if base.is_empty() {
        return None;
    }
    for set in grammar_sets() {
        if base.starts_with("dockerfile.") {
            if let Some(syntax) = set.find_syntax_by_extension("dockerfile") {
                return Some(Grammar { syntax, set });
            }
        }
        if let Some(syntax) = find_by_path(set, &base) {
            return Some(Grammar { syntax, set });
        }
    }
    None
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
    ("csharp", "cs"),
    ("docker", "Dockerfile"),
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
/// names and special files (`Dockerfile`, `Makefile`, `justfile`).
pub(crate) fn syntax_for_language(language: &str) -> Option<Grammar> {
    let lang = language.trim().to_lowercase();
    if lang.is_empty() || PLAIN_LANGUAGES.contains(&lang.as_str()) {
        return None;
    }
    let aliased = LANGUAGE_ALIASES
        .iter()
        .find(|(from, _)| *from == lang)
        .map(|(_, to)| *to)
        .unwrap_or(lang.as_str());
    for set in grammar_sets() {
        if let Some(syntax) = set.find_syntax_by_token(aliased) {
            if !is_plain_text(syntax) {
                return Some(Grammar { syntax, set });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jsonc_maps_to_the_json_grammar() {
        let syntax = detect_syntax("config.jsonc").expect("jsonc resolves");
        assert_eq!(syntax.syntax.name, "JSON");
    }

    #[test]
    fn jsonc_fence_resolves_to_json() {
        let syntax = syntax_for_language("jsonc").expect("jsonc fence resolves");
        assert_eq!(syntax.syntax.name, "JSON");
    }

    #[test]
    fn justfile_resolves_to_the_vendored_grammar() {
        for path in ["justfile", "Justfile", ".justfile", "tools/justfile"] {
            let syntax = detect_syntax(path).expect("justfile resolves");
            assert_eq!(syntax.syntax.name, "Just", "for {path}");
        }
        for tag in ["just", "justfile"] {
            let syntax = syntax_for_language(tag).expect("just fence resolves");
            assert_eq!(syntax.syntax.name, "Just", "for {tag}");
        }
    }

    #[test]
    fn every_grammar_extension_is_reachable() {
        for syntax in syntax_set().syntaxes() {
            if is_plain_text(syntax) {
                continue;
            }
            for extension in &syntax.file_extensions {
                let path = format!("sample.{extension}");
                let detected = detect_syntax(&path).unwrap_or_else(|| {
                    panic!("{extension:?} from {} does not resolve", syntax.name)
                });
                assert!(
                    detected
                        .syntax
                        .file_extensions
                        .iter()
                        .any(|claimed| claimed.eq_ignore_ascii_case(extension)),
                    "{path:?} resolved to {:?}, which does not claim {extension:?}",
                    detected.syntax.name
                );
            }
        }
    }

    #[test]
    fn filename_grammars_resolve_without_an_extension() {
        let cases = [
            ("Dockerfile", "Dockerfile"),
            ("Dockerfile.prod", "Dockerfile"),
            ("Makefile", "Makefile"),
            ("CMakeLists.txt", "CMake"),
            ("Cargo.lock", "TOML"),
            (".gitignore", "Git Ignore"),
            (".env", "DotENV"),
        ];
        for (path, grammar) in cases {
            let syntax = detect_syntax(path).unwrap_or_else(|| panic!("{path} resolves"));
            assert_eq!(syntax.syntax.name, grammar, "for {path}");
        }
    }

    #[test]
    fn plain_text_and_unknown_files_stay_unhighlighted() {
        for path in ["notes.txt", "LICENSE", "README", "data.unknownext"] {
            assert!(
                detect_syntax(path).is_none(),
                "expected no grammar for {path}"
            );
        }
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
            "csharp",
            "bash",
            "diff",
            "cmake",
            "zig",
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
