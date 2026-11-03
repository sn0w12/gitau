use std::str::FromStr;

use syntect::highlighting::{
    Color, FontStyle, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings,
};
use two_face::theme::EmbeddedThemeName;

const GITAU_LIGHT_KEY: &str = "gitauLight";
const GITAU_DARK_KEY: &str = "gitauDark";

pub const DEFAULT_LIGHT_THEME: &str = GITAU_LIGHT_KEY;
pub const DEFAULT_DARK_THEME: &str = GITAU_DARK_KEY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeSource {
    Bundled(EmbeddedThemeName),
    GitauLight,
    GitauDark,
}

struct ThemeEntry {
    key: &'static str,
    label: &'static str,
    dark: bool,
    source: ThemeSource,
}

/// Themes built into the binary and exposed as settings. Entries must stay
/// sorted: light themes first, then dark ones, so each settings dropdown
/// reads in a sensible order.
const THEMES: &[ThemeEntry] = &[
    ThemeEntry {
        key: GITAU_LIGHT_KEY,
        label: "Gitau Light",
        dark: false,
        source: ThemeSource::GitauLight,
    },
    ThemeEntry {
        key: "catppuccinLatte",
        label: "Catppuccin Latte",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::CatppuccinLatte),
    },
    ThemeEntry {
        key: "coldarkCold",
        label: "Coldark Cold",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::ColdarkCold),
    },
    ThemeEntry {
        key: "github",
        label: "GitHub",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::Github),
    },
    ThemeEntry {
        key: "gruvboxLight",
        label: "gruvbox Light",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::GruvboxLight),
    },
    ThemeEntry {
        key: "inspiredGithub",
        label: "Inspired GitHub",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::InspiredGithub),
    },
    ThemeEntry {
        key: "monokaiExtendedLight",
        label: "Monokai Extended Light",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::MonokaiExtendedLight),
    },
    ThemeEntry {
        key: "oneHalfLight",
        label: "One Half Light",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::OneHalfLight),
    },
    ThemeEntry {
        key: "solarizedLight",
        label: "Solarized Light",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::SolarizedLight),
    },
    ThemeEntry {
        key: "base16OceanLight",
        label: "Base16 Ocean Light",
        dark: false,
        source: ThemeSource::Bundled(EmbeddedThemeName::Base16OceanLight),
    },
    ThemeEntry {
        key: GITAU_DARK_KEY,
        label: "Gitau Dark",
        dark: true,
        source: ThemeSource::GitauDark,
    },
    ThemeEntry {
        key: "catppuccinFrappe",
        label: "Catppuccin Frappe",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::CatppuccinFrappe),
    },
    ThemeEntry {
        key: "catppuccinMacchiato",
        label: "Catppuccin Macchiato",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::CatppuccinMacchiato),
    },
    ThemeEntry {
        key: "catppuccinMocha",
        label: "Catppuccin Mocha",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::CatppuccinMocha),
    },
    ThemeEntry {
        key: "coldarkDark",
        label: "Coldark Dark",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::ColdarkDark),
    },
    ThemeEntry {
        key: "darkNeon",
        label: "Dark Neon",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::DarkNeon),
    },
    ThemeEntry {
        key: "dracula",
        label: "Dracula",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::Dracula),
    },
    ThemeEntry {
        key: "gruvboxDark",
        label: "gruvbox Dark",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::GruvboxDark),
    },
    ThemeEntry {
        key: "monokaiExtended",
        label: "Monokai Extended",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::MonokaiExtended),
    },
    ThemeEntry {
        key: "monokaiExtendedBright",
        label: "Monokai Extended Bright",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::MonokaiExtendedBright),
    },
    ThemeEntry {
        key: "monokaiExtendedOrigin",
        label: "Monokai Extended Origin",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::MonokaiExtendedOrigin),
    },
    ThemeEntry {
        key: "nord",
        label: "Nord",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::Nord),
    },
    ThemeEntry {
        key: "oneHalfDark",
        label: "One Half Dark",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::OneHalfDark),
    },
    ThemeEntry {
        key: "solarizedDark",
        label: "Solarized Dark",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::SolarizedDark),
    },
    ThemeEntry {
        key: "sublimeSnazzy",
        label: "Sublime Snazzy",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::SublimeSnazzy),
    },
    ThemeEntry {
        key: "twoDark",
        label: "TwoDark",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::TwoDark),
    },
    ThemeEntry {
        key: "base16EightiesDark",
        label: "Base16 Eighties Dark",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::Base16EightiesDark),
    },
    ThemeEntry {
        key: "base16MochaDark",
        label: "Base16 Mocha Dark",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::Base16MochaDark),
    },
    ThemeEntry {
        key: "base16OceanDark",
        label: "Base16 Ocean Dark",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::Base16OceanDark),
    },
    ThemeEntry {
        key: "zenburn",
        label: "Zenburn",
        dark: true,
        source: ThemeSource::Bundled(EmbeddedThemeName::Zenburn),
    },
];

