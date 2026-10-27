// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { DiffSectionBar } from "@/components/diff/diff-section-bar";
import type { SettingsSchema } from "@/lib/backend/protocol";
import {
    initializeSettings,
    resetSettingsForTests,
    seedSettingsForTests,
} from "@/stores/settings-store";

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
                            key: "imageDiffViewMode",
                            label: "Image diff view",
                            kind: "select",
                            options: [
                                { value: "sideBySide", label: "Side by side" },
                                { value: "swipe", label: "Swipe" },
                                { value: "difference", label: "Difference" },
                            ],
                            defaultValue: "sideBySide",
                            hidden: false,
                        },
                    ],
                },
            ],
        },
    ],
};

beforeEach(() => {
    resetSettingsForTests();
    initializeSettings({
        schema,
        values: { diffViewMode: "unified", imageDiffViewMode: "sideBySide" },
        savedKeys: [],
    });
});

afterEach(() => {
    cleanup();
    resetSettingsForTests();
});

function menuLabels(): string[] {
    return Array.from(
        document.querySelectorAll('[data-slot="menu-radio-item"]')
    ).map((item) => item.textContent?.trim() ?? "");
}

describe("DiffSectionBar view menu", () => {
    it("lists diff view options from the setting schema", async () => {
        render(
            <DiffSectionBar
                path="src/a.txt"
                kind="modified"
                mode="unified"
                onModeChange={() => {}}
                dispatch={() => {}}
            />
        );

        fireEvent.click(
            screen.getByRole("button", { name: "Diff view settings" })
        );
        expect(await screen.findByText("Unified")).toBeDefined();
        expect(menuLabels()).toEqual(["Unified", "Split"]);
    });

    it("lists image diff options from the setting schema", async () => {
        render(
            <DiffSectionBar
                path="images/logo.png"
                kind="modified"
                mode="unified"
                onModeChange={() => {}}
                imageMode="sideBySide"
                onImageModeChange={() => {}}
                dispatch={() => {}}
            />
        );

        fireEvent.click(
            screen.getByRole("button", { name: "Diff view settings" })
        );
        expect(await screen.findByText("Difference")).toBeDefined();
        expect(menuLabels()).toEqual(["Side by side", "Swipe", "Difference"]);
    });

    it("shows the path when not viewing a commit", () => {
        seedSettingsForTests({
            commitFileListOpen: true,
            diffViewMode: "unified",
            imageDiffViewMode: "sideBySide",
        });
        render(
            <DiffSectionBar
                path="src/a.txt"
                kind="modified"
                mode="unified"
                onModeChange={() => {}}
                dispatch={() => {}}
            />
        );

        const filename = screen.getByText("a.txt");
        expect(filename.closest('[aria-hidden="false"]')).not.toBeNull();
    });

    it("keeps the commit view before the message arrives", () => {
        seedSettingsForTests({
            commitFileListOpen: true,
            diffViewMode: "unified",
            imageDiffViewMode: "sideBySide",
        });
        render(
            <DiffSectionBar
                path="src/a.txt"
                kind="modified"
                isCommitView
                mode="unified"
                onModeChange={() => {}}
                dispatch={() => {}}
            />
        );

        expect(
            screen.getByText("a.txt").closest('[aria-hidden="false"]')
        ).toBeNull();
    });

    it("shows the commit subject in place of the path while the file list is open", () => {
        seedSettingsForTests({
            commitFileListOpen: true,
            diffViewMode: "unified",
            imageDiffViewMode: "sideBySide",
        });
        render(
            <DiffSectionBar
                path="src/a.txt"
                kind="modified"
                isCommitView
                commitMessage="Fix the section bar"
                mode="unified"
                onModeChange={() => {}}
                dispatch={() => {}}
            />
        );

        const subject = screen.getByText("Fix the section bar");
        expect(subject.closest('[aria-hidden="false"]')).not.toBeNull();
        expect(
            screen.getByText("a.txt").closest('[aria-hidden="false"]')
        ).toBeNull();
    });
});
