// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import {
    cleanup,
    fireEvent,
    render,
    screen,
    waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { FontControl } from "@/components/settings/font-control";

const backend = vi.hoisted(() => ({
    fonts: {
        list: vi.fn(async () => ({
            ok: true as const,
            value: ["Segoe UI", "Cascadia Code"],
        })),
    },
}));

vi.mock("@/contexts/services-context", () => ({
    useAppServices: () => ({ backend }),
}));

// jsdom has no ResizeObserver; Base UI's popup positioner constructs one.
if (typeof globalThis.ResizeObserver === "undefined") {
    (globalThis as Record<string, unknown>).ResizeObserver = class {
        observe(): void {}
        unobserve(): void {}
        disconnect(): void {}
    };
}

function renderControl(onChange = vi.fn()) {
    const queryClient = new QueryClient({
        defaultOptions: { queries: { retry: false } },
    });
    const view = render(
        <QueryClientProvider client={queryClient}>
            <FontControl
                value="Inter Variable"
                fallback="sans-serif"
                onChange={onChange}
            />
        </QueryClientProvider>
    );
    return { ...view, onChange };
}

async function openList(): Promise<void> {
    const trigger = await waitFor(() =>
        screen.getByLabelText("Open font list")
    );
    fireEvent.click(trigger);
}

describe("FontControl", () => {
    beforeEach(() => {
        backend.fonts.list.mockClear();
    });

    afterEach(() => {
        cleanup();
    });

    it("shows the selected family in its own typeface", () => {
        renderControl();
        const input = screen.getByLabelText("Font family") as HTMLInputElement;

        expect(input.value).toBe("Inter Variable");
        expect(input.style.fontFamily).toContain("Inter Variable");
    });

    it("lists installed fonts alongside the bundled ones", async () => {
        renderControl();
        await openList();

        await waitFor(() => {
            expect(screen.getByText("Segoe UI")).toBeDefined();
        });
        expect(screen.getByText("Cascadia Code")).toBeDefined();
        expect(screen.getByText("Geist Mono Variable")).toBeDefined();

        const item = screen.getByText("Cascadia Code");
        expect(item.style.fontFamily).toContain("Cascadia Code");
    });

    it("filters the list by the typed query", async () => {
        renderControl();
        await openList();

        await waitFor(() => {
            expect(screen.getByText("Segoe UI")).toBeDefined();
        });

        fireEvent.change(screen.getByLabelText("Font family"), {
            target: { value: "casc" },
        });

        await waitFor(() => {
            expect(screen.queryByText("Segoe UI")).toBeNull();
        });
        expect(screen.getByText("Cascadia Code")).toBeDefined();
    });

    it("reports the picked family", async () => {
        const { onChange } = renderControl();
        await openList();

        const item = await waitFor(() => screen.getByText("Cascadia Code"));
        fireEvent.click(item);

        expect(onChange).toHaveBeenCalledWith("Cascadia Code");
    });
});
