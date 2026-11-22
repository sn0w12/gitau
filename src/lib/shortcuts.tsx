import { formatForDisplay } from "@tanstack/hotkeys";
import {
    ArrowBigUp,
    ArrowRightToLine,
    ChevronUp,
    Command,
    CornerDownLeft,
    OptionIcon,
} from "lucide-react";

import type { RepoView } from "@/lib/routing/repo-search";
import type { ShortcutSettingKey } from "@/lib/settings/settings.generated";
import { isMac } from "@/lib/utils";

export const REPO_VIEW_SHORTCUTS: Record<RepoView, ShortcutSettingKey> = {
    overview: "switchOverviewViewShortcut",
    graph: "switchGraphViewShortcut",
    issues: "switchIssuesViewShortcut",
    pulls: "switchPullsViewShortcut",
    readme: "switchReadmeViewShortcut",
};

// Lucide icons share a 24-unit viewBox but not an ink width, so each trim
// compensates its own empty side padding to keep the visible gaps equal.
export const SHORTCUT_ICON_CLASS =
    "[&_svg:not([class*='size-'])]:size-[1.4cap] [&_svg]:shrink-0 [&_.lucide-chevron-up]:-mx-[0.25cap] [&_.lucide-arrow-big-up]:-mx-[0.1cap] [&_.lucide-command]:-mx-[0.05cap] [&_.lucide-option]:-mx-[0.05cap] [&_.lucide-corner-down-left]:-mx-[0.1cap] [&_.lucide-arrow-right-to-line]:-mx-[0.05cap]";

export function displayToken(token: string): React.ReactNode {
    switch (token.toLowerCase()) {
        case "mod":
            return isMac() ? <Command /> : <ChevronUp />;
        case "ctrl":
            return <ChevronUp />;
        case "shift":
            return <ArrowBigUp />;
        case "alt":
            return <OptionIcon className="size-[1.2cap]" />;
        case "meta":
            return <Command />;
        case "enter":
            return <CornerDownLeft />;
        case "escape":
            return "Esc";
        case "tab":
            return <ArrowRightToLine />;
        default:
            return token.startsWith("[") && token.endsWith("]")
                ? formatForDisplay(token)
                : token.length === 1
                  ? token.toUpperCase()
                  : token;
    }
}

const MODIFIER_TOKENS = new Set([
    "mod",
    "ctrl",
    "control",
    "alt",
    "option",
    "shift",
    "meta",
    "cmd",
    "command",
    "super",
    "win",
]);

export function isActivationToken(token: string): boolean {
    if (!token || MODIFIER_TOKENS.has(token.toLowerCase())) return false;
    const characters = Array.from(token);
    return characters.length === 1 && characters[0] !== " ";
}

export function splitChord(chord: string): string[] {
    return chord
        .split("+")
        .map((part) => part.trim())
        .filter(Boolean);
}
