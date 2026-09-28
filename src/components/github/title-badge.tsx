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

const ISSUE_PATH = /\/repo\/\d+\/issue\/(\d+)(?:[/?#]|$)/;
const PULL_PATH = /\/repo\/\d+\/pull\/(\d+)(?:[/?#]|$)/;

/** Repo label plus the thread number scraped from the tab's last resolved
 * href, which is the only record of which thread the tab is showing once the
 * route has scrolled out of the router's memory. */
function ThreadTitleBadge({
    pattern,
    icon,
}: {
    pattern: RegExp;
    icon: React.ReactNode;
}) {
    const tabId = useTitlebarTabId();
    const tab = useSelector(appStore, (state) =>
        state.tabs.find((candidate) => candidate.tabId === tabId)
    );
    const repoId = useSelector(repositoryStore, (state) =>
        selectRepoIdByPath(state, tab?.repoPath)
    );

    const identity = useRepoIdentity(repoId, tab?.repoPath ?? "");
    const repo = tab?.repoPath ? formatRepoLabel(identity) : "";
    const number = tab?.lastResolvedHref.match(pattern)?.[1] ?? "";

    return (
        <div className="flex items-center gap-0.5">
            <RepoLabel repo={repo} />
            {icon}
            <span>{number}</span>
        </div>
    );
}

export function IssueTitleBadge() {
    return (
        <ThreadTitleBadge
            pattern={ISSUE_PATH}
            icon={<Hash className="ml-0.5 size-2.5" />}
        />
    );
}

export function PullRequestTitleBadge() {
    return (
        <ThreadTitleBadge
            pattern={PULL_PATH}
            icon={<Hash className="ml-0.5 size-2.5" />}
        />
    );
}
