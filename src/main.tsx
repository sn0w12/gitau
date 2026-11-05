import { MotionConfig } from "motion/react";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { ActiveTabHost } from "@/components/active-tab-host";
import { AppLayout } from "@/components/app-layout";
import { AppServicesProvider } from "@/components/providers/app-services-provider";
import { loadHistoryChart } from "@/components/repo/history/history-chart-loader";
import { Button } from "@/components/ui/button";
import { WindowReveal } from "@/components/window-reveal";
import { AppCommandProvider } from "@/contexts/app-command-context";
import { expectOk } from "@/lib/backend/transport/result";
import { getAppRuntime } from "@/lib/bootstrap/app-runtime";
import {
    bootLongFrames,
    bootLongTasks,
    bootMark,
    bootNavigationTimings,
    bootPaintTimings,
    bootResourceTimings,
    bootSpan,
} from "@/lib/bootstrap/boot-timing";
import {
    hydrateSession,
    initSessionPersistence,
    reopenSessionRepositories,
    scheduleReopenRetry,
} from "@/lib/bootstrap/session-persistence";
import { startWindowControlsOverlay } from "@/lib/window-controls-overlay";

import "./styles.css";
import { startFontEngine } from "@/stores/font-store";
import {
    configureSettingsSaver,
    initializeSettings,
} from "@/stores/settings-store";
import { startThemeEngine } from "@/stores/theme-store";
const container = document.getElementById("root");
if (!container) throw new Error("missing #root element");
const root = createRoot(container);
const bootstrapSpan = bootSpan("bootstrap");

bootPaintTimings();
bootLongTasks();
bootLongFrames();
window.addEventListener(
    "load",
    () => {
        bootResourceTimings();
        void document.fonts.ready.then(() => bootMark("fonts.ready"));
        // loadEventEnd is only set once the load event finishes dispatching.
        setTimeout(bootNavigationTimings, 0);
    },
    { once: true }
);

async function loadInitialSnapshot() {
    const backend = getAppRuntime().backend.settings;
    const span = bootSpan("settings.load");
    const loaded = await backend.load();
    span.end();
    if (!loaded.ok) throw loaded.error;
    return loaded.value;
}

type RestoreOutcome =
    | { kind: "restored"; failedPaths: string[] }
    | { kind: "empty" }
    | { kind: "loadFailed"; message: string };

// Restores the session from disk and remaps fresh backend ids onto the
// restored tabs. A failed read aborts bootstrap: persisting over a file we
// could not read would destroy the user's tabs.
async function restoreSession(): Promise<RestoreOutcome> {
    const backend = getAppRuntime().backend.session;
    const loadSpan = bootSpan("session.load");
    const loaded = await backend.load();
    loadSpan.end();
    if (!loaded.ok) {
        return {
            kind: "loadFailed",
            message: loaded.error.message,
        };
    }
    const doc = loaded.value;
    if (!doc || !Array.isArray(doc.tabs)) return { kind: "empty" };

    const reopenSpan = bootSpan("repos.reopen");
    const { idByPath, failedPaths } = await reopenSessionRepositories(doc);
    reopenSpan.end();
    hydrateSession(doc, idByPath);
    return { kind: "restored", failedPaths };
}

function BootstrapError({ message }: { message: string }) {
    return (
        <div className="flex h-svh items-center justify-center p-6">
            <WindowReveal />
            <div className="max-w-sm space-y-2 text-center">
                <h1 className="text-lg font-semibold tracking-tight">
                    Session failed to load
                </h1>
                <p className="text-sm text-muted-foreground">{message}</p>
                <Button
                    variant="outline"
                    onClick={() => window.location.reload()}
                >
                    Retry
                </Button>
            </div>
        </div>
    );
}

async function bootstrap(): Promise<void> {
    const services = getAppRuntime();

    // Settings and session are independent disk reads; loading them
    // concurrently removes one IPC round trip from the pre-render path.
    const [snapshot, restored] = await Promise.all([
        loadInitialSnapshot(),
        restoreSession(),
    ]);

    // Settings must be ready before any component reads them.
    configureSettingsSaver((key, value) =>
        services.backend.settings.set(key, value).then(expectOk)
    );
    initializeSettings(snapshot);

    if (restored.kind === "loadFailed") {
        root.render(<BootstrapError message={restored.message} />);
        bootstrapSpan.end();
        return;
    }

    startThemeEngine();
    startFontEngine();
    startWindowControlsOverlay();

    // Save failures are logged loudly: silent data loss is worse than noise.
    const disposePersistence = initSessionPersistence(async (doc) => {
        const saved = await services.backend.session.save(doc);
        if (!saved.ok) {
            console.error("session save failed:", saved.error.message);
        }
    });
    // Best-effort flush of the debounce tail when the window goes away.
    window.addEventListener("pagehide", () => disposePersistence(), {
        once: true,
    });

    // One delayed retry for repos that failed to reopen (offline network
    // drives often come online shortly after login).
    if (restored.kind === "restored") {
        scheduleReopenRetry(restored.failedPaths, services.backend);
    }

    bootMark("render.call");
    root.render(
        <StrictMode>
            <AppServicesProvider>
                <AppCommandProvider>
                    <WindowReveal />
                    <MotionConfig reducedMotion="user">
                        <AppLayout>
                            <ActiveTabHost />
                        </AppLayout>
                    </MotionConfig>
                </AppCommandProvider>
            </AppServicesProvider>
        </StrictMode>
    );
    bootMark("first.render");
    bootstrapSpan.end();
    requestIdleCallback(() => void loadHistoryChart(), { timeout: 1000 });
}

bootstrap().catch((error) => {
    root.render(
        <BootstrapError
            message={error instanceof Error ? error.message : String(error)}
        />
    );
    bootstrapSpan.end();
});
