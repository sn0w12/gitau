import { Check, Copy } from "lucide-react";

import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tooltip, TooltipPopup, TooltipTrigger } from "@/components/ui/tooltip";
import { useHighlightedSnippet } from "@/hooks/highlight/use-highlighted-snippet";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";
import type { DiffRow } from "@/lib/backend/protocol";
import { cn } from "@/lib/utils";

import { HighlightedLine } from "../diff/highlight-line";
import { KIND_ROW_BORDER_CLASS, KIND_ROW_CLASS } from "../diff/rows";
import { Frame, FramePanel } from "../ui/frame";

/** The one-character diff marker at the head of a hunk line. */
const DIFF_MARKER = /^[+\- ]/;

/** A hunk header, which is diff metadata rather than code and must not be
 * syntax highlighted. */
const HUNK_HEADER = /^@@/;

/** Drops span coverage that falls before `offset`, so a marker rendered
 * outside the highlighted text is never painted by the highlighter. */
function clipSpans(spans: readonly number[], offset: number): number[] {
    const out: number[] = [];
    for (let i = 0; i < spans.length; i += 3) {
        const start = spans[i];
        const len = spans[i + 1];
        const styleId = spans[i + 2];
        const end = start + len;
        if (end <= offset) continue;
        const from = Math.max(start, offset);
        out.push(from - offset, end - from, styleId);
    }
    return out;
}

/** Copies the block's source, revealed on hover so it never covers the first
 * line. */
function CopyCodeButton({ text }: { text: string }) {
    const { copyToClipboard, isCopied } = useCopyToClipboard();
    return (
        <Tooltip>
            <TooltipTrigger
                render={
                    <Button
                        aria-label="Copy code"
                        size="icon-xs"
                        variant="ghost"
                        className="absolute end-1 top-1 z-10 opacity-0 transition-opacity group-hover/code:opacity-100 focus-visible:opacity-100"
                        onClick={() => copyToClipboard(text)}
                    />
                }
            >
                {isCopied ? <Check /> : <Copy />}
            </TooltipTrigger>
            <TooltipPopup sideOffset={8}>
                {isCopied ? "Copied" : "Copy"}
            </TooltipPopup>
        </Tooltip>
    );
}
/**
 * One fenced code block with backend syntax highlighting. Renders plain
 * text immediately and upgrades to highlighted spans once the snippet
 * query lands; unknown languages stay plain.
 *
 * `diff` marks the text as a unified hunk, whose leading `+`, `-`, and `@@`
 * markers are rendered unstyled, since they are diff punctuation rather than
 * code.
 */
export function CodeBlock({
    language,
    text,
    diff = false,
}: {
    language?: string;
    text: string;
    diff?: boolean;
}) {
    const snippet = useHighlightedSnippet(language, text);
    const data = snippet.data;
    const highlighted = data?.highlighted === true;
    const lines = text.split("\n");

    return (
        <Frame className="group/code relative">
            {language && language !== "plaintext" ? (
                <div className="px-2 pt-1 font-mono text-muted-foreground">
                    {language}
                </div>
            ) : null}
            <FramePanel className="code-hl p-0 font-mono text-xs">
                <CopyCodeButton text={text} />
                <ScrollArea
                    scrollFade
                    overscrollContain
                    className="[&_[data-slot=scroll-area-viewport]]:max-h-96"
                >
                    <div className="ui-selectable p-2 whitespace-pre">
                        {lines.map((rawLine, index) => {
                            // Spans are computed on the line without its
                            // terminator; a stray \r would render a phantom
                            // break.
                            const line = rawLine.endsWith("\r")
                                ? rawLine.slice(0, -1)
                                : rawLine;
                            if (diff && HUNK_HEADER.test(line)) {
                                return (
                                    <div
                                        key={index}
                                        className="text-muted-foreground"
                                    >
                                        {line}
                                    </div>
                                );
                            }
                            const marker =
                                diff && DIFF_MARKER.test(line) ? line[0] : null;
                            const body = marker === null ? line : line.slice(1);
                            const spans =
                                highlighted && data
                                    ? (data.spansByLine[index] ?? [])
                                    : [];
                            const row: DiffRow | undefined =
                                spans.length > 0
                                    ? {
                                          kind: "context",
                                          content: body,
                                          spans:
                                              marker === null
                                                  ? spans
                                                  : clipSpans(spans, 1),
                                      }
                                    : undefined;
                            return (
                                <div
                                    key={index}
                                    className={cn(
                                        "border-l-3 border-transparent",
                                        marker === "+" &&
                                            `${KIND_ROW_CLASS["addition"]} ${KIND_ROW_BORDER_CLASS["addition"]}`,
                                        marker === "-" &&
                                            `${KIND_ROW_CLASS["deletion"]} ${KIND_ROW_BORDER_CLASS["deletion"]}`
                                    )}
                                >
                                    {marker === null ? null : (
                                        <span className="ml-1 inline-block w-4 shrink-0 text-muted-foreground select-none">
                                            {marker}
                                        </span>
                                    )}
                                    <HighlightedLine
                                        code={body}
                                        row={row}
                                        styles={data?.styles}
                                    />
                                </div>
                            );
                        })}
                    </div>
                </ScrollArea>
            </FramePanel>
        </Frame>
    );
}
