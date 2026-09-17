import * as React from "react";

import type { DiffRow, SyntaxStyle } from "@/lib/backend/protocol";
import { cn } from "@/lib/utils";

export interface CodeSegment {
    text: string;
    styleId: number | null;
}

/**
 * Splits one diff line into styled and plain segments from backend-provided
 * flat [start, len, styleId] triples. Malformed triples (out of range,
 * non-covering) yield null so the caller renders the line plain.
 */
export function segmentsFromSpans(
    code: string,
    spans: readonly number[] | undefined
): CodeSegment[] | null {
    if (!spans || spans.length === 0 || spans.length % 3 !== 0) return null;
    const limit = code.length;
    const segments: CodeSegment[] = [];
    let cursor = 0;
    for (let i = 0; i < spans.length; i += 3) {
        const start = spans[i];
        const len = spans[i + 1];
        const styleId = spans[i + 2];
        if (start < cursor || start > limit) return null;
        if (len === 0 || start + len > limit) return null;
        if (styleId <= 0 || !Number.isInteger(styleId)) return null;
        if (start > cursor) {
            segments.push({ text: code.slice(cursor, start), styleId: null });
        }
        segments.push({ text: code.slice(start, start + len), styleId });
        cursor = start + len;
    }
    if (cursor < limit) {
        segments.push({ text: code.slice(cursor), styleId: null });
    }
    return segments.every((s) => s.text.length > 0) ? segments : null;
}

function sanitizeLine(content: string | undefined): string | null {
    if (content === undefined) return null;
    const cleaned = content.replace(/\r/g, "");
    // A newline cannot be mapped onto one rendered line; bail out instead
    // of shifting every following token.
    return cleaned.includes("\n") ? null : cleaned;
}

/**
 * Renders one diff line with backend syntax highlighting: spans index into
 * the section's resolved style table; rows without usable span data render
 * as plain text. Layouts own their wrapper span and only swap children.
 */
export function HighlightedLine({
    code,
    row,
    styles,
}: {
    code: string | undefined;
    row?: DiffRow;
    styles?: readonly SyntaxStyle[];
}) {
    if (code === undefined) return "\u00a0";
    const cleaned = sanitizeLine(code);
    if (cleaned === null || cleaned === "") {
        return "\u00a0";
    }

    const segments =
        row?.spans && row.spans.length > 0
            ? segmentsFromSpans(cleaned, row.spans)
            : null;
    if (!segments) return code;

    return segments.map((segment, i) => {
        if (segment.styleId === null) {
            return <React.Fragment key={i}>{segment.text}</React.Fragment>;
        }
        const style = styles?.[segment.styleId - 1];
        if (!style) {
            return <React.Fragment key={i}>{segment.text}</React.Fragment>;
        }
        return (
            <span
                key={i}
                className={cn(
                    "syn",
                    style.b && "syn-b",
                    style.i && "syn-i",
                    style.u && "syn-u"
                )}
                style={
                    {
                        "--syn-light": style.light,
                        "--syn-dark": style.dark,
                    } as React.CSSProperties
                }
            >
                {segment.text}
            </span>
        );
    });
}
