import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { DiffViewer } from "@/components/diff/diff-viewer";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    DiffEvent,
    DiffRow,
    DiffRowKind,
    RangeResult,
} from "@/lib/backend/protocol";
import { diffSessionRegistry } from "@/lib/backend/streams/diff-session-registry";
import type { AppServices } from "@/lib/bootstrap/app-runtime";
import type { reducerAction } from "@/routes/repo-page";
import { seedSettingsForTests } from "@/stores/settings-store";

type ChannelSink = (event: DiffEvent) => void;

function row(kind: DiffRowKind, content: string): DiffRow {
    return { kind, content };
}

function fakeBackend(events: DiffEvent[]) {
    const backend = {
        diff: {
            open: async (
                _repoId: number,
                _request: unknown,
                onEvent: ChannelSink
            ) => {
                queueMicrotask(() => {
                    for (const event of events) onEvent(event);
                });
                return { ok: true as const, value: 1 };
            },
            readRange: async (): Promise<{
                ok: true;
                value: RangeResult;
            }> => ({
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
        },
    };
    return { backend } as unknown as AppServices;
}

const streamedEvents: DiffEvent[] = [
    {
        event: "started",
        operationId: 1,
        snapshotId: 1,
        generation: 1,
        sections: [
            {
                sectionId: 0,
                path: "src/a.txt",
                kind: "modified",
                binary: false,
                complete: false,
            },
        ],
        estimatedTotalRows: 4,
    },
    {
        event: "sectionLayout",
        operationId: 1,
        sectionId: 0,
        startRow: 0,
        rowCount: 4,
    },
    {
        event: "chunk",
        operationId: 1,
        sectionId: 0,
        rowStart: 0,
        rows: [
            row("fileHeader", "diff --git a/src/a.txt b/src/a.txt"),
            row("hunkHeader", "@@ -1,2 +1,2 @@"),
            { ...row("deletion", "old"), oldLineno: 1 },
            { ...row("addition", "new"), newLineno: 1 },
        ],
    },
    { event: "layoutReady", operationId: 1, totalRows: 4 },
    {
        event: "completed",
        operationId: 1,
        totalRows: 4,
        additions: 1,
        deletions: 1,
        durationMs: 3,
    },
];

function renderWith(backend: AppServices, ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={backend}>
                {ui}
            </AppServicesContext.Provider>
        );
    });
    return {
        container,
        unmount() {
            act(() => root.unmount());
            container.remove();
        },
    };
}

async function settle() {
    await act(async () => {
        await new Promise((r) => setTimeout(r, 30));
    });
}

beforeAll(() => {
    // jsdom reports zero rects; give the virtualizer a real viewport.
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
        x: 0,
        y: 0,
        top: 0,
        left: 0,
        right: 900,
        bottom: 700,
        width: 900,
        height: 700,
        toJSON: () => ({}),
    });
});
describe("DiffViewer", () => {
    beforeEach(() => {
        // Sessions persist across mounts by design; tests need isolation.
        diffSessionRegistry.resetForTests();
    });

    it("renders streamed rows in unified mode by default", async () => {
        seedSettingsForTests({ diffViewMode: "unified" });
        const actions: reducerAction[] = [];
        const view = renderWith(
            fakeBackend(streamedEvents),
            <DiffViewer
                repoId={1}
                tabId="t1"
                dispatch={(action) => {
                    actions.push(action);
                }}
            />
        );
        await settle();

        expect(view.container.textContent).toContain("src/a.txt");
        await expect.poll(() => view.container.textContent).toContain("old");
        await expect.poll(() => view.container.textContent).toContain("new");

        // The section bar's close control clears the repo-page selection.
        const bar = view.container.querySelector("div.border-b")!;
        const buttons = bar.querySelectorAll<HTMLButtonElement>("button");
        act(() => buttons[buttons.length - 1].click());
        expect(actions).toEqual([{ type: "CLEAR_SELECTION" }]);
        view.unmount();
    });

    it("renders split pairs when the setting prefers split mode", async () => {
        seedSettingsForTests({ diffViewMode: "split" });
        const view = renderWith(
            fakeBackend(streamedEvents),
            <DiffViewer repoId={1} dispatch={() => {}} />
        );
        await settle();

        // Split rows render as two-column grids.
        const splitRows = view.container.querySelectorAll(".grid-cols-2");
        expect(splitRows.length).toBeGreaterThanOrEqual(1);
        view.unmount();
    });

    it("shows the empty state when a completed stream has no sections", async () => {
        seedSettingsForTests({ diffViewMode: "unified" });
        const view = renderWith(
            fakeBackend([
                {
                    event: "started",
                    operationId: 9,
                    snapshotId: 1,
                    generation: 1,
                    sections: [],
                    estimatedTotalRows: 0,
                },
                {
                    event: "completed",
                    operationId: 9,
                    totalRows: 0,
                    additions: 0,
                    deletions: 0,
                    durationMs: 0,
                },
            ]),
            <DiffViewer repoId={1} dispatch={() => {}} />
        );
        await settle();
        expect(view.container.textContent).toContain("No changes");
        view.unmount();
    });
});
