//! The single place where application settings are declared. Adding a
//! setting here automatically exposes it to persistence, validation, the
//! typed frontend hooks, and (unless hidden) the settings UI.

use super::model::{
    SelectOption, SettingDefinition, SettingType, SettingValue, SettingsSchema, SettingsSection,
    SettingsTab,
};

const SYNTAX_THEME_LIGHT_KEY: &str = "syntaxThemeLight";
const SYNTAX_THEME_DARK_KEY: &str = "syntaxThemeDark";

/// The select options mirror the highlighter's embedded theme registry so
/// schema validation and theme resolution can never disagree.
fn syntax_theme_options(dark: bool) -> Vec<SelectOption> {
    git_backend::engines::gix::highlight::theme_options()
        .filter(|(_, _, theme_dark)| *theme_dark == dark)
        .map(|(value, label, _)| SelectOption::new(value, label))
        .collect()
}

fn syntax_theme_select(
    key: &'static str,
    label: &'static str,
    description: &'static str,
    default: &'static str,
    dark: bool,
) -> SettingDefinition {
    SettingDefinition::new(
        key,
        label,
        SettingType::Select {
            options: syntax_theme_options(dark),
        },
    )
    .description(description)
    .default_value(SettingValue::Str(default.to_owned()))
}

fn boolean(key: &'static str, label: &'static str) -> SettingDefinition {
    SettingDefinition::new(key, label, SettingType::Boolean)
}

fn select(
    key: &'static str,
    label: &'static str,
    options: &[(&'static str, &'static str)],
) -> SettingDefinition {
    SettingDefinition::new(
        key,
        label,
        SettingType::Select {
            options: options
                .iter()
                .map(|(value, option_label)| SelectOption::new(value, option_label))
                .collect(),
        },
    )
}

fn number(
    key: &'static str,
    label: &'static str,
    min: f64,
    max: f64,
    step: f64,
) -> SettingDefinition {
    SettingDefinition::new(
        key,
        label,
        SettingType::Number {
            min: Some(min),
            max: Some(max),
            step: Some(step),
        },
    )
}

fn string(key: &'static str, label: &'static str) -> SettingDefinition {
    SettingDefinition::new(key, label, SettingType::String)
}

fn multiline_string(key: &'static str, label: &'static str) -> SettingDefinition {
    SettingDefinition::new(key, label, SettingType::MultilineString)
}

fn shortcut(key: &'static str, label: &'static str) -> SettingDefinition {
    SettingDefinition::new(key, label, SettingType::Shortcut)
}

fn string_list(key: &'static str, label: &'static str) -> SettingDefinition {
    SettingDefinition::new(key, label, SettingType::StringList)
}

