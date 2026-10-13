import * as React from "react";

import { DelayedSpinner } from "@/components/diff/delayed-spinner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyTitle,
} from "@/components/ui/empty";
import { useDiffSession } from "@/hooks/changes/use-diff-session";
import { useStreamWatchdog } from "@/hooks/changes/use-stream-watchdog";
import type { DiffRequest } from "@/lib/backend/protocol";
import type { DiffSessionState } from "@/lib/backend/streams/diff-session";

/**
 * Shared session lifecycle for streaming diff views: acquires the session,
 * arms the stuck-stream watchdog, and renders the failed / empty / loading
 * states so each view only supplies its content.
 */
export function DiffSessionGate({
    repoId,
    request,
    generation,
    tabId,
    pendingPath,
    emptyTitle,
    emptyDescription,
    children,
}: {
    repoId: number;
    request?: DiffRequest;
    generation?: number;
    tabId?: string;
    pendingPath?: string;
    emptyTitle: string;
    emptyDescription: string;
    children: (
        state: DiffSessionState,
        retry: () => void,
        controller: import("@/lib/backend/streams/diff-session").DiffSessionController
    ) => React.ReactNode;
}) {
    // Stable identity: useStreamWatchdog restarts its timer whenever
    // onRestart changes, so an unstable callback would postpone firing.
    const [retryKey, setRetryKey] = React.useState(0);
    const onRetry = React.useCallback(() => setRetryKey((k) => k + 1), []);

    return (
        <GateSession
            key={retryKey}
            repoId={repoId}
            request={request}
            generation={generation}
            tabId={tabId}
            pendingPath={pendingPath}
            emptyTitle={emptyTitle}
            emptyDescription={emptyDescription}
            onRetry={onRetry}
        >
            {children}
        </GateSession>
    );
}

function GateSession({
    repoId,
    request,
    generation,
    tabId,
    pendingPath,
    emptyTitle,
    emptyDescription,
    onRetry,
    children,
}: {
    repoId: number;
    request?: DiffRequest;
    generation?: number;
    tabId?: string;
    pendingPath?: string;
    emptyTitle: string;
    emptyDescription: string;
    onRetry: () => void;
    children: (
        state: DiffSessionState,
        retry: () => void,
        controller: import("@/lib/backend/streams/diff-session").DiffSessionController
    ) => React.ReactNode;
}) {
    const session = useDiffSession(
        { repoId, request, generation },
        tabId ? { tabId } : undefined
    );
    const { controller } = session;
    const state = session;

    // Keep the last fully rendered diff on screen until the next one has rows
    // to paint, so switching files or commits swaps rows in place instead of
    // blanking the pane between the new `started` event and its first folded
    // chunk. The stale state always belongs to this same view, and the failed
    // and completed-empty states below still win.
    /* oxlint-disable react/refs */
    const lastContentRef = React.useRef<DiffSessionState | null>(null);
    const ready = state.sections.length > 0 && state.rowsBySection.size > 0;
    if (ready) {
        lastContentRef.current = state;
    }
    const showStale =
        lastContentRef.current !== null &&
        !ready &&
        state.status !== "completed";

    useStreamWatchdog({
        stuck: state.status === "running" && !ready,
        sessionKey: state.sessionId,
        onRestart: onRetry,
    });

    if (state.status === "failed") {
        return (
            <div className="flex size-full flex-col items-center justify-center gap-2 p-4">
                <Badge variant="destructive">diff failed</Badge>
                <p className="max-w-prose text-center text-xs text-muted-foreground">
                    {state.error?.message ?? "The diff operation failed."}
                </p>
                <Button variant="outline" size="sm" onClick={onRetry}>
                    Retry
                </Button>
            </div>
        );
    }

    if (showStale) {
        return <>{children(lastContentRef.current!, onRetry, controller)}</>;
    }

    if (state.sections.length === 0 && state.status === "completed") {
        return (
            <Empty className="ui-selectable size-full">
                <EmptyHeader>
                    <EmptyTitle>{emptyTitle}</EmptyTitle>
                    <EmptyDescription>{emptyDescription}</EmptyDescription>
                </EmptyHeader>
            </Empty>
        );
    }

    if (!ready) {
        return (
            <div className="flex size-full min-h-0 min-w-0 flex-col">
                {pendingPath && (
                    <div className="flex h-8 shrink-0 items-center border-b px-2">
                        <span className="truncate font-mono text-xs">
                            {pendingPath}
                        </span>
                    </div>
                )}
                <div className="flex min-h-0 flex-1 items-center justify-center">
                    <DelayedSpinner className="size-6" />
                </div>
            </div>
        );
    }

    return <>{children(state, onRetry, controller)}</>;
}
