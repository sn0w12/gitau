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

    it("clips the swipe halves so transparency never reveals the other side", () => {
        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);
        act(() => {
            root.render(
                <ImageDiffView
                    mode="swipe"
                    oldImage={{ dataUrl: "data:image/png;base64,AAAA" }}
                    newImage={{ dataUrl: "data:image/png;base64,BBBB" }}
                />
            );
        });

        const clips = Array.from(container.querySelectorAll("img")).map(
            (img) => (img as HTMLImageElement).style.clipPath
        );
        expect(clips).toEqual(["inset(0 0 0 50%)", "inset(0 50% 0 0)"]);

        act(() => root.unmount());
        container.remove();
    });

    it("stacks both sides with a difference blend in difference mode", () => {
        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);
        act(() => {
            root.render(
                <ImageDiffView
                    mode="difference"
                    oldImage={{ dataUrl: "data:image/png;base64,AAAA" }}
                    newImage={{ dataUrl: "data:image/png;base64,BBBB" }}
                />
            );
        });

        const images = Array.from(container.querySelectorAll("img"));
        expect(images.map((img) => img.getAttribute("src"))).toEqual([
            "data:image/png;base64,AAAA",
            "data:image/png;base64,BBBB",
        ]);
        expect(images[1].classList.contains("mix-blend-difference")).toBe(true);
        expect(container.querySelector(".invert")).not.toBeNull();

        act(() => root.unmount());
        container.remove();
    });
});
