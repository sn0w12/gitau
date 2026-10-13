import { act } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { describe, expect, it } from "vitest";

import { ImageDiffView } from "@/components/diff/image-diff";

describe("ImageDiffView", () => {
    it("renders each side straight from its data url", () => {
        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);
        act(() => {
            root.render(
                <ImageDiffView
                    mode="sideBySide"
                    oldImage={{ dataUrl: "data:image/png;base64,AAAA" }}
                    newImage={{ dataUrl: "data:image/png;base64,BBBB" }}
                />
            );
        });

        const srcs = Array.from(container.querySelectorAll("img")).map((img) =>
            img.getAttribute("src")
        );
        expect(srcs).toEqual([
            "data:image/png;base64,AAAA",
            "data:image/png;base64,BBBB",
        ]);

        act(() => root.unmount());
        container.remove();
    });
});
