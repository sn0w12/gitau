import { Check, ChevronDown, GitPullRequest, LayoutGrid } from "lucide-react";

import {
    Menu,
    MenuItem,
    MenuPopup,
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
                <MenuItem
                    onClick={() => onView("overview")}
                    aria-selected={view === "overview" || undefined}
                >
                    <Check
                        className={cn(
                            "size-3.5 shrink-0",
                            view === "overview" ? "opacity-100" : "opacity-0"
                        )}
                    />
                    <span className="min-w-0 flex-1 truncate pl-1">
                        Overview
                    </span>
                </MenuItem>
                <MenuItem
                    onClick={() => onView("graph")}
                    aria-selected={view === "graph" || undefined}
                >
                    <Check
                        className={cn(
                            "size-3.5 shrink-0",
                            view === "graph" ? "opacity-100" : "opacity-0"
                        )}
                    />
                    <span className="min-w-0 flex-1 truncate pl-1">
                        Commit Graph
                    </span>
                </MenuItem>
                <MenuItem
                    onClick={() => onView("issues")}
                    aria-selected={view === "issues" || undefined}
                >
                    <Check
                        className={cn(
                            "size-3.5 shrink-0",
                            view === "issues" ? "opacity-100" : "opacity-0"
                        )}
                    />
                    <span className="min-w-0 flex-1 truncate pl-1">Issues</span>
                </MenuItem>
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
