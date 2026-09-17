/**
 * GENERATED FILE - do not edit by hand.
 *
 * Source of truth: crates/tauri/src/settings/schema.rs
 * Regenerate with: cargo run -p gitau --bin export_settings
 * Freshness is verified by cargo test (settings::tests::generated_typescript_is_fresh).
 */

export interface SettingsValues {
    closeRepoWithLastTab: boolean;
    pinnedRepos: string[];
    commitFileListOpen: boolean;
    lastRepositoryDirectory: string;
    repoSidebarSize: number;
    onboardingComplete: boolean;
    theme: "system" | "light" | "dark";
    diffViewMode: "unified" | "split";
    imageDiffViewMode: "sideBySide" | "swipe";
    syntaxThemeLight: "catppuccinLatte" | "coldarkCold" | "github" | "gruvboxLight" | "inspiredGithub" | "monokaiExtendedLight" | "oneHalfLight" | "solarizedLight" | "base16OceanLight";
    syntaxThemeDark: "catppuccinFrappe" | "catppuccinMacchiato" | "catppuccinMocha" | "coldarkDark" | "darkNeon" | "dracula" | "gruvboxDark" | "monokaiExtended" | "monokaiExtendedBright" | "monokaiExtendedOrigin" | "nord" | "oneHalfDark" | "solarizedDark" | "sublimeSnazzy" | "twoDark" | "base16EightiesDark" | "base16MochaDark" | "base16OceanDark" | "zenburn";
    editorCommand: string;
    defaultBranchName: string;
    historyPageSize: number;
    globalGitignore: string;
    goToSettings: string;
    openCommand: string;
    openShortcutsHelp: string;
    switchChangesPanelShortcut: string;
    switchHistoryPanelShortcut: string;
    toggleGraphViewShortcut: string;
    filterChangesShortcut: string;
    stageAllShortcut: string;
    unstageAllShortcut: string;
    discardAllShortcut: string;
    focusCommitSummaryShortcut: string;
    fetchShortcut: string;
    pullShortcut: string;
    pushShortcut: string;
    newTabShortcut: string;
    closeTabShortcut: string;
    reopenClosedTabShortcut: string;
    nextTabShortcut: string;
    previousTabShortcut: string;
}

export type SettingKey = keyof SettingsValues;
export type ThemeMode = SettingsValues["theme"];

export const SHORTCUT_SETTING_KEYS = [
    "goToSettings",
    "openCommand",
    "openShortcutsHelp",
    "switchChangesPanelShortcut",
    "switchHistoryPanelShortcut",
    "toggleGraphViewShortcut",
    "filterChangesShortcut",
    "stageAllShortcut",
    "unstageAllShortcut",
    "discardAllShortcut",
    "focusCommitSummaryShortcut",
    "fetchShortcut",
    "pullShortcut",
    "pushShortcut",
    "newTabShortcut",
    "closeTabShortcut",
    "reopenClosedTabShortcut",
    "nextTabShortcut",
    "previousTabShortcut",
] as const;

export type ShortcutSettingKey = typeof SHORTCUT_SETTING_KEYS[number];
