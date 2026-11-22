// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from "vitest";

import { activateMenuShortcut } from "@/lib/menu-shortcuts";
import { seedSettingsForTests } from "@/stores/settings-store";

function createPopup(markup: string): HTMLElement {
    const popup = document.createElement("div");
    popup.setAttribute("data-slot", "menu-popup");
    popup.innerHTML = markup;
    document.body.append(popup);
    return popup;
}

function press(
    popup: HTMLElement,
    target: EventTarget,
    key: string,
    init: Partial<{
        repeat: boolean;
        isComposing: boolean;
        ctrlKey: boolean;
        altKey: boolean;
        metaKey: boolean;
    }> = {}
) {
    const preventDefault = vi.fn();
    const stopPropagation = vi.fn();
    const handled = activateMenuShortcut({
        key,
        target,
        currentTarget: popup,
        preventDefault,
        stopPropagation,
        ...init,
    });
    return { handled, preventDefault, stopPropagation };
}

afterEach(() => {
    document.body.innerHTML = "";
});

describe("activateMenuShortcut", () => {
    it("activates the item whose shortcut ends with the pressed key", () => {
        const popup = createPopup(
            '<div role="menuitem" id="item"><span data-shortcut="Mod+Shift+E"></span></div>'
        );
        const item = popup.querySelector("#item")!;
        const clicked = vi.fn();
        item.addEventListener("click", clicked);

        const result = press(
            popup,
            popup.querySelector("[data-shortcut]")!,
            "e"
        );

        expect(result.handled).toBe(true);
        expect(result.preventDefault).toHaveBeenCalledOnce();
        expect(result.stopPropagation).toHaveBeenCalledOnce();
        expect(clicked).toHaveBeenCalledOnce();
    });

    it("matches without regard to letter case", () => {
        const popup = createPopup(
            '<div role="menuitem" id="item"><span data-shortcut="Mod+Shift+E"></span></div>'
        );
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);

        press(popup, popup.querySelector("[data-shortcut]")!, "E");

        expect(clicked).toHaveBeenCalledOnce();
    });

    it("resolves shortcut setting keys", () => {
        seedSettingsForTests({ openInEditorShortcut: "Mod+Shift+E" });
        const popup = createPopup(
            '<div role="menuitem" id="item"><span data-shortcut="openInEditorShortcut"></span></div>'
        );
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);

        press(popup, popup.querySelector("[data-shortcut]")!, "e");

        expect(clicked).toHaveBeenCalledOnce();
    });

    it("ignores a key held with a modifier", () => {
        const popup = createPopup(
            '<div role="menuitem" id="item"><span data-shortcut="Mod+Shift+E"></span></div>'
        );
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);

        for (const init of [
            { ctrlKey: true },
            { altKey: true },
            { metaKey: true },
        ]) {
            const result = press(
                popup,
                popup.querySelector("[data-shortcut]")!,
                "e",
                init
            );
            expect(result.handled).toBe(false);
            expect(result.preventDefault).not.toHaveBeenCalled();
        }
        expect(clicked).not.toHaveBeenCalled();
    });

    it("ignores ambiguous keys", () => {
        const popup = createPopup(`
            <div role="menuitem" id="one"><span data-shortcut="Mod+E"></span></div>
            <div role="menuitem" id="two"><span data-shortcut="Mod+Shift+E"></span></div>
        `);
        const first = vi.fn();
        const second = vi.fn();
        popup.querySelector("#one")!.addEventListener("click", first);
        popup.querySelector("#two")!.addEventListener("click", second);

        const result = press(
            popup,
            popup.querySelector("#one [data-shortcut]")!,
            "e"
        );

        expect(result.handled).toBe(false);
        expect(first).not.toHaveBeenCalled();
        expect(second).not.toHaveBeenCalled();
    });

    it("counts an item with two matching badges once", () => {
        const popup = createPopup(
            '<div role="menuitem" id="item"><span data-shortcut="Mod+E"></span><span data-shortcut="Alt+E"></span></div>'
        );
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);

        press(popup, popup.querySelector("[data-shortcut]")!, "e");

        expect(clicked).toHaveBeenCalledOnce();
    });

    it("ignores disabled matches", () => {
        const popup = createPopup(
            '<div role="menuitem" data-disabled id="item"><span data-shortcut="Mod+E"></span></div>'
        );
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);

        const result = press(
            popup,
            popup.querySelector("[data-shortcut]")!,
            "e"
        );

        expect(result.handled).toBe(false);
        expect(clicked).not.toHaveBeenCalled();
    });

    it("ignores keys typed into an editable control", () => {
        const popup = createPopup(`
            <div role="menuitem" id="item"><span data-shortcut="Mod+E"></span></div>
            <input id="filter">
        `);
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);

        const result = press(popup, popup.querySelector("#filter")!, "e");

        expect(result.handled).toBe(false);
        expect(clicked).not.toHaveBeenCalled();
    });

    it("ignores keys aimed at another popup", () => {
        const popup = createPopup(
            '<div role="menuitem" id="item"><span data-shortcut="Mod+E"></span></div>'
        );
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);

        const result = press(popup, document.body, "e");

        expect(result.handled).toBe(false);
        expect(clicked).not.toHaveBeenCalled();
    });

    it("ignores non-character keys, space, and repeated events", () => {
        const popup = createPopup(
            '<div role="menuitem" id="item"><span data-shortcut="Mod+Enter"></span><span data-shortcut="Mod+Shift+E"></span></div>'
        );
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);
        const badge = popup.querySelector("[data-shortcut]")!;

        expect(press(popup, badge, "Enter").handled).toBe(false);
        expect(press(popup, badge, " ").handled).toBe(false);
        expect(press(popup, badge, "e", { repeat: true }).handled).toBe(false);
        expect(press(popup, badge, "e", { isComposing: true }).handled).toBe(
            false
        );
        expect(clicked).not.toHaveBeenCalled();
    });

    it("leaves unbound shortcuts inert", () => {
        seedSettingsForTests({ openInEditorShortcut: "" });
        const popup = createPopup(
            '<div role="menuitem" id="item"><span data-shortcut="openInEditorShortcut"></span></div>'
        );
        const clicked = vi.fn();
        popup.querySelector("#item")!.addEventListener("click", clicked);

        const result = press(
            popup,
            popup.querySelector("[data-shortcut]")!,
            "e"
        );

        expect(result.handled).toBe(false);
        expect(clicked).not.toHaveBeenCalled();
    });
});
