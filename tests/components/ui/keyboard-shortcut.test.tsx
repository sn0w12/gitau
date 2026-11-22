// @vitest-environment jsdom

import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, describe, expect, it } from "vitest";

import { KeyboardShortcut, MenuShortcutContext } from "@/components/ui/kbd";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function render(ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(ui);
    });
    return {
        container,
        unmount() {
            act(() => root.unmount());
            container.remove();
        },
    };
}

afterEach(() => {
    document.body.innerHTML = "";
});

describe("KeyboardShortcut", () => {
    it("highlights the activation key inside a menu", () => {
        const view = render(
            <MenuShortcutContext.Provider value>
                <KeyboardShortcut shortcut="Mod+Shift+E" />
            </MenuShortcutContext.Provider>
        );

        const activations =
            view.container.querySelectorAll("[data-activation]");
        expect(activations).toHaveLength(1);
        expect(activations[0].textContent).toBe("E");
        view.unmount();
    });

    it("leaves shortcuts outside menus unhighlighted", () => {
        const view = render(<KeyboardShortcut shortcut="Mod+Shift+E" />);

        expect(view.container.querySelector("[data-activation]")).toBeNull();
        view.unmount();
    });

    it("does not highlight non-character keys", () => {
        const view = render(
            <MenuShortcutContext.Provider value>
                <KeyboardShortcut shortcut="Mod+Enter" />
            </MenuShortcutContext.Provider>
        );

        expect(view.container.querySelector("[data-activation]")).toBeNull();
        view.unmount();
    });

    it("highlights setting shortcuts from their resolved chord", () => {
        seedSettingsForTests({ openInEditorShortcut: "Mod+Shift+E" });
        const view = render(
            <MenuShortcutContext.Provider value>
                <KeyboardShortcut shortcut="openInEditorShortcut" />
            </MenuShortcutContext.Provider>
        );

        const activations =
            view.container.querySelectorAll("[data-activation]");
        expect(activations).toHaveLength(1);
        expect(activations[0].textContent).toBe("E");
        view.unmount();
    });
});
