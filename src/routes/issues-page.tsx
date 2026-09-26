"use no memo";

import { CircleDot, Inbox, RotateCw } from "lucide-react";
import { useMemo } from "react";

import { ExternalLink } from "@/components/external-link";
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
import {
    useGithubSearchIssues,
    useLocalIssueRepoMap,
} from "@/hooks/github/use-github-issues";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import type { SearchIssueItem } from "@/lib/backend/protocol";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import { parseIssueUrl } from "@/lib/github/repo-coords";
import { formatRelativeDate } from "@/lib/utils";

/**
 * Signed-in GitHub issues matching `is:issue involves:@me
 * sort:updated-desc`, newest first, paged from the backend 100 items
 * at a time. Styled like the inbox page.
 */
export function IssuesPage() {
    const router = useActiveTabRouter();
    const account = useGithubAccount();
    const search = useGithubSearchIssues();
    const localIssueRepos = useLocalIssueRepoMap();

    const issues = search.issues;
    const groups = useMemo(() => {
        const byRepo = new Map<string, SearchIssueItem[]>();
        for (const issue of issues) {
            const key = issue.repoFullName || "Unknown repository";
            const list = byRepo.get(key);
            if (list) list.push(issue);
            else byRepo.set(key, [issue]);
        }
        return [...byRepo.entries()];
    }, [issues]);
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
                            Issues
                        </FrameTitle>
                        <div className="ml-auto flex items-center gap-2">
                            <Button
                                variant="ghost"
                                size="icon-sm"
                                aria-label="Refresh issues"
                                disabled={search.isFetching}
                                loading={search.isFetching}
                                onClick={() => void search.refetch()}
                            >
                                <RotateCw />
                            </Button>
                        </div>
                    </div>
                    <FrameDescription className="text-sm text-muted-foreground">
                        GitHub issues involving you, sorted by most recently
                        updated.
                    </FrameDescription>
                </FrameHeader>
                <FramePanel data-testid="issues-panel" className="min-h-0 p-2">
                    <ScrollArea scrollFade fill>
                        {account.isLoading ? (
                            <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
                                <Spinner className="size-4" />
                                Checking connection...
                            </div>
                        ) : !account.data ? (
                            <Empty>
                                <EmptyMedia variant="icon">
                                    <Inbox />
                                </EmptyMedia>
                                <EmptyHeader>
                                    <EmptyTitle>Connect GitHub</EmptyTitle>
                                    <EmptyDescription>
                                        Sign in to see issues that involve you.
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
                                        : "Could not load issues"}
                                </p>
                                {showScopeHint ? (
                                    <p className="max-w-sm text-sm text-muted-foreground">
                                        Tokens granted before the notifications
                                        permission existed cannot search issues.
                                        Sign out and back in on the Account page
                                        to grant it.
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
                        ) : issues.length === 0 ? (
                            <div className="flex flex-col">
                                <Empty>
                                    <EmptyMedia variant="icon">
                                        <CircleDot />
                                    </EmptyMedia>
                                    <EmptyHeader>
                                        <EmptyTitle>No issues found</EmptyTitle>
                                        <EmptyDescription>
                                            No issues involving you on the
                                            loaded pages.
                                        </EmptyDescription>
                                    </EmptyHeader>
                                </Empty>
                                {loadMore}
                            </div>
                        ) : (
                            <div className="flex flex-col">
                                {groups.map(([repo, repoIssues]) => (
                                    <section key={repo}>
                                        <h2 className="px-2 pt-2 pb-1 text-sm font-semibold text-muted-foreground">
                                            {repo}
                                        </h2>
                                        {repoIssues.map((issue, index) => (
                                            <div
                                                key={`${issue.repoFullName}-${issue.number}`}
                                            >
                                                {index > 0 ? (
                                                    <Separator />
                                                ) : null}
                                                <IssuesRow
                                                    issue={issue}
                                                    localIssueRepos={
                                                        localIssueRepos
                                                    }
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

function IssuesRow({
    issue,
    localIssueRepos,
}: {
    issue: SearchIssueItem;
    localIssueRepos: Map<string, number>;
}) {
    const router = useActiveTabRouter();

    const issueTarget = parseIssueUrl(issue.htmlUrl);
    const inAppRepoId = issueTarget
        ? localIssueRepos.get(
              `${issueTarget.owner.toLowerCase()}/${issueTarget.repo.toLowerCase()}`
          )
        : undefined;

    const openInApp = () => {
        if (inAppRepoId === undefined || !issueTarget) return;
        router?.navigate({
            to: `/repo/${inAppRepoId}/issue/${issueTarget.number}`,
        });
    };

    const title =
        inAppRepoId !== undefined ? (
            <button
                type="button"
                onClick={openInApp}
                className="flex min-w-0 cursor-pointer items-center gap-1 truncate text-left font-medium hover:underline"
            >
                <span className="truncate">{issue.title || "(no title)"}</span>
            </button>
        ) : (
            <ExternalLink href={issue.htmlUrl} className="truncate font-medium">
                {issue.title || "(no title)"}
            </ExternalLink>
        );

    return (
        <div className="flex items-center gap-2 px-2 py-2">
            <span className="shrink-0 text-muted-foreground [&_svg:not([class*='size-'])]:size-4">
                <CircleDot />
            </span>
            <div className="min-w-0 flex-1">
                <div className="truncate text-sm">{title}</div>
                <p className="truncate text-xs text-muted-foreground">
                    #{issue.number} · {issue.state}
                    {issue.updatedAt
                        ? ` · ${formatRelativeDate(issue.updatedAt)}`
                        : ""}
                </p>
            </div>
        </div>
    );
}