fn parse_theme_key(key: &str) -> Option<ThemeSource> {
    THEMES
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| entry.source)
}

pub fn theme_options() -> impl Iterator<Item = (&'static str, &'static str, bool)> {
    THEMES
        .iter()
        .map(|entry| (entry.key, entry.label, entry.dark))
}

pub(crate) struct Themes {
    pub(crate) light: Theme,
    pub(crate) dark: Theme,
}

static ACTIVE_THEMES: std::sync::Mutex<[ThemeSource; 2]> =
    std::sync::Mutex::new([ThemeSource::GitauLight, ThemeSource::GitauDark]);

// Each snapshot is intentionally leaked: SideStream holds
// Highlighter<'static> borrows into it, so freeing a swapped-out pair would
// dangle them. A snapshot is a few KB, and swaps only happen on a settings
// change.
static THEMES_ACTIVE: std::sync::RwLock<Option<&'static Themes>> = std::sync::RwLock::new(None);

fn resolve_theme(source: ThemeSource) -> Theme {
    match source {
        ThemeSource::Bundled(name) => two_face::theme::extra().get(name).clone(),
        ThemeSource::GitauLight => gitau_theme(false),
        ThemeSource::GitauDark => gitau_theme(true),
    }
}

fn build_themes(light: ThemeSource, dark: ThemeSource) -> &'static Themes {
    Box::leak(Box::new(Themes {
        light: resolve_theme(light),
        dark: resolve_theme(dark),
    }))
}

/// Swaps the (light, dark) theme pair used for all later highlighting.
/// Sections already streamed keep their old colors until they are re-opened.
pub fn set_theme_pair(light: &str, dark: &str) {
    let light = parse_theme_key(light).unwrap_or(ThemeSource::GitauLight);
    let dark = parse_theme_key(dark).unwrap_or(ThemeSource::GitauDark);
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
    let mut themes = unwrap_poisoned_rw(&THEMES_ACTIVE);
    *themes = Some(build_themes(light, dark));
}

pub(crate) fn themes() -> &'static Themes {
    {
        let loaded = unwrap_poisoned_rw(&THEMES_ACTIVE);
        if let Some(snapshot) = *loaded {
            return snapshot;
        }
    }
    let mut loaded = unwrap_poisoned_rw(&THEMES_ACTIVE);
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

struct Palette {
    foreground: &'static str,
    background: &'static str,
    comment: &'static str,
    keyword: &'static str,
    control: &'static str,
    function: &'static str,
    type_name: &'static str,
    tag: &'static str,
    attribute: &'static str,
    variable: &'static str,
    string: &'static str,
    number: &'static str,
    constant: &'static str,
    boolean: &'static str,
    operator: &'static str,
    punctuation: &'static str,
    regex: &'static str,
    special: &'static str,
    markup: &'static str,
    invalid: &'static str,
    inserted: &'static str,
    removed: &'static str,
    escape: &'static str,
}

const LIGHT: Palette = Palette {
    foreground: "#262626",
    background: "#ffffff",
    comment: "#737373",
    keyword: "#b23b1d",
    control: "#b23b1d",
    function: "#aa6000",
    type_name: "#1c719a",
    tag: "#386e99",
    attribute: "#1c719a",
    variable: "#262626",
    string: "#007145",
    number: "#724aab",
    constant: "#724aab",
    boolean: "#724aab",
    operator: "#a34e55",
    punctuation: "#696969",
    regex: "#00736f",
    special: "#a85e2c",
    markup: "#b6303e",
    invalid: "#bd1f3d",
    inserted: "#007145",
    removed: "#bd1f3d",
    escape: "#aa6000",
};

