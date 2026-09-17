import { useCallback, useEffect, useRef } from "react";

import { useAppServices } from "@/contexts/services-context";
import { prefetchDiff } from "@/lib/backend/streams/unified-streams";

const PREFETCH_DELAY_MS = 120;

/**
 * Warms a commit's diff on pointer-rest/focus so the click swaps to
 * already-streamed rows; the real view reuses the same registry session.
 */
export function useCommitDiffPrefetch(
    repoId: number,
    tabId?: string
): {
    warm: (commitId: string) => void;
    cancelWarm: () => void;
} {
    const { backend } = useAppServices();
    const timerRef = useRef<number | null>(null);
    const releaseRef = useRef<(() => void) | null>(null);

    const cancelWarm = useCallback(() => {
        if (timerRef.current !== null) {
            window.clearTimeout(timerRef.current);
            timerRef.current = null;
        }
        releaseRef.current?.();
        releaseRef.current = null;
    }, []);

    const warm = useCallback(
        (commitId: string) => {
            if (!Number.isInteger(repoId) || repoId <= 0) return;
            if (timerRef.current !== null || releaseRef.current !== null)
                return;
            timerRef.current = window.setTimeout(() => {
                timerRef.current = null;
                releaseRef.current = prefetchDiff(backend, {
                    repoId,
                    tabId,
                    request: {
                        comparison: { commitToParent: { commit: commitId } },
                    },
                });
            }, PREFETCH_DELAY_MS);
        },
        [backend, repoId, tabId]
    );

    useEffect(() => cancelWarm, [cancelWarm]);

    return { warm, cancelWarm };
}
