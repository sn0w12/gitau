// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
    capabilityAllows,
    useActiveConnection,
} from "@/hooks/forge/use-forge-account";
import type { Capabilities } from "@/lib/backend/protocol";
import {
    clearActiveConnection,
    connectionStore,
    setActiveConnection,
} from "@/stores/connection-store";

function capabilities(overrides: Partial<Capabilities> = {}): Capabilities {
    return {
        issues: true,
        pullRequests: true,
        reviews: true,
        notifications: true,
        checks: true,
        commitStatuses: true,
        workflowRuns: true,
        issuesWrite: true,
        pullRequestsWrite: true,
        publish: true,
        orgs: true,
        ...overrides,
    };
}

const { fakeBackend } = vi.hoisted(() => {
    const NONE = {
        issues: false,
        pullRequests: false,
        reviews: false,
        notifications: false,
        checks: false,
        commitStatuses: false,
        workflowRuns: false,
        issuesWrite: false,
        pullRequestsWrite: false,
        publish: false,
        orgs: false,
    };
    const account = (login: string) => ({
        login,
        name: null,
        avatarUrl: "",
        htmlUrl: "",
        scopes: [],
        connectedAtMs: 0,
    });
    const fakeBackend = {
        forge: {
            providers: async () => ({
                ok: true as const,
                value: [
                    {
                        id: "github",
                        label: "GitHub",
                        hosts: ["github.com"],
                        capabilities: NONE,
                    },
                    {
                        id: "gitlab",
                        label: "GitLab",
                        hosts: ["gitlab.com"],
                        capabilities: NONE,
                    },
                ],
            }),
            connections: async () => ({
                ok: true as const,
                value: [
                    {
                        provider: "github",
                        host: "github.com",
                        account: account("octocat"),
                    },
                    {
                        provider: "gitlab",
                        host: "gitlab.com",
                        account: account("tanuki"),
                    },
                ],
            }),
        },
    };
    return { fakeBackend };
});

vi.mock("@/contexts/services-context", () => ({
    useAppServices: () => ({ backend: fakeBackend }),
}));

function Probe() {
    const connection = useActiveConnection();
    return <span data-testid="login">{connection?.account.login ?? "-"}</span>;
}

function renderProbe() {
    const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
    });
    return render(
        <QueryClientProvider client={queryClient}>
            <Probe />
        </QueryClientProvider>
    );
}

async function expectLogin(
    container: HTMLElement,
    value: string
): Promise<void> {
    await waitFor(() =>
        expect(
            container.querySelector("[data-testid=login]")?.textContent
        ).toBe(value)
    );
}

describe("useActiveConnection", () => {
    beforeEach(() => {
        clearActiveConnection();
    });

    afterEach(() => {
        cleanup();
        clearActiveConnection();
    });

    it("follows the first connection while nothing is selected", async () => {
        const view = renderProbe();
        await expectLogin(view.container, "octocat");
    });

    it("follows an explicit selection", async () => {
        setActiveConnection("gitlab", "gitlab.com");
        const view = renderProbe();
        await expectLogin(view.container, "tanuki");
    });

    it("falls back to the first connection when the selection is gone", async () => {
        setActiveConnection("bitbucket", "bitbucket.org");
        const view = renderProbe();
        await expectLogin(view.container, "octocat");
        expect(connectionStore.state.activeKey).toBe("bitbucket:bitbucket.org");
    });
});

describe("capabilityAllows", () => {
    it("reads unknown capabilities as available", () => {
        expect(capabilityAllows(null, "issues")).toBe(true);
        expect(capabilityAllows(null, "publish")).toBe(true);
    });

    it("reports the flag once capabilities are known", () => {
        const full = capabilities();
        expect(capabilityAllows(full, "issues")).toBe(true);
        expect(capabilityAllows({ ...full, issues: false }, "issues")).toBe(
            false
        );
        expect(
            capabilityAllows({ ...full, pullRequests: false }, "pullRequests")
        ).toBe(false);
    });
});