const DARK: Palette = Palette {
    foreground: "#f5f5f5",
    background: "#1b1b1b",
    comment: "#8a8a8a",
    keyword: "#4a9fd0",
    control: "#b2aaff",
    function: "#e1d195",
    type_name: "#5dd3af",
    tag: "#5ec2eb",
    attribute: "#9dd7f2",
    variable: "#9dd7f2",
    string: "#eb9e84",
    number: "#b0dabd",
    constant: "#8fe8ff",
    boolean: "#8fe8ff",
    operator: "#bebebe",
    punctuation: "#bebebe",
    regex: "#dd7577",
    special: "#5ec2eb",
    markup: "#5ec2eb",
    invalid: "#fb4c62",
    inserted: "#56c9a2",
    removed: "#fb4c62",
    escape: "#e1d195",
};

fn color(hex: &str) -> Color {
    let channel = |start: usize| {
        u8::from_str_radix(&hex[start..start + 2], 16).expect("hardcoded color channel")
    };
    Color {
        r: channel(1),
        g: channel(3),
        b: channel(5),
        a: 0xFF,
    }
}

fn item(scope: &str, style: StyleModifier) -> ThemeItem {
    ThemeItem {
        scope: ScopeSelectors::from_str(scope).expect("hardcoded scope selector"),
        style,
    }
}

fn rule(scope: &str, foreground: &str) -> ThemeItem {
    item(
        scope,
        StyleModifier {
            foreground: Some(color(foreground)),
            background: None,
            font_style: None,
        },
    )
}

fn styled(scope: &str, foreground: &str, font_style: FontStyle) -> ThemeItem {
    item(
        scope,
        StyleModifier {
            foreground: Some(color(foreground)),
            background: None,
            font_style: Some(font_style),
        },
    )
}

fn font_style(scope: &str, font_style: FontStyle) -> ThemeItem {
    item(
        scope,
        StyleModifier {
            foreground: None,
            background: None,
            font_style: Some(font_style),
        },
    )
}

