import { cleanup, fireEvent, render, screen } from "@testing-library/react";
// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { TabContext } from "@/contexts/tab-context";
import type { SettingsSchema } from "@/lib/backend/protocol";
import { SettingsPage } from "@/routes/settings-page";
import {
    configureSettingsSaver,
    getSetting,
    initializeSettings,
    resetSettingsForTests,
} from "@/stores/settings-store";

// jsdom has no ResizeObserver; the settings page constructs one to measure
// its animated tab panel during render.
if (typeof globalThis.ResizeObserver === "undefined") {
    (globalThis as Record<string, unknown>).ResizeObserver = class {
        observe(): void {}
        unobserve(): void {}
        disconnect(): void {}
    };
}

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
                            description: "Closes the repo.",
                            kind: "boolean",
                            defaultValue: true,
                            hidden: false,
                        },
                        {
                            key: "pinnedRepos",
                            label: "Pinned repositories",
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
                    ],
                },
                {
                    id: "shortcuts",
                    title: "Shortcuts",
                    settings: [
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

function initialize(): void {
    initializeSettings({
        schema,
        values: {
            closeRepoWithLastTab: true,
            pinnedRepos: [],
            editorCommand: "",
            newTabShortcut: "Mod+T",
            theme: "system",
        },
        savedKeys: [],
    });
}

function renderPage(): ReturnType<typeof render> {
    return render(
        <TabContext.Provider value="test-tab">
            <SettingsPage />
        </TabContext.Provider>
    );
}

describe("SettingsPage", () => {
    beforeEach(() => {
        resetSettingsForTests();
        configureSettingsSaver(null);
        initialize();
    });

    afterEach(() => {
        cleanup();
    });

    it("renders visible settings and excludes hidden ones", () => {
        const { container } = renderPage();

        expect(
            screen.getByText("Close repository with last tab")
        ).toBeDefined();
        expect(screen.queryByText("Pinned repositories")).toBeNull();
        // Hidden stringList never renders a control.
        expect(container.textContent).not.toContain("Pinned");
    });

    it("renders the editor command as a text input", () => {
        const { container } = renderPage();

        expect(screen.getByText("Editor command")).toBeDefined();
        const input = container.querySelector('input[data-slot="input"]');
        expect(input).not.toBeNull();
        expect((input as HTMLInputElement).value).toBe("");
    });

    it("maps each setting kind to its control", () => {
        const { container } = renderPage();

        // boolean -> switch (Base UI renders a span[role=switch])
        expect(container.querySelector('[role="switch"]')).not.toBeNull();

        // shortcut -> kbd chord tokens (Mod resolves to Ctrl off-macOS)
        expect(screen.getByText("Ctrl")).toBeDefined();
        expect(screen.getByText("T")).toBeDefined();
    });

    it("switches between schema tabs", () => {
        const { container } = renderPage();

        // Only the active panel is mounted; General starts active.
        expect(
            container.querySelector('[data-slot="select-trigger"]')
        ).toBeNull();

        fireEvent.click(screen.getByText("Appearance"));

        expect(
            container.querySelector('[data-slot="select-trigger"]')
        ).not.toBeNull();
        expect(screen.queryByText("Close repository with last tab")).toBeNull();
    });

    it("writes through the typed store when a boolean is toggled", () => {
        const { container } = renderPage();

        const toggle = container.querySelector('[role="switch"]');
        if (!toggle) throw new Error("switch not rendered");
        fireEvent.click(toggle);

        expect(getSetting("closeRepoWithLastTab")).toBe(false);
    });
});
