"use no memo";

import { GitPullRequestArrow, RotateCw } from "lucide-react";
import { useMemo } from "react";

import { ExternalLink } from "@/components/external-link";
import { LabelBadge } from "@/components/github/label-badge";
import {
    pullRequestStatusOf,
    StatusBadge,
} from "@/components/github/status-badge";
import { Button } from "@/components/ui/button";
import {
    Empty,
    EmptyContent,
    EmptyDescription,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
} from "@/components/ui/empty";
import {
    Frame,
    FrameDescription,
    FrameHeader,
    FramePanel,
    FrameTitle,
} from "@/components/ui/frame";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { useGithubAccount } from "@/hooks/github/use-github-account";
import { useLocalRepoMap } from "@/hooks/github/use-github-issues";
import { useGithubSearchPullRequests } from "@/hooks/github/use-github-pull-requests";
import { useOpenThread } from "@/hooks/github/use-open-thread";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import type { GithubPullRequestListItem } from "@/lib/backend/protocol";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import { parsePullUrl } from "@/lib/github/repo-coords";
import { formatRelativeDate } from "@/lib/utils";

/**
 * Signed-in GitHub pull requests matching `is:pr involves:@me
 * sort:updated-desc`, newest first, paged from the backend 100 items at a
 * time. Styled like the inbox and issues pages.
 */
export function PullsPage() {
    const router = useActiveTabRouter();
    const account = useGithubAccount();
    const search = useGithubSearchPullRequests();
    const localRepos = useLocalRepoMap();

    const pulls = search.pulls;
    const groups = useMemo(() => {
        const byRepo = new Map<string, GithubPullRequestListItem[]>();
        for (const pull of pulls) {
            const key = pull.repoFullName || "Unknown repository";
            const list = byRepo.get(key);
            if (list) list.push(pull);
            else byRepo.set(key, [pull]);
        }
        return [...byRepo.entries()];
    }, [pulls]);
    const showScopeHint =
        search.error instanceof GitBackendError && !search.error.retryable;

    const loadMore = search.hasNextPage ? (
        <div className="flex justify-center py-3">
            <Button
                variant="outline"
                size="sm"
                disabled={search.isFetchingNextPage}
                loading={search.isFetchingNextPage}
                onClick={() => void search.fetchNextPage()}
            >
                Load more
            </Button>
        </div>
    ) : null;

    return (
        <div className="container h-full min-h-0 p-2">
            <Frame className="mx-auto flex h-full max-w-2xl flex-col gap-1 p-1">
                <FrameHeader className="px-3 py-1.5">
                    <div className="flex items-center gap-2">
                        <FrameTitle className="text-base font-semibold">
                            Pull Requests
                        </FrameTitle>
                        <div className="ml-auto flex items-center gap-2">
                            <Button
                                variant="ghost"
                                size="icon-sm"
                                aria-label="Refresh pull requests"
                                disabled={search.isFetching}
                                loading={search.isFetching}
                                onClick={() => void search.refetch()}
                            >
                                <RotateCw />
                            </Button>
                        </div>
                    </div>
                    <FrameDescription className="text-sm text-muted-foreground">
                        GitHub pull requests involving you, sorted by most
                        recently updated.
                    </FrameDescription>
                </FrameHeader>
                <FramePanel data-testid="pulls-panel" className="min-h-0 p-2">
                    <ScrollArea scrollFade fill>
                        {account.isLoading ? (
                            <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
                                <Spinner className="size-4" />
                                Checking connection...
                            </div>
                        ) : !account.data ? (
                            <Empty>
                                <EmptyMedia variant="icon">
                                    <GitPullRequestArrow />
                                </EmptyMedia>
                                <EmptyHeader>
                                    <EmptyTitle>Connect GitHub</EmptyTitle>
                                    <EmptyDescription>
                                        Sign in to see pull requests that
                                        involve you.
                                    </EmptyDescription>
                                </EmptyHeader>
                                <EmptyContent>
                                    <Button
                                        onClick={() =>
                                            void router?.navigate({
                                                to: "/account",
                                            })
                                        }
                                    >
                                        Go to Account
                                    </Button>
                                </EmptyContent>
                            </Empty>
                        ) : search.isLoading ? (
                            <div className="flex flex-col gap-2 py-2">
                                {[0, 1, 2, 3].map((index) => (
                                    <Skeleton
                                        key={index}
                                        className="h-14 w-full"
                                    />
                                ))}
                            </div>
                        ) : search.isError ? (
                            <div className="flex flex-col items-center gap-2 py-8 text-center">
                                <p
                                    className="text-sm text-destructive"
                                    role="alert"
                                >
                                    {search.error instanceof Error
                                        ? search.error.message
                                        : "Could not load pull requests"}
                                </p>
                                {showScopeHint ? (
                                    <p className="max-w-sm text-sm text-muted-foreground">
                                        Tokens granted before the repo
                                        permission existed cannot search pull
                                        requests. Sign out and back in on the
                                        Account page to grant it.
                                    </p>
                                ) : null}
                                <div className="flex items-center gap-2">
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={() => void search.refetch()}
                                    >
                                        Retry
                                    </Button>
                                    {showScopeHint ? (
                                        <Button
                                            variant="outline"
                                            size="sm"
                                            onClick={() =>
                                                void router?.navigate({
                                                    to: "/account",
                                                })
                                            }
                                        >
                                            Go to Account
                                        </Button>
                                    ) : null}
                                </div>
                            </div>
                        ) : pulls.length === 0 ? (
                            <div className="flex flex-col">
                                <Empty>
                                    <EmptyMedia variant="icon">
                                        <GitPullRequestArrow />
                                    </EmptyMedia>
                                    <EmptyHeader>
                                        <EmptyTitle>
                                            No pull requests found
                                        </EmptyTitle>
                                        <EmptyDescription>
                                            No pull requests involving you on
                                            the loaded pages.
                                        </EmptyDescription>
                                    </EmptyHeader>
                                </Empty>
                                {loadMore}
                            </div>
                        ) : (
                            <div className="flex flex-col">
                                {groups.map(([repo, repoPulls]) => (
                                    <section key={repo}>
                                        <h2 className="px-2 pt-2 pb-1 text-sm font-semibold text-muted-foreground">
                                            {repo}
                                        </h2>
                                        {repoPulls.map((pull, index) => (
                                            <div
                                                key={`${pull.repoFullName}-${pull.number}`}
                                            >
                                                {index > 0 ? (
                                                    <Separator />
                                                ) : null}
                                                <PullsRow
                                                    pull={pull}
                                                    localRepos={localRepos}
                                                />
                                            </div>
                                        ))}
                                    </section>
                                ))}
                                {loadMore}
                            </div>
                        )}
                    </ScrollArea>
                </FramePanel>
            </Frame>
        </div>
    );
}

