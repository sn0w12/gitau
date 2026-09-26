import { ChevronDown, GitPullRequest, LayoutGrid } from "lucide-react";

import {
    Menu,
    MenuItem,
    MenuPopup,
    MenuRadioGroup,
    MenuRadioItem,
    MenuSeparator,
    MenuTrigger,
} from "@/components/ui/menu";
import { BORDER_GRADIENT, REPO_TOOLBAR_TRIGGER_CLASS } from "@/lib/constants";
import { cn } from "@/lib/utils";
import type { RepoView } from "@/routes/repo-page";

const VIEW_LABELS: Record<RepoView, string> = {
    overview: "Overview",
    graph: "Commit Graph",
    issues: "Issues",
};

/**
 * The right-panel view menu: switches the main area between the available
 * views. Pull requests stay a placeholder until their backend exists.
 */
export function RepoViewSwitcher({
    view,
    onView,
}: {
    view: RepoView;
    onView: (view: RepoView) => void;
}) {
    return (
        <Menu>
            <MenuTrigger
                className={cn(REPO_TOOLBAR_TRIGGER_CLASS, BORDER_GRADIENT)}
                aria-label="Current view"
            >
                <span className="flex items-center gap-2.5 pr-2">
                    <LayoutGrid className="size-7" strokeWidth="1.5px" />
                    <span className="flex flex-col">
                        <span className="hidden text-start text-xs text-muted-foreground lg:block">
                            Current View
                        </span>
                        <span className="font-semibold">
                            {VIEW_LABELS[view]}
                        </span>
                    </span>
                </span>
                <ChevronDown className="size-4" />
            </MenuTrigger>
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
                            </MenuRadioItem>
                        );
                    })}
                </MenuRadioGroup>
                <MenuSeparator />
                <MenuItem disabled>
                    <GitPullRequest />
                    <span className="min-w-0 flex-1 truncate">
                        Pull requests
                    </span>
                    <span className="ms-auto text-xs text-muted-foreground">
                        Soon
                    </span>
                </MenuItem>
            </MenuPopup>
        </Menu>
    );
}
