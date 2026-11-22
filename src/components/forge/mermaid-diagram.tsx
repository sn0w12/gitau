import { Maximize2, TriangleAlert, ZoomIn, ZoomOut } from "lucide-react";
import type { PointerEvent as ReactPointerEvent, Ref } from "react";
import {
    useCallback,
    useEffect,
    useId,
    useLayoutEffect,
    useRef,
    useState,
} from "react";

import { CodeBlock } from "@/components/forge/code-block";
import { Button } from "@/components/ui/button";
import { Dialog, DialogPopup, DialogTitle } from "@/components/ui/dialog";
import { Frame, FrameHeader, FramePanel } from "@/components/ui/frame";
import { Skeleton } from "@/components/ui/skeleton";
import { Tooltip, TooltipPopup, TooltipTrigger } from "@/components/ui/tooltip";
import { useSettingValue } from "@/hooks/settings/use-setting";
import { fontFamilyStack } from "@/lib/fonts";
import { cn } from "@/lib/utils";

import {
    InsetInput,
    InsetInputAddition,
    InsetInputValue,
} from "../ui/inset-input";
import { Scritto } from "../ui/scritto";

function describe(cause: unknown): string {
    if (cause instanceof Error) {
        return cause.message.split("\n")[0] ?? cause.name;
    }
    return String(cause);
}

/** Pele escapes label text and filters links and styles, so the markup it
 * builds from untrusted diagram source carries no scripts or handlers. */
function DiagramSvg({
    svg,
    className,
    ref,
}: {
    svg: string;
    className?: string;
    ref?: Ref<HTMLDivElement>;
}) {
    return (
        <div
            ref={ref}
            className={className}
            dangerouslySetInnerHTML={{ __html: svg }}
        />
    );
}

const DIAGRAM_PADDING = 24;
const MIN_ZOOM = 0.25;
const MAX_ZOOM = 8;
const ZOOM_STEP = 1.25;

type DiagramView = { scale: number; x: number; y: number };

const IDENTITY_VIEW: DiagramView = { scale: 1, x: 0, y: 0 };

function clamp(value: number, min: number, max: number): number {
    return Math.min(max, Math.max(min, value));
}

function zoomView(
    view: DiagramView,
    scale: number,
    anchorX: number,
    anchorY: number,
    minScale: number,
    maxScale: number
): DiagramView {
    const next = clamp(scale, minScale, maxScale);
    if (next === view.scale) return view;
    const ratio = next / view.scale;
    return {
        scale: next,
        x: anchorX + (view.x - anchorX) * ratio,
        y: anchorY + (view.y - anchorY) * ratio,
    };
}

function isFitted(view: DiagramView, fitScale: number): boolean {
    return (
        Math.abs(view.scale - fitScale) < fitScale * 0.001 &&
        Math.abs(view.x) < 0.5 &&
        Math.abs(view.y) < 0.5
    );
}

