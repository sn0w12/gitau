import * as React from "react";

import { diffSessionRegistry } from "@/lib/backend/streams/diff-session-registry";

/**
 * Watchdog for streaming sessions: when a freshly opened session stays
 * "running" without producing any content past `timeoutMs`, restarts it (up
 * to `maxRestarts`). Covers startup races between channel registration and
 * event delivery that would otherwise spin forever.
 */
export function useStreamWatchdog({
    stuck,
    sessionKey,
    onRestart,
    timeoutMs = 2500,
    maxRestarts = 2,
}: {
    stuck: boolean;
    sessionKey: string;
    onRestart: () => void;
    timeoutMs?: number;
    maxRestarts?: number;
}): void {
    const attempts = React.useRef(0);
    const lastSession = React.useRef(sessionKey);

    React.useEffect(() => {
        if (lastSession.current !== sessionKey) {
            lastSession.current = sessionKey;
            attempts.current = 0;
        }
        if (!stuck) {
            attempts.current = 0;
            return;
        }
        if (attempts.current >= maxRestarts) return;

        const timer = window.setTimeout(() => {
            attempts.current += 1;
            // Sessions are reused across remounts, so the retry must reset
            // the controller itself; begin() on a still-running session
            // would otherwise be a no-op.
            diffSessionRegistry.get(sessionKey)?.restart();
            onRestart();
        }, timeoutMs);
        return () => window.clearTimeout(timer);
    }, [stuck, sessionKey, onRestart, timeoutMs, maxRestarts]);
}
