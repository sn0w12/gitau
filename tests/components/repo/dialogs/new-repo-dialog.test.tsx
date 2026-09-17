// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { NewRepoDialog } from "@/components/repo/dialogs/new-repo-dialog";
import { ToastProvider } from "@/components/ui/toast";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    CreateRepositoryResult,
    GitignoreTemplateInfo,
    LicenseTemplateInfo,
    RepoSnapshot,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import { getSetting, seedSettingsForTests } from "@/stores/settings-store";

vi.mock("@tauri-apps/plugin-dialog", () => ({
    open: vi.fn(async () => "/repo-parent"),
}));

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const TEMPLATES: GitignoreTemplateInfo[] = [{ id: "rust", label: "Rust" }];
const LICENSES: LicenseTemplateInfo[] = [
    {
        id: "mit",
        name: "MIT License",
        description: "Permissive, keep copyright and license notice",
    },
];

function created(path = "/repo-parent/demo"): Result<CreateRepositoryResult> {
    const snapshot: RepoSnapshot = {
        id: 9,
        generation: 1,
        workdir: path,
        gitDir: `${path}/.git`,
        head: { state: "unborn", branch: "main" },
        shaKind: "sha1",
    };
    return { ok: true, value: { id: 9, path, snapshot } };
}

function backendWith() {
    const createCalls: Record<string, unknown>[] = [];
    let createOutcome: Result<CreateRepositoryResult> | null = null;

    const backend = {
        repositories: {
            gitignoreTemplates: async (): Promise<
                Result<GitignoreTemplateInfo[]>
            > => ({ ok: true, value: TEMPLATES }),
            licenses: async (): Promise<Result<LicenseTemplateInfo[]>> => ({
                ok: true,
                value: LICENSES,
            }),
            create: async (
                request: Record<string, unknown>
            ): Promise<Result<CreateRepositoryResult>> => {
                createCalls.push(request);
                return createOutcome ?? created();
            },
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        setCreateOutcome(next: Result<CreateRepositoryResult>) {
            createOutcome = next;
        },
        createCalls,
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
                    <ToastProvider>{ui}</ToastProvider>
                </QueryClientProvider>
            </AppServicesContext.Provider>
        );
    });
    return {
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

async function typeInto(selector: string, value: string): Promise<void> {
    const element = await waitFor<HTMLInputElement>(selector);
    const descriptor = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        "value"
    )!;
    descriptor.set!.call(element, value);
    await act(async () => {
        element.dispatchEvent(new Event("input", { bubbles: true }));
        await new Promise((resolve) => setTimeout(resolve, 10));
    });
}

async function clickButtonWithText(text: string): Promise<void> {
    await flush();
    const button = [...document.body.querySelectorAll("button")].find(
        (candidate) => candidate.textContent!.includes(text)
    );
    if (!button) throw new Error(`button not found: ${text}`);
    await act(async () => {
        button.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 30));
    });
}

async function waitFor<T extends Element>(selector: string): Promise<T> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        const element = document.body.querySelector<T>(selector);
        if (element) return element;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    throw new Error(`element not found: ${selector}`);
}

describe("NewRepoDialog", () => {
    beforeEach(() => {
        seedSettingsForTests({ lastRepositoryDirectory: "" });
        document.body.innerHTML = "";
        vi.clearAllMocks();
    });

    it("creates from the picked parent and remembers it for next time", async () => {
        const services = backendWith();
        const onCreated = vi.fn();

        renderWith(
            services,
            <NewRepoDialog open onClose={() => {}} onCreated={onCreated} />
        );
        await flush();

        await clickButtonWithText("Browse");
        await flush();
        await typeInto('input[placeholder="my-project"]', "demo");

        await clickButtonWithText("Create");
        await flush();

        expect(services.createCalls.length).toBe(1);
        expect(services.createCalls[0]).toMatchObject({
            parentDirectory: "/repo-parent",
            name: "demo",
            readme: true,
            gitignoreTemplate: null,
            license: null,
        });
        expect(onCreated).toHaveBeenCalledWith("/repo-parent/demo");
        expect(getSetting("lastRepositoryDirectory")).toBe("/repo-parent");
    });

    it("seeds the parent from the remembered setting", async () => {
        seedSettingsForTests({ lastRepositoryDirectory: "/repo-parent" });
        const services = backendWith();
        const onCreated = vi.fn();

        renderWith(
            services,
            <NewRepoDialog open onClose={() => {}} onCreated={onCreated} />
        );
        await flush();

        // No Browse needed: the remembered parent is already active.
        await typeInto('input[placeholder="my-project"]', "demo");
        await clickButtonWithText("Create");
        await flush();

        expect(services.createCalls.length).toBe(1);
        expect(onCreated).toHaveBeenCalledWith("/repo-parent/demo");
    });

    it("surfaces backend failures inline", async () => {
        const services = backendWith();
        services.setCreateOutcome({
            ok: false,
            error: new GitBackendError({
                code: "conflict",
                message: "directory is not empty",
                detail: undefined,
                retryable: false,
            }),
        });

        renderWith(
            services,
            <NewRepoDialog open onClose={() => {}} onCreated={() => {}} />
        );
        await flush();

        await clickButtonWithText("Browse");
        await flush();
        await typeInto('input[placeholder="my-project"]', "demo");
        await clickButtonWithText("Create");
        await flush();

        const alert = document.body.querySelector('[role="alert"]');
        expect(alert?.textContent).toContain("directory is not empty");
    });
});
