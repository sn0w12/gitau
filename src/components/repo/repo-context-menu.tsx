import { FolderOpen, Pin, PinOff, SquarePen, Trash } from "lucide-react";
import type * as React from "react";

import { useOpenInEditor } from "@/hooks/repositories/use-open-in-editor";
import { useRemoveRepository } from "@/hooks/repositories/use-remove-repository";
import { useRevealInFileManager } from "@/hooks/repositories/use-reveal-in-file-manager";
import { useSettingValue } from "@/hooks/settings/use-setting";
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
    const { enabled, openInEditor } = useOpenInEditor();
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
                    disabled={!enabled}
                    onClick={() => void openInEditor(repoPath)}
                    data-testid="repo-menu-open-in-editor"
                >
                    <SquarePen />
                    Open in editor
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
