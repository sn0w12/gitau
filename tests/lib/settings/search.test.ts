import { describe, expect, it } from "vitest";

import type { SettingsSchema } from "@/lib/backend/protocol";
import { searchSettings } from "@/lib/settings/search";
import type { SettingsValues } from "@/lib/settings/settings.generated";

const schema: SettingsSchema = {
    tabs: [
        {
            id: "general",
            label: "General",
            sections: [
                {
                    id: "repositories",
                    title: "Repositories",
                    settings: [
                        {
                            key: "closeRepoWithLastTab",
                            label: "Close repository with last tab",
                            description:
                                "Close the repository when its last open tab is closed.",
                            kind: "boolean",
                            defaultValue: true,
                            hidden: false,
                        },
                        {
                            key: "pinnedRepos",
                            label: "Pinned repositories",
                            description:
                                "Repository paths pinned in the sidebar.",
                            kind: "stringList",
                            defaultValue: [],
                            hidden: true,
                        },
                    ],
                },
                {
                    id: "editor",
                    title: "Editor",
                    settings: [
                        {
                            key: "editorCommand",
                            label: "Editor command",
                            description: "Command used by Open in editor.",
                            kind: "string",
                            defaultValue: "",
                            hidden: false,
                        },
                        {
                            key: "newTabShortcut",
                            label: "New tab",
                            kind: "shortcut",
                            defaultValue: "Mod+T",
                            hidden: false,
                        },
                    ],
                },
            ],
        },
        {
            id: "appearance",
            label: "Appearance",
            sections: [
                {
                    id: "theme-section",
                    title: "Theme",
                    settings: [
                        {
                            key: "theme",
                            label: "Theme",
                            description: "Color theme for the app.",
                            kind: "select",
                            options: [
                                { value: "system", label: "System" },
                                { value: "light", label: "Light" },
                                { value: "dark", label: "Dark" },
                            ],
                            defaultValue: "system",
                            hidden: false,
                        },
                    ],
                },
            ],
        },
    ],
};

const values = {
    closeRepoWithLastTab: true,
    pinnedRepos: [],
    editorCommand: "code --reuse-window",
    newTabShortcut: "Mod+T",
    theme: "dark",
} as unknown as SettingsValues;

function matchKeys(
    query: string,
    overrides?: Partial<SettingsValues>
): string[] {
    return searchSettings(schema, { ...values, ...overrides }, query).flatMap(
        (group) => group.settings.map((setting) => setting.key)
    );
}

describe("searchSettings", () => {
    it("returns nothing for an empty query", () => {
        expect(matchKeys("")).toEqual([]);
        expect(matchKeys("   ")).toEqual([]);
    });

    it("matches labels case-insensitively", () => {
        expect(matchKeys("EDITOR")).toEqual(["editorCommand"]);
    });

    it("matches descriptions", () => {
        expect(matchKeys("open in editor")).toEqual(["editorCommand"]);
    });

    it("matches the active select option", () => {
        expect(matchKeys("dark")).toEqual(["theme"]);
        expect(matchKeys("system")).toEqual([]);
    });

    it("matches boolean state", () => {
        expect(matchKeys("enabled")).toEqual(["closeRepoWithLastTab"]);
        expect(matchKeys("disabled")).toEqual([]);
        expect(matchKeys("disabled", { closeRepoWithLastTab: false })).toEqual([
            "closeRepoWithLastTab",
        ]);
    });

    it("matches string and shortcut values", () => {
        expect(matchKeys("reuse-window")).toEqual(["editorCommand"]);
        expect(matchKeys("mod+t")).toEqual(["newTabShortcut"]);
        expect(matchKeys("ctrl+t")).toEqual(["newTabShortcut"]);
        expect(matchKeys("cmd t")).toEqual(["newTabShortcut"]);
    });

    it("requires every token across label and value", () => {
        expect(matchKeys("dark theme")).toEqual(["theme"]);
    });

    it("skips hidden settings and unrendered kinds", () => {
        expect(matchKeys("pinned")).toEqual([]);
    });

    it("keeps tab and section context on each group", () => {
        const [group] = searchSettings(schema, values, "theme");
        expect(group.tabLabel).toBe("Appearance");
        expect(group.section.title).toBe("Theme");
    });
});