fn gitau_theme(dark: bool) -> Theme {
    let palette = if dark { &DARK } else { &LIGHT };
    let scopes = vec![
        rule("comment, punctuation.definition.comment", palette.comment),
        rule("keyword", palette.keyword),
        rule("keyword.control, keyword.import", palette.control),
        rule("keyword.operator", palette.operator),
        rule("keyword.operator.type.annotation", palette.punctuation),
        rule("storage", palette.keyword),
        rule("storage.modifier.lifetime", palette.type_name),
        rule("variable", palette.variable),
        rule("entity.name.function", palette.function),
        rule("support.function", palette.function),
        rule("variable.function", palette.function),
        rule("entity.name.type", palette.type_name),
        rule("support.type", palette.type_name),
        rule("entity.name.class", palette.type_name),
        rule("entity.other.inherited-class", palette.type_name),
        rule(
            "entity.name.struct, entity.name.enum, entity.name.impl, entity.name.trait, entity.name.interface, entity.name.namespace, entity.name.module, entity.name.table",
            palette.type_name,
        ),
        rule("support.class", palette.type_name),
        rule("entity.name.tag", palette.tag),
        rule("entity.name.tag support.class.component", palette.type_name),
        rule("support.class.component", palette.type_name),
        rule("entity.other.attribute-name", palette.attribute),
        rule("entity.other.attribute-name.class.css", palette.tag),
        rule(
            "entity.other.pseudo-class, entity.other.pseudo-element",
            palette.tag,
        ),
        rule(
            "variable.other.property, variable.object.property",
            palette.attribute,
        ),
        rule("support.variable.property", palette.attribute),
        rule("support.variable", palette.variable),
        rule("variable.annotation", palette.attribute),
        rule("support.type.property-name", palette.attribute),
        rule("support.type.custom-property", palette.attribute),
        rule("variable.language", palette.special),
        rule("string", palette.string),
        rule("string.regexp", palette.regex),
        rule("string.special", palette.regex),
        rule("constant.character.escape", palette.escape),
        rule("constant.numeric", palette.number),
        rule("keyword.other.unit", palette.number),
        rule("constant.language", palette.boolean),
        rule("constant.boolean", palette.boolean),
        rule("constant.other", palette.constant),
        rule("support.constant", palette.constant),
        rule("invalid", palette.invalid),
        rule("punctuation", palette.punctuation),
        rule("meta.brace", palette.punctuation),
        rule("punctuation.definition.string", palette.string),
        rule("punctuation.definition.raw", palette.string),
        rule("punctuation.definition.comment", palette.comment),
        rule("meta.mapping.key string", palette.attribute),
        rule(
            "meta.mapping.key punctuation.definition.string",
            palette.attribute,
        ),
        rule(
            "meta.link.inline.description, string.other.link",
            palette.attribute,
        ),
        styled(
            "markup.heading, entity.name.section",
            palette.markup,
            FontStyle::BOLD,
        ),
        rule("punctuation.definition.heading", palette.punctuation),
        styled("markup.bold", palette.markup, FontStyle::BOLD),
        rule("punctuation.definition.bold", palette.punctuation),
        font_style("markup.italic", FontStyle::ITALIC),
        rule("punctuation.definition.italic", palette.punctuation),
        rule("markup.raw", palette.string),
        rule("markup.quote", palette.comment),
        rule("markup.inserted", palette.inserted),
        rule("markup.deleted", palette.removed),
        rule("markup.changed", palette.escape),
        styled(
            "markup.link, markup.underline.link",
            palette.keyword,
            FontStyle::UNDERLINE,
        ),
        rule("punctuation.definition.link", palette.punctuation),
        rule("punctuation.definition.list_item", palette.punctuation),
    ];
    Theme {
        name: Some(if dark { "Gitau Dark" } else { "Gitau Light" }.to_owned()),
        author: Some("gitau".to_owned()),
        settings: ThemeSettings {
            foreground: Some(color(palette.foreground)),
            background: Some(color(palette.background)),
            ..ThemeSettings::default()
        },
        scopes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use syntect::highlighting::Highlighter;
    use syntect::parsing::ScopeStack;

    /// The theme pair is process-global: tests that swap it hold this lock for
    /// their whole body so parallel tests cannot swap the pair mid-assertion.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn style(theme: &Theme, scopes: &str) -> syntect::highlighting::Style {
        let stack = ScopeStack::from_str(scopes).expect("scope stack");
        Highlighter::new(theme).style_for_stack(stack.as_slice())
    }

    #[test]
    fn every_theme_has_a_stable_key() {
        for (key, label, dark) in theme_options() {
            assert!(parse_theme_key(key).is_some(), "{key} unresolvable");
            assert!(!label.is_empty());
            let _ = dark;
        }
    }

    #[test]
    fn defaults_resolve_to_the_gitau_pair() {
        assert_eq!(
            parse_theme_key(DEFAULT_LIGHT_THEME),
            Some(ThemeSource::GitauLight)
        );
        assert_eq!(
            parse_theme_key(DEFAULT_DARK_THEME),
            Some(ThemeSource::GitauDark)
        );
    }

    #[test]
    fn gitau_scopes_resolve_to_the_designed_roles() {
        let light = gitau_theme(false);
        let dark = gitau_theme(true);
        let cases = [
            (
                "source.rust meta.function.rust entity.name.function.rust",
                LIGHT.function,
                DARK.function,
            ),
            (
                "source.rust meta.annotation.rust meta.annotation.parameters.rust meta.group.rust meta.function-call.rust variable.function.rust",
                LIGHT.function,
                DARK.function,
            ),
            (
                "source.rust meta.function.rust storage.type.function.rust",
                LIGHT.keyword,
                DARK.keyword,
            ),
            (
                "source.rust meta.function.rust meta.block.rust keyword.control.rust",
                LIGHT.control,
                DARK.control,
            ),
            (
                "source.rust meta.function.rust meta.block.rust variable.language.rust",
                LIGHT.special,
                DARK.special,
            ),
            (
                "source.rust meta.function.rust meta.function.parameters.rust variable.parameter.rust",
                LIGHT.variable,
                DARK.variable,
            ),
            (
                "source.rust meta.struct.rust entity.name.struct.rust",
                LIGHT.type_name,
                DARK.type_name,
            ),
            (
                "source.rust meta.annotation.rust variable.annotation.rust",
                LIGHT.attribute,
                DARK.attribute,
            ),
            (
                "source.rust meta.function.rust meta.block.rust comment.line.double-slash.rust",
                LIGHT.comment,
                DARK.comment,
            ),
            (
                "source.tsx meta.function.tsx meta.block.tsx meta.tag.tsx entity.name.tag.tsx support.class.component.tsx",
                LIGHT.type_name,
                DARK.type_name,
            ),
            (
                "source.tsx meta.function.tsx meta.block.tsx meta.tag.tsx meta.tag.attributes.tsx entity.other.attribute-name.tsx",
                LIGHT.attribute,
                DARK.attribute,
            ),
            (
                "source.tsx meta.function.tsx meta.block.tsx meta.var.expr.tsx string.quoted.double.tsx",
                LIGHT.string,
                DARK.string,
            ),
            (
                "source.tsx meta.function.tsx meta.block.tsx meta.var.expr.tsx variable.other.property.tsx",
                LIGHT.attribute,
                DARK.attribute,
            ),
            (
                "source.tsx meta.function.tsx meta.block.tsx meta.var.expr.tsx constant.numeric.decimal.tsx",
                LIGHT.number,
                DARK.number,
            ),
            (
                "source.json meta.mapping.key.json string.quoted.double.json",
                LIGHT.attribute,
                DARK.attribute,
            ),
            (
                "source.json meta.mapping.value.json string.quoted.double.json",
                LIGHT.string,
                DARK.string,
            ),
            (
                "source.json meta.mapping.value.json constant.language.json",
                LIGHT.boolean,
                DARK.boolean,
            ),
            (
                "source.css meta.selector.css entity.other.attribute-name.class.css",
                LIGHT.tag,
                DARK.tag,
            ),
            (
                "source.css meta.selector.css entity.other.pseudo-class.css",
                LIGHT.tag,
                DARK.tag,
            ),
            (
                "source.css meta.property-list.css meta.property-name.css support.type.property-name.css",
                LIGHT.attribute,
                DARK.attribute,
            ),
            (
                "source.sql constant.boolean.sql",
                LIGHT.boolean,
                DARK.boolean,
            ),
            (
                "text.html.markdown meta.block-level.markdown markup.heading.1.markdown entity.name.section.markdown",
                LIGHT.markup,
                DARK.markup,
            ),
            (
                "text.html.markdown meta.paragraph.markdown markup.raw.inline.markdown",
                LIGHT.string,
                DARK.string,
            ),
            (
                "text.html.markdown meta.paragraph.markdown meta.link.inline.description.markdown",
                LIGHT.attribute,
                DARK.attribute,
            ),
        ];
        for (scopes, light_hex, dark_hex) in cases {
            assert_eq!(
                style(&light, scopes).foreground,
                color(light_hex),
                "light {scopes}"
            );
            assert_eq!(
                style(&dark, scopes).foreground,
                color(dark_hex),
                "dark {scopes}"
            );
        }

        let heading = style(
            &light,
            "text.html.markdown meta.block-level.markdown markup.heading.1.markdown",
        );
        assert!(heading.font_style.contains(FontStyle::BOLD));

        let marker = style(
            &light,
            "text.html.markdown meta.block-level.markdown markup.heading.1.markdown punctuation.definition.heading.begin.markdown",
        );
        assert_eq!(marker.foreground, color(LIGHT.punctuation));
    }

    #[test]
    fn unknown_keys_fall_back_to_defaults() {
        let _serial = serial();
        set_theme_pair("", "nope");
        let themes = themes();
        assert_eq!(
            themes.light.settings.foreground,
            Some(color(LIGHT.foreground))
        );
        assert_eq!(
            themes.dark.settings.foreground,
            Some(color(DARK.foreground))
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
