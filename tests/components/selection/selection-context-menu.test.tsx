// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SelectionContextMenu } from "@/components/selection/selection-context-menu";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const selectionMocks = vi.hoisted(() => ({
    getSelectionText: vi.fn((): string => ""),
    clearSelection: vi.fn(),
    selectElementContents: vi.fn(),
    selectionInSurface: vi.fn((): boolean => false),
}));

const toastErrorMock = vi.hoisted(() => vi.fn());

vi.mock("@/lib/selection/text-selection", () => selectionMocks);
vi.mock("@/lib/toast-error", () => ({ toastError: toastErrorMock }));

type ViewRoot = ReturnType<typeof createRoot>;
type MountedView = {
    root: ViewRoot;
    surface: HTMLDivElement;
};

let mountedViews: Array<{ root: ViewRoot; container: HTMLDivElement }> = [];
let writeText: ReturnType<typeof vi.fn>;

function mount(): MountedView {
    const container = document.createElement("div");
    const surface = document.createElement("div");
    surface.className = "ui-selectable";
    const target = document.createElement("span");
    target.textContent = "diff line body";
    surface.appendChild(target);
    document.body.append(container, surface);

    const root = createRoot(container);
    act(() => {
        root.render(<SelectionContextMenu />);
    });

    mountedViews.push({ root, container });
    return { root, surface };
}

async function flush(ms = 20): Promise<void> {
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, ms));
    });
}

async function rightClick(element: Element): Promise<void> {
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("contextmenu", {
                bubbles: true,
                cancelable: true,
                clientX: 17,
                clientY: 31,
            })
        );
        await flush();
    });
}

async function clickTarget(element: Element): Promise<void> {
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await flush();
    });
}

function rightClickTarget(view: MountedView): Promise<void> {
    const target = view.surface.querySelector("span");
    if (!target) throw new Error("missing selectable span");
    return rightClick(target);
}

function rightClickField(field: HTMLInputElement | HTMLTextAreaElement) {
    return rightClick(field);
}

function findPopup(): HTMLElement | null {
    return document.querySelector<HTMLElement>('[data-slot="menu-popup"]');
}

function menuItems(): HTMLElement[] {
    return Array.from(
        document.querySelectorAll<HTMLElement>('[data-slot="menu-item"]')
    );
}

function itemByLabel(label: string): HTMLElement {
    const item = menuItems().find((node) =>
        node.textContent?.startsWith(label)
    );
    if (!item) throw new Error(`menu item "${label}" not found`);
    return item;
}

