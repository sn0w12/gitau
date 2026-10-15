import * as React from "react";

import type { ImageDiffViewMode } from "@/hooks/changes/use-image-diff-view-mode";
import type { SectionMeta, SyntaxStyle } from "@/lib/backend/protocol";
import type {
    SectionGapExpansions,
    SectionRows,
} from "@/lib/backend/streams/diff-session";
import {
    buildDisplayItems,
    computeDiffGaps,
    type ExpandWindow,
} from "@/lib/diff/context-gaps";
import {
    diffCharWidth,
    subscribeDiffCharWidth,
    watchDiffCharWidth,
} from "@/lib/diff/measure";
import { materializeSectionRows } from "@/lib/diff/split-rows";

import { DelayedSpinner } from "./delayed-spinner";
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
    expansions,
    oldLineCount,
    canExpand,
    onExpand,
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
    expansions?: SectionGapExpansions;
    oldLineCount?: number;
    canExpand: boolean;
    onExpand: (
        gapKey: number,
        direction: "up" | "down",
        window: ExpandWindow
    ) => void;
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

    const displayItems = React.useMemo(
        () =>
            buildDisplayItems(
                rows,
                computeDiffGaps(rows, oldLineCount),
                expansions,
                canExpand
            ),
        [rows, oldLineCount, expansions, canExpand]
    );

    /* oxlint-disable react/refs */
    // Keep the last painted body until an image section's bytes arrive, so
    // switching files swaps content in place instead of blanking the pane.
    const lastBodyRef = React.useRef<React.ReactNode>(null);
    const pendingImage =
        Boolean(section?.image) && Boolean(imageMode) && !image;
    if (pendingImage) {
        return (
            lastBodyRef.current ?? (
                <div className="flex flex-1 items-center justify-center">
                    <DelayedSpinner className="size-5" />
                </div>
            )
        );
    }

    let body: React.ReactNode = null;
    if (section?.image) {
        if (!imageMode) {
            body = <BinaryDiffView path={section.path} />;
        } else if (image) {
            body = (
                <ImageDiffView
                    mode={imageMode}
                    oldImage={image.old}
                    newImage={image.new}
                />
            );
        }
    } else if (section?.binary) {
        body = <BinaryDiffView path={section.path} />;
    } else {
        body =
            mode === "split" ? (
                <SplitRows
                    items={displayItems}
                    charWidth={charWidth}
                    styles={styles}
                    onExpand={onExpand}
                    key={`split-${sectionId}`}
                />
            ) : (
                <UnifiedRows
                    items={displayItems}
                    contentMinWidthPx={contentMinWidthPx}
                    styles={styles}
                    onExpand={onExpand}
                    key={`unified-${sectionId}`}
                />
            );
    }
    lastBodyRef.current = body;
    return body;
}
