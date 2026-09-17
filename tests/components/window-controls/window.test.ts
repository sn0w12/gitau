import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

// Controllable stand-in for the Tauri window: tests flip `maximized` to
// simulate OS-level changes (Win+Up, taskbar) and fire the resize/focus
// listeners the real backend would invoke.
const fake = vi.hoisted(() => ({
    maximized: false,
    resizeHandlers: [] as Array<() => void>,
    focusHandlers: [] as Array<(event: { payload: boolean }) => void>,
}));

vi.mock("@tauri-apps/api/window", () => ({
    getCurrentWindow: () => ({
        minimize: async () => {},
        close: async () => {},
        startDragging: async () => {},
        isMaximized: async () => fake.maximized,
        maximize: async () => {
            fake.maximized = true;
            for (const handler of fake.resizeHandlers) handler();
        },
        unmaximize: async () => {
            fake.maximized = false;
            for (const handler of fake.resizeHandlers) handler();
        },
        toggleMaximize: async () => {
            fake.maximized = !fake.maximized;
            for (const handler of fake.resizeHandlers) handler();
        },
        onResized: async (handler: () => void) => {
            fake.resizeHandlers.push(handler);
            return () => {
                fake.resizeHandlers = fake.resizeHandlers.filter(
                    (other) => other !== handler
                );
            };
        },
        onFocusChanged: async (
            handler: (event: { payload: boolean }) => void
        ) => {
            fake.focusHandlers.push(handler);
            return () => {
                fake.focusHandlers = fake.focusHandlers.filter(
                    (other) => other !== handler
                );
            };
        },
        outerSize: async () => ({ width: 800, height: 600 }),
        setSize: async () => {},
        isFullscreen: async () => false,
        setFullscreen: async () => {},
        setSimpleFullscreen: async () => {},
    }),
}));

// Fresh module per test: maximized listeners live in module state.
async function loadModule() {
    vi.resetModules();
    return import("@/components/window-controls/window");
}

beforeEach(() => {
    fake.maximized = false;
    fake.resizeHandlers = [];
    fake.focusHandlers = [];
    vi.useFakeTimers();
});

afterEach(() => {
    vi.useRealTimers();
});

describe("maximized tracking", () => {
    it("delivers the initial maximized state to the first subscriber", async () => {
        fake.maximized = true;
        const mod = await loadModule();
        const seen: boolean[] = [];
        await mod
            .createWindowControls()
            .onMaximizedChange((maximized) => seen.push(maximized));
        await vi.advanceTimersByTimeAsync(0);
        expect(seen).toEqual([true]);
    });

    it("notifies subscribers after toggling maximize on", async () => {
        const mod = await loadModule();
        const seen: boolean[] = [];
        const api = mod.createWindowControls();
        await api.onMaximizedChange((maximized) => seen.push(maximized));
        await vi.advanceTimersByTimeAsync(0);
        expect(seen).toEqual([false]);

        await api.maximize();
        expect(seen).toEqual([false, true]);
    });

    it("notifies subscribers after unmaximizing", async () => {
        fake.maximized = true;
        const mod = await loadModule();
        const seen: boolean[] = [];
        const api = mod.createWindowControls();
        await api.onMaximizedChange((maximized) => seen.push(maximized));
        await vi.advanceTimersByTimeAsync(0);
        expect(seen).toEqual([true]);

        await api.unmaximize();
        expect(seen).toEqual([true, false]);
    });

    it("keeps an already-maximized window maximized on maximize", async () => {
        fake.maximized = true;
        const mod = await loadModule();
        const seen: boolean[] = [];
        const api = mod.createWindowControls();
        await api.onMaximizedChange((maximized) => seen.push(maximized));
        await vi.advanceTimersByTimeAsync(0);
        expect(seen).toEqual([true]);

        // Single click on the maximize button of an OS-maximized window
        // must not toggle it off again.
        await api.maximize();
        await vi.advanceTimersByTimeAsync(150);
        expect(fake.maximized).toBe(true);
        expect(seen).toEqual([true]);
    });

    it("picks up OS-level maximizes from resize events", async () => {
        const mod = await loadModule();
        const seen: boolean[] = [];
        await mod
            .createWindowControls()
            .onMaximizedChange((maximized) => seen.push(maximized));
        await vi.advanceTimersByTimeAsync(0);
        expect(seen).toEqual([false]);

        fake.maximized = true;
        for (const handler of fake.resizeHandlers) handler();
        expect(seen).toEqual([false]);

        await vi.advanceTimersByTimeAsync(150);
        expect(seen).toEqual([false, true]);
    });

    it("refreshes maximized state when the window regains focus", async () => {
        const mod = await loadModule();
        const seen: boolean[] = [];
        await mod
            .createWindowControls()
            .onMaximizedChange((maximized) => seen.push(maximized));
        await vi.advanceTimersByTimeAsync(0);
        expect(seen).toEqual([false]);

        fake.maximized = true;
        for (const handler of fake.focusHandlers) handler({ payload: true });
        await vi.advanceTimersByTimeAsync(0);
        expect(seen).toEqual([false, true]);
    });
});
