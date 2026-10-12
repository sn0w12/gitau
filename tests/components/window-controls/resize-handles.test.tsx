// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

const os = vi.hoisted(() => ({ current: "linux" }));

vi.mock("@tauri-apps/plugin-os", () => ({ type: () => os.current }));

import {
    dragRegionProps,
    WindowResizeHandles,
} from "@/components/window-controls";

afterEach(() => {
    cleanup();
    os.current = "linux";
});

describe("Linux window chrome shims", () => {
    it("drags manually on Linux", () => {
        os.current = "linux";
        expect(dragRegionProps()).toHaveProperty("onPointerDown");
    });

    it("keeps the native drag region off Linux", () => {
        os.current = "windows";
        expect(dragRegionProps()).toEqual({ "data-tauri-drag-region": true });
    });

    it("renders resize handles on Linux", () => {
        os.current = "linux";
        render(<WindowResizeHandles />);
        expect(
            document.querySelectorAll("[data-resize-direction]")
        ).toHaveLength(8);
    });

    it("renders no resize handles elsewhere", () => {
        os.current = "windows";
        render(<WindowResizeHandles />);
        expect(
            document.querySelectorAll("[data-resize-direction]")
        ).toHaveLength(0);
    });
});
