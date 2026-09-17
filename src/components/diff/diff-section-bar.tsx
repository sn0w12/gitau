import { Settings, X } from "lucide-react";
import * as React from "react";

import { Button } from "@/components/ui/button";
import type { DiffViewMode } from "@/hooks/changes/use-diff-view-mode";
import type { ImageDiffViewMode } from "@/hooks/changes/use-image-diff-view-mode";
import type { SectionKind } from "@/lib/backend/protocol";
import type { reducerAction } from "@/routes/repo-page";

import { SplitPath } from "../repo/split-path";
import {
    Menu,
    MenuGroup,
    MenuGroupLabel,
    MenuPopup,
    MenuRadioGroup,
    MenuRadioItem,
    MenuTrigger,
} from "../ui/menu";
import { ChangeIcon } from "./change-icon";

export function DiffSectionBar({
    path,
    oldPath,
    kind,
    mode,
    onModeChange,
    imageMode,
    onImageModeChange,
    dispatch,
}: {
    path: string;
    oldPath?: string;
    kind: SectionKind;
    mode: DiffViewMode;
    onModeChange: (mode: DiffViewMode) => void;
    imageMode?: ImageDiffViewMode;
    onImageModeChange?: (mode: ImageDiffViewMode) => void;
    dispatch: React.ActionDispatch<[action: reducerAction]>;
}) {
    return (
        <div className="flex h-8 shrink-0 items-center gap-1 border-b px-1">
            <div className="mx-1 flex min-w-0 flex-1 items-baseline gap-1 font-mono text-xs">
                {oldPath && (
                    <span className="min-w-0 truncate text-muted-foreground">
                        {oldPath} →
                    </span>
                )}
                <SplitPath path={path} />
            </div>

            <ChangeIcon change={kind} />
            <Menu>
                <MenuTrigger
                    render={
                        <Button size="icon-xs" variant="outline">
                            <Settings />
                        </Button>
                    }
                />
                <MenuPopup>
                    <MenuGroup>
                        <MenuGroupLabel>View</MenuGroupLabel>
                        {imageMode && onImageModeChange ? (
                            <MenuRadioGroup
                                value={imageMode}
                                onValueChange={(value) => {
                                    onImageModeChange(
                                        value as ImageDiffViewMode
                                    );
                                }}
                            >
                                <MenuRadioItem value="sideBySide">
                                    Side by side
                                </MenuRadioItem>
                                <MenuRadioItem value="swipe">
                                    Swipe
                                </MenuRadioItem>
                            </MenuRadioGroup>
                        ) : (
                            <MenuRadioGroup
                                value={mode}
                                onValueChange={(value) => {
                                    onModeChange(value as DiffViewMode);
                                }}
                            >
                                <MenuRadioItem value="unified">
                                    Unified
                                </MenuRadioItem>
                                <MenuRadioItem value="split">
                                    Split
                                </MenuRadioItem>
                            </MenuRadioGroup>
                        )}
                    </MenuGroup>
                </MenuPopup>
            </Menu>
            <Button
                size="icon-xs"
                variant="outline"
                onClick={() => {
                    dispatch({ type: "CLEAR_SELECTION" });
                }}
            >
                <X />
            </Button>
        </div>
    );
}

export function useClampedIndex(
    length: number,
    resetKey: string
): [number, React.Dispatch<React.SetStateAction<number>>] {
    const [state, setState] = React.useState<{
        resetKey: string;
        index: number;
    }>({ resetKey, index: 0 });

    // Derived reset: switching sections reads as 0 without an effect or a
    // stored write; the next selection persists under the new key.
    const base = state.resetKey === resetKey ? state.index : 0;
    const index = Math.min(base, Math.max(0, length - 1));

    const setIndex = React.useCallback(
        (value: React.SetStateAction<number>) => {
            setState((prev) => {
                const prevBase = prev.resetKey === resetKey ? prev.index : 0;
                const next =
                    typeof value === "function"
                        ? (value as (prev: number) => number)(prevBase)
                        : value;
                if (prev.resetKey === resetKey && prev.index === next) {
                    return prev;
                }
                return { resetKey, index: next };
            });
        },
        [resetKey]
    );

    return [index, setIndex];
}
