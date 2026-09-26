// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { CustomMarkdown } from "@/components/issues/markdown";
import { AppServicesContext } from "@/contexts/services-context";
import type { HighlightedSnippet, SyntaxStyle } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function servicesWith(snippet: HighlightedSnippet, onCall: () => void) {
    const backend = {
        highlight: {
            code: async (): Promise<Result<HighlightedSnippet>> => {
                onCall();
                return { ok: true, value: snippet };
            },
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
    };
}

function renderWith(
    services: ReturnType<typeof servicesWith>,
    ui: ReactElement
) {
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

async function waitFor(predicate: () => boolean): Promise<boolean> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    return predicate();
}

const STYLE: SyntaxStyle = { light: "#ff0000", dark: "#00ff00" };

describe("CodeBlock", () => {
    beforeEach(() => {
        seedSettingsForTests({});
        document.body.innerHTML = "";
    });

    it("renders fence text plain, then upgrades to backend spans", async () => {
        let calls = 0;
        const services = servicesWith(
            {
                highlighted: true,
                // `let` on line 1 carries style 1; line 2 is plain.
                spansByLine: [[0, 3, 1], []],
                styles: [STYLE],
            },
            () => {
                calls += 1;
            }
        );
        const view = renderWith(
            services,
            <CustomMarkdown>{"```rust\nlet x = 1;\n}\n```"}</CustomMarkdown>
        );
        await flush();

        // Plain text paints synchronously before the query lands.
        expect(view.container.textContent).toContain("let x = 1;");
        const upgraded = await waitFor(
            () => view.container.querySelectorAll("span.syn").length > 0
        );
        expect(upgraded).toBe(true);
        expect(calls).toBe(1);
        const styled = view.container.querySelector("span.syn")!;
        expect(styled.textContent).toBe("let");
        view.unmount();
    });

    it("leaves plaintext fences alone without calling the backend", async () => {
        let calls = 0;
        const services = servicesWith(
            { highlighted: false, spansByLine: [], styles: [] },
            () => {
                calls += 1;
            }
        );
        const view = renderWith(
            services,
            <CustomMarkdown>{"```\njust text\n```"}</CustomMarkdown>
        );
        await flush();
        expect(view.container.textContent).toContain("just text");
        expect(view.container.querySelectorAll("span.syn").length).toBe(0);
        expect(calls).toBe(0);
        view.unmount();
    });
});
