import * as React from "react";

import type { ImageDiffViewMode } from "@/hooks/changes/use-image-diff-view-mode";
import type { SectionMeta, SyntaxStyle } from "@/lib/backend/protocol";
import type { SectionRows } from "@/lib/backend/streams/diff-session";
import {
    diffCharWidth,
    subscribeDiffCharWidth,
    watchDiffCharWidth,
} from "@/lib/diff/measure";
import { materializeSectionRows } from "@/lib/diff/split-rows";

import { BinaryDiffView, ImageDiffView } from "./image-diff";
import { SplitRows } from "./split-rows";
import { UnifiedRows } from "./unified-rows";

// Re-renders once pretext can measure the real loaded webfont.
function useDiffCharWidth(): number {
    const width = React.useSyncExternalStore(
        subscribeDiffCharWidth,
        diffCharWidth,
        diffCharWidth
    );
    React.useEffect(() => watchDiffCharWidth(), []);
    return width;
}

export function DiffBody({
    rowsBySection,
    sectionId,
    rowCount,
    mode,
    styles,
    section,
    imageMode,
    image,
    onLoadImage,
}: {
    rowsBySection: SectionRows | undefined;
    sectionId: number;
    rowCount: number;
    mode: "unified" | "split";
    styles?: readonly SyntaxStyle[];
    section?: SectionMeta;
    imageMode?: ImageDiffViewMode;
    image?: import("@/lib/backend/protocol").DiffImage;
    onLoadImage?: (sectionId: number) => void;
}) {
    // The section bar already shows path/kind/oldPath, so the wire-format
    // file-header rows carry nothing new here - drop them from the body.
    React.useEffect(() => {
        if (section?.image && imageMode && !image) {
            void onLoadImage?.(section.sectionId);
        }
    }, [image, imageMode, onLoadImage, section]);

    const rows = React.useMemo(
        () =>
            materializeSectionRows(rowsBySection, rowCount).filter(
                (row) => row !== undefined && row.kind !== "fileHeader"
            ),
        [rowsBySection, rowCount]
    );

    const charWidth = useDiffCharWidth();
    // Only the unified view scrolls horizontally; the split view wraps.
    const contentMinWidthPx = React.useMemo(() => {
        if (mode !== "unified") return 0;
        let maxChars = 0;
        for (const row of rows) {
            if (row && row.content.length > maxChars) {
                maxChars = row.content.length;
            }
        }
        return Math.ceil(maxChars * charWidth);
    }, [mode, rows, charWidth]);

    if (section?.image) {
        if (!imageMode) return <BinaryDiffView path={section.path} />;
        if (!image)
            return (
                <div className="flex flex-1 items-center justify-center text-xs text-muted-foreground">
                    Loading image diff...
                </div>
            );
        return (
            <ImageDiffView
                mode={imageMode}
                oldImage={image.old}
                newImage={image.new}
            />
        );
    }
    if (section?.binary) return <BinaryDiffView path={section.path} />;

    return mode === "split" ? (
        <SplitRows
            rows={rows}
            charWidth={charWidth}
            styles={styles}
            key={`split-${sectionId}`}
        />
    ) : (
        <UnifiedRows
            rows={rows}
            contentMinWidthPx={contentMinWidthPx}
            styles={styles}
            key={`unified-${sectionId}`}
        />
    );
}
