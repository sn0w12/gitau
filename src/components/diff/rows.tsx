import type { DiffRow } from "@/lib/backend/protocol";

export const DIFF_ROW_HEIGHT = 20;

export const KIND_ROW_CLASS: Record<DiffRow["kind"], string> = {
    addition: "bg-success/12",
    deletion: "bg-destructive/12",
    context: "",
    hunkHeader: "bg-muted/60 text-muted-foreground",
    fileHeader: "bg-muted font-semibold border-b",
    binaryNotice: "bg-muted/60 text-muted-foreground italic",
};

export const KIND_ROW_BORDER_CLASS: Record<DiffRow["kind"], string> = {
    addition: "border-success",
    deletion: "border-destructive border-slashed",
    context: "border-transparent",
    hunkHeader: "border-muted",
    fileHeader: "border-muted",
    binaryNotice: "border-muted",
};

export function Lineno({ value }: { value?: number }) {
    return (
        <span className="inline-block w-10 shrink-0 pr-2 text-right text-muted-foreground/70 select-none">
            {value ?? ""}
        </span>
    );
}
