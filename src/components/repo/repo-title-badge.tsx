import { useSelector } from "@tanstack/react-store";
import { ChevronRight } from "lucide-react";

import { useTitlebarTabId } from "@/contexts/tab-title-context";
import {
    formatRepoLabel,
    useRepoIdentity,
} from "@/hooks/repositories/use-repo-identity";
import { useRepositorySnapshot } from "@/hooks/repositories/use-repository-queries";
import { headBranch } from "@/lib/utils";
import { appStore } from "@/stores/app-store";
import { selectRepoIdByPath, repositoryStore } from "@/stores/repository-store";

import { RepoLabel } from "./repo-label";

/**
 * Live titlebar badge for repository tabs. Rendered prop-less by the
 * titlebar, so it resolves its own tab through TabTitleContext; the
 * component identity must stay render-stable (an inline closure would loop
 * useTabTitle forever).
 */
export function RepoTitleBadge() {
    const tabId = useTitlebarTabId();
    const tab = useSelector(appStore, (state) =>
        state.tabs.find((candidate) => candidate.tabId === tabId)
    );
    const repoId = useSelector(repositoryStore, (state) =>
        selectRepoIdByPath(state, tab?.repoPath)
    );

    const snapshot = useRepositorySnapshot(repoId);
    const identity = useRepoIdentity(repoId, tab?.repoPath ?? "");

    const repo = tab?.repoPath ? formatRepoLabel(identity) : "";
    const branch = headBranch(snapshot.data?.head);

    return (
        <div className="flex items-center gap-0.5">
            <RepoLabel repo={repo} />
            <ChevronRight className="size-2.5" />
            <span>{branch}</span>
        </div>
    );
}
