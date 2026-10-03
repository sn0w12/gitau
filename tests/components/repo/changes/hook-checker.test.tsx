// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { HookChecker } from "@/components/repo/changes/hook-checker";
import { AppServicesContext } from "@/contexts/services-context";
import {
    useCommitHookRunner,
    type HookRunner,
} from "@/hooks/repositories/use-commit-hooks";
import type {
    CommitHookEvent,
    GitHook,
    HookOutputChunk,
    HookRunResult,
    SyntaxStyle,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const PRE_COMMIT: GitHook = {
    name: "pre-commit",
    path: "/repo/.git/hooks/pre-commit",
    executable: true,
};

const POST_COMMIT: GitHook = {
    name: "post-commit",
    path: "/repo/.git/hooks/post-commit",
    executable: true,
};

const RED: SyntaxStyle = {
    light: "#cf222e",
    dark: "#ff7b72",
};

function hookResult(overrides: Partial<HookRunResult> = {}): HookRunResult {
    return {
        hook: "pre-commit",
        exitCode: 0,
        success: true,
        stdout: "",
        stderr: "",
        durationMs: 12,
        lines: [],
        styles: [],
        ...overrides,
    };
}

/** A run as the backend reports it: the lines it streamed, then the same
 * lines parsed again in the result it resolves with. A line that interned no
 * style carries no `styles` key at all, which is what the wire does. */
function runOf(
    lines: { text: string; spans: number[] }[],
    overrides: Partial<HookRunResult> = {}
): {
    chunks: HookOutputChunk[];
    outcome: Result<HookRunResult>;
} {
    const interned = lines.some((line) => line.spans.length > 0) ? [RED] : [];
    return {
        chunks: lines.map((line) =>
            line.spans.length > 0 ? { ...line, styles: interned } : { ...line }
        ),
        outcome: {
            ok: true,
            value: hookResult({
                lines,
                styles: interned,
                ...overrides,
            }),
        },
    };
}

interface RunPlan {
    chunks: HookOutputChunk[];
    outcome: Result<HookRunResult>;
}

function backendWith(hooksList: GitHook[]) {
    let plan: RunPlan = {
        chunks: [],
        outcome: { ok: true, value: hookResult() },
    };
    let gate: Promise<void> = Promise.resolve();
    let open = (): void => {};
    let deliver: (chunk: HookOutputChunk) => void = () => {};
    const runCalls: string[] = [];

    const backend = {
        hooks: {
            list: async (): Promise<Result<GitHook[]>> => ({
                ok: true,
                value: hooksList,
            }),
            runStreamed: async (
                _repoId: number,
                hook: string,
                onLine: (chunk: HookOutputChunk) => void
            ): Promise<Result<HookRunResult>> => {
                runCalls.push(hook);
                deliver = onLine;
                for (const chunk of plan.chunks) onLine(chunk);
                await gate;
                return plan.outcome;
            },
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        setPlan(next: RunPlan) {
            plan = next;
        },
        /** Keeps the run in flight so the streamed state can be inspected. */
        holdRun(): () => void {
            gate = new Promise<void>((resolve) => {
                open = resolve;
            });
            return () => open();
        },
        /** Hands one more line to the running hook's channel. */
        emit(chunk: HookOutputChunk): void {
            deliver(chunk);
        },
        runCalls: () => runCalls,
    };
}

type Services = ReturnType<typeof backendWith>;

/** The checker over a real runner, wired the way the commit form wires it,
 * so a test can drive either producer: the run buttons or the events a
 * commit reports. */
function Harness({ repoId }: { repoId: number }) {
    const runner = useCommitHookRunner(repoId);
    useEffect(() => {
        capture.runner = runner;
    }, [runner]);
    return <HookChecker repoId={repoId} runner={runner} />;
}

/** Replaced on every render; `act` flushes the effect that writes it, so a
 * test holds a runner bound to the state the checker renders. */
const capture: { runner: HookRunner | null } = { runner: null };

function renderChecker(services: Services, repoId = 7) {
    capture.runner = null;
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    <Harness repoId={repoId} />
                </QueryClientProvider>
            </AppServicesContext.Provider>
        );
    });
    return {
        container,
        /** One event off a commit's hook channel. */
        report(event: CommitHookEvent) {
            act(() => {
                capture.runner?.reportCommit(event);
            });
        },
        unmount() {
            act(() => root.unmount());
            container.remove();
        },
    };
}

