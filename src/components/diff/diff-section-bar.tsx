import { ChevronsDownUp, ChevronsUpDown, Settings, X } from "lucide-react";
import * as React from "react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import type { DiffViewMode } from "@/hooks/changes/use-diff-view-mode";
import type { ImageDiffViewMode } from "@/hooks/changes/use-image-diff-view-mode";
import { useSetting, useSettingOptions } from "@/hooks/settings/use-setting";
import type { SectionKind } from "@/lib/backend/protocol";
import type { RepoStateAction } from "@/lib/routing/repo-search";
import { cn } from "@/lib/utils";

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
import { ScrollArea } from "../ui/scroll-area";
import { Swap } from "../ui/swap";
import { ChangeIcon } from "./change-icon";

export function DiffSectionBar({
    path,
    oldPath,
    kind,
    isCommitView = false,
    commitMessage,
    commitDescription,
    mode,
    onModeChange,
    imageMode,
    onImageModeChange,
    dispatch,
}: {
    path: string;
    oldPath?: string;
    kind: SectionKind;
    isCommitView?: boolean;
    commitMessage?: string;
    commitDescription?: string;
    mode: DiffViewMode;
    onModeChange: (mode: DiffViewMode) => void;
    imageMode?: ImageDiffViewMode;
    onImageModeChange?: (mode: ImageDiffViewMode) => void;
    dispatch: React.ActionDispatch<[action: RepoStateAction]>;
}) {
    const [open, setOpen] = useState(false);
    const { value: commitFileListOpen } = useSetting("commitFileListOpen");
    const diffOptions = useSettingOptions("diffViewMode");
    const imageOptions = useSettingOptions("imageDiffViewMode");

    const showCommit = commitFileListOpen && isCommitView;
    const showDescription = showCommit && open && Boolean(commitDescription);

    return (
        <div
            className={cn(
                "relative flex shrink-0 items-start gap-1 border-b pt-1 pl-1 transition-[height] duration-200 ease-snappy",
                showDescription ? "h-16 pb-1" : "h-8"
            )}
        >
            <Swap
                when={showCommit}
                align="start"
                className="mx-1 h-full min-w-0 flex-1 grid-rows-1 text-xs *:self-stretch"
                on={
                    <div className="flex h-full min-h-0 flex-col">
                        <div className="flex shrink-0 flex-row items-center gap-1.5 pt-0.5">
                            <span className="min-w-0 flex-1 truncate pt-px">
                                {commitMessage}
                            </span>
                            {commitDescription && (
                                <Button
                                    size="icon-2xs"
                                    variant="outline"
                                    aria-label={
                                        open
                                            ? "Hide commit description"
                                            : "Show commit description"
                                    }
                                    onClick={() => {
                                        setOpen(!open);
                                    }}
                                >
                                    <Swap
                                        when={open}
                                        on={<ChevronsDownUp />}
                                        off={<ChevronsUpDown />}
                                    />
                                </Button>
                            )}
                        </div>
                        <Swap
                            when={showDescription}
                            className="min-h-0 flex-1 grid-rows-1 *:self-stretch"
                            on={
                                <ScrollArea className="pt-0.5 whitespace-pre-wrap text-muted-foreground">
                                    {commitDescription}
                                </ScrollArea>
                            }
                            off={null}
                        />
                    </div>
                }
                off={
                    <div className="flex w-full items-center gap-1 pt-0.75 font-mono">
                        {oldPath && (
                            <span className="min-w-0 truncate text-muted-foreground">
                                {oldPath} →
                            </span>
                        )}
                        <SplitPath path={path} />
                    </div>
                }
            />

            <div className="absolute right-1 flex gap-0.5">
                <ChangeIcon change={kind} className="mt-0.5" />
                <Menu>
                    <MenuTrigger
                        render={
                            <Button
                                size="icon-xs"
                                variant="outline"
                                aria-label="Diff view settings"
                            >
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
                                    {imageOptions.map((option) => (
                                        <MenuRadioItem
                                            key={option.value}
                                            value={option.value}
                                        >
                                            {option.label}
                                        </MenuRadioItem>
                                    ))}
                                </MenuRadioGroup>
                            ) : (
                                <MenuRadioGroup
                                    value={mode}
                                    onValueChange={(value) => {
                                        onModeChange(value as DiffViewMode);
                                    }}
                                >
                                    {diffOptions.map((option) => (
                                        <MenuRadioItem
                                            key={option.value}
                                            value={option.value}
                                        >
                                            {option.label}
                                        </MenuRadioItem>
                                    ))}
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