describe("SelectionContextMenu", () => {
    beforeEach(() => {
        mountedViews = [];
        writeText = vi.fn(() => Promise.resolve());
        Object.defineProperty(navigator, "clipboard", {
            configurable: true,
            value: { writeText },
            writable: true,
        });
    });

    afterEach(() => {
        for (const view of mountedViews.splice(0)) {
            act(() => view.root.unmount());
            view.container.remove();
        }
        for (const surface of document.querySelectorAll(".ui-selectable")) {
            surface.remove();
        }
        selectionMocks.getSelectionText.mockReset();
        selectionMocks.clearSelection.mockReset();
        selectionMocks.selectElementContents.mockReset();
        selectionMocks.selectionInSurface.mockReset();
        selectionMocks.getSelectionText.mockReturnValue("");
        selectionMocks.selectionInSurface.mockReturnValue(false);
        vi.clearAllMocks();
    });

    it("opens on right click inside a selectable region with its three actions", async () => {
        const view = mount();

        await rightClickTarget(view);

        const popup = findPopup();
        expect(popup).not.toBeNull();
        expect(menuItems()).toHaveLength(3);
        expect(itemByLabel("Copy")).not.toBeNull();
        expect(itemByLabel("Deselect")).not.toBeNull();
        expect(itemByLabel("Select all")).not.toBeNull();
    });

    it("prevents the native menu inside selectable regions but leaves other regions untouched", async () => {
        const view = mount();
        const preventDefault = vi.spyOn(MouseEvent.prototype, "preventDefault");

        await rightClickTarget(view);
        expect(preventDefault).toHaveBeenCalled();
        expect(findPopup()).not.toBeNull();
        preventDefault.mockRestore();

        const outside = document.createElement("p");
        outside.textContent = "plain chrome";
        document.body.appendChild(outside);

        const probe = vi.spyOn(MouseEvent.prototype, "preventDefault");
        await rightClick(outside);
        // No suppression and no snapshot update: the native webview menu wins.
        expect(probe).not.toHaveBeenCalled();
        probe.mockRestore();
        outside.remove();
    });

    it.each(["input", "textarea"] as const)(
        "acts on the %s value instead of DOM selection",
        async (tagName) => {
            const view = mount();
            const field = document.createElement(tagName);
            field.value = "editable text";
            view.surface.appendChild(field);
            field.focus();
            field.setSelectionRange(2, 6);
            const event = new MouseEvent("contextmenu", {
                bubbles: true,
                cancelable: true,
            });

            await act(async () => {
                field.dispatchEvent(event);
                await flush();
            });

            expect(event.defaultPrevented).toBe(true);
            expect(findPopup()).not.toBeNull();

            const [copy] = menuItems();
            expect(copy?.getAttribute("data-disabled")).toBeNull();
            await clickTarget(copy!);

            expect(writeText).toHaveBeenCalledWith("itab");
            await flush(0);
            expect(findPopup()).toBeNull();

            await rightClickField(field);
            await act(async () => {
                itemByLabel("Select all").dispatchEvent(
                    new MouseEvent("click", {
                        bubbles: true,
                        cancelable: true,
                    })
                );
                // The webview drops the selection when it restores focus on
                // menu close; the deferred pass must re-apply it.
                field.setSelectionRange(field.value.length, field.value.length);
                await flush();
            });
            expect(field.selectionStart).toBe(0);
            expect(field.selectionEnd).toBe(field.value.length);
            expect(document.activeElement).toBe(field);
        }
    );

    it("disables copy and deselect when there is no selection", async () => {
        const view = mount();
        await rightClickTarget(view);

        const items = menuItems();

        expect(items[0]?.getAttribute("data-disabled")).not.toBeNull();
        expect(items[1]?.getAttribute("data-disabled")).not.toBeNull();
        expect(items[2]?.getAttribute("data-disabled")).toBeNull();
    });

    it("enables copy and deselect when the selection intersects the surface", async () => {
        selectionMocks.selectionInSurface.mockReturnValue(true);
        selectionMocks.getSelectionText.mockReturnValue("picked text");
        const view = mount();
        await rightClickTarget(view);

        const [copy, deselect] = menuItems();

        expect(copy?.getAttribute("data-disabled")).toBeNull();
        expect(deselect?.getAttribute("data-disabled")).toBeNull();
    });

    it("copies the snapshotted selection text and closes the menu", async () => {
        selectionMocks.selectionInSurface.mockReturnValue(true);
        selectionMocks.getSelectionText.mockReturnValue("picked text");
        const view = mount();
        await rightClickTarget(view);

        await clickTarget(itemByLabel("Copy"));

        expect(writeText).toHaveBeenCalledWith("picked text");
        await flush(0);
        expect(findPopup()).toBeNull();
    });

    it("toasts when copying fails", async () => {
        selectionMocks.selectionInSurface.mockReturnValue(true);
        writeText.mockImplementation(() =>
            Promise.reject(new Error("blocked"))
        );
        const view = mount();
        await rightClickTarget(view);

        await clickTarget(itemByLabel("Copy"));

        await flush(0);
        expect(toastErrorMock).toHaveBeenCalledTimes(1);
        expect(toastErrorMock.mock.calls[0]?.[0]).toBe("Could not copy");
    });

    it("deselects through the shared helper", async () => {
        selectionMocks.selectionInSurface.mockReturnValue(true);
        const view = mount();
        await rightClickTarget(view);

        await clickTarget(itemByLabel("Deselect"));

        expect(selectionMocks.clearSelection).toHaveBeenCalledTimes(1);
    });

    it("selects all contents scoped to the right-clicked surface", async () => {
        const view = mount();
        await rightClickTarget(view);

        await clickTarget(itemByLabel("Select all"));

        expect(selectionMocks.selectElementContents).toHaveBeenCalledTimes(1);
        expect(selectionMocks.selectElementContents).toHaveBeenCalledWith(
            view.surface
        );
    });
});
