// @vitest-environment jsdom
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it } from "vitest";

import { RepoViewSwitcher } from "@/components/repo/history/view-switcher";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function renderWith(ui: ReactElement) {
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

async function flush(): Promise<void> {
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
    });
}

async function click(element: Element): Promise<void> {
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 10));
    });
}

async function waitFor(predicate: () => boolean): Promise<boolean> {
    const deadline = Date.now() + 1_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    return predicate();
}

describe("RepoViewSwitcher", () => {
    it("shows the active view, switches on click, and disables placeholders", async () => {
        // Issues is a live view backed by the GitHub API; only pull
        // requests remain a disabled placeholder.
        const picked: string[] = [];
        const view = renderWith(
            <RepoViewSwitcher
                view="overview"
                onView={(next) => picked.push(next)}
            />
        );

        expect(view.container.textContent).toContain("Current View");
        expect(view.container.textContent).toContain("Overview");

        const trigger = view.container.querySelector(
            '[data-slot="menu-trigger"]'
        ) as HTMLElement;
        await click(trigger);
        const opened = await waitFor(
            () =>
                view.container.ownerDocument.querySelector(
                    '[data-slot="menu-popup"]'
                ) !== null
        );
        expect(opened).toBe(true);

        const popup = view.container.ownerDocument.querySelector(
            '[data-slot="menu-popup"]'
        ) as HTMLElement;
        expect(popup.textContent).toContain("Commit Graph");
        expect(popup.textContent).toContain("Issues");
        expect(popup.textContent).toContain("Pull requests");

        const items = [
            ...popup.querySelectorAll('[data-slot="menu-item"]'),
        ] as HTMLElement[];

        const graphItem = items.find((item) =>
            item.textContent?.includes("Commit Graph")
        );
        expect(graphItem).toBeDefined();
        await click(graphItem as HTMLElement);
        await flush();
        expect(picked).toEqual(["graph"]);

        // The menu closes after picking; reopen it for the next pick.
        await click(trigger);
        const reopened = await waitFor(
            () =>
                view.container.ownerDocument.querySelector(
                    '[data-slot="menu-popup"]'
                ) !== null
        );
        expect(reopened).toBe(true);
        const popupAgain = view.container.ownerDocument.querySelector(
            '[data-slot="menu-popup"]'
        ) as HTMLElement;
        const itemsAgain = [
            ...popupAgain.querySelectorAll('[data-slot="menu-item"]'),
        ] as HTMLElement[];

        const issueItem = itemsAgain.find((item) =>
            item.textContent?.includes("Issues")
        );
        expect(issueItem?.getAttribute("aria-disabled")).toBe(null);
        await click(issueItem as HTMLElement);
        await flush();
        expect(picked).toEqual(["graph", "issues"]);

        const prItem = itemsAgain.find((item) =>
            item.textContent?.includes("Pull requests")
        );
        expect(prItem?.getAttribute("aria-disabled")).toBe("true");
        view.unmount();
    });
});
