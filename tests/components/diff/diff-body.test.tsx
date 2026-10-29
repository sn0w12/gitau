import { act } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { describe, expect, it } from "vitest";

import { DiffBody } from "@/components/diff/diff-body";
import type { DiffImage, SectionMeta } from "@/lib/backend/protocol";

const imageSection: SectionMeta = {
    sectionId: 0,
    path: "images/logo.png",
    kind: "modified",
    binary: false,
    image: true,
    complete: true,
};

const image: DiffImage = {
    new: { dataUrl: "data:image/png;base64,AAAA" },
};

describe("DiffBody", () => {
    it("shows no loading indicator on the frame before an image arrives", () => {
        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);
        act(() => {
            root.render(
                <DiffBody
                    rowsBySection={undefined}
                    sectionId={0}
                    rowCount={0}
                    mode="unified"
                    imageMode="sideBySide"
                    section={imageSection}
                    onLoadImage={() => {}}
                    canExpand={false}
                    onExpand={() => {}}
                />
            );
        });

        expect(container.querySelector('[aria-label="Loading"]')).toBeNull();
        expect(container.textContent).not.toContain("Loading image diff");

        act(() => root.unmount());
        container.remove();
    });

    it("holds the previous image until the next one arrives", () => {
        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);
        const render = (section: SectionMeta, next?: DiffImage) => {
            act(() => {
                root.render(
                    <DiffBody
                        rowsBySection={undefined}
                        sectionId={section.sectionId}
                        rowCount={0}
                        mode="unified"
                        imageMode="sideBySide"
                        section={section}
                        image={next}
                        onLoadImage={() => {}}
                        canExpand={false}
                        onExpand={() => {}}
                    />
                );
            });
        };

        render(imageSection, image);
        expect(container.querySelector("img")?.getAttribute("src")).toBe(
            "data:image/png;base64,AAAA"
        );

        render({ ...imageSection, sectionId: 1, path: "images/b.png" });
        expect(container.querySelector("img")?.getAttribute("src")).toBe(
            "data:image/png;base64,AAAA"
        );

        act(() => root.unmount());
        container.remove();
    });
});
