use syntect::highlighting::Theme;

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

pub(crate) struct Themes {
    pub(crate) light: Theme,
    pub(crate) dark: Theme,
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

pub(crate) fn themes() -> &'static Themes {
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

#[cfg(test)]
mod tests {
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
