import { ChevronDown, LayoutGrid } from "lucide-react";

import { ToolbarTriggerFrame } from "@/components/repo/toolbar-trigger";
import {
    Menu,
    MenuPopup,
    MenuRadioGroup,
    MenuRadioItem,
    MenuShortcutBinding,
    MenuTrigger,
} from "@/components/ui/menu";
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
    view,
    onView,
}: {
    view: RepoView;
    onView: (view: RepoView) => void;
}) {
    return (
        <Menu>
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
                    {Object.entries(VIEW_LABELS).map(([key, label]) => {
                        const repoView = key as RepoView;

                        return (
                            <MenuRadioItem
                                key={repoView}
                                value={repoView}
                                onClick={() => onView(repoView)}
                                aria-selected={view === repoView || undefined}
                            >
                                {label}
                                <MenuShortcutBinding
                                    shortcut={REPO_VIEW_SHORTCUTS[repoView]}
                                />
                            </MenuRadioItem>
                        );
                    })}
                </MenuRadioGroup>
            </MenuPopup>
        </Menu>
    );
}
