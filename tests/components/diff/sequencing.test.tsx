import { act } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { beforeAll, describe, expect, it } from "vitest";

import { CommitDiffView } from "@/components/diff/commit-diff-view";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    DiffEvent,
    DiffRow,
    DiffRowKind,
    RangeResult,
} from "@/lib/backend/protocol";
import type { AppServices } from "@/lib/bootstrap/app-runtime";
import { seedSettingsForTests } from "@/stores/settings-store";

type Sink = (event: DiffEvent) => void;

beforeAll(() => {
    (
        globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
    ).IS_REACT_ACT_ENVIRONMENT = true;
});

function makeEvents(commitId: string): DiffEvent[] {
    const r = (kind: DiffRowKind, text: string): DiffRow => ({
        kind,
        content: text,
    });
    return [
        {
            event: "started",
            operationId: 1,
            snapshotId: 1,
            generation: 1,
            sections: [
                {
                    sectionId: 0,
                    path: `${commitId}/file.txt`,
                    kind: "modified",
                    binary: false,
                    complete: false,
                },
            ],
            estimatedTotalRows: 2,
        },
        {
            event: "sectionLayout",
            operationId: 1,
            sectionId: 0,
            startRow: 0,
            rowCount: 2,
        },
        {
            event: "chunk",
            operationId: 1,
            sectionId: 0,
            rowStart: 0,
            rows: [
                r("fileHeader", `diff --git a/${commitId}/f b/${commitId}/f`),
                r("addition", `${commitId}-line`),
            ],
        },
        { event: "layoutReady", operationId: 1, totalRows: 2 },
        {
            event: "completed",
            operationId: 1,
            totalRows: 2,
            additions: 1,
            deletions: 0,
            durationMs: 1,
        },
    ];
}

describe("CommitDiffView session switching", () => {
    it("re-renders when switching between commits", async () => {
        seedSettingsForTests({ diffViewMode: "unified" });

        const sinks = new Map<number, Sink>();
        let opSeq = 0;

        const diffClient = {
            open: (
                _repo: number,
                request:
                    | { comparison?: { commitToParent?: { commit?: string } } }
                    | undefined,
                onEvent: Sink
            ) => {
                const op = ++opSeq;
                const id =
                    request?.comparison &&
                    "commitToParent" in request.comparison
                        ? (request.comparison.commitToParent?.commit ?? "?")
                        : "?";
                sinks.set(op, onEvent);
                queueMicrotask(() => {
                    for (const ev of makeEvents(id)) onEvent(ev);
                });
                return { ok: true as const, value: op };
            },
            readRange: async (): Promise<{ ok: true; value: RangeResult }> => ({
                ok: true as const,
                value: {
                    rows: [],
                    nextCursor: 0,
                    hasMore: false,
                    knownTotalRows: 0,
                    complete: true,
                },
            }),
            cancel: async () => ({ ok: true as const, value: true }),
        };

        const services = {
            backend: { diff: diffClient },
        } as unknown as AppServices;

        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);

        function renderFor(commitId: string) {
            act(() => {
                root.render(
                    <AppServicesContext.Provider value={services}>
                        <CommitDiffView
                            repoId={1}
                            commitId={commitId}
                            dispatch={() => {}}
                        />
                    </AppServicesContext.Provider>
                );
            });
        }

        renderFor("commit-AAA");
        await act(async () => {
            await new Promise((r) => setTimeout(r, 50));
        });
        expect(container.textContent).toContain("commit-AAA/file.txt");

        renderFor("commit-BBB");
        await act(async () => {
            await new Promise((r) => setTimeout(r, 50));
        });
        expect(container.textContent).toContain("commit-BBB/file.txt");

        renderFor("commit-AAA");
        await act(async () => {
            await new Promise((r) => setTimeout(r, 50));
        });
        expect(container.textContent).toContain("commit-AAA/file.txt");

        act(() => root.unmount());
        container.remove();
    });
});
