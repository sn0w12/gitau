import { beforeEach, describe, expect, it } from "vitest";

import type {
    SettingValue,
    SettingsSchema,
    ValuesSnapshot,
} from "@/lib/backend/protocol";
import {
    configureSettingsSaver,
    flushSettingsSaves,
    getSetting,
    initializeSettings,
    resetSettingsForTests,
    setSetting,
    settingsStore,
} from "@/stores/settings-store";

function fixtureSchema(): SettingsSchema {
    return {
        tabs: [
            {
                id: "general",
                label: "General",
                sections: [
                    {
                        id: "s1",
                        title: "Section",
                        settings: [
                            {
                                key: "closeRepoWithLastTab",
                                label: "Close repo",
                                kind: "boolean",
                                defaultValue: true,
                                hidden: false,
                            },
                            {
                                key: "historyPageSize",
                                label: "Page size",
                                kind: "number",
                                min: 10,
                                max: 200,
                                step: 10,
                                defaultValue: 50,
                                hidden: false,
                            },
                            {
                                key: "pinnedRepos",
                                label: "Pinned",
                                kind: "stringList",
                                defaultValue: [],
                                hidden: true,
                            },
                        ],
                    },
                ],
            },
        ],
    };
}

function fixtureValues(): Record<string, SettingValue> {
    return {
        closeRepoWithLastTab: true,
        historyPageSize: 50,
        pinnedRepos: [] as string[],
    };
}

describe("settings store", () => {
    beforeEach(() => {
        resetSettingsForTests();
    });

    it("throws when read or written before initialization", () => {
        expect(() => getSetting("closeRepoWithLastTab")).toThrow();
        expect(() => setSetting("closeRepoWithLastTab", false)).toThrow();
    });

    it("initializes from a backend snapshot", () => {
        initializeSettings({
            schema: fixtureSchema(),
            values: fixtureValues(),
            savedKeys: ["theme"],
        });

        expect(settingsStore.state.ready).toBe(true);
        expect(getSetting("closeRepoWithLastTab")).toBe(true);
        expect(getSetting("historyPageSize")).toBe(50);
        expect(getSetting("pinnedRepos")).toEqual([]);
        expect(settingsStore.state.savedKeys.has("theme")).toBe(true);
    });

    it("applies writes synchronously", () => {
        initializeSettings({
            schema: fixtureSchema(),
            values: fixtureValues(),
            savedKeys: [],
        });

        setSetting("closeRepoWithLastTab", false);

        // Same tick: any reader already sees the new value.
        expect(getSetting("closeRepoWithLastTab")).toBe(false);
    });

    it("coalesces rapid changes into one latest-wins save", async () => {
        const calls: Array<[string, unknown]> = [];
        let release!: () => void;
        const gate = new Promise<void>((resolve) => {
            release = resolve;
        });

        configureSettingsSaver(async (key, value) => {
            calls.push([key, value]);
            await gate;
            return { values: {}, savedKeys: [] };
        });
        initializeSettings({
            schema: fixtureSchema(),
            values: fixtureValues(),
            savedKeys: [],
        });

        setSetting("historyPageSize", 10);
        setSetting("historyPageSize", 20);
        release();
        await flushSettingsSaves();

        expect(calls).toEqual([["historyPageSize", 20]]);
        expect(settingsStore.state.pendingKeys.size).toBe(0);
    });

    it("applies the canonical snapshot after a successful save", async () => {
        configureSettingsSaver(async (key, value) => ({
            values: {
                ...fixtureValues(),
                [key]: value,
                serverOnlyKey: "from-backend",
            },
            savedKeys: [key],
        }));
        initializeSettings({
            schema: fixtureSchema(),
            values: fixtureValues(),
            savedKeys: [],
        });

        setSetting("historyPageSize", 80);
        await flushSettingsSaves();

        expect(getSetting("historyPageSize")).toBe(80);
        expect(settingsStore.state.savedKeys.has("historyPageSize")).toBe(true);
        expect(settingsStore.state.lastError).toBeNull();
    });

    it("keeps the local value and records the error when saving fails", async () => {
        configureSettingsSaver(async () => {
            throw new Error("backend unavailable");
        });
        initializeSettings({
            schema: fixtureSchema(),
            values: fixtureValues(),
            savedKeys: [],
        });

        setSetting("closeRepoWithLastTab", false);
        await flushSettingsSaves();

        expect(getSetting("closeRepoWithLastTab")).toBe(false);
        expect(settingsStore.state.lastError).toBe("backend unavailable");
        expect(settingsStore.state.pendingKeys.size).toBe(0);
    });

    it("marks keys pending while a save is in flight", async () => {
        let release!: () => void;
        const gate = new Promise<ValuesSnapshot>((resolve) => {
            release = () => resolve({ values: {}, savedKeys: [] });
        });
        configureSettingsSaver(async () => gate);
        initializeSettings({
            schema: fixtureSchema(),
            values: fixtureValues(),
            savedKeys: [],
        });

        setSetting("historyPageSize", 30);
        expect(settingsStore.state.pendingKeys.has("historyPageSize")).toBe(
            true
        );

        release();
        await flushSettingsSaves();
        expect(settingsStore.state.pendingKeys.size).toBe(0);
    });
});
