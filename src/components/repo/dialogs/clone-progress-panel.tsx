import type { ClonePhase, CloneProgress } from "@/lib/backend/protocol";

import { GridProgress } from "../../ui/grid-progress";

const PHASE_LABELS: Record<ClonePhase, string> = {
    counting: "Counting objects",
    receiving: "Receiving objects",
    resolving: "Resolving deltas",
    checkingOut: "Checking out files",
};

/** Live transfer view for one running clone; pure presentation. */
export function CloneProgressPanel({
    progress,
}: {
    progress: CloneProgress | null;
}) {
    const phase: ClonePhase = progress?.phase ?? "counting";
    const indeterminate = !progress || progress.progress <= 0;

    return (
        <div className="flex flex-col gap-1">
            <div className="flex items-baseline justify-between">
                <span data-testid="clone-phase">{PHASE_LABELS[phase]}</span>
                {progress && progress.objectsTotal !== null ? (
                    <span
                        className="font-mono text-xs text-muted-foreground tabular-nums"
                        data-testid="clone-objects"
                    >
                        {progress.objectsReceived.toLocaleString()} /{" "}
                        {progress.objectsTotal.toLocaleString()}
                    </span>
                ) : null}
            </div>
            <GridProgress
                value={
                    indeterminate ? 0 : Math.min(progress!.progress, 1) * 100
                }
                size={{ cols: 32, rows: 8 }}
            />
        </div>
    );
}

export function formatBytes(bytes: number): string {
    if (bytes <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    const exponent = Math.min(
        Math.floor(Math.log(bytes) / Math.log(1024)),
        units.length - 1
    );
    const value = bytes / 1024 ** exponent;
    return `${exponent === 0 ? value : value.toFixed(1)} ${units[exponent]}`;
}
