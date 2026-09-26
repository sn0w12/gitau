import { useSelector } from "@tanstack/react-store";
import { Hash } from "lucide-react";

import { useTitlebarTabId } from "@/contexts/tab-title-context";
import {
    formatRepoLabel,
    useRepoIdentity,
} from "@/hooks/repositories/use-repo-identity";
import { appStore } from "@/stores/app-store";
import { selectRepoIdByPath, repositoryStore } from "@/stores/repository-store";

import { RepoLabel } from "../repo/repo-label";

export function IssueTitleBadge() {
    const tabId = useTitlebarTabId();
    const tab = useSelector(appStore, (state) =>
        state.tabs.find((candidate) => candidate.tabId === tabId)
    );
    const repoId = useSelector(repositoryStore, (state) =>
        selectRepoIdByPath(state, tab?.repoPath)
    );

    const identity = useRepoIdentity(repoId, tab?.repoPath ?? "");
    const repo = tab?.repoPath ? formatRepoLabel(identity) : "";
    const issueNumber =
        tab?.lastResolvedHref.match(
            /\/repo\/\d+\/issue\/(\d+)(?:[/?#]|$)/
        )?.[1] ?? "";

    return (
        <div className="flex items-center gap-0.5">
            <RepoLabel repo={repo} />
            <Hash className="ml-0.5 size-2.5" />
            <span>{issueNumber}</span>
        </div>
    );
}
