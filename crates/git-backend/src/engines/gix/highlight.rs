use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::OnceLock;

use syntect::highlighting::{FontStyle, HighlightIterator, HighlightState, Highlighter, Theme};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

use crate::streaming::model::{DiffRow, DiffRowKind, WireStyle};

/// Files above this size render without highlighting rather than stalling
/// section delivery.
pub const MAX_HIGHLIGHT_BYTES: usize = 512 * 1024;

pub(crate) const NO_NEWLINE_MARKER: &str = "\\ No newline at end of file";

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
const EXTENSION_ALIASES: &[(&str, &str)] = &[("ph", "php")];

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAX_SET.get_or_init(|| {
        // bat's JSON grammar already tolerates comments; registering the
        // jsonc spelling points JSONC files at that same grammar.
        let mut builder = two_face::syntax::extra_newlines().into_builder();
        if let Some(json) = builder.syntaxes().iter().find(|s| s.name == "JSON") {
            let mut jsonc = json.clone();
            jsonc.file_extensions.push("jsonc".to_string());
            builder.add(jsonc);
        }
        builder.build()
    })
}

/// Themes built into the binary and exposed as settings. Entries must stay
/// sorted: light themes first, then dark ones, so each settings dropdown
/// reads in a sensible order.
const EMBEDDED_THEMES: &[(two_face::theme::EmbeddedThemeName, &str, bool)] = &[
    (
        two_face::theme::EmbeddedThemeName::CatppuccinLatte,
        "Catppuccin Latte",
        false,
    ),
    (
        two_face::theme::EmbeddedThemeName::ColdarkCold,
        "Coldark Cold",
        false,
    ),
    (two_face::theme::EmbeddedThemeName::Github, "GitHub", false),
    (
        two_face::theme::EmbeddedThemeName::GruvboxLight,
        "gruvbox Light",
        false,
    ),
    (
        two_face::theme::EmbeddedThemeName::InspiredGithub,
        "Inspired GitHub",
        false,
    ),
    (
        two_face::theme::EmbeddedThemeName::MonokaiExtendedLight,
        "Monokai Extended Light",
        false,
    ),
    (
        two_face::theme::EmbeddedThemeName::OneHalfLight,
        "One Half Light",
        false,
    ),
    (
        two_face::theme::EmbeddedThemeName::SolarizedLight,
        "Solarized Light",
        false,
    ),
    (
        two_face::theme::EmbeddedThemeName::Base16OceanLight,
        "Base16 Ocean Light",
        false,
    ),
    (
        two_face::theme::EmbeddedThemeName::CatppuccinFrappe,
        "Catppuccin Frappe",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::CatppuccinMacchiato,
        "Catppuccin Macchiato",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::CatppuccinMocha,
        "Catppuccin Mocha",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::ColdarkDark,
        "Coldark Dark",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::DarkNeon,
        "Dark Neon",
        true,
    ),
    (two_face::theme::EmbeddedThemeName::Dracula, "Dracula", true),
    (
        two_face::theme::EmbeddedThemeName::GruvboxDark,
        "gruvbox Dark",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::MonokaiExtended,
        "Monokai Extended",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::MonokaiExtendedBright,
        "Monokai Extended Bright",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::MonokaiExtendedOrigin,
        "Monokai Extended Origin",
        true,
    ),
    (two_face::theme::EmbeddedThemeName::Nord, "Nord", true),
    (
        two_face::theme::EmbeddedThemeName::OneHalfDark,
        "One Half Dark",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::SolarizedDark,
        "Solarized Dark",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::SublimeSnazzy,
        "Sublime Snazzy",
        true,
    ),
    (two_face::theme::EmbeddedThemeName::TwoDark, "TwoDark", true),
    (
        two_face::theme::EmbeddedThemeName::Base16EightiesDark,
        "Base16 Eighties Dark",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::Base16MochaDark,
        "Base16 Mocha Dark",
        true,
    ),
    (
        two_face::theme::EmbeddedThemeName::Base16OceanDark,
        "Base16 Ocean Dark",
        true,
    ),
    (two_face::theme::EmbeddedThemeName::Zenburn, "Zenburn", true),
];

