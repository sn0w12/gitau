import { QueryClientProvider } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import type { ReactNode } from "react";

import { BackgroundFetch } from "@/components/background-fetch";
import { AppServicesContext } from "@/contexts/services-context";
import { invalidateAllRepositoryData } from "@/lib/backend/mutations/invalidation";
import { markStreamsStale } from "@/lib/backend/streams/unified-streams";
import { listenRepositoryInvalidations } from "@/lib/backend/transport/client";
import { getAppRuntime } from "@/lib/bootstrap/app-runtime";
import { markRepositoryError } from "@/stores/repository-store";

/**
 * Fast Refresh boundary: this file must export ONLY this component. Any
 * additional export disqualifies the module as a refresh boundary, and every
 * HMR update beneath it would remount the document shell (html/head/body).
 */
export function AppServicesProvider({ children }: { children: ReactNode }) {
    const services = useMemo(() => getAppRuntime(), []);

    useEffect(() => {
        let disposed = false;
        let disposer: { dispose: () => void } | null = null;

        void listenRepositoryInvalidations((invalidation) => {
            if (disposed) return;
            markStreamsStale(invalidation.repoId);
            void invalidateAllRepositoryData(
                services.queryClient,
                invalidation.repoId
            );
        })
            .then((listener) => {
                if (disposed) listener.dispose();
                else disposer = listener;
            })
            .catch((error) => {
                markRepositoryError(
                    0,
                    `invalidation bridge failed: ${String(error)}`
                );
            });

        return () => {
            disposed = true;
            disposer?.dispose();
        };
    }, [services]);

    // No unmount reset: the runtime is a process-lifetime singleton, and a
    // StrictMode remount would orphan the QueryClient captured by context.

    return (
        <AppServicesContext.Provider value={services}>
            <QueryClientProvider client={services.queryClient}>
                <BackgroundFetch />
                {children}
            </QueryClientProvider>
        </AppServicesContext.Provider>
    );
}
