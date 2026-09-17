// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { HookChecker } from "@/components/repo/changes/hook-checker";
import { AppServicesContext } from "@/contexts/services-context";
import type { GitHook, HookRunResult } from "@/lib/backend/protocol";
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

function hookResult(overrides: Partial<HookRunResult> = {}): HookRunResult {
    return {
        hook: "pre-commit",
        exitCode: 0,
        success: true,
        stdout: "",
        stderr: "",
        durationMs: 12,
        ...overrides,
    };
}

function backendWith(hooksList: GitHook[]) {
    let nextRun: Result<HookRunResult> = { ok: true, value: hookResult() };
    const runCalls: string[] = [];

    const backend = {
        hooks: {
            list: async (): Promise<Result<GitHook[]>> => ({
                ok: true,
                value: hooksList,
            }),
            run: async (
                _repoId: number,
                hook: string
            ): Promise<Result<HookRunResult>> => {
                runCalls.push(hook);
                return nextRun;
            },
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        setNextRun(outcome: Result<HookRunResult>) {
            nextRun = outcome;
        },
        runCalls: () => runCalls,
    };
}

type Services = ReturnType<typeof backendWith>;

function renderWith(services: Services, ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    {ui}
                </QueryClientProvider>
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

function openChecker(view: ReturnType<typeof renderWith>) {
    return click(
        view.container.querySelector<HTMLButtonElement>(
            '[aria-label="Pre-commit hooks"]'
        )!
    );
}

describe("HookChecker", () => {
    beforeEach(() => {
        document.body.innerHTML = "";
    });

    it("renders an empty state when no hooks exist", async () => {
        const services = backendWith([]);
        const view = renderWith(services, <HookChecker repoId={7} />);
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

    it("lists discovered hooks and shows captured output after a run", async () => {
        const services = backendWith([PRE_COMMIT]);
        services.setNextRun({
            ok: true,
            value: hookResult({ stdout: "formatted 2 files\n" }),
        });
        const view = renderWith(services, <HookChecker repoId={7} />);
        await flush();
        await openChecker(view);

        const listed = await waitFor(() =>
            [...document.querySelectorAll("li button")].some((el) =>
                el.textContent!.includes("pre-commit")
            )
        );
        expect(listed).toBe(true);

        const play = await waitFor(
            () =>
                document.querySelector<HTMLButtonElement>(
                    '[aria-label="Run pre-commit"]'
                ) != null
        );
        expect(play).toBe(true);
        await click(
            document.querySelector<HTMLButtonElement>(
                '[aria-label="Run pre-commit"]'
            )!
        );

        const shown = await waitFor(
            () =>
                document
                    .querySelector('[data-testid="hook-checker-output"]')
                    ?.textContent!.includes("formatted 2 files") === true
        );
        expect(shown).toBe(true);
        expect(services.runCalls()).toEqual(["pre-commit"]);
        // Successful run lights the success dot, not the failure one.
        const dot = document.querySelector(
            '[data-testid="hook-checker-status-dot"]'
        );
        expect(dot?.className).toContain("bg-success");
        view.unmount();
    });

    it("marks failing runs with their exit code", async () => {
        const services = backendWith([PRE_COMMIT]);
        services.setNextRun({
            ok: true,
            value: hookResult({
                success: false,
                exitCode: 3,
                stderr: "lint found problems",
            }),
        });
        const view = renderWith(services, <HookChecker repoId={7} />);
        await flush();
        await openChecker(view);
        await waitFor(
            () =>
                document.querySelector<HTMLButtonElement>(
                    '[aria-label="Run pre-commit"]'
                ) != null
        );

        await click(
            document.querySelector<HTMLButtonElement>(
                '[aria-label="Run pre-commit"]'
            )!
        );

        const shown = await waitFor(
            () =>
                document
                    .querySelector('[data-testid="hook-checker-output"]')
                    ?.textContent!.includes("lint found problems") === true
        );
        expect(shown).toBe(true);
        expect(document.body.textContent).toContain("exit 3");
        const dot = document.querySelector(
            '[data-testid="hook-checker-status-dot"]'
        );
        expect(dot?.className).toContain("bg-destructive");
        view.unmount();
    });

    it("mirrors results from a real commit without a manual run", async () => {
        const services = backendWith([
            PRE_COMMIT,
            {
                name: "post-commit",
                path: "/repo/.git/hooks/post-commit",
                executable: true,
            },
        ]);
        const view = renderWith(
            services,
            <HookChecker
                repoId={7}
                naturalRuns={[
                    hookResult({ hook: "pre-commit", durationMs: 40 }),
                    hookResult({
                        hook: "post-commit",
                        durationMs: 3,
                        stdout: "post-ran\n",
                    }),
                ]}
            />
        );
        await flush();
        await openChecker(view);

        // Both natural runs appear as settled rows with no manual clicks.
        const bothShown = await waitFor(() =>
            ["pre-commit", "post-commit"].every((name) =>
                [...document.querySelectorAll("li button")].some((el) =>
                    el.textContent!.includes(name)
                )
            )
        );
        expect(bothShown).toBe(true);
        expect(services.runCalls()).toEqual([]);
        // Latest entry is the default output pane selection.
        expect(
            document.querySelector('[data-testid="hook-checker-output"]')
                ?.textContent
        ).toContain("post-ran");
        const dot = document.querySelector(
            '[data-testid="hook-checker-status-dot"]'
        );
        expect(dot?.className).toContain("bg-success");
        view.unmount();
    });
});