async function flush(): Promise<void> {
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
    });
}

async function click(element: Element): Promise<void> {
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 10));
    });
}

async function waitFor(predicate: () => boolean): Promise<boolean> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    return predicate();
}

function openChecker(view: ReturnType<typeof renderChecker>) {
    return click(
        view.container.querySelector<HTMLButtonElement>(
            '[aria-label="Pre-commit hooks"]'
        )!
    );
}

function runButton(hook: string): HTMLButtonElement {
    return document.querySelector<HTMLButtonElement>(
        `[aria-label="Run ${hook}"]`
    )!;
}

function rowTrigger(hook: string): HTMLButtonElement {
    return [...document.querySelectorAll("li button")].find(
        (element) =>
            element.textContent!.includes(hook) &&
            !element.getAttribute("aria-label")?.startsWith("Run")
    ) as HTMLButtonElement;
}

function rowText(hook: string): string {
    return rowTrigger(hook).closest("li")?.textContent ?? "";
}

function output(): Element | null {
    return document.querySelector('[data-testid="hook-checker-output"]');
}

/** jsdom reports every scroll metric as zero, so the viewport's geometry is
 * stubbed here to drive the tail-following. Scroll positions clamp like a real
 * viewport's, and a scroll event fires on every move. `lateScrollEvents` queues
 * those events to a later task, the way a browser dispatches them on its own
 * frame rather than the one that moved the viewport. */
function stubViewport(
    content: number,
    { lateScrollEvents = false }: { lateScrollEvents?: boolean } = {}
) {
    const viewport = document.querySelector<HTMLElement>(
        '[data-slot="scroll-area-viewport"]'
    )!;
    const client = 40;
    let height = content;
    let scrollTop = 0;
    const clamp = (value: number) =>
        Math.max(0, Math.min(value, Math.max(0, height - client)));
    const notify = () =>
        lateScrollEvents
            ? setTimeout(() => viewport.dispatchEvent(new Event("scroll")), 0)
            : viewport.dispatchEvent(new Event("scroll"));
    Object.defineProperty(viewport, "clientHeight", {
        get: () => client,
        configurable: true,
    });
    Object.defineProperty(viewport, "scrollHeight", {
        get: () => height,
        configurable: true,
    });
    Object.defineProperty(viewport, "scrollTop", {
        get: () => scrollTop,
        set: (value: number) => {
            scrollTop = clamp(value);
            notify();
        },
        configurable: true,
    });
    return {
        grow(to: number) {
            height = to;
        },
        scrollTo(value: number) {
            scrollTop = clamp(value);
            notify();
        },
        /** The furthest down the viewport goes, which is the tail. */
        tail: () => Math.max(0, height - client),
        position: () => scrollTop,
    };
}

