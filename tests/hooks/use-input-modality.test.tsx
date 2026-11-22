// @vitest-environment jsdom

import { cleanup, fireEvent, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { useInputModality } from "@/hooks/use-input-modality";

function modality(): string | undefined {
    return document.documentElement.dataset.inputModality;
}

afterEach(() => {
    cleanup();
});

describe("useInputModality", () => {
    it("starts in keyboard modality", () => {
        renderHook(() => useInputModality());
        expect(modality()).toBe("keyboard");
    });

    it("switches to pointer on pointer input", () => {
        renderHook(() => useInputModality());
        fireEvent.pointerDown(document.body);
        expect(modality()).toBe("pointer");
    });

    it("returns to keyboard on navigation keys", () => {
        renderHook(() => useInputModality());
        fireEvent.pointerDown(document.body);
        fireEvent.keyDown(document.body, { key: "Tab" });
        expect(modality()).toBe("keyboard");
    });

    it("does not treat dismissals or shortcuts as navigation", () => {
        renderHook(() => useInputModality());
        fireEvent.pointerDown(document.body);
        fireEvent.keyDown(document.body, { key: "Escape" });
        fireEvent.keyDown(document.body, { key: "e" });
        fireEvent.keyDown(document.body, { key: "Enter", ctrlKey: true });
        expect(modality()).toBe("pointer");
    });

    it("restores the root attribute on unmount", () => {
        const { unmount } = renderHook(() => useInputModality());
        unmount();
        expect(modality()).toBeUndefined();
    });
});
