import {
    CornerUpRight,
    FolderOpen,
    Pin,
    PinOff,
    SquareArrowOutUpRight,
    SquarePen,
    Terminal,
    Trash,
} from "lucide-react";
import type * as React from "react";

import { useOpenInEditor } from "@/hooks/repositories/use-open-in-editor";
import { useOpenInTerminal } from "@/hooks/repositories/use-open-in-terminal";
import { useOpenRepository } from "@/hooks/repositories/use-open-repository";
import { useRemoveRepository } from "@/hooks/repositories/use-remove-repository";
import { repoDisplayName } from "@/hooks/repositories/use-repo-identity";
import { useRevealInFileManager } from "@/hooks/repositories/use-reveal-in-file-manager";
import { useSettingValue } from "@/hooks/settings/use-setting";
import { useOpenInNewTab } from "@/hooks/tabs/use-open-in-new-tab";
import { toastError } from "@/lib/toast-error";
import { setSetting } from "@/stores/settings-store";

import {
    ContextMenu,
    ContextMenuItem,
    ContextMenuPopup,
    ContextMenuSeparator,
    ContextMenuTrigger,
} from "../ui/context-menu";

interface RepoContextMenuProps {
    repoPath: string;
    children: React.ReactElement;
    side?: "top" | "bottom" | "left" | "right";
}

/**
 * Right-click menu for a repository entry: shared by the sidebar and the
 * homepage so both surfaces offer the same actions. Removal always
 * confirms first; failures toast and leave the repo listed.
 */
export function RepoContextMenu({
    repoPath,
    children,
    side = "right",
}: RepoContextMenuProps): React.ReactElement {
    const removeRepo = useRemoveRepository();
    const openRepo = useOpenRepository();
    const openInNewTab = useOpenInNewTab();
    const { enabled, openInEditor } = useOpenInEditor();
    const openInTerminal = useOpenInTerminal();
    const revealInFileManager = useRevealInFileManager();
    const pinnedRepos = useSettingValue("pinnedRepos") ?? [];

    const isPinned = pinnedRepos.some(
        (path) => path.toLowerCase() === repoPath.toLowerCase()
    );

    const togglePin = () => {
        const canonical = pinnedRepos.find(
            (path) => path.toLowerCase() === repoPath.toLowerCase()
        );
        setSetting(
            "pinnedRepos",
            isPinned
                ? pinnedRepos.filter((path) => path !== canonical)
                : [...pinnedRepos, repoPath]
        );
    };

    const handleRemove = async () => {
        const outcome = await removeRepo(repoPath);
        if (outcome.status === "failed") {
            toastError("Could not remove repository", outcome.error);
        }
    };

    return (
        <ContextMenu>
            <ContextMenuTrigger render={children} />
            <ContextMenuPopup side={side}>
                <ContextMenuItem
                    onClick={() => void openRepo(repoPath)}
                    data-testid="repo-menu-open"
                >
                    <CornerUpRight />
                    Open
                </ContextMenuItem>
                <ContextMenuItem
                    onClick={() =>
                        void openInNewTab({
                            title: repoDisplayName(repoPath),
                            repoPath,
                        })
                    }
                    data-testid="repo-menu-open-in-new-tab"
                >
                    <SquareArrowOutUpRight />
                    Open in new tab
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                    disabled={!enabled}
                    onClick={() => void openInEditor(repoPath)}
                    data-testid="repo-menu-open-in-editor"
                >
                    <SquarePen />
                    Open in editor
                </ContextMenuItem>
                <ContextMenuItem
                    onClick={() => void openInTerminal(repoPath)}
                    data-testid="repo-menu-open-in-terminal"
                >
                    <Terminal />
                    Open in terminal
                </ContextMenuItem>
                <ContextMenuItem
                    onClick={() => void revealInFileManager(repoPath)}
                    data-testid="repo-menu-reveal-in-file-manager"
                >
                    <FolderOpen />
                    Reveal in file manager
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                    onClick={togglePin}
                    data-testid="repo-menu-pin"
                >
                    {isPinned ? <PinOff /> : <Pin />}
                    {isPinned ? "Unpin" : "Pin"}
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                    variant="destructive"
                    onClick={() => void handleRemove()}
                >
                    <Trash />
                    Remove
                </ContextMenuItem>
            </ContextMenuPopup>
        </ContextMenu>
    );
}