function PullsRow({
    pull,
    localRepos,
}: {
    pull: GithubPullRequestListItem;
    localRepos: Map<string, string>;
}) {
    const openThread = useOpenThread();

    const target = parsePullUrl(pull.htmlUrl);
    const repoPath = target
        ? localRepos.get(
              `${target.owner.toLowerCase()}/${target.repo.toLowerCase()}`
          )
        : undefined;

    const title =
        repoPath !== undefined ? (
            <button
                type="button"
                onClick={() => void openThread(repoPath, "pull", pull.number)}
                className="flex min-w-0 cursor-pointer items-center gap-1 truncate text-left font-medium hover:underline"
            >
                <span className="truncate">{pull.title || "(no title)"}</span>
            </button>
        ) : (
            <ExternalLink href={pull.htmlUrl} className="truncate font-medium">
                {pull.title || "(no title)"}
            </ExternalLink>
        );

    return (
        <div className="flex items-center gap-2 px-2 py-2">
            <StatusBadge
                status={pullRequestStatusOf(pull)}
                className="shrink-0"
            />
            <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                    <span className="truncate text-sm">{title}</span>
                    <div className="flex shrink-0 flex-wrap gap-1">
                        {pull.labels.map((label) => (
                            <LabelBadge key={label.name} label={label} />
                        ))}
                    </div>
                </div>
                <p className="truncate text-xs text-muted-foreground">
                    #{pull.number}
                    {pull.author.login ? ` · ${pull.author.login}` : ""}
                    {pull.updatedAt
                        ? ` · ${formatRelativeDate(pull.updatedAt)}`
                        : ""}
                </p>
            </div>
        </div>
    );
}
