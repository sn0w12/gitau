import { getAppRuntime } from "@/lib/bootstrap/app-runtime";

const startedAt = performance.now();
let bootstrapMs = 0;

export function markBootstrapComplete(): void {
    bootstrapMs = Math.round(performance.now() - startedAt);
}

export function reportLaunchReady(): void {
    void getAppRuntime().backend.app.launchReady({
        bootstrapMs,
        renderMs: Math.round(performance.now() - startedAt),
    });
}
