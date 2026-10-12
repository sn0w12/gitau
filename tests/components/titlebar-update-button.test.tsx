// @vitest-environment jsdom
import {
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TitlebarHistory } from "@/components/titlebar";
import {
    openUpdateDialog,
    updateDialogOpenStore,
    updaterController,
} from "@/lib/updates/update-manager";

// Tooltips instantiate ResizeObserver-backed positioning on mount.
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

describe("titlebar update button", () => {
    beforeEach(() => {
        // Start from a pristine singleton for every test.
        updaterController.store.setState(() => ({ status: "idle" }));
        updateDialogOpenStore.setState(() => false);
    });

    afterEach(() => {
        cleanup();
    });

    it("is hidden while no update is available", () => {
        render(<TitlebarHistory />);
        expect(queryUpdateButton()).toBeNull();
    });

    it("appears once an update is detected", () => {
        updaterController.store.setState(() => ({
            status: "available",
            info: { version: "0.2.0", currentVersion: "0.1.0" },
        }));
        render(<TitlebarHistory />);
        expect(queryUpdateButton()).not.toBeNull();
    });

    it("opens the update dialog on click", async () => {
        updaterController.store.setState(() => ({
            status: "available",
            info: { version: "0.2.0", currentVersion: "0.1.0" },
        }));
        render(<TitlebarHistory />);

        fireEvent.click(screen.getByRole("button", { name: "Install Update" }));

        await waitFor(() => expect(updateDialogOpenStore.state).toBe(true));
    });

    it("stays hidden while the dialog is merely open", () => {
        openUpdateDialog();
        render(<TitlebarHistory />);
        expect(queryUpdateButton()).toBeNull();
    });
});

function queryUpdateButton(): HTMLElement | null {
    return screen.queryByRole("button", { name: "Install Update" });
}
