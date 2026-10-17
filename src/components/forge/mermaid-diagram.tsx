import { Maximize2, TriangleAlert, XIcon } from "lucide-react";
import { useEffect, useId, useState } from "react";

import { CodeBlock } from "@/components/forge/code-block";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogBackdrop,
    DialogClose,
    DialogPrimitive,
    DialogTitle,
} from "@/components/ui/dialog";
import { Frame, FrameHeader, FramePanel } from "@/components/ui/frame";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipPopup, TooltipTrigger } from "@/components/ui/tooltip";

/** Pele measures labels with the font it draws them in, so the family is
 * passed at render time; the `--pele-font` variable carries the same stack. */
const FONT = '"Inter Variable", sans-serif';

function describe(cause: unknown): string {
    if (cause instanceof Error) {
        return cause.message.split("\n")[0] ?? cause.name;
    }
    return String(cause);
}

/** Pele escapes label text and filters links and styles, so the markup it
 * builds from untrusted diagram source carries no scripts or handlers. */
function DiagramSvg({ svg, className }: { svg: string; className?: string }) {
    return (
        <div className={className} dangerouslySetInnerHTML={{ __html: svg }} />
    );
}

/**
 * A mermaid fence rendered as a diagram. The source is a fallback rather than
 * an error state: a diagram that will not parse is still worth reading as
 * text, and it stays copyable.
 */
export function MermaidDiagram({ text }: { text: string }) {
    const [svg, setSvg] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const [expanded, setExpanded] = useState(false);
    const diagramId = useId();
    // Pele names each diagram's accessibility title, so the prefix has to be
    // unique across a page or the ids collide between diagrams.
    const idPrefix = `pele${diagramId.replaceAll(":", "")}`;

    useEffect(() => {
        let live = true;
        void (async () => {
            const { renderAsync } = await import("pele/lazy");
            const result = await renderAsync(text, {
                fontFamily: FONT,
                idPrefix,
            });
            if (live) setSvg(result.svg);
        })().catch((cause: unknown) => {
            console.error("diagram render failed", cause);
            if (live) setError(describe(cause));
        });
        return () => {
            live = false;
        };
    }, [idPrefix, text]);

    if (error !== null) {
        // The source is still the content, so it stays readable and
        // copyable; the reason sits above it rather than only in a console
        // nobody has open.
        return (
            <>
                <Frame className="my-2">
                    <FrameHeader className="flex flex-row items-center gap-1.5 px-2 py-1.5">
                        <TriangleAlert className="size-3.5 text-destructive" />
                        <span className="text-xs font-medium text-destructive">
                            Diagram could not be rendered
                        </span>
                    </FrameHeader>
                    <FramePanel className="px-3 py-1.5">
                        <p className="ui-selectable font-mono text-xs break-words text-muted-foreground">
                            {error}
                        </p>
                    </FramePanel>
                </Frame>
                <CodeBlock language="mermaid" text={text} />
            </>
        );
    }

    return (
        <Frame className="my-2">
            <FramePanel
                className="group/diagram relative overflow-auto p-3"
                data-testid="mermaid-diagram"
            >
                {svg === null ? (
                    <div className="flex flex-col gap-2" aria-busy="true">
                        <Skeleton className="h-32 w-full" />
                    </div>
                ) : (
                    <>
                        <DiagramSvg svg={svg} />
                        <Tooltip>
                            <TooltipTrigger
                                render={
                                    <Button
                                        aria-label="Expand diagram"
                                        aria-haspopup="dialog"
                                        size="icon-xs"
                                        variant="ghost"
                                        className="absolute end-1 top-1 z-10 opacity-0 transition-opacity group-hover/diagram:opacity-100 focus-visible:opacity-100"
                                        onClick={() => setExpanded(true)}
                                    />
                                }
                            >
                                <Maximize2 />
                            </TooltipTrigger>
                            <TooltipPopup sideOffset={8}>Expand</TooltipPopup>
                        </Tooltip>
                    </>
                )}
            </FramePanel>
            <Dialog open={expanded} onOpenChange={setExpanded}>
                <DialogPrimitive.Portal>
                    <DialogBackdrop />
                    <DialogPrimitive.Popup className="fixed inset-0 z-50 m-auto flex h-[88svh] w-[92vw] scale-100 flex-col overflow-hidden rounded-2xl border bg-popover bg-clip-padding text-popover-foreground shadow-lg/5 transition-[scale,opacity] duration-200 ease-snappy will-change-transform outline-none data-ending-style:scale-95 data-ending-style:opacity-0 data-starting-style:scale-95 data-starting-style:opacity-0">
                        <DialogTitle className="sr-only">Diagram</DialogTitle>
                        {svg !== null && (
                            <DiagramSvg
                                svg={svg}
                                className="flex min-h-0 flex-1 items-center justify-center p-6 [&>svg]:max-h-full [&>svg]:w-full"
                            />
                        )}
                        <DialogClose
                            aria-label="Close diagram"
                            className="absolute end-3 top-3 bg-background/80 backdrop-blur-sm"
                            render={<Button size="icon-sm" variant="ghost" />}
                        >
                            <XIcon />
                        </DialogClose>
                    </DialogPrimitive.Popup>
                </DialogPrimitive.Portal>
            </Dialog>
        </Frame>
    );
}
