import type { RepoView } from "@/lib/routing/repo-search";
import type { ShortcutSettingKey } from "@/lib/settings/settings.generated";
import { isMac } from "@/lib/utils";

export const REPO_VIEW_SHORTCUTS: Record<RepoView, ShortcutSettingKey> = {
    overview: "switchOverviewViewShortcut",
    graph: "switchGraphViewShortcut",
    issues: "switchIssuesViewShortcut",
    pulls: "switchPullsViewShortcut",
};

export function displayToken(token: string): string {
    switch (token.toLowerCase()) {
        case "mod":
            return isMac() ? "⌘" : "Ctrl";
        case "ctrl":
            return isMac() ? "⌃" : "Ctrl";
        case "shift":
            return "⇧";
        case "alt":
            return isMac() ? "⌥" : "Alt";
        case "meta":
            return "⌘";
        default:
            return token.length === 1 ? token.toUpperCase() : token;
    }
}

export function splitChord(chord: string): string[] {
    return chord
        .split("+")
        .map((part) => part.trim())
        .filter(Boolean);
}
