// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { ShortcutControl } from "@/components/settings/shortcut-control";

vi.mock("@tauri-apps/plugin-os", () => ({
    platform: () => "windows",
}));

afterEach(cleanup);

describe("ShortcutControl", () => {
    it("records logical keys instead of physical codes", () => {
        const onChange = vi.fn();
        const { container } = render(
            <ShortcutControl value="" onChange={onChange} />
        );

        const edit = container.querySelector("button");
        if (!edit) throw new Error("edit button not rendered");
        fireEvent.click(edit);
        fireEvent.keyDown(document, {
            key: "k",
            code: "KeyK",
            ctrlKey: true,
        });

        expect(onChange).toHaveBeenCalledWith("Mod+K");
    });

    it("renders a stored physical-code chord as its key", () => {
        const { container } = render(
            <ShortcutControl value="Mod+[KeyK]" onChange={() => {}} />
        );

        expect(container.querySelector("svg.lucide-chevron-up")).not.toBeNull();
        expect(screen.getByText("K")).toBeDefined();
        expect(screen.queryByText("[KeyK]")).toBeNull();
    });
});
