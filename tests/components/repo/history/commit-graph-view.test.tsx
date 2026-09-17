// @vitest-environment jsdom
import { QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it, beforeEach } from "vitest";

import { CommitGraphView } from "@/components/repo/history/commit-graph-view";
import { AppServicesContext } from "@/contexts/services-context";
import { TabContext } from "@/contexts/tab-context";
import type { GraphEvent, GraphRow } from "@/lib/backend/protocol";
import { graphSessionRegistry } from "@/lib/backend/streams/graph-session-registry";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import type { AppServices } from "@/lib/bootstrap/app-runtime";

const globalScope = globalThis as Record<string, unknown>;
globalScope.ResizeObserver ??= class {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
};
// Base UI's scroll-area viewport probes animations after mount.
(Element.prototype as unknown as Record<string, unknown>).getAnimations ??=
    () => [];

function row(index: number, overrides: Partial<GraphRow> = {}): GraphRow {
    return {
        index,
        id: `aabbccddeeff00112233445566778899aabbccd${index.toString(36)}`,
        lane: 0,
        edges: [],
        kind: "commit",
        summaryLine: `Commit ${index}`,
        authorName: "Ada",
        authorEmail: "ada@example.com",
        timeSeconds: 1_700_000_000,
        tags: [],
        refs: [],
        ...overrides,
    };
}

function makeServices(): AppServices & {
    emit: (event: GraphEvent) => void;
    listeners: Array<(event: GraphEvent) => void>;
} {
    const listeners: Array<(event: GraphEvent) => void> = [];
    let seq = 0;
    const backend = {
        graph: {
            open: async (
                _repoId: number,
                _query: unknown,
                onEvent: (event: GraphEvent) => void
            ): Promise<Result<number>> => {
                listeners.push(onEvent);
                return { ok: true, value: ++seq };
            },
            readRange: async (): Promise<Result<never>> => {
                throw new Error("no gap expected");
            },
            cancel: async () => ({ ok: true, value: true }),
        },
    } as unknown as BackendClient;
    return {
        backend,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        listeners,
        emit: (event) => {
            const listener = listeners.at(-1);
            if (listener) listener(event);
        },
    };
}

function renderWith(services: AppServices, ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <TabContext.Provider value="tab-1">{ui}</TabContext.Provider>
            </AppServicesContext.Provider>
        );
    });
    return {
        container,
        unmount: () => act(() => root.unmount()),
    };
}

const flushMicrotasks = async () => {
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
    });
};

beforeEach(() => {
    graphSessionRegistry.resetForTests();
});

describe("CommitGraphView", () => {
    it("renders streamed rows at fixed positions and dispatches selection", async () => {
        const services = makeServices();
        let selected: string | null = null;
        const view = renderWith(
            services,
            <CommitGraphView
                repoId={1}
                selectedId={null}
                onSelect={(id) => (selected = id)}
            />
        );
        await flushMicrotasks();

        const op = services.listeners.length; // latest operation id
        act(() => {
            services.emit({
                event: "started",
                operationId: op,
                snapshotId: 1,
                generation: 1,
            });
            services.emit({
                event: "chunk",
                operationId: op,
                rowStart: 0,
                rows: [
                    row(0, { refs: ["main"], lane: 0 }),
                    row(1, { lane: 1 }),
                ],
            });
            services.emit({
                event: "completed",
                operationId: op,
                totalRows: 2,
            });
        });
        await flushMicrotasks();

        const rowsEl = view.container.querySelectorAll(
            '[data-slot="graph-row"]'
        );
        expect(rowsEl.length).toBeGreaterThan(0);
        expect(view.container.textContent).toContain("Commit 0");
        expect(view.container.textContent).toContain("Commit 1");
        // Branch decoration pill renders.
        expect(view.container.textContent).toContain("main");
        // The SVG gutter is present (edges + nodes).
        expect(view.container.querySelector("svg")).not.toBeNull();

        // Clicking a row reports its commit id.
        const first = rowsEl[0] as HTMLElement;
        act(() =>
            first.dispatchEvent(new MouseEvent("click", { bubbles: true }))
        );
        expect(selected).toBe("aabbccddeeff00112233445566778899aabbccd0");
        view.unmount();
    });

    it("shows the empty state for a completed stream with no rows", async () => {
        const services = makeServices();
        const view = renderWith(
            services,
            <CommitGraphView repoId={2} selectedId={null} onSelect={() => {}} />
        );
        await flushMicrotasks();
        const op = services.listeners.length;
        act(() => {
            services.emit({
                event: "started",
                operationId: op,
                snapshotId: 1,
                generation: 1,
            });
            services.emit({
                event: "completed",
                operationId: op,
                totalRows: 0,
            });
        });
        await flushMicrotasks();
        expect(
            view.container.querySelector('[data-slot="graph-empty"]')
        ).not.toBeNull();
        view.unmount();
    });
});
