import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from "vitest";

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
import type { RepoStateAction } from "@/lib/routing/repo-search";
import { seedSettingsForTests } from "@/stores/settings-store";

const changeIconRenders = vi.hoisted(() => ({ count: 0 }));
vi.mock("@/components/diff/change-icon", () => ({
    ChangeIcon: () => {
        changeIconRenders.count += 1;
        return null;
    },
}));

/** Records dispatched repo-page actions so interactions can be asserted. */
function recordedActions(): {
    dispatch: (action: RepoStateAction) => void;
    actions: RepoStateAction[];
} {
    const actions: RepoStateAction[] = [];
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

function fakeBackend(events: DiffEvent[], message = "Fix the bar") {
    const withMessage = events.map((event) =>
        event.event === "started"
            ? {
                  ...event,
                  commitMessage: {
                      summaryLine: message.split("\n")[0],
                      message,
                  },
              }
            : event
    );
    const backend = {
        diff: {
            open: async (
                _repoId: number,
                _request: unknown,
                onEvent: (event: DiffEvent) => void
            ) => {
                queueMicrotask(() => {
                    for (const event of withMessage) onEvent(event);
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
            oldLineCount: 0,
            newLineCount: 0,
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
            oldLineCount: 0,
            newLineCount: 0,
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

function eventsForManyFiles(count: number): DiffEvent[] {
    return [
        {
            event: "started",
            operationId: 2,
            snapshotId: 1,
            generation: 1,
            sections: Array.from({ length: count }, (_, i) => ({
                sectionId: i,
                path: `src/file-${i}.txt`,
                kind: "modified" as const,
                binary: false,
                complete: false,
            })),
            estimatedTotalRows: 0,
        },
        {
            event: "sectionLayout",
            operationId: 2,
            sectionId: 0,
            startRow: 0,
            rowCount: 1,
            oldLineCount: 0,
            newLineCount: 1,
        },
        {
            event: "chunk",
            operationId: 2,
            sectionId: 0,
            rowStart: 0,
            rows: [row("addition", "hello")],
        },
        { event: "layoutReady", operationId: 2, totalRows: 0 },
        {
            event: "completed",
            operationId: 2,
            totalRows: 0,
            additions: 0,
            deletions: 0,
            durationMs: 0,
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
        changeIconRenders.count = 0;
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

    it("windows the file list when a commit changes many files", async () => {
        seedSettingsForTests({ diffViewMode: "unified" });
        const { dispatch } = recordedActions();
        const sectionCount = 60;
        const view = renderWith(
            fakeBackend(eventsForManyFiles(sectionCount)),
            <CommitDiffView repoId={1} commitId="big" dispatch={dispatch} />
        );
        await settle();

        const list = view.container.querySelector<HTMLUListElement>(
            '[data-slot="commit-file-list"]'
        )!;
        expect(list).not.toBeNull();

        const rendered = list.querySelectorAll("button").length;
        expect(rendered).toBeGreaterThan(0);
        expect(rendered).toBeLessThan(sectionCount);
        // The full scroll height is reserved even though offscreen rows are
        // not mounted.
        expect(list.style.height).toBe(`${sectionCount * 30}px`);
        expect(list.textContent).not.toContain("file-59.txt");
        view.unmount();
    });

    it("re-renders only the rows that scroll into the file list window", async () => {
        seedSettingsForTests({ diffViewMode: "unified" });
        const { dispatch } = recordedActions();
        const sectionCount = 60;
        const view = renderWith(
            fakeBackend(eventsForManyFiles(sectionCount)),
            <CommitDiffView repoId={1} commitId="big" dispatch={dispatch} />
        );
        await settle();

        const list = view.container.querySelector<HTMLUListElement>(
            '[data-slot="commit-file-list"]'
        )!;
        const viewport = view.container.querySelector<HTMLElement>(
            '[data-slot="scroll-area-viewport"]'
        )!;
        let scrollTop = 0;
        Object.defineProperty(viewport, "scrollTop", {
            get: () => scrollTop,
            set: (value: number) => {
                scrollTop = value;
                viewport.dispatchEvent(new Event("scroll"));
            },
            configurable: true,
        });

        changeIconRenders.count = 0;
        act(() => {
            viewport.scrollTop = 300;
        });
        await settle();

        // Rows 30..39 scroll into view; the 30 already-mounted rows must skip
        // re-rendering instead of rebuilding the whole window.
        expect(changeIconRenders.count).toBeGreaterThan(0);
        expect(changeIconRenders.count).toBeLessThan(20);
        expect(list.textContent).toContain("file-39.txt");
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

    it("clears the selection when a commit has no changes", async () => {
        seedSettingsForTests({ diffViewMode: "split" });
        const { dispatch, actions } = recordedActions();
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
            <CommitDiffView repoId={1} commitId="empty" dispatch={dispatch} />
        );
        await settle();
        expect(actions).toEqual([{ type: "CLEAR_SELECTION" }]);
        view.unmount();
    });

    it("shows the commit subject and its description from the commit detail", async () => {
        seedSettingsForTests({
            diffViewMode: "unified",
            commitFileListOpen: true,
        });
        const { dispatch } = recordedActions();
        const view = renderWith(
            fakeBackend(
                eventsForTwoFiles(),
                "Fix the section bar\n\nExplain the details."
            ),
            <CommitDiffView repoId={1} commitId="abc123" dispatch={dispatch} />
        );
        await settle();

        expect(view.container.textContent).toContain("Fix the section bar");
        expect(view.container.textContent).toContain("Explain the details.");
        expect(view.container.textContent).not.toContain("Commit Message");

        const expand = view.container.querySelector<HTMLButtonElement>(
            'button[aria-label="Show commit description"]'
        );
        expect(expand).not.toBeNull();
        act(() => expand!.click());
        await settle();
        expect(
            view.container.querySelector(
                'button[aria-label="Hide commit description"]'
            )
        ).not.toBeNull();
        view.unmount();
    });

    it("keeps the previous commit's diff painted while the next one opens", async () => {
        seedSettingsForTests({ diffViewMode: "unified" });
        const emitting = fakeBackend(eventsForTwoFiles());
        const silent = fakeBackend([]);
        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);
        const render = (backend: AppServices, commitId: string) => {
            act(() => {
                root.render(
                    <AppServicesContext.Provider value={backend}>
                        <CommitDiffView
                            repoId={1}
                            commitId={commitId}
                            dispatch={() => {}}
                        />
                    </AppServicesContext.Provider>
                );
            });
        };

        render(emitting, "abc123");
        await settle();
        expect(container.textContent).toContain("a-old");

        // The second commit's stream never emits; the first diff must stay
        // painted instead of blanking the pane.
        render(silent, "def456");
        expect(container.textContent).toContain("a-old");

        act(() => root.unmount());
        container.remove();
    });
});
