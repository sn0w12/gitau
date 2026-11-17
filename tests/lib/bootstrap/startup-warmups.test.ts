import { afterEach, describe, expect, it, vi } from "vitest";

const { loadHistoryChart, systemFontsQuery, query } = vi.hoisted(() => ({
    loadHistoryChart: vi.fn(),
    systemFontsQuery: vi.fn((_deps: unknown) => ({ queryKey: ["fonts"] })),
    query: vi.fn(),
}));

vi.mock("@/components/repo/history/history-chart-loader", () => ({
    loadHistoryChart,
}));
vi.mock("@/lib/backend/queries/font-queries", () => ({ systemFontsQuery }));
vi.mock("@/routes/settings-page", () => ({ SettingsPage: () => null }));

import type { AppServices } from "@/lib/bootstrap/app-runtime";
import { scheduleStartupWarmups } from "@/lib/bootstrap/startup-warmups";

describe("scheduleStartupWarmups", () => {
    afterEach(() => {
        vi.unstubAllGlobals();
        loadHistoryChart.mockClear();
        query.mockClear();
        systemFontsQuery.mockClear();
    });

    it("starts every warmup from one idle callback", () => {
        const idle = vi.fn((callback: () => void) => {
            callback();
            return 1;
        });
        vi.stubGlobal("requestIdleCallback", idle);

        const backend = {} as AppServices["backend"];
        const services = {
            backend,
            queryClient: { query },
        } as unknown as AppServices;

        scheduleStartupWarmups(services);

        expect(idle).toHaveBeenCalledTimes(1);
        expect(idle).toHaveBeenCalledWith(expect.any(Function), {
            timeout: 1000,
        });
        expect(loadHistoryChart).toHaveBeenCalledTimes(1);
        expect(systemFontsQuery).toHaveBeenCalledWith({ backend });
        expect(query).toHaveBeenCalledWith({ queryKey: ["fonts"] });
    });
});
