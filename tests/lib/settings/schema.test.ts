import { describe, expect, it } from "vitest";

import type { SettingsSchema } from "@/lib/backend/protocol";
import {
    findSettingDefinition,
    getSettingOptions,
} from "@/lib/settings/schema";

const schema: SettingsSchema = {
    tabs: [
        {
            id: "general",
            label: "General",
            sections: [
                {
                    id: "appearance",
                    title: "Appearance",
                    settings: [
                        {
                            key: "diffViewMode",
                            label: "Diff view",
                            kind: "select",
                            options: [
                                { value: "unified", label: "Unified" },
                                { value: "split", label: "Split" },
                            ],
                            defaultValue: "unified",
                            hidden: false,
                        },
                        {
                            key: "commitFileListOpen",
                            label: "Commit file list open",
                            kind: "boolean",
                            defaultValue: false,
                            hidden: false,
                        },
                    ],
                },
            ],
        },
    ],
};

describe("settings schema lookups", () => {
    it("finds a definition by key across tabs and sections", () => {
        expect(findSettingDefinition(schema, "diffViewMode")?.label).toBe(
            "Diff view"
        );
        expect(findSettingDefinition(schema, "missing")).toBeUndefined();
    });

    it("returns a select's options and nothing for other kinds or keys", () => {
        expect(getSettingOptions(schema, "diffViewMode")).toEqual([
            { value: "unified", label: "Unified" },
            { value: "split", label: "Split" },
        ]);
        expect(getSettingOptions(schema, "commitFileListOpen")).toEqual([]);
        expect(getSettingOptions(schema, "missing")).toEqual([]);
    });
});
