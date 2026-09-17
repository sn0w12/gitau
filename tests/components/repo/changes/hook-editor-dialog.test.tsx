// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { HookChecker } from "@/components/repo/changes/hook-checker";
import { HookEditorDialog } from "@/components/repo/changes/hook-editor-dialog";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    GitHook,
    HookContent,
    HookRunResult,
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

interface BackendFixture {
    backend: BackendClient;
    queryClient: QueryClient;
    readCalls: () => string[];
    writeCalls: () => { hook: string; content: string }[];
}

function backendWith({
    hooksList,
    contents,
}: {
    hooksList: GitHook[];
    contents: Record<string, string>;
}): BackendFixture {
    let list = hooksList;
    const readCalls: string[] = [];
    const writeCalls: { hook: string; content: string }[] = [];

    const read = async (
        _repoId: number,
        hook: string
    ): Promise<Result<HookContent>> => {
        readCalls.push(hook);
        const content = contents[hook] ?? "";
        const exists =
            content !== "" || list.some((item) => item.name === hook);
        return {
            ok: true,
            value: {
                hook,
                path: `/repo/.git/hooks/${hook}`,
                exists,
                content,
            },
        };
    };

    const write = async (
        _repoId: number,
        hook: string,
        content: string
    ): Promise<Result<void>> => {
        writeCalls.push({ hook, content });
        contents[hook] = content;
        if (!list.some((item) => item.name === hook)) {
            list = [
                ...list,
                {
                    name: hook,
                    path: `/repo/.git/hooks/${hook}`,
                    executable: true,
                },
            ];
        }
        return { ok: true, value: undefined };
    };

    const backend = {
        hooks: {
            list: async (): Promise<Result<GitHook[]>> => ({
                ok: true,
                value: list,
            }),
            run: async (): Promise<Result<HookRunResult>> => ({
                ok: true,
                value: hookResult(),
            }),
            read,
            write,
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        readCalls: () => readCalls,
        writeCalls: () => writeCalls,
    };
}

function renderWith(
    fixture: BackendFixture,
    ui: ReactElement
): { container: HTMLDivElement; unmount: () => void } {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={fixture}>
                <QueryClientProvider client={fixture.queryClient}>
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

describe("HookEditorDialog", () => {
    beforeEach(() => {
        document.body.innerHTML = "";
    });

    it("opens from the popover's edit button and loads the active hook", async () => {
        const fixture = backendWith({
            hooksList: [PRE_COMMIT],
            contents: { "pre-commit": "#!/bin/sh\necho lint\n" },
        });
        const view = renderWith(
            fixture,
            <HookChecker repoId={7} naturalRuns={[]} />
        );
        await flush();

        await click(
            view.container.querySelector<HTMLButtonElement>(
                '[aria-label="Pre-commit hooks"]'
            )!
        );
        await waitFor(
            () =>
                document.querySelector<HTMLButtonElement>(
                    '[aria-label="Edit commit hooks"]'
                ) != null
        );
        await click(
            document.querySelector<HTMLButtonElement>(
                '[aria-label="Edit commit hooks"]'
            )!
        );

        const opened = await waitFor(
            () =>
                document.querySelector('[data-testid="hook-editor-dialog"]') !=
                null
        );
        expect(opened).toBe(true);
        await flush();

        const textarea = document.querySelector<HTMLTextAreaElement>(
            "#hook-editor-content"
        );
        expect(textarea?.value).toBe("#!/bin/sh\necho lint\n");
        // All standard hooks are read up front so switching never reloads.
        expect(fixture.readCalls()).toEqual([
            "pre-commit",
            "prepare-commit-msg",
            "commit-msg",
            "post-commit",
        ]);
        view.unmount();
    });

    it("switches hooks and loads their content on select", async () => {
        const fixture = backendWith({
            hooksList: [PRE_COMMIT],
            contents: {
                "pre-commit": "#!/bin/sh\necho lint\n",
                "commit-msg": "#!/bin/sh\necho check message\n",
            },
        });
        const view = renderWith(
            fixture,
            <HookEditorDialog
                repoId={7}
                hooks={[PRE_COMMIT]}
                open
                onClose={() => {}}
            />
        );
        await flush();
        expect(
            document.querySelector<HTMLTextAreaElement>("#hook-editor-content")
                ?.value
        ).toBe("#!/bin/sh\necho lint\n");

        await click(
            [...document.querySelectorAll("button")].find(
                (button) => button.textContent?.trim() === "commit-msg"
            )!
        );
        // Content is already cached from the open-time batch read, so the
        // textarea stays mounted and switching never flashes a loader.
        const afterSwitch = document.querySelector<HTMLTextAreaElement>(
            "#hook-editor-content"
        );
        expect(afterSwitch?.value).toBe("#!/bin/sh\necho check message\n");
        expect(fixture.readCalls()).toEqual([
            "pre-commit",
            "prepare-commit-msg",
            "commit-msg",
            "post-commit",
        ]);
        view.unmount();
    });

    it("saves edits, marks the hook installed, and refreshes the popover list", async () => {
        const fixture = backendWith({
            hooksList: [],
            contents: {},
        });
        const view = renderWith(
            fixture,
            <HookChecker repoId={7} naturalRuns={[]} />
        );
        await flush();
        await click(
            view.container.querySelector<HTMLButtonElement>(
                '[aria-label="Pre-commit hooks"]'
            )!
        );
        await waitFor(
            () =>
                document.querySelector<HTMLButtonElement>(
                    '[aria-label="Edit commit hooks"]'
                ) != null
        );
        await click(
            document.querySelector<HTMLButtonElement>(
                '[aria-label="Edit commit hooks"]'
            )!
        );
        await waitFor(
            () =>
                document.querySelector<HTMLTextAreaElement>(
                    "#hook-editor-content"
                ) != null
        );

        const save = document.querySelector<HTMLButtonElement>(
            '[data-slot="dialog-footer"] button:last-child'
        );
        expect(save?.hasAttribute("disabled")).toBe(true);

        const textarea = document.querySelector<HTMLTextAreaElement>(
            "#hook-editor-content"
        );
        await act(async () => {
            // React tracks controlled values; assign through the native
            // setter so the change is observed like real typing.
            const setter = Object.getOwnPropertyDescriptor(
                window.HTMLTextAreaElement.prototype,
                "value"
            )!.set!;
            setter.call(textarea, "#!/bin/sh\necho hi\n");
            textarea!.dispatchEvent(new Event("input", { bubbles: true }));
            await new Promise((resolve) => setTimeout(resolve, 10));
        });
        await click(
            document.querySelector<HTMLButtonElement>(
                '[data-slot="dialog-footer"] button:last-child'
            )!
        );
        // Let the hooks-list invalidation refetch settle inside act.
        await flush();

        expect(fixture.writeCalls()).toEqual([
            { hook: "pre-commit", content: "#!/bin/sh\necho hi\n" },
        ]);
        // The picker row now shows the installed glyph and the popover list
        // refetched after invalidation.
        const installed = await waitFor(
            () =>
                document.querySelector(
                    '[data-testid="hook-picker-pre-commit-installed"]'
                ) != null
        );
        expect(installed).toBe(true);
        expect(
            [...document.querySelectorAll("li button")].some((el) =>
                el.textContent!.includes("pre-commit")
            )
        ).toBe(true);
        view.unmount();
    });
});
