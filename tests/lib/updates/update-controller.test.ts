import type { Update } from "@tauri-apps/plugin-updater";
import { describe, expect, it, vi } from "vitest";

import { UpdateController } from "@/lib/updates/update-controller";

function fakeUpdate(config: Partial<Update> = {}): Update {
    return {
        available: true,
        currentVersion: "0.1.0",
        version: "0.2.0",
        date: undefined,
        body: undefined,
        rawJson: {},
        download: vi.fn().mockResolvedValue(undefined),
        install: vi.fn().mockResolvedValue(undefined),
        downloadAndInstall: vi.fn().mockResolvedValue(undefined),
        close: vi.fn().mockResolvedValue(undefined),
        ...config,
    } as unknown as Update;
}

function makeController(overrides: {
    check?: () => Promise<Update | null>;
    relaunch?: () => Promise<void>;
}) {
    const check = overrides.check ?? vi.fn().mockResolvedValue(null);
    const relaunch = overrides.relaunch ?? vi.fn().mockResolvedValue(undefined);
    const controller = new UpdateController({ check: () => check(), relaunch });
    return { controller, check, relaunch };
}

describe("UpdateController", () => {
    it("settles into none when no update is available", async () => {
        const { controller } = makeController({});
        await controller.start();
        expect(controller.store.state.status).toBe("none");
    });

    it("reports an available update with its version info", async () => {
        const update = fakeUpdate({ version: "0.2.0", body: "notes" });
        const { controller } = makeController({
            check: vi.fn().mockResolvedValue(update),
        });
        await controller.start();
        const state = controller.store.state;
        if (state.status !== "available") {
            throw new Error(`expected available, got ${state.status}`);
        }
        expect(state.info).toMatchObject({
            version: "0.2.0",
            currentVersion: "0.1.0",
            body: "notes",
        });
    });

    it("checks the endpoint exactly once across repeated calls", async () => {
        const { controller, check } = makeController({});
        await controller.start();
        await controller.start();
        expect(check).toHaveBeenCalledTimes(1);
    });

    it("surfaces a check failure without an update to show", async () => {
        const { controller } = makeController({
            check: vi.fn().mockRejectedValue(new Error("offline")),
        });
        await controller.start();
        expect(controller.store.state).toEqual({
            status: "failed",
            info: null,
            message: "offline",
        });
    });

    it("trails download progress and relaunches on success", async () => {
        const events = [
            { event: "Started", data: { contentLength: 100 } },
            { event: "Progress", data: { chunkLength: 40 } },
            { event: "Progress", data: { chunkLength: 60 } },
            { event: "Finished" },
        ] as const;
        const downloadAndInstall = vi
            .fn()
            .mockImplementation(async (onEvent?: (event: unknown) => void) => {
                for (const event of events) onEvent?.(event);
            });
        const update = fakeUpdate({ downloadAndInstall });
        const { controller, relaunch } = makeController({
            check: vi.fn().mockResolvedValue(update),
        });

        await controller.start();
        await controller.install();

        const lastProgress = controller.store.state;
        expect(lastProgress.status).toBe("installing");
        expect(relaunch).toHaveBeenCalledTimes(1);
        expect(downloadAndInstall).toHaveBeenCalledTimes(1);
    });

    it("returns to the dialog state when install fails", async () => {
        const downloadAndInstall = vi
            .fn()
            .mockRejectedValue(new Error("disk full"));
        const update = fakeUpdate({ downloadAndInstall });
        const { controller, relaunch } = makeController({
            check: vi.fn().mockResolvedValue(update),
        });

        await controller.start();
        await controller.install();

        expect(controller.store.state).toEqual({
            status: "failed",
            info: {
                version: "0.2.0",
                currentVersion: "0.1.0",
                body: undefined,
                date: undefined,
            },
            message: "disk full",
        });
        expect(relaunch).not.toHaveBeenCalled();
    });

    it("resets to a pristine idle state", async () => {
        const update = fakeUpdate({});
        const { controller } = makeController({
            check: vi.fn().mockResolvedValue(update),
        });
        await controller.start();
        controller.reset();
        expect(controller.hasChecked).toBe(true);
        expect(controller.store.state).toEqual({ status: "idle" });
    });
});
