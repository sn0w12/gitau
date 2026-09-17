// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { createElement, forwardRef, useEffect } from "react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// The dialog swaps steps inside AnimatePresence mode="wait". Motion waits for
// the exit animation before mounting the next step, and jsdom never finishes
// Web Animations, so the next step would never appear. Render motion elements
// as plain tags and bypass the exit gate so tests assert step content.
vi.mock("motion/react", () => {
    const renderTag = (tag: string) =>
        forwardRef<HTMLElement, Record<string, unknown>>((props, ref) => {
            const { children } = props;
            return createElement(tag, { ref }, children as ReactNode);
        });
    const motion = new Proxy(
        {},
        {
            get: (_target, tag: string) => renderTag(tag),
        }
    );
    const AnimatePresence = ({ children }: { children: ReactNode }) => (
        <>{children}</>
    );
    return { motion, AnimatePresence, useReducedMotion: () => false };
});

import { OnboardingDialog } from "@/components/onboarding-dialog";
import {
    AppCommandProvider,
    useAppCommands,
} from "@/contexts/app-command-context";
import type { SettingsSchema } from "@/lib/backend/protocol";
import {
    configureSettingsSaver,
    getSetting,
    initializeSettings,
    resetSettingsForTests,
} from "@/stores/settings-store";

// jsdom has no ResizeObserver; Base UI dialog and field primitives build one
// while mounting.
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
                    id: "appearance",
                    title: "Appearance",
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
                {
                    id: "editor",
                    title: "Editor",
                    settings: [
                        {
                            key: "editorCommand",
                            label: "Editor command",
                            kind: "string",
                            defaultValue: "",
                            hidden: false,
                        },
                    ],
                },
                {
                    id: "repositories",
                    title: "Repositories",
                    settings: [
                        {
                            key: "onboardingComplete",
                            label: "Onboarding complete",
                            kind: "boolean",
                            defaultValue: false,
                            hidden: true,
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
            onboardingComplete: false,
            theme: "system",
            editorCommand: "",
        },
        savedKeys: [],
    });
}

function Harness({ onCommand }: { onCommand: (name: string) => void }) {
    const { register } = useAppCommands();

    useEffect(() => {
        const disposers = [
            register("addRepository", () => onCommand("addRepository")),
            register("cloneRepository", () => onCommand("cloneRepository")),
            register("newRepository", () => onCommand("newRepository")),
        ];
        return () => {
            for (const dispose of disposers) dispose();
        };
    }, [register, onCommand]);

    return <OnboardingDialog />;
}

function renderDialog(onCommand: (name: string) => void = () => {}) {
    return render(
        <AppCommandProvider>
            <Harness onCommand={onCommand} />
        </AppCommandProvider>
    );
}

function advance(times: number): void {
    for (let index = 0; index < times; index += 1) {
        fireEvent.click(screen.getByRole("button", { name: /Continue/ }));
    }
}

describe("OnboardingDialog", () => {
    beforeEach(() => {
        resetSettingsForTests();
        configureSettingsSaver(null);
        initialize();
    });

    afterEach(() => {
        cleanup();
    });

    it("opens on the theme step with a preview per option", () => {
        renderDialog();

        expect(screen.getByText("Pick a theme")).toBeDefined();
        expect(document.querySelectorAll("button[aria-pressed]").length).toBe(
            3
        );
        expect(screen.getByRole("button", { name: "Light" })).toBeDefined();
    });

    it("writes the chosen theme", () => {
        renderDialog();

        fireEvent.click(screen.getByRole("button", { name: "Dark" }));

        expect(getSetting("theme")).toBe("dark");
    });

    it("picks an editor preset", () => {
        renderDialog();
        advance(1);

        expect(screen.getByText("Choose your editor")).toBeDefined();
        fireEvent.click(screen.getByRole("button", { name: /VS Code/ }));

        expect(getSetting("editorCommand")).toBe("code");
        expect(
            (screen.getByLabelText("Editor command") as HTMLInputElement)
                .disabled
        ).toBe(true);
    });

    it("accepts a custom editor command", () => {
        renderDialog();
        advance(1);

        const input = screen.getByLabelText(
            "Editor command"
        ) as HTMLInputElement;
        expect(input.disabled).toBe(true);

        fireEvent.click(screen.getByRole("button", { name: /Custom/ }));

        expect(input.disabled).toBe(false);
        fireEvent.change(input, { target: { value: "nvim" } });

        expect(getSetting("editorCommand")).toBe("nvim");
    });

    it("runs a repository action from the last step", () => {
        const onCommand = vi.fn();
        renderDialog(onCommand);
        advance(2);

        expect(screen.getByText("Add a repository")).toBeDefined();
        fireEvent.click(
            screen.getByRole("button", { name: /Clone a repository/ })
        );

        expect(getSetting("onboardingComplete")).toBe(true);
        expect(onCommand).toHaveBeenCalledWith("cloneRepository");
    });

    it("skips setup from the first step", () => {
        renderDialog();

        fireEvent.click(screen.getByRole("button", { name: "Skip" }));

        expect(getSetting("onboardingComplete")).toBe(true);
    });
});