/// The builtin settings tree. Hidden entries live alongside visible ones so
/// they persist and validate identically.
pub fn builtin() -> SettingsSchema {
    let schema = SettingsSchema {
        tabs: vec![
            SettingsTab {
                id: "general",
                label: "General",
                sections: vec![
                    SettingsSection {
                        id: "repositories",
                        title: "Repositories",
                        settings: vec![
                            boolean("closeRepoWithLastTab", "Close repository with last tab")
                                .description(
                                    "Close the repository when its last open tab is closed.",
                                )
                                .default_value(SettingValue::Bool(true)),
                            string_list("pinnedRepos", "Pinned repositories")
                                .description("Repository paths pinned in the sidebar.")
                                .default_value(SettingValue::StringList(Vec::new()))
                                .hidden(),
                            boolean("commitFileListOpen", "Commit file list open")
                                .description("If the commit file list is open.")
                                .default_value(SettingValue::Bool(false))
                                .hidden(),
                            string("lastRepositoryDirectory", "Last repository directory")
                                .description(
                                    "Parent directory remembered across repository creation and cloning.",
                                )
                                .default_value(SettingValue::Str(String::new()))
                                .hidden(),
                            number("repoSidebarSize", "Repository sidebar size", 20.0, 35.0, 1.0)
                                .description(
                                    "Sidebar width in percent shared by all repository tabs.",
                                )
                                .default_value(SettingValue::Number(25.0))
                                .hidden(),
                            boolean("onboardingComplete", "Onboarding complete")
                                .description("Indicates if the onboarding process is complete.")
                                .default_value(SettingValue::Bool(false))
                                .hidden(),
                        ],
                    },
                    SettingsSection {
                        id: "appearance",
                        title: "Appearance",
                        settings: vec![
                            select(
                                "theme",
                                "Theme",
                                &[("system", "System"), ("light", "Light"), ("dark", "Dark")],
                            )
                            .description("Color scheme used across the app.")
                            .default_value(SettingValue::Str("system".to_owned())),
                            select(
                                "diffViewMode",
                                "Diff view",
                                &[("unified", "Unified"), ("split", "Split")],
                            )
                            .description("Layout used by the diff viewer.")
                            .default_value(SettingValue::Str("unified".to_owned())),
                            select(
                                "imageDiffViewMode",
                                "Image diff view",
                                &[("sideBySide", "Side by side"), ("swipe", "Swipe")],
                            )
                            .description("Layout used by image diffs.")
                            .default_value(SettingValue::Str("sideBySide".to_owned())),
                            syntax_theme_select(
                                SYNTAX_THEME_LIGHT_KEY,
                                "Syntax theme (light mode)",
                                "Highlighting colors used when the app theme is light.",
                                git_backend::engines::gix::highlight::DEFAULT_LIGHT_THEME,
                                false,
                            ),
                            syntax_theme_select(
                                SYNTAX_THEME_DARK_KEY,
                                "Syntax theme (dark mode)",
                                "Highlighting colors used when the app theme is dark.",
                                git_backend::engines::gix::highlight::DEFAULT_DARK_THEME,
                                true,
                            ),
                        ],
                    },
                    SettingsSection {
                        id: "editor",
                        title: "Editor",
                        settings: vec![string("editorCommand", "Editor command")
                            .description(
                                "Command used by \"Open in editor\", e.g. `code --reuse-window` or the full path to an editor executable.",
                            )
                            .default_value(SettingValue::Str(String::new()))],
                    },
                    SettingsSection {
                        id: "git",
                        title: "Git",
                        settings: vec![
                            string("defaultBranchName", "Default branch name")
                                .description("Branch name used when initializing new repositories.")
                                .default_value(SettingValue::Str("main".to_owned())),
                            number("historyPageSize", "History page size", 10.0, 200.0, 10.0)
                                .description("Commits loaded per page in history views.")
                                .default_value(SettingValue::Number(50.0)),
                            multiline_string("globalGitignore", "Global gitignore")
                                .description("Ignore rules applied to every repository on this computer, merged with the git system excludes file. One pattern per line.")
                                .default_value(SettingValue::Str(String::new())),
                        ],
                    },
                ],
            },
            SettingsTab {
                id: "shortcuts",
                label: "Shortcuts",
                sections: vec![
                    SettingsSection {
                        id: "navigation",
                        title: "Navigation",
                        settings: vec![
                            shortcut("goToSettings", "Go to Settings")
                                .description("Opens the settings page.")
                                .default_value(SettingValue::Str("Mod+,".to_owned())),
                            shortcut("openCommand", "Open Command")
                                .description("Opens the command palette.")
                                .default_value(SettingValue::Str("Mod+K".to_owned())),
                            shortcut("openShortcutsHelp", "Open shortcut list")
                                .description("Toggles the keyboard shortcut list.")
                                .default_value(SettingValue::Str("F1".to_owned())),
                            shortcut("switchChangesPanelShortcut", "Switch to Changes panel")
                                .description("Shows the Changes panel in the sidebar.")
                                .default_value(SettingValue::Str("Alt+1".to_owned())),
                            shortcut("switchHistoryPanelShortcut", "Switch to History panel")
                                .description("Shows the History panel in the sidebar.")
                                .default_value(SettingValue::Str("Alt+2".to_owned())),
                            shortcut("toggleGraphViewShortcut", "Toggle commit graph view")
                                .description("Switches between Overview and Commit Graph.")
                                .default_value(SettingValue::Str("Mod+G".to_owned())),
                            shortcut("filterChangesShortcut", "Filter file changes")
                                .description("Focuses the file filter in the Changes panel.")
                                .default_value(SettingValue::Str("Mod+F".to_owned())),
                        ],
                    },
                    SettingsSection {
                        id: "repository",
                        title: "Repository",
                        settings: vec![
                            shortcut("stageAllShortcut", "Stage all changes")
                                .description("Stages every unstaged change.")
                                .default_value(SettingValue::Str("Mod+Shift+A".to_owned())),
                            shortcut("unstageAllShortcut", "Unstage all changes")
                                .description("Unstages every staged change.")
                                .default_value(SettingValue::Str("Mod+Shift+U".to_owned())),
                            shortcut("discardAllShortcut", "Discard all changes")
                                .description("Discards every unstaged change after confirmation.")
                                .default_value(SettingValue::Str("Mod+Shift+D".to_owned())),
                            shortcut("focusCommitSummaryShortcut", "Focus commit summary")
                                .description("Moves focus to the commit summary field.")
                                .default_value(SettingValue::Str("Mod+Shift+C".to_owned())),
                            shortcut("fetchShortcut", "Fetch")
                                .description("Fetches from the primary remote.")
                                .default_value(SettingValue::Str("Mod+Shift+F".to_owned())),
                            shortcut("pullShortcut", "Pull")
                                .description("Pulls from the primary remote.")
                                .default_value(SettingValue::Str("Mod+Shift+L".to_owned())),
                            shortcut("pushShortcut", "Push")
                                .description("Pushes to the primary remote.")
                                .default_value(SettingValue::Str("Mod+Shift+P".to_owned())),
                        ],
                    },
                    SettingsSection {
                        id: "tabs",
                        title: "Tabs",
                        settings: vec![
                            shortcut("newTabShortcut", "New tab")
                                .description("Open a new tab.")
                                .default_value(SettingValue::Str("Mod+T".to_owned())),
                            shortcut("closeTabShortcut", "Close tab")
                                .description("Close the active tab.")
                                .default_value(SettingValue::Str("Mod+W".to_owned())),
                            shortcut("reopenClosedTabShortcut", "Reopen closed tab")
                                .description("Reopen the most recently closed tab.")
                                .default_value(SettingValue::Str("Mod+Shift+T".to_owned())),
                            shortcut("nextTabShortcut", "Next tab")
                                .description("Switch to the next tab.")
                                .default_value(SettingValue::Str("Mod+Tab".to_owned())),
                            shortcut("previousTabShortcut", "Previous tab")
                                .description("Switch to the previous tab.")
                                .default_value(SettingValue::Str("Mod+Shift+Tab".to_owned())),
                        ],
                    },
                ],
            },
        ],
    };

    if let Err(reason) = schema.validate_invariants() {
        panic!("builtin settings schema invalid: {reason}");
    }
    schema
}
