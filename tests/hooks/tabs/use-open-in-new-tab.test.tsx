// @vitest-environment jsdom
import { QueryClient } from "@tanstack/react-query";
import { cleanup, render } from "@testing-library/react";
import { act, useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { ToastProvider } from "@/components/ui/toast";
import { useOpenInNewTab } from "@/hooks/tabs/use-open-in-new-tab";
import type { BackendClient } from "@/lib/backend/transport/client";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import { resetAppRuntime } from "@/lib/bootstrap/app-runtime";
import { appStore, selectActiveTab } from "@/stores/app-store";
import { repositoryStore } from "@/stores/repository-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const REPO_PATH = "C:\\repos\\alpha";

type OpenFn = (path: string) => Promise<Result<{ id: number }>>;

let openFn: OpenFn = () => Promise.resolve({ ok: true, value: { id: 7 } });

function fakeServices() {
    const backend = {
        repositories: {
            open: (path: string) => openFn(path),
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
    };
}

const captured: {
    open: ReturnType<typeof useOpenInNewTab> | null;
} = { open: null };

function Harness() {
    const open = useOpenInNewTab();
    useEffect(() => {
        captured.open = open;
    }, [open]);
    return null;
}

function resetStores() {
    appStore.setState(() => ({ tabs: [], activeTabId: null }));
    repositoryStore.setState(() => ({ entries: new Map() }));
}

beforeEach(() => {
    resetStores();
    captured.open = null;
    openFn = () => Promise.resolve({ ok: true, value: { id: 7 } });
});

afterEach(() => {
    cleanup();
    resetAppRuntime(null);
    resetStores();
});

describe("useOpenInNewTab", () => {
    it("opens a route target with its href", async () => {
        resetAppRuntime(fakeServices());
        render(
            <ToastProvider>
                <Harness />
            </ToastProvider>
        );

        await act(async () => {
            await captured.open?.({ title: "Inbox", href: "/inbox" });
        });

        const tab = selectActiveTab();
        expect(tab?.title).toBe("Inbox");
        expect(tab?.repoPath).toBeUndefined();
        expect(tab?.lastResolvedHref).toBe("/inbox");
    });

    it("resolves a repo target to this session's id", async () => {
        resetAppRuntime(fakeServices());
        render(
            <ToastProvider>
                <Harness />
            </ToastProvider>
        );

        await act(async () => {
            await captured.open?.({ title: "alpha", repoPath: REPO_PATH });
        });

        const tab = selectActiveTab();
        expect(tab?.repoPath).toBe(REPO_PATH);
        expect(tab?.lastResolvedHref).toBe("/repo/7");
    });

    it("opens no tab when the repository fails to open", async () => {
        openFn = () =>
            Promise.resolve({
                ok: false,
                error: new GitBackendError({
                    code: "notFound",
                    message: "missing",
                    detail: undefined,
                    retryable: false,
                }),
            });
        resetAppRuntime(fakeServices());
        render(
            <ToastProvider>
                <Harness />
            </ToastProvider>
        );

        await act(async () => {
            await captured.open?.({ title: "alpha", repoPath: REPO_PATH });
        });

        expect(appStore.state.tabs).toHaveLength(0);
    });
});