pub const DEFAULT_LIGHT_THEME: &str = "github";
pub const DEFAULT_DARK_THEME: &str = "oneHalfDark";

/// Stable setting value for a theme: the enum variant's camelCase spelling
/// (e.g. `Base16OceanLight` -> `base16OceanLight`).
const fn camel_key(name: two_face::theme::EmbeddedThemeName) -> &'static str {
    match name {
        two_face::theme::EmbeddedThemeName::CatppuccinLatte => "catppuccinLatte",
        two_face::theme::EmbeddedThemeName::ColdarkCold => "coldarkCold",
        two_face::theme::EmbeddedThemeName::Github => "github",
        two_face::theme::EmbeddedThemeName::GruvboxLight => "gruvboxLight",
        two_face::theme::EmbeddedThemeName::InspiredGithub => "inspiredGithub",
        two_face::theme::EmbeddedThemeName::MonokaiExtendedLight => "monokaiExtendedLight",
        two_face::theme::EmbeddedThemeName::OneHalfLight => "oneHalfLight",
        two_face::theme::EmbeddedThemeName::SolarizedLight => "solarizedLight",
        two_face::theme::EmbeddedThemeName::Base16OceanLight => "base16OceanLight",
        two_face::theme::EmbeddedThemeName::CatppuccinFrappe => "catppuccinFrappe",
        two_face::theme::EmbeddedThemeName::CatppuccinMacchiato => "catppuccinMacchiato",
        two_face::theme::EmbeddedThemeName::CatppuccinMocha => "catppuccinMocha",
        two_face::theme::EmbeddedThemeName::ColdarkDark => "coldarkDark",
        two_face::theme::EmbeddedThemeName::DarkNeon => "darkNeon",
        two_face::theme::EmbeddedThemeName::Dracula => "dracula",
        two_face::theme::EmbeddedThemeName::GruvboxDark => "gruvboxDark",
        two_face::theme::EmbeddedThemeName::MonokaiExtended => "monokaiExtended",
        two_face::theme::EmbeddedThemeName::MonokaiExtendedBright => "monokaiExtendedBright",
        two_face::theme::EmbeddedThemeName::MonokaiExtendedOrigin => "monokaiExtendedOrigin",
        two_face::theme::EmbeddedThemeName::Nord => "nord",
        two_face::theme::EmbeddedThemeName::OneHalfDark => "oneHalfDark",
        two_face::theme::EmbeddedThemeName::SolarizedDark => "solarizedDark",
        two_face::theme::EmbeddedThemeName::SublimeSnazzy => "sublimeSnazzy",
        two_face::theme::EmbeddedThemeName::TwoDark => "twoDark",
        two_face::theme::EmbeddedThemeName::Base16EightiesDark => "base16EightiesDark",
        two_face::theme::EmbeddedThemeName::Base16MochaDark => "base16MochaDark",
        two_face::theme::EmbeddedThemeName::Base16OceanDark => "base16OceanDark",
        two_face::theme::EmbeddedThemeName::Zenburn => "zenburn",
        _ => "github",
    }
}

fn parse_theme_key(key: &str) -> Option<two_face::theme::EmbeddedThemeName> {
    EMBEDDED_THEMES
        .iter()
        .find(|(name, _, _)| camel_key(*name) == key)
        .map(|(name, _, _)| *name)
}

pub fn theme_options() -> impl Iterator<Item = (&'static str, &'static str, bool)> {
    EMBEDDED_THEMES
        .iter()
        .map(|(name, label, dark)| (camel_key(*name), *label, *dark))
}

struct Themes {
    light: Theme,
    dark: Theme,
}

static ACTIVE_THEMES: std::sync::Mutex<[two_face::theme::EmbeddedThemeName; 2]> =
    std::sync::Mutex::new([
        two_face::theme::EmbeddedThemeName::Github,
        two_face::theme::EmbeddedThemeName::OneHalfDark,
    ]);

// Each snapshot is intentionally leaked: SideStream holds
// Highlighter<'static> borrows into it, so freeing a swapped-out pair would
// dangle them. A snapshot is a few KB, and swaps only happen on a settings
// change.
static THEMES: std::sync::RwLock<Option<&'static Themes>> = std::sync::RwLock::new(None);

