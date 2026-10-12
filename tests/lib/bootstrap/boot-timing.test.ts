import { beforeEach, describe, expect, it, vi } from "vitest";

const { info } = vi.hoisted(() => ({
    info: vi.fn(async (_message: string) => undefined),
}));

vi.mock("@tauri-apps/plugin-log", () => ({ info }));

import { bootMark, bootSpan } from "@/lib/bootstrap/boot-timing";

function lastEvent() {
    const call = info.mock.calls.at(-1);
    if (!call) throw new Error("no boot trace emitted");
    return JSON.parse(call[0].slice("[boottrace] ".length));
}

describe("boot timing trace", () => {
    beforeEach(() => {
        info.mockClear();
    });

    it("emits a complete event with wall-clock start and duration", () => {
        bootSpan("settings.load").end();

        const event = lastEvent();
        expect(event).toMatchObject({
            name: "settings.load",
            cat: "boot",
            ph: "X",
            pid: 2,
        });
        expect(event.ts).toBeGreaterThan((Date.now() - 60_000) * 1000);
        expect(event.ts).toBeLessThan((Date.now() + 60_000) * 1000);
        expect(event.dur).toBeGreaterThanOrEqual(0);
    });

    it("ignores a repeat end call on one span", () => {
        const span = bootSpan("session.load");
        span.end();
        span.end();

        expect(info).toHaveBeenCalledTimes(1);
    });

    it("emits milestones as instant events", () => {
        bootMark("first.paint");

        expect(lastEvent()).toMatchObject({
            name: "first.paint",
            ph: "i",
            s: "g",
            pid: 2,
        });
    });

    it("reuses a lane per name and opens new lanes for other names", () => {
        bootSpan("lane-alpha").end();
        const alpha = lastEvent();
        bootSpan("lane-beta").end();
        const beta = lastEvent();
        bootSpan("lane-alpha").end();
        const alphaAgain = lastEvent();

        expect(alphaAgain.tid).toBe(alpha.tid);
        expect(beta.tid).not.toBe(alpha.tid);
    });

    it("swallows a rejected log transport", async () => {
        info.mockRejectedValueOnce(new Error("no ipc"));
        bootMark("settings.load");
        await Promise.resolve();

        expect(info).toHaveBeenCalledTimes(1);
    });
});