describe("HookChecker", () => {
    beforeEach(() => {
        document.body.innerHTML = "";
    });

    it("renders an empty state when no hooks exist", async () => {
        const services = backendWith([]);
        const view = renderChecker(services);
        await flush();
        await openChecker(view);

        const shown = await waitFor(
            () =>
                document.querySelector('[data-testid="hook-checker-empty"]') !=
                null
        );
        expect(shown).toBe(true);
        view.unmount();
    });

    it("lists discovered hooks and streams output into the row", async () => {
        const services = backendWith([PRE_COMMIT]);
        const plan = runOf([
            { text: "Checking formatting", spans: [] },
            { text: "formatted 2 files", spans: [] },
        ]);
        services.setPlan(plan);
        const release = services.holdRun();

        const view = renderChecker(services);
        await flush();
        await openChecker(view);

        const listed = await waitFor(() => runButton("pre-commit") != null);
        expect(listed).toBe(true);
        await click(runButton("pre-commit"));

        // Both lines are on screen while the hook is still running.
        expect(output()?.textContent).toContain("Checking formatting");
        expect(output()?.textContent).toContain("formatted 2 files");
        expect(services.runCalls()).toEqual(["pre-commit"]);

        await act(async () => {
            release();
            await new Promise((resolve) => setTimeout(resolve, 20));
        });

        // A run that passes folds its output away once it settles, the way a
        // passing CI step does, and the dot reports the pass.
        expect(output()).toBeNull();
        const dot = document.querySelector(
            '[data-testid="hook-checker-status-dot"]'
        );
        expect(dot?.className).toContain("bg-success");
        view.unmount();
    });

    it("collapses and expands a row's output on demand", async () => {
        const services = backendWith([PRE_COMMIT]);
        services.setPlan(runOf([{ text: "formatted 2 files", spans: [] }]));
        const view = renderChecker(services);
        await flush();
        await openChecker(view);
        await waitFor(() => runButton("pre-commit") != null);

        // A passing run leaves its output collapsed.
        await click(runButton("pre-commit"));
        await flush();
        expect(output()).toBeNull();

        await click(rowTrigger("pre-commit"));
        expect(output()?.textContent).toContain("formatted 2 files");

        await click(rowTrigger("pre-commit"));
        expect(output()).toBeNull();
        view.unmount();
    });

    it("opens a failing row's output without being asked", async () => {
        const services = backendWith([PRE_COMMIT]);
        services.setPlan(
            runOf([{ text: "lint found problems", spans: [] }], {
                success: false,
                exitCode: 3,
            })
        );
        const view = renderChecker(services);
        await flush();
        await openChecker(view);
        await waitFor(() => runButton("pre-commit") != null);

        await click(runButton("pre-commit"));
        await flush();

        expect(output()?.textContent).toContain("lint found problems");
        const dot = document.querySelector(
            '[data-testid="hook-checker-status-dot"]'
        );
        expect(dot?.className).toContain("bg-destructive");
        view.unmount();
    });

    it("renders the colours the hook wrote", async () => {
        const services = backendWith([PRE_COMMIT]);
        services.setPlan(
            runOf([
                { text: "src/app.ts", spans: [] },
                { text: "2 problems", spans: [0, 10, 1] },
            ])
        );
        const view = renderChecker(services);
        await flush();
        await openChecker(view);
        await waitFor(() => runButton("pre-commit") != null);

        await click(runButton("pre-commit"));
        await flush();
        await click(rowTrigger("pre-commit"));

        const styled = output()!.querySelector("span.syn");
        expect(styled?.textContent).toBe("2 problems");
        expect(styled?.getAttribute("style")).toContain("#cf222e");
        view.unmount();
    });

    it("follows the tail of a running hook but not a reader who scrolled back", async () => {
        const services = backendWith([PRE_COMMIT]);
        const release = services.holdRun();
        const view = renderChecker(services);
        await flush();
        await openChecker(view);
        await waitFor(() => runButton("pre-commit") != null);

        await click(runButton("pre-commit"));
        const viewport = stubViewport(40);

        // Output grows past the viewport: the tail comes into view on its own.
        await act(async () => {
            viewport.grow(120);
            services.emit({ text: "checking", spans: [] });
            await new Promise((resolve) => setTimeout(resolve, 10));
        });
        expect(viewport.position()).toBe(viewport.tail());

        // Scrolled back to read the first line, the stream leaves it alone.
        viewport.scrollTo(0);
        await act(async () => {
            viewport.grow(200);
            services.emit({ text: "still checking", spans: [] });
            await new Promise((resolve) => setTimeout(resolve, 10));
        });
        expect(viewport.position()).toBe(0);

        // Back at the tail, it follows again.
        viewport.scrollTo(viewport.tail());
        await act(async () => {
            viewport.grow(240);
            services.emit({ text: "formatted 2 files", spans: [] });
            await new Promise((resolve) => setTimeout(resolve, 10));
        });
        expect(viewport.position()).toBe(viewport.tail());

        // Settling the run does not yank the view either.
        viewport.scrollTo(60);
        await act(async () => {
            release();
            await new Promise((resolve) => setTimeout(resolve, 20));
        });
        expect(viewport.position()).toBe(60);
        view.unmount();
    });

    it("keeps following when output outruns the scroll events it raises", async () => {
        const services = backendWith([PRE_COMMIT]);
        services.setPlan({
            chunks: [],
            outcome: {
                ok: true,
                value: hookResult({
                    success: false,
                    exitCode: 3,
                    lines: [
                        { text: "checking", spans: [] },
                        { text: "still checking", spans: [] },
                        { text: "formatted 2 files", spans: [] },
                    ],
                }),
            },
        });
        const release = services.holdRun();
        const view = renderChecker(services);
        await flush();
        await openChecker(view);
        await waitFor(() => runButton("pre-commit") != null);

        await click(runButton("pre-commit"));
        const viewport = stubViewport(40, { lateScrollEvents: true });

        // Output still fits the panel, so there is nothing to follow yet.
        await act(async () => {
            viewport.grow(40);
            services.emit({ text: "checking", spans: [] });
            await new Promise((resolve) => setTimeout(resolve, 10));
        });
        expect(viewport.position()).toBe(0);

        // The next line crosses the max height and the viewport starts
        // scrolling, then a burst of output lands before the browser gets
        // around to dispatching the scroll event that follow raised. The
        // tail has moved on, and the follow must not read as the reader
        // having scrolled away from it.
        await act(async () => {
            viewport.grow(400);
            services.emit({ text: "still checking", spans: [] });
            viewport.grow(900);
            await new Promise((resolve) => setTimeout(resolve, 10));
            services.emit({ text: "formatted 2 files", spans: [] });
            await new Promise((resolve) => setTimeout(resolve, 10));
        });
        expect(viewport.position()).toBe(viewport.tail());

        // Settling on the failed run leaves the tail where it was.
        await act(async () => {
            release();
            await new Promise((resolve) => setTimeout(resolve, 20));
        });
        expect(output()?.textContent).toContain("formatted 2 files");
        expect(viewport.position()).toBe(viewport.tail());
        view.unmount();
    });

    it("shows a hook the commit pipeline is running", async () => {
        const services = backendWith([PRE_COMMIT, POST_COMMIT]);
        const view = renderChecker(services);
        await flush();
        await openChecker(view);
        await waitFor(() => runButton("post-commit") != null);

        // A commit reports the hook it is about to run, before it has said
        // anything. The row shows it as running, with its output open.
        view.report({ type: "started", hook: "pre-commit" });
        expect(services.runCalls()).toEqual([]);
        expect(rowText("pre-commit")).toContain("pre-commit");
        expect(output()?.textContent).toContain("running");
        // Nothing has settled, so the trigger has no verdict to report yet.
        expect(
            document.querySelector('[data-testid="hook-checker-status-dot"]')
        ).toBeNull();
        expect(runButton("pre-commit").disabled).toBe(true);

        view.report({
            type: "line",
            hook: "pre-commit",
            text: "Checking formatting",
            spans: [],
        });
        expect(output()?.textContent).toContain("Checking formatting");

        // Settling it folds the output away and turns the dot green.
        view.report({
            type: "settled",
            result: hookResult({ hook: "pre-commit", durationMs: 40 }),
        });
        expect(output()).toBeNull();
        const dot = document.querySelector(
            '[data-testid="hook-checker-status-dot"]'
        );
        expect(dot?.className).toContain("bg-success");
        view.unmount();
    });

    it("keeps a failing commit's hook output, which the error alone drops", async () => {
        const services = backendWith([PRE_COMMIT]);
        const view = renderChecker(services);
        await flush();
        await openChecker(view);
        await waitFor(() => runButton("pre-commit") != null);

        // A blocked commit resolves as an error with the output flattened into
        // its details, so the streamed lines are all the reader is left with.
        view.report({ type: "started", hook: "pre-commit" });
        view.report({
            type: "line",
            hook: "pre-commit",
            text: "lint found problems",
            spans: [],
        });
        view.report({
            type: "settled",
            result: hookResult({
                hook: "pre-commit",
                success: false,
                exitCode: 3,
                durationMs: 90,
                lines: [{ text: "lint found problems", spans: [] }],
            }),
        });

        // A failed hook stays open, showing what it printed.
        expect(output()?.textContent).toContain("lint found problems");
        const dot = document.querySelector(
            '[data-testid="hook-checker-status-dot"]'
        );
        expect(dot?.className).toContain("bg-destructive");
        expect(
            view.container.querySelector<HTMLElement>(
                '[aria-label="Pre-commit hooks"]'
            )?.dataset.failed
        ).toBe("true");
        view.unmount();
    });
});