fn build_themes(
    light: two_face::theme::EmbeddedThemeName,
    dark: two_face::theme::EmbeddedThemeName,
) -> &'static Themes {
    let set = two_face::theme::extra();
    Box::leak(Box::new(Themes {
        light: set.get(light).clone(),
        dark: set.get(dark).clone(),
    }))
}

/// Swaps the (light, dark) theme pair used for all later highlighting.
/// Sections already streamed keep their old colors until they are re-opened.
pub fn set_theme_pair(light: &str, dark: &str) {
    let light = parse_theme_key(light).unwrap_or(two_face::theme::EmbeddedThemeName::Github);
    let dark = parse_theme_key(dark).unwrap_or(two_face::theme::EmbeddedThemeName::OneHalfDark);
    {
        let active = unwrap_poisoned(&ACTIVE_THEMES);
        if active[0] == light && active[1] == dark {
            return;
        }
    }
    {
        let mut active = unwrap_poisoned(&ACTIVE_THEMES);
        *active = [light, dark];
    }
    let mut themes = unwrap_poisoned_rw(&THEMES);
    *themes = Some(build_themes(light, dark));
}

fn themes() -> &'static Themes {
    {
        let loaded = unwrap_poisoned_rw(&THEMES);
        if let Some(snapshot) = *loaded {
            return snapshot;
        }
    }
    let mut loaded = unwrap_poisoned_rw(&THEMES);
    if let Some(snapshot) = *loaded {
        return snapshot;
    }
    let active = *unwrap_poisoned(&ACTIVE_THEMES);
    let snapshot = build_themes(active[0], active[1]);
    *loaded = Some(snapshot);
    snapshot
}

fn unwrap_poisoned<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn unwrap_poisoned_rw<T>(rwlock: &std::sync::RwLock<T>) -> std::sync::RwLockWriteGuard<'_, T> {
    rwlock
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Loads the syntax set and themes so the first user-facing diff never pays
/// lazy-init cost. Safe to call multiple times; runs once.
pub fn warm_up() {
    let _ = syntax_set();
    let _ = themes();
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    /// The theme pair is process-global: tests that swap it hold this lock for
    /// their whole body so parallel tests cannot swap the pair mid-assertion.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn every_embedded_theme_has_a_stable_key() {
        for (name, label, dark) in theme_options() {
            assert!(parse_theme_key(name).is_some(), "{name} unresolvable");
            assert!(!label.is_empty());
            let _ = dark;
        }
    }

    #[test]
    fn unknown_keys_fall_back_to_defaults() {
        let _serial = serial();
        set_theme_pair("", "nope");
        let themes = themes();
        let expected = two_face::theme::extra();
        assert_eq!(
            themes.light.settings.foreground,
            expected
                .get(two_face::theme::EmbeddedThemeName::Github)
                .settings
                .foreground
        );
        assert_eq!(
            themes.dark.settings.foreground,
            expected
                .get(two_face::theme::EmbeddedThemeName::OneHalfDark)
                .settings
                .foreground
        );
        set_theme_pair(DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME);
    }

    #[test]
    fn swapping_themes_changes_resolved_colors() {
        let _serial = serial();
        set_theme_pair(DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME);
        let before = themes().light.settings.foreground;
        set_theme_pair("catppuccinLatte", "dracula");
        let after = themes().light.settings.foreground;
        assert_ne!(before, after);
        set_theme_pair(DEFAULT_LIGHT_THEME, DEFAULT_DARK_THEME);
    }
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

