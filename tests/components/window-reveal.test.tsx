// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { WindowReveal } from "@/components/window-reveal";

const show = vi.hoisted(() => vi.fn(() => Promise.resolve()));

vi.mock("@tauri-apps/api/window", () => ({
    getCurrentWindow: () => ({ show }),
}));

// Mirrors the flag Tauri's webview injection sets before page scripts run;
// without it isTauri() reports false and there is nothing to reveal.
function claimTauriEnvironment(): void {
    (globalThis as { isTauri?: boolean }).isTauri = true;
}

describe("WindowReveal", () => {
    beforeEach(() => {
        show.mockClear();
        claimTauriEnvironment();
    });

    afterEach(() => {
        delete (globalThis as { isTauri?: boolean }).isTauri;
        cleanup();
    });

    it("renders nothing and reveals the window after commit", () => {
        const { container } = render(<WindowReveal />);

        expect(container.innerHTML).toBe("");
        expect(show).toHaveBeenCalledTimes(1);
    });

    it("does not reveal outside a Tauri webview", () => {
        delete (globalThis as { isTauri?: boolean }).isTauri;

        render(<WindowReveal />);

        expect(show).not.toHaveBeenCalled();
    });

    it("swallows a rejected reveal", async () => {
        show.mockRejectedValueOnce(new Error("window destroyed"));
        const errors = vi.fn();
        process.on("unhandledRejection", errors);

        render(<WindowReveal />);
        await new Promise((resolve) => setTimeout(resolve, 0));

        expect(show).toHaveBeenCalledTimes(1);
        expect(errors).not.toHaveBeenCalled();
        process.off("unhandledRejection", errors);
    });
});
