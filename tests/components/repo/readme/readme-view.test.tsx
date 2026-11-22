// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { ReadmeView } from "@/components/repo/readme/readme-view";
import { AppServicesContext } from "@/contexts/services-context";
import type { ProviderDescriptor, ReadmeContent } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";

const OK_PROVIDERS: Result<ProviderDescriptor[]> = { ok: true, value: [] };

function renderReadme(
    readme: Result<ReadmeContent | null>,
    text: (path: string) => Result<string | null>
) {
    const backend = {
        repositories: {
            readme: async (): Promise<Result<ReadmeContent | null>> => readme,
            worktreeText: async (
                _repoId: number,
                path: string
            ): Promise<Result<string | null>> => text(path),
            worktreeImage: async (): Promise<Result<string | null>> => ({
                ok: true,
                value: null,
            }),
        },
        forge: {
            providers: async (): Promise<Result<ProviderDescriptor[]>> =>
                OK_PROVIDERS,
        },
    } as unknown as BackendClient;
    const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
    });
    render(
        <AppServicesContext.Provider value={{ backend, queryClient }}>
            <QueryClientProvider client={queryClient}>
                <ReadmeView repoId={1} />
            </QueryClientProvider>
        </AppServicesContext.Provider>
    );
}

afterEach(() => {
    cleanup();
});

describe("ReadmeView", () => {
    it("opens a relative Markdown link and returns to the readme", async () => {
        renderReadme(
            {
                ok: true,
                value: {
                    path: "README.md",
                    content: "# Project\n\n[Guide](docs/guide.md)",
                },
            },
            (path) =>
                path === "docs/guide.md"
                    ? { ok: true, value: "# Guide\n\nGuide body" }
                    : { ok: true, value: null }
        );

        fireEvent.click(await screen.findByText("Guide"));

        expect(await screen.findByText("Guide body")).toBeTruthy();

        fireEvent.click(screen.getByRole("button", { name: "Back" }));

        await waitFor(() => {
            expect(screen.queryByText("Guide body")).toBeNull();
        });
        expect(screen.getByText("Project")).toBeTruthy();
        expect(screen.queryByRole("button", { name: "Back" })).toBeNull();
    });

    it("rejects links to files it cannot preview", async () => {
        renderReadme(
            {
                ok: true,
                value: {
                    path: "README.md",
                    content: "# Project\n\n[Logo](docs/logo.png)",
                },
            },
            () => ({ ok: true, value: null })
        );

        fireEvent.click(await screen.findByText("Logo"));

        expect(
            await screen.findByText("Cannot preview this file")
        ).toBeTruthy();
        expect(screen.queryByText("Guide body")).toBeNull();
    });
});
