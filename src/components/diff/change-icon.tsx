import {
    Asterisk,
    ChevronRight,
    Copy,
    Dot,
    Plus,
    Repeat2,
    Squircle,
    X,
} from "lucide-react";

import type { ChangeKind } from "@/lib/backend/protocol";
import { cn } from "@/lib/utils";

const CHANGE_COLOR: Record<ChangeKind, string> = {
    added: "text-success",
    conflicted: "text-destructive",
    copied: "text-info",
    deleted: "text-destructive",
    modified: "text-warning",
    renamed: "text-info",
    typeChanged: "text-warning",
    untracked: "text-muted-foreground",
} as const;

const CHANGE_ICON: Record<ChangeKind, React.ElementType> = {
    added: Plus,
    conflicted: Asterisk,
    copied: Copy,
    deleted: X,
    modified: Dot,
    renamed: ChevronRight,
    typeChanged: Repeat2,
    untracked: Dot,
} as const;

export function ChangeIcon({
    change,
    className,
}: {
    change: ChangeKind;
    className?: string;
}) {
    const Icon = CHANGE_ICON[change];

    return (
        <Squircle
            size={24}
            className={cn("size-5 shrink-0", CHANGE_COLOR[change], className)}
        >
            <Icon size={16} x={4} y={4} className={CHANGE_COLOR[change]} />
        </Squircle>
    );
}
