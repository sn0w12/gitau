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
 * states so each view only supplies its content. `resetKey` participates in
 * the remount key, so a changed target (e.g. another commit) restarts clean.
 */
export function DiffSessionGate({
    repoId,
    request,
    generation,
    resetKey,
    tabId,
    emptyTitle,
    emptyDescription,
    children,
}: {
    repoId: number;
    request?: DiffRequest;
    generation?: number;
    resetKey?: string;
    tabId?: string;
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
            key={resetKey ? `${resetKey}:${retryKey}` : retryKey}
            repoId={repoId}
            request={request}
            generation={generation}
            tabId={tabId}
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
    emptyTitle,
    emptyDescription,
    onRetry,
    children,
}: {
    repoId: number;
    request?: DiffRequest;
    generation?: number;
    tabId?: string;
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

    // Keep the last painted content on screen while a re-stream runs so a
    // refresh swaps rows in place instead of flashing the loader. Reset when
    // the request (target file/commit) changes so an old diff never fills a
    // fresh view. A completed-but-empty result is a real "no changes"
    // outcome, not a reload, so it must show the empty state, not old rows.
    /* oxlint-disable react/refs */
    const contentKey = JSON.stringify(request ?? null);
    const lastContentRef = React.useRef<{
        contentKey: string;
        state: DiffSessionState;
    } | null>(null);
    if (lastContentRef.current?.contentKey !== contentKey) {
        lastContentRef.current = null;
    }
    if (state.sections.length > 0) {
        lastContentRef.current = { contentKey, state };
    }
    const showStale =
        state.sections.length === 0 &&
        state.status !== "completed" &&
        lastContentRef.current !== null;

    useStreamWatchdog({
        stuck:
            state.status === "running" &&
            state.sections.length === 0 &&
            !showStale,
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
        return (
            <>{children(lastContentRef.current!.state, onRetry, controller)}</>
        );
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

    if (state.sections.length === 0) {
        return (
            <div className="flex size-full items-center justify-center">
                <DelayedSpinner className="size-6" />
            </div>
        );
    }

    return <>{children(state, onRetry, controller)}</>;
}
