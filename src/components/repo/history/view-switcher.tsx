import { ChevronDown, LayoutGrid } from "lucide-react";
import { useState } from "react";

import { ToolbarTriggerFrame } from "@/components/repo/toolbar-trigger";
import {
    Menu,
    MenuPopup,
    MenuRadioGroup,
    MenuRadioItem,
    MenuShortcutBinding,
    MenuTrigger,
} from "@/components/ui/menu";
import { capabilityAllows } from "@/hooks/forge/use-forge-account";
import { useRepoCapabilities } from "@/hooks/forge/use-forge-issues";
import { REPO_TOOLBAR_TRIGGER_CLASS } from "@/lib/constants";
import type { RepoView } from "@/lib/routing/repo-search";
import { REPO_VIEW_SHORTCUTS } from "@/lib/shortcuts";

const VIEW_LABELS: Record<RepoView, string> = {
    overview: "Overview",
    graph: "Commit Graph",
    issues: "Issues",
    pulls: "Pull requests",
};

/** The right-panel view menu: switches the main area between the available
 * views. */
export function RepoViewSwitcher({
    repoId,
    view,
    onView,
}: {
    repoId: number;
    view: RepoView;
    onView: (view: RepoView) => void;
}) {
    const [open, setOpen] = useState(false);
    const capabilities = useRepoCapabilities(repoId);

    // A view the connected provider cannot serve stays in the menu so the set
    // of views never shifts; it is disabled rather than hidden.
    const available = (candidate: RepoView): boolean => {
        if (candidate === "issues")
            return capabilityAllows(capabilities, "issues");
        if (candidate === "pulls")
            return capabilityAllows(capabilities, "pullRequests");
        return true;
    };

    return (
        <Menu open={open} onOpenChange={setOpen}>
            <ToolbarTriggerFrame>
                <MenuTrigger
                    className={REPO_TOOLBAR_TRIGGER_CLASS}
                    aria-label="Current view"
                >
                    <span className="flex items-center gap-2.5 pr-2">
                        <LayoutGrid className="size-7" strokeWidth="1.5px" />
                        <span className="flex flex-col">
                            <span className="hidden text-start text-xs text-muted-foreground lg:block">
                                Current View
                            </span>
                            <span className="text-start font-semibold">
                                {VIEW_LABELS[view]}
                            </span>
                        </span>
                    </span>
                    <ChevronDown className="size-4" />
                </MenuTrigger>
            </ToolbarTriggerFrame>
            <MenuPopup align="center">
                <MenuRadioGroup value={view}>
                    {(Object.entries(VIEW_LABELS) as [RepoView, string][]).map(
                        ([repoView, label]) => {
                            return (
                                <MenuRadioItem
                                    key={repoView}
                                    value={repoView}
                                    disabled={!available(repoView)}
                                    onClick={() => {
                                        onView(repoView);
                                        setOpen(false);
                                    }}
                                    aria-selected={
                                        view === repoView || undefined
                                    }
                                >
                                    {label}
                                    <MenuShortcutBinding
                                        shortcut={REPO_VIEW_SHORTCUTS[repoView]}
                                    />
                                </MenuRadioItem>
                            );
                        }
                    )}
                </MenuRadioGroup>
            </MenuPopup>
        </Menu>
    );
}