fn hex(color: syntect::highlighting::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

fn packed(color: syntect::highlighting::Color) -> u32 {
    (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)
}

fn font_bits(style: FontStyle) -> u8 {
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
struct StyleInterner {
    map: HashMap<(u32, u32, u8), u16>,
    entries: Vec<WireStyle>,
    flushed: usize,
}

impl StyleInterner {
    fn intern(&mut self, key: (u32, u32, u8), build: impl FnOnce() -> WireStyle) -> u16 {
        if let Some(&id) = self.map.get(&key) {
            return id;
        }
        let id = self.entries.len() as u16 + 1;
        self.entries.push(build());
        self.map.insert(key, id);
        id
    }

    /// Styles appended since the previous call, for delta transport.
    fn take_new(&mut self) -> Vec<WireStyle> {
        let new = self.entries[self.flushed..].to_vec();
        self.flushed = self.entries.len();
        new
    }
}

/// One side's resumable parser: grammar state and position persist across
/// calls so a section can be highlighted chunk-by-chunk, each call paying
/// only the lines that call needs (the parse-state checkpoint pattern from
/// syntect's docs).
struct SideStream {
    lines: Vec<String>,
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
    fn new(text: &str, syntax: &'static SyntaxReference) -> SideStream {
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
    fn advance(
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
            let id = interner.intern(key, || WireStyle {
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
        warm_up();
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

    /// Fills `spans` on every highlightable row in `rows` using full-file
    /// grammar context. Rows that cannot be highlighted safely stay plain.
    pub fn highlight_rows(&mut self, rows: &mut [DiffRow]) {
        let mut need_old: HashSet<u32> = HashSet::new();
        let mut need_new: HashSet<u32> = HashSet::new();
        for row in rows.iter() {
            if !is_highlightable(&row.kind)
                || row.content.is_empty()
                || row.content == NO_NEWLINE_MARKER
            {
                continue;
            }
            if let Some(lineno) = row.old_lineno {
                need_old.insert(lineno);
            }
            if let Some(lineno) = row.new_lineno {
                need_new.insert(lineno);
            }
        }

        // Old side fills gaps first; new side wins on context rows, matching
        // byte-identical content on both sides.
        if !need_old.is_empty() {
            if let Some(side) = self.old.as_mut() {
                for (lineno, spans) in side.advance(max_of(&need_old), &need_old, &mut self.styles)
                {
                    fill_rows(rows, lineno, &spans, false);
                }
            }
        }
        if !need_new.is_empty() {
            if let Some(side) = self.new.as_mut() {
                for (lineno, spans) in side.advance(max_of(&need_new), &need_new, &mut self.styles)
                {
                    fill_rows(rows, lineno, &spans, true);
                }
            }
        }
    }

    /// Style entries appended since the previous call, for delta transport.
    pub fn take_new_styles(&mut self) -> Vec<WireStyle> {
        self.styles.take_new()
    }
}

fn max_of(wanted: &HashSet<u32>) -> u32 {
    wanted.iter().copied().max().unwrap_or(0)
}

fn fill_rows(rows: &mut [DiffRow], lineno: u32, spans: &[u32], overwrite: bool) {
    for row in rows.iter_mut() {
        if !is_highlightable(&row.kind)
            || row.content.is_empty()
            || row.content == NO_NEWLINE_MARKER
            || (row.spans.is_some() && !overwrite)
        {
            continue;
        }
        let matches = match (overwrite, row.new_lineno, row.old_lineno) {
            (true, Some(new), _) => new == lineno,
            (false, _, Some(old)) => old == lineno,
            _ => false,
        };
        if !matches {
            continue;
        }
        let limit = row.content.len() as u32;
        if spans.chunks_exact(3).all(|t| t[0] + t[1] <= limit) {
            row.spans = Some(spans.to_vec());
        }
    }
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
fn syntax_for_language(language: &str) -> Option<&'static SyntaxReference> {
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

/// Highlighted spans for one standalone snippet (a markdown code fence),
/// aligned with `text.split('\n')`: empty entries render plain.
pub struct SnippetHighlight {
    pub spans_by_line: Vec<Vec<u32>>,
    pub styles: Vec<WireStyle>,
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

/// Highlights every context/addition/deletion row in one call using
/// full-file grammar context from both blobs. Convenience wrapper over
/// [`SectionHighlighter`] for callers that buffer a whole section.
pub fn attach(rows: &mut [DiffRow], path: &str, old: &[u8], new: &[u8]) -> Vec<WireStyle> {
    let Some(mut highlighter) = SectionHighlighter::new(path, old, new) else {
        return Vec::new();
    };
    highlighter.highlight_rows(rows);
    highlighter.take_new_styles()
}

fn is_highlightable(kind: &DiffRowKind) -> bool {
    matches!(
        kind,
        DiffRowKind::Context | DiffRowKind::Addition | DiffRowKind::Deletion
    )
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
