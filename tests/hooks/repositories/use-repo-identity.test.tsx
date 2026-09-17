// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, waitFor } from "@testing-library/react";
import { useEffect, type ReactElement } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
    formatRepoLabel,
    ownerFromRemote,
    repoDisplayName,
    useRepoIdentity,
    useRepoIdentities,
} from "@/hooks/repositories/use-repo-identity";

const { fakeBackend, remoteCalls } = vi.hoisted(() => {
    const remoteCalls: number[] = [];
    type Remote = { name: string; url?: string };
    const remotesByRepo = new Map<number, Remote[]>([
        [1, [{ name: "origin", url: "https://github.com/octo/alpha.git" }]],
        [2, [{ name: "upstream", url: "git@gitlab.com:beta-group/beta.git" }]],
    ]);
    const fakeBackend = {
        remotes: {
            list: async (
                repoId: number
            ): Promise<{ ok: true; value: Remote[] }> => {
                remoteCalls.push(repoId);
                return {
                    ok: true,
                    value: remotesByRepo.get(repoId) ?? [],
                };
            },
        },
    };
    return { fakeBackend, remoteCalls };
});

vi.mock("@/contexts/services-context", () => ({
    useAppServices: () => ({ backend: fakeBackend }),
}));

function renderWithClient(node: ReactElement) {
    const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
    });
    return render(
        <QueryClientProvider client={queryClient}>{node}</QueryClientProvider>
    );
}

describe("repoDisplayName", () => {
    it("uses the last path segment and tolerates separators", () => {
        expect(repoDisplayName("C:\\repos\\demo")).toBe("demo");
        expect(repoDisplayName("/home/user/project/")).toBe("project");
        expect(repoDisplayName("")).toBe("repo");
    });
});

describe("ownerFromRemote", () => {
    it("parses https and scp-like remotes", () => {
        expect(ownerFromRemote("https://github.com/Octo/hello.git")).toBe(
            "Octo"
        );
        expect(ownerFromRemote("git@github.com:octo/hello.git")).toBe("octo");
        expect(ownerFromRemote(undefined)).toBeUndefined();
        expect(ownerFromRemote("/local/path/only")).toBeUndefined();
    });
});

describe("formatRepoLabel", () => {
    it("prefixes the owner when known", () => {
        expect(
            formatRepoLabel({ name: "demo", owner: "octo", initial: "D" })
        ).toBe("octo/demo");
        expect(
            formatRepoLabel({ name: "demo", owner: undefined, initial: "D" })
        ).toBe("demo");
    });
});

describe("useRepoIdentity", () => {
    beforeEach(() => {
        remoteCalls.length = 0;
    });
    afterEach(() => {
        cleanup();
    });

    it("derives owner from the origin remote", async () => {
        const captured: { owner?: string; name?: string } = {};
        function Harness() {
            const identity = useRepoIdentity(1, "C:\\repos\\alpha");
            useEffect(() => {
                captured.owner = identity.owner;
                captured.name = identity.name;
            });
            return null;
        }

        renderWithClient(<Harness />);
        await waitFor(() => expect(captured.owner).toBe("octo"));
        expect(captured.name).toBe("alpha");
    });

    it("falls back to the first remote when origin is missing", async () => {
        const captured: { owner?: string | undefined } = { owner: "sentinel" };
        function Harness() {
            const identity = useRepoIdentity(2, "C:\\repos\\beta");
            useEffect(() => {
                captured.owner = identity.owner;
            });
            return null;
        }

        renderWithClient(<Harness />);
        await waitFor(() => expect(captured.owner).toBe("beta-group"));
    });

    it("never queries the backend for invalid repo ids", async () => {
        function Harness() {
            useRepoIdentity(0, "C:\\repos\\alpha");
            return null;
        }

        renderWithClient(<Harness />);
        await waitFor(() => expect(remoteCalls).toHaveLength(0));
    });
});

describe("useRepoIdentities", () => {
    beforeEach(() => {
        remoteCalls.length = 0;
    });
    afterEach(() => {
        cleanup();
    });

    it("maps owner lookups by path for lists", async () => {
        const captured: { owners?: Map<string, string | undefined> } = {};
        function Harness() {
            const owners = useRepoIdentities([
                { repoId: 1, path: "C:\\repos\\alpha" },
                { repoId: 2, path: "C:\\repos\\beta" },
                { repoId: 3, path: "C:\\repos\\gamma" },
            ]);
            useEffect(() => {
                captured.owners = owners;
            });
            return null;
        }

        renderWithClient(<Harness />);
        await waitFor(() =>
            expect(captured.owners?.get("C:\\repos\\alpha")).toBe("octo")
        );
        expect(captured.owners?.get("C:\\repos\\beta")).toBe("beta-group");
        expect(captured.owners?.get("C:\\repos\\gamma")).toBeUndefined();
    });

    it("skips unbound entries without backend calls", async () => {
        function Harness() {
            useRepoIdentities([{ repoId: 0, path: "C:\\repos\\alpha" }]);
            return null;
        }

        renderWithClient(<Harness />);
        await waitFor(() => expect(remoteCalls).toHaveLength(0));
    });
});
