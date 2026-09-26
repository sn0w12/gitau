import { useHighlightedSnippet } from "@/hooks/highlight/use-highlighted-snippet";
import type { DiffRow } from "@/lib/backend/protocol";

import { HighlightedLine } from "../diff/highlight-line";
import { Frame, FramePanel } from "../ui/frame";

/**
 * One fenced code block with backend syntax highlighting. Renders plain
 * text immediately and upgrades to highlighted spans once the snippet
 * query lands; unknown languages stay plain.
 */
export function CodeBlock({
    language,
    text,
}: {
    language?: string;
    text: string;
}) {
    const snippet = useHighlightedSnippet(language, text);
    const data = snippet.data;
    const highlighted = data?.highlighted === true;
    const lines = text.split("\n");

    return (
        <Frame className="my-2">
            {language && language !== "plaintext" ? (
                <div className="px-2 pt-1 font-mono text-muted-foreground">
                    {language}
                </div>
            ) : null}
            <FramePanel className="overflow-x-auto p-2 font-mono">
                <div className="whitespace-pre">
                    {lines.map((rawLine, index) => {
                        // Spans are computed on the line without its
                        // terminator; a stray \r would render a phantom break.
                        const line = rawLine.endsWith("\r")
                            ? rawLine.slice(0, -1)
                            : rawLine;
                        const spans =
                            highlighted && data
                                ? (data.spansByLine[index] ?? [])
                                : [];
                        const row: DiffRow | undefined =
                            spans.length > 0
                                ? { kind: "context", content: line, spans }
                                : undefined;
                        return (
                            <div key={index}>
                                <HighlightedLine
                                    code={line}
                                    row={row}
                                    styles={data?.styles}
                                />
                            </div>
                        );
                    })}
                </div>
            </FramePanel>
        </Frame>
    );
}
