import {
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
// @vitest-environment jsdom
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ActiveTabHost } from "@/components/active-tab-host";
import { AppServicesProvider } from "@/components/providers/app-services-provider";
import { TitlebarHistory, TitlebarTabList } from "@/components/titlebar";
import {
    activateTab,
    appStore,
    createTabRecord,
    openTab,
} from "@/stores/app-store";
import { seedSettingsForTests } from "@/stores/settings-store";
import { resetRuntimesForTests } from "@/stores/tab-runtime";

// ActiveTabHost imports the route tree, which pulls in @dnd-kit; its dom
// package instantiates a ResizeObserver-based ResizeNotifier at import time.
const globalsInstalled = vi.hoisted(() => {
    const globalScope = globalThis as Record<string, unknown>;

    globalScope.ResizeObserver ??= class {
        observe(): void {}
        unobserve(): void {}
        disconnect(): void {}
    };

    globalScope.matchMedia ??= () =>
        ({
            matches: false,
            addEventListener: (): void => {},
            removeEventListener: (): void => {},
            addListener: (): void => {},
            removeListener: (): void => {},
            onchange: null,
            dispatchEvent: (): boolean => false,
        }) as unknown as MediaQueryList;
});
void globalsInstalled;

vi.mock("@/components/window-controls", () => ({
    WindowTitlebar: ({ children }: { children?: React.ReactNode }) => children,
}));

function Shell() {
    return (
        <StrictMode>
            <AppServicesProvider>
                <TitlebarHistory />
                <TitlebarTabList />
                <ActiveTabHost />
            </AppServicesProvider>
        </StrictMode>
    );
}

/**
 * Regression test: mounting a tab's context menu used to close its own tab
 * during render (`onClick={void closeTabFully(...)}` executed immediately),
 * which reverted every new tab back to a single tab and logged
 * "Cannot update a component while rendering a different component".
 */
describe("titlebar tab strip", () => {
    let errors: string[] = [];

    beforeEach(() => {
        resetRuntimesForTests();
        localStorage.clear();
        // The real bootstrap initializes settings before React renders;
        // shell components (shortcut badges) read them at render time.
        seedSettingsForTests({
            closeRepoWithLastTab: true,
            pinnedRepos: [],
            theme: "system",
            defaultBranchName: "main",
            historyPageSize: 50,
            newTabShortcut: "Mod+T",
            closeTabShortcut: "Mod+W",
            reopenClosedTabShortcut: "Mod+Shift+T",
            nextTabShortcut: "Ctrl+Tab",
            previousTabShortcut: "Ctrl+Shift+Tab",
        });
        errors = [];
        vi.spyOn(console, "error").mockImplementation((...args: unknown[]) => {
            errors.push(args.map(String).join(" "));
        });
    });

    afterEach(() => {
        cleanup();
        vi.restoreAllMocks();
    });

    it("appends a second tab when the new-tab button is clicked", async () => {
        render(<Shell />);

        await waitFor(() => expect(appStore.state.tabs).toHaveLength(1));

        fireEvent.click(screen.getByRole("button", { name: "New tab" }));

        await waitFor(() =>
            expect(appStore.state.tabs.map((tab) => tab.title)).toEqual([
                "Home",
                "Home",
            ])
        );

        await waitFor(() => expect(screen.getAllByRole("tab")).toHaveLength(2));

        expect(errors.join("\n")).not.toContain("Cannot update a component");
    });

    it("duplicates a tab from its context menu", async () => {
        render(<Shell />);

        await waitFor(() => expect(appStore.state.tabs).toHaveLength(1));

        const [firstTab] = screen.getAllByRole("tab");
        fireEvent.contextMenu(firstTab);
        fireEvent.click(
            await screen.findByRole("menuitem", { name: /duplicate/i })
        );

        await waitFor(() => expect(appStore.state.tabs).toHaveLength(2));

        const titles = appStore.state.tabs.map((tab) => tab.title);
        expect(titles[0]).toBe(titles[1]);
        expect(appStore.state.activeTabId).toBe(appStore.state.tabs[1].tabId);
        expect(errors.join("\n")).not.toContain("Cannot update a component");
    });

    it("closes other tabs from the context menu", async () => {
        render(<Shell />);

        await waitFor(() => expect(appStore.state.tabs).toHaveLength(1));
        const anchor = appStore.state.tabs[0];
        openTab(createTabRecord({ title: "Second" }));
        openTab(createTabRecord({ title: "Third" }));
        activateTab(anchor.tabId);

        const tabs = screen.getAllByRole("tab");
        fireEvent.contextMenu(tabs[0]);
        fireEvent.click(
            await screen.findByRole("menuitem", { name: /close others/i })
        );

        await waitFor(() => expect(appStore.state.tabs).toHaveLength(1));
        expect(appStore.state.tabs[0].tabId).toBe(anchor.tabId);
        expect(appStore.state.activeTabId).toBe(anchor.tabId);
        expect(errors.join("\n")).not.toContain("Cannot update a component");
    });

    it("disables close items when only one tab exists", async () => {
        render(<Shell />);

        await waitFor(() => expect(appStore.state.tabs).toHaveLength(1));

        fireEvent.contextMenu(screen.getAllByRole("tab")[0]);

        // Every close-flavored item's accessible name includes its
        // shortcut badge concatenated without whitespace ("CloseCtrlW"),
        // so match by prefix; all of them must be disabled with one tab.
        await waitFor(() => {
            expect(
                screen.getAllByRole("menuitem", { name: /^close/i }).length
            ).toBeGreaterThanOrEqual(3);
        });
        for (const item of screen.getAllByRole("menuitem", {
            name: /^close/i,
        })) {
            expect(item.getAttribute("aria-disabled")).not.toBe("false");
        }
    });
});
