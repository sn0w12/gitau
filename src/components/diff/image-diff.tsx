import * as React from "react";

import type { ImageDiffViewMode } from "@/hooks/changes/use-image-diff-view-mode";
import type { DiffImageSide } from "@/lib/backend/protocol";
import { cn } from "@/lib/utils";

export function BinaryDiffView({ path }: { path: string }) {
    return (
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 text-sm text-muted-foreground">
            <p className="font-medium">Binary diff</p>
            <p className="text-xs">{path} cannot be shown as text.</p>
        </div>
    );
}

const OLD_IMAGE_CLASS = "border-dashed border-destructive";
const NEW_IMAGE_CLASS = "border-success";

export function ImageDiffView({
    oldImage,
    newImage,
    mode,
}: {
    oldImage?: DiffImageSide;
    newImage?: DiffImageSide;
    mode: ImageDiffViewMode;
}) {
    const oldUrl = oldImage?.dataUrl;
    const newUrl = newImage?.dataUrl;
    if (mode === "swipe") {
        return <SwipeImage oldUrl={oldUrl} newUrl={newUrl} />;
    }
    if (mode === "difference") {
        return <DifferenceImage oldUrl={oldUrl} newUrl={newUrl} />;
    }
    return (
        <div className="flex min-h-0 flex-1 gap-2 overflow-auto p-3">
            <ImagePane label="Old" url={oldUrl} className={OLD_IMAGE_CLASS} />
            <ImagePane label="New" url={newUrl} className={NEW_IMAGE_CLASS} />
        </div>
    );
}

function ImagePane({
    label,
    url,
    className,
}: {
    label: string;
    url?: string;
    className?: string;
}) {
    return (
        <div className="flex min-w-0 flex-1 flex-col items-center gap-2">
            <span className="shrink-0 text-xs text-muted-foreground">
                {label}
            </span>
            <div className="flex min-h-0 w-full flex-1 items-center justify-center">
                {url ? (
                    <ImageWithFallback
                        src={url}
                        alt={`${label} version`}
                        className={className}
                    />
                ) : (
                    <span className="text-xs text-muted-foreground">
                        No image
                    </span>
                )}
            </div>
        </div>
    );
}

function ImageWithFallback({
    src,
    alt,
    className,
}: {
    src: string;
    alt: string;
    className?: string;
}) {
    const [failed, setFailed] = React.useState(false);
    if (failed) {
        return (
            <span className="text-xs text-muted-foreground">
                Image could not be displayed
            </span>
        );
    }
    return (
        <img
            src={src}
            alt=""
            draggable="false"
            aria-label={alt}
            onError={() => setFailed(true)}
            className={cn(
                "max-h-full max-w-full border-1 object-contain",
                className
            )}
        />
    );
}

function SwipeEmpty({ label, clipPath }: { label: string; clipPath?: string }) {
    return (
        <div
            aria-label={label}
            style={clipPath ? { clipPath } : undefined}
            className="absolute inset-0 flex size-full items-center justify-center text-xs text-muted-foreground"
        >
            No image
        </div>
    );
}

function SwipeImage({ oldUrl, newUrl }: { oldUrl?: string; newUrl?: string }) {
    const [position, setPosition] = React.useState(50);
    const ref = React.useRef<HTMLDivElement>(null);
    const update = React.useCallback((clientX: number) => {
        const rect = ref.current?.getBoundingClientRect();
        if (!rect || rect.width <= 0) return;
        setPosition(
            Math.max(
                0,
                Math.min(100, ((clientX - rect.left) / rect.width) * 100)
            )
        );
    }, []);
    const newClip = `inset(0 ${100 - position}% 0 0)`;
    const oldClip = `inset(0 0 0 ${position}%)`;
    return (
        <div className="flex min-h-0 min-w-0 flex-1 overflow-auto bg-muted/20 p-3">
            <div
                ref={ref}
                className="relative m-auto aspect-video w-full max-w-5xl touch-none select-none"
                onPointerMove={(event) =>
                    event.buttons > 0 && update(event.clientX)
                }
                onPointerDown={(event) => {
                    event.currentTarget.setPointerCapture(event.pointerId);
                    update(event.clientX);
                }}
                onKeyDown={(event) => {
                    if (event.key === "ArrowLeft")
                        setPosition((value) => Math.max(0, value - 5));
                    if (event.key === "ArrowRight")
                        setPosition((value) => Math.min(100, value + 5));
                    if (event.key === "Home") setPosition(0);
                    if (event.key === "End") setPosition(100);
                }}
                role="slider"
                tabIndex={0}
                aria-label="Image diff divider"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={position}
            >
                {oldUrl ? (
                    <img
                        src={oldUrl}
                        alt=""
                        draggable="false"
                        className="absolute inset-0 size-full object-contain"
                        style={{ clipPath: oldClip }}
                    />
                ) : (
                    <SwipeEmpty label="Old version" clipPath={oldClip} />
                )}
                {newUrl ? (
                    <img
                        src={newUrl}
                        alt=""
                        draggable="false"
                        className="absolute inset-0 size-full object-contain"
                        style={{ clipPath: newClip }}
                    />
                ) : (
                    <SwipeEmpty label="New version" clipPath={newClip} />
                )}
                <div
                    draggable="false"
                    className="absolute inset-y-0 w-0.5 -translate-x-1/2 bg-primary"
                    style={{ left: `${position}%` }}
                />
            </div>
        </div>
    );
}

function DifferenceImage({
    oldUrl,
    newUrl,
}: {
    oldUrl?: string;
    newUrl?: string;
}) {
    if (!oldUrl || !newUrl) {
        const url = newUrl ?? oldUrl;
        return (
            <div className="flex min-h-0 flex-1 items-center justify-center p-3">
                {url ? (
                    <ImageWithFallback
                        src={url}
                        alt="Image"
                        className={newUrl ? NEW_IMAGE_CLASS : OLD_IMAGE_CLASS}
                    />
                ) : (
                    <span className="text-xs text-muted-foreground">
                        No image
                    </span>
                )}
            </div>
        );
    }
    return (
        <div className="flex min-h-0 min-w-0 flex-1 p-3 invert dark:invert-0">
            <div className="relative min-h-0 min-w-0 flex-1">
                <img
                    src={oldUrl}
                    alt=""
                    draggable="false"
                    className="absolute top-1/2 left-1/2 max-h-full max-w-full -translate-x-1/2 -translate-y-1/2 bg-black object-contain dark:bg-background"
                />
                <img
                    src={newUrl}
                    alt=""
                    draggable="false"
                    className="absolute top-1/2 left-1/2 max-h-full max-w-full -translate-x-1/2 -translate-y-1/2 object-contain mix-blend-difference"
                />
            </div>
        </div>
    );
}