function DiagramViewer({ svg }: { svg: string }) {
    const [view, setView] = useState<DiagramView>(IDENTITY_VIEW);
    const [fitScale, setFitScale] = useState(1);
    const [animating, setAnimating] = useState(false);
    const [panning, setPanning] = useState(false);
    const viewportRef = useRef<HTMLDivElement>(null);
    const diagramRef = useRef<HTMLDivElement>(null);
    const fitScaleRef = useRef(1);
    const hover = useRef<{ x: number; y: number } | null>(null);
    const pointers = useRef(new Map<number, { x: number; y: number }>());
    const pinch = useRef<{ distance: number; x: number; y: number } | null>(
        null
    );

    const fitDiagram = useCallback(() => {
        const viewport = viewportRef.current;
        const diagram = diagramRef.current?.querySelector("svg");
        if (!viewport || !diagram) return;
        const box = diagram.viewBox.baseVal;
        const width = box.width || diagram.width.baseVal.value;
        const height = box.height || diagram.height.baseVal.value;
        const measurable =
            viewport.clientWidth > 0 &&
            viewport.clientHeight > 0 &&
            width > 0 &&
            height > 0;
        const scale = measurable
            ? Math.min(
                  (viewport.clientWidth - DIAGRAM_PADDING * 2) / width,
                  (viewport.clientHeight - DIAGRAM_PADDING * 2) / height
              )
            : fitScaleRef.current;
        fitScaleRef.current = scale;
        setFitScale(scale);
        setView({ scale, x: 0, y: 0 });
    }, []);

    useLayoutEffect(() => {
        fitDiagram();
    }, [fitDiagram, svg]);

    const resetView = useCallback(() => {
        if (isFitted(view, fitScale)) return;
        setAnimating(true);
        fitDiagram();
    }, [fitDiagram, fitScale, view]);

    useEffect(() => {
        const viewport = viewportRef.current;
        if (!viewport) return;
        const handleWheel = (event: WheelEvent) => {
            event.preventDefault();
            setAnimating(false);
            const rect = viewport.getBoundingClientRect();
            const point =
                event.clientX === 0 && event.clientY === 0
                    ? (hover.current ?? {
                          x: rect.left + rect.width / 2,
                          y: rect.top + rect.height / 2,
                      })
                    : { x: event.clientX, y: event.clientY };
            hover.current = point;
            const anchorX = point.x - rect.left - rect.width / 2;
            const anchorY = point.y - rect.top - rect.height / 2;
            const factor = Math.exp(-event.deltaY * 0.0015);
            const minScale = fitScaleRef.current * MIN_ZOOM;
            const maxScale = fitScaleRef.current * MAX_ZOOM;
            setView((current) =>
                zoomView(
                    current,
                    current.scale * factor,
                    anchorX,
                    anchorY,
                    minScale,
                    maxScale
                )
            );
        };
        viewport.addEventListener("wheel", handleWheel, { passive: false });
        return () => viewport.removeEventListener("wheel", handleWheel);
    }, []);

    const zoomBy = (factor: number, animated = false) => {
        setAnimating(animated);
        setView((current) =>
            zoomView(
                current,
                current.scale * factor,
                0,
                0,
                fitScaleRef.current * MIN_ZOOM,
                fitScaleRef.current * MAX_ZOOM
            )
        );
    };

    const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
        if (event.button !== 0) return;
        setAnimating(false);
        event.currentTarget.setPointerCapture?.(event.pointerId);
        hover.current = { x: event.clientX, y: event.clientY };
        pointers.current.set(event.pointerId, {
            x: event.clientX,
            y: event.clientY,
        });
        pinch.current = null;
        setPanning(true);
    };

    const handlePointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
        hover.current = { x: event.clientX, y: event.clientY };
        const previous = pointers.current.get(event.pointerId);
        if (!previous) return;
        pointers.current.set(event.pointerId, {
            x: event.clientX,
            y: event.clientY,
        });

        if (pointers.current.size === 1) {
            setView((current) => ({
                ...current,
                x: current.x + event.clientX - previous.x,
                y: current.y + event.clientY - previous.y,
            }));
            return;
        }

        if (pointers.current.size !== 2) return;
        const [first, second] = [...pointers.current.values()];
        const distance = Math.hypot(second.x - first.x, second.y - first.y);
        const midpointX = (first.x + second.x) / 2;
        const midpointY = (first.y + second.y) / 2;
        const rect = viewportRef.current?.getBoundingClientRect();
        const last = pinch.current;
        pinch.current = { distance, x: midpointX, y: midpointY };
        if (!last || !rect || last.distance === 0) return;
        const factor = distance / last.distance;
        const anchorX = midpointX - rect.left - rect.width / 2;
        const anchorY = midpointY - rect.top - rect.height / 2;
        const shiftX = midpointX - last.x;
        const shiftY = midpointY - last.y;
        setView((current) => {
            const zoomed = zoomView(
                current,
                current.scale * factor,
                anchorX,
                anchorY,
                fitScaleRef.current * MIN_ZOOM,
                fitScaleRef.current * MAX_ZOOM
            );
            return {
                ...zoomed,
                x: zoomed.x + shiftX,
                y: zoomed.y + shiftY,
            };
        });
    };

    const handlePointerEnd = (event: ReactPointerEvent<HTMLDivElement>) => {
        pointers.current.delete(event.pointerId);
        pinch.current = null;
        if (pointers.current.size === 0) setPanning(false);
    };

    return (
        <div className="relative flex min-h-0 flex-1 flex-col">
            <div
                ref={viewportRef}
                data-slot="diagram-viewport"
                className={cn(
                    "relative flex min-h-0 flex-1 touch-none items-center justify-center overflow-hidden select-none",
                    panning ? "cursor-grabbing" : "cursor-grab"
                )}
                onPointerDown={handlePointerDown}
                onPointerMove={handlePointerMove}
                onPointerUp={handlePointerEnd}
                onPointerCancel={handlePointerEnd}
                onDoubleClick={resetView}
            >
                <div
                    data-slot="diagram-canvas"
                    className={cn(
                        "flex w-full origin-center items-center justify-center p-6",
                        animating &&
                            "transition-transform duration-200 ease-snappy motion-reduce:transition-none"
                    )}
                    style={{
                        transform: `translate(${view.x}px, ${view.y}px) scale(${view.scale})`,
                    }}
                    onTransitionEnd={(event) => {
                        if (
                            event.target === event.currentTarget &&
                            event.propertyName === "transform"
                        ) {
                            setAnimating(false);
                        }
                    }}
                >
                    <DiagramSvg
                        ref={diagramRef}
                        svg={svg}
                        className="flex items-center justify-center [&>svg]:shrink-0"
                    />
                </div>
            </div>
            <div className="pointer-events-none absolute inset-x-0 bottom-0 flex justify-center p-3">
                <InsetInput
                    aria-label="Diagram zoom"
                    className="pointer-events-auto w-42 backdrop-blur-xl"
                >
                    <InsetInputAddition align="inline-start">
                        <Button
                            aria-label="Zoom out"
                            disabled={view.scale <= fitScale * MIN_ZOOM}
                            onClick={() => zoomBy(1 / ZOOM_STEP, true)}
                            size="icon-sm"
                            variant="ghost"
                        >
                            <ZoomOut />
                        </Button>
                    </InsetInputAddition>
                    <InsetInputValue className="mx-auto font-mono">
                        <Scritto
                            value={`${Math.round((view.scale / fitScale) * 100)}%`}
                        />
                    </InsetInputValue>
                    <InsetInputAddition align="inline-end">
                        <Button
                            size="icon-sm"
                            variant="ghost"
                            aria-label="Zoom in"
                            disabled={view.scale >= fitScale * MAX_ZOOM}
                            onClick={() => zoomBy(ZOOM_STEP, true)}
                        >
                            <ZoomIn />
                        </Button>
                    </InsetInputAddition>
                </InsetInput>
            </div>
        </div>
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
    const fontSans = useSettingValue("fontSans");
    const font = fontFamilyStack(
        typeof fontSans === "string" ? fontSans : "",
        "sans-serif"
    );

    useEffect(() => {
        let live = true;
        void (async () => {
            const { renderAsync } = await import("pele/lazy");
            // Pele measures labels with the font it draws them in, so the
            // family has to be passed alongside the source.
            const result = await renderAsync(text, {
                fontFamily: font,
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
    }, [font, idPrefix, text]);

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
                <DialogPopup
                    showCloseButton
                    className="h-full w-[90%] max-w-none"
                >
                    <DialogTitle className="sr-only">Diagram</DialogTitle>
                    {svg !== null && <DiagramViewer svg={svg} />}
                </DialogPopup>
            </Dialog>
        </Frame>
    );
}
