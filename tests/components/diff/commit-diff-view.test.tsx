import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";

import { CommitDiffView } from "@/components/diff/commit-diff-view";
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

/** Records dispatched repo-page actions so interactions can be asserted. */
function recordedActions(): {
    dispatch: (action: reducerAction) => void;
    actions: reducerAction[];
} {
    const actions: reducerAction[] = [];
    return {
        dispatch: (action) => {
            actions.push(action);
        },
        actions,
    };
}

function row(kind: DiffRowKind, content: string): DiffRow {
    return { kind, content };
}

function fakeBackend(events: DiffEvent[]) {
    const backend = {
        diff: {
            open: async (
                _repoId: number,
                _request: unknown,
                onEvent: (event: DiffEvent) => void
            ) => {
                queueMicrotask(() => {
                    for (const event of events) onEvent(event);
                });
                return { ok: true as const, value: 1 };
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
        },
    };
    return { backend } as unknown as AppServices;
}

function eventsForTwoFiles(): DiffEvent[] {
    return [
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
                {
                    sectionId: 1,
                    path: "src/b.txt",
                    kind: "added",
                    binary: false,
                    complete: false,
                },
            ],
            estimatedTotalRows: 6,
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
                row("fileHeader", "diff --git a/src/a.txt b/src/a.txt"),
                row("deletion", "a-old"),
            ],
        },
        {
            event: "sectionLayout",
            operationId: 1,
            sectionId: 1,
            startRow: 2,
            rowCount: 3,
        },
        {
            event: "chunk",
            operationId: 1,
            sectionId: 1,
            rowStart: 2,
            rows: [
                row("fileHeader", "diff --git a/src/b.txt b/src/b.txt"),
                row("addition", "b-new-1"),
                row("addition", "b-new-2"),
            ],
        },
        { event: "layoutReady", operationId: 1, totalRows: 5 },
        {
            event: "completed",
            operationId: 1,
            totalRows: 5,
            additions: 2,
            deletions: 1,
            durationMs: 2,
        },
    ];
}

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

describe("CommitDiffView", () => {
    beforeEach(() => {
        diffSessionRegistry.resetForTests();
    });

    it("lists every file of the commit and shows the active file's diff", async () => {
        seedSettingsForTests({ diffViewMode: "unified" });
        const { dispatch } = recordedActions();
        const view = renderWith(
            fakeBackend(eventsForTwoFiles()),
            <CommitDiffView repoId={1} commitId="abc123" dispatch={dispatch} />
        );
        await settle();

        const list = view.container.querySelector(
            '[data-slot="commit-file-list"]'
        );
        expect(list?.textContent).toContain("src/a.txt");
        expect(list?.textContent).toContain("src/b.txt");

        // Active file defaults to the first section; its rows are visible.
        expect(view.container.textContent).toContain("a-old");

        // Selecting the second file switches the rendered diff.
        const buttons = [
            ...list!.querySelectorAll<HTMLButtonElement>("button"),
        ];
        act(() => buttons[1].click());
        await settle();
        await expect
            .poll(() => view.container.textContent)
            .toContain("b-new-2");
        expect(view.container.textContent).not.toContain("a-old");
        view.unmount();
    });

    it("clears the selection when the section bar close button is clicked", async () => {
        seedSettingsForTests({ diffViewMode: "unified" });
        const { dispatch, actions } = recordedActions();
        const view = renderWith(
            fakeBackend(eventsForTwoFiles()),
            <CommitDiffView repoId={1} commitId="abc123" dispatch={dispatch} />
        );
        await settle();

        // The section bar's last button is the X (close) control.
        const bar = view.container.querySelector("div.border-b")!;
        expect(bar).not.toBeNull();
        const closeButton =
            bar.querySelectorAll<HTMLButtonElement>("button")[
                bar.querySelectorAll("button").length - 1
            ];
        act(() => closeButton.click());
        await settle();

        expect(actions).toEqual([{ type: "CLEAR_SELECTION" }]);
        view.unmount();
    });

    it("shows an empty state for a commit without changes", async () => {
        seedSettingsForTests({ diffViewMode: "split" });
        const view = renderWith(
            fakeBackend([
                {
                    event: "started",
                    operationId: 5,
                    snapshotId: 1,
                    generation: 1,
                    sections: [],
                    estimatedTotalRows: 0,
                },
                {
                    event: "completed",
                    operationId: 5,
                    totalRows: 0,
                    additions: 0,
                    deletions: 0,
                    durationMs: 0,
                },
            ]),
            <CommitDiffView repoId={1} commitId="empty" dispatch={() => {}} />
        );
        await settle();
        expect(view.container.textContent).toContain(
            "No changes in this commit"
        );
        view.unmount();
    });
});
