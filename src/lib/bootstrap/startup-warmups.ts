import { loadHistoryChart } from "@/components/repo/history/history-chart-loader";
import { systemFontsQuery } from "@/lib/backend/queries/font-queries";

import type { AppServices } from "./app-runtime";

type StartupWarmup = (services: AppServices) => void;

/**
 * Work that is useful soon but not needed for first paint. Runs once the
 * browser is idle; add new warmups to this list instead of scheduling more
 * idle callbacks.
 */
const WARMUPS: readonly StartupWarmup[] = [
    () => void loadHistoryChart(),
    () => void import("@/routes/settings-page"),
    (services) => {
        void services.queryClient.query(
            systemFontsQuery({ backend: services.backend })
        );
    },
];

export function scheduleStartupWarmups(services: AppServices): void {
    requestIdleCallback(
        () => {
            for (const warmup of WARMUPS) warmup(services);
        },
        { timeout: 1000 }
    );
}
