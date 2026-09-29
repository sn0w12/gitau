import { GitPullRequest } from "lucide-react";
import { useEffect, useMemo, useRef } from "react";

import { LabelBadge } from "@/components/github/label-badge";
import {
    pullRequestStatusOf,
    StatusBadge,
} from "@/components/github/status-badge";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
} from "@/components/ui/empty";
import { Frame } from "@/components/ui/frame";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from "@/components/ui/table";
import { Tabs, TabsList, TabsPanel, TabsTab } from "@/components/ui/tabs";
import { useGithubAccount } from "@/hooks/github/use-github-account";
import { useGithubCoords } from "@/hooks/github/use-github-issues";
import { useRepoPullRequests } from "@/hooks/github/use-github-pull-requests";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import type {
    GithubLabel,
    GithubPullRequestListItem,
} from "@/lib/backend/protocol";
import type { ListTab } from "@/lib/routing/repo-search";

export function PullRequestsView({
    repoId,
    tab,
    onTabChange,
    label,
    onLabelChange,
}: {
    repoId: number;
    tab: ListTab;
    onTabChange: (tab: ListTab) => void;
    label: string;
    onLabelChange: (label: string) => void;
}) {
    const account = useGithubAccount();
    const { coords, isLoading: coordsLoading } = useGithubCoords(repoId);
    const pulls = useRepoPullRequests(repoId, tab);

    const labels = useMemo(() => {
        const seen = new Map<string, GithubLabel>();
        for (const pull of pulls.pulls) {
            for (const item of pull.labels) {
                if (!seen.has(item.name)) seen.set(item.name, item);
            }
        }
        return [...seen.values()].sort((a, b) => a.name.localeCompare(b.name));
    }, [pulls.pulls]);

    const rows = useMemo(
        () =>
            pulls.pulls.filter(
                (pull) =>
                    label === "none" ||
                    pull.labels.some((item) => item.name === label)
            ),
        [pulls.pulls, label]
    );

    const shared = {
        repoId,
        rows,
        isLoading: account.isLoading || coordsLoading || pulls.isLoading,
        isError: pulls.isError,
        errorMessage:
            pulls.error instanceof Error
                ? pulls.error.message
                : "Could not load pull requests",
        onRetry: () => void pulls.refetch(),
        hasCoords: coords !== null,
        signedIn: account.data != null,
        hasNextPage: pulls.hasNextPage ?? false,
        isFetchingNextPage: pulls.isFetchingNextPage,
        onLoadMore: () => void pulls.fetchNextPage(),
    };

    return (
        <div className="flex h-full min-h-0 flex-col p-0.5">
            <Tabs
                className="min-h-0 flex-1 gap-0.5"
                value={tab}
                onValueChange={(value) => onTabChange(value as ListTab)}
            >
                <div className="flex justify-between">
                    <TabsList>
                        <TabsTab value="open">Open</TabsTab>
                        <TabsTab value="closed">Closed</TabsTab>
                    </TabsList>
                    <div className="flex gap-1">
                        <Select
                            aria-label="Select label"
                            value={label}
                            onValueChange={(next) => {
                                if (typeof next === "string")
                                    onLabelChange(next);
                            }}
                            items={[
                                { value: "none", label: "None" },
                                ...labels.map((item) => ({
                                    value: item.name,
                                    label: item.name,
                                })),
                            ]}
                        >
                            <SelectTrigger>
                                <SelectValue />
                            </SelectTrigger>
                            <SelectPopup>
                                <SelectItem value="none">
                                    <span className="flex items-center gap-2">
                                        <span
                                            aria-hidden="true"
                                            className="size-1.5 rounded-full bg-muted-foreground"
                                        />
                                        <span className="truncate">None</span>
                                    </span>
                                </SelectItem>
                                {labels.map((item) => (
                                    <SelectItem
                                        key={item.name}
                                        value={item.name}
                                    >
                                        <span className="flex items-center gap-2">
                                            <LabelBadge label={item} />
                                        </span>
                                    </SelectItem>
                                ))}
                            </SelectPopup>
                        </Select>
                    </div>
                </div>
                <Frame className="min-h-0 w-full flex-1">
                    {(["open", "closed"] as const).map((value) => (
                        <TabsPanel
                            key={value}
                            className="flex min-h-0 flex-1 flex-col"
                            value={value}
                        >
                            <PullsTable {...shared} state={tab} />
                        </TabsPanel>
                    ))}
                </Frame>
            </Tabs>
        </div>
    );
}

function PullsTableHead() {
    return (
        <TableHeader>
            <TableRow>
                <TableHead className="h-9">Title</TableHead>
                <TableHead className="h-9">Status</TableHead>
                <TableHead className="h-9">Labels</TableHead>
                <TableHead className="h-9">Comments</TableHead>
                <TableHead className="h-9 text-right">Author</TableHead>
            </TableRow>
        </TableHeader>
    );
}

function PullsTable({
    repoId,
    state,
    rows,
    isLoading,
    isError,
    errorMessage,
    onRetry,
    hasCoords,
    signedIn,
    hasNextPage,
    isFetchingNextPage,
    onLoadMore,
}: {
    repoId: number;
    state: ListTab;
    rows: GithubPullRequestListItem[];
    isLoading: boolean;
    isError: boolean;
    errorMessage: string;
    onRetry: () => void;
    hasCoords: boolean;
    signedIn: boolean;
    hasNextPage: boolean;
    isFetchingNextPage: boolean;
    onLoadMore: () => void;
}) {
    const sentinelRef = useRef<HTMLDivElement>(null);
    useEffect(() => {
        const sentinel = sentinelRef.current;
        if (!sentinel || !hasNextPage) return;
        const observer = new IntersectionObserver(
            (entries) => {
                if (entries[0]?.isIntersecting && !isFetchingNextPage) {
                    onLoadMore();
                }
            },
            { rootMargin: "200px" }
        );
        observer.observe(sentinel);
        return () => observer.disconnect();
    }, [hasNextPage, isFetchingNextPage, onLoadMore]);

    const router = useActiveTabRouter();

    if (isLoading) {
        return <PullsTableSkeleton />;
    }

    if (!signedIn) {
        return (
            <Empty>
                <EmptyMedia variant="icon">
                    <GitPullRequest />
                </EmptyMedia>
                <EmptyHeader>
                    <EmptyTitle>Connect GitHub</EmptyTitle>
                    <EmptyDescription>
                        Sign in on the Account page to read pull requests.
                    </EmptyDescription>
                </EmptyHeader>
            </Empty>
        );
    }

    if (!hasCoords) {
        return (
            <Empty>
                <EmptyMedia variant="icon">
                    <GitPullRequest />
                </EmptyMedia>
                <EmptyHeader>
                    <EmptyTitle>No GitHub remote</EmptyTitle>
                    <EmptyDescription>
                        This repository has no github.com remote, so there are
                        no pull requests to show.
                    </EmptyDescription>
                </EmptyHeader>
            </Empty>
        );
    }

    if (isError) {
        return (
            <Empty>
                <EmptyHeader>
                    <EmptyTitle>Could not load pull requests</EmptyTitle>
                    <EmptyDescription>{errorMessage}</EmptyDescription>
                </EmptyHeader>
                <Button variant="outline" size="sm" onClick={onRetry}>
                    Retry
                </Button>
            </Empty>
        );
    }

    if (rows.length === 0) {
        return (
            <Empty>
                <EmptyMedia variant="icon" className="mb-0">
                    <GitPullRequest />
                </EmptyMedia>
                <EmptyHeader>
                    <EmptyTitle>
                        {state === "open"
                            ? "No open pull requests"
                            : "No closed pull requests"}
                    </EmptyTitle>
                </EmptyHeader>
            </Empty>
        );
    }

    return (
        <div className="flex h-full min-h-0 flex-col">
            <Table containerClassName="min-h-0 flex-1" variant="card">
                <PullsTableHead />
                <TableBody>
                    {rows.map((pull) => (
                        <TableRow
                            key={pull.number}
                            className="cursor-pointer"
                            onClick={() => {
                                router?.navigate({
                                    to: `/repo/${repoId}/pull/${pull.number}`,
                                });
                            }}
                        >
                            <TableCell className="font-medium">
                                {pull.title}
                            </TableCell>
                            <TableCell>
                                <StatusBadge
                                    status={pullRequestStatusOf(pull)}
                                />
                            </TableCell>
                            <TableCell className="space-x-1">
                                {pull.labels.map((item) => (
                                    <LabelBadge key={item.name} label={item} />
                                ))}
                            </TableCell>
                            <TableCell className="font-mono">
                                {pull.commentCount}
                            </TableCell>
                            <TableCell>
                                <div className="flex min-h-5 items-center justify-end gap-1.5">
                                    <span className="truncate text-xs text-muted-foreground">
                                        {pull.author.login}
                                    </span>
                                    <Avatar className="size-5">
                                        <AvatarImage
                                            src={
                                                pull.author.avatarUrl ||
                                                undefined
                                            }
                                        />
                                        <AvatarFallback>
                                            {pull.author.login
                                                .slice(0, 1)
                                                .toUpperCase()}
                                        </AvatarFallback>
                                    </Avatar>
                                </div>
                            </TableCell>
                        </TableRow>
                    ))}
                    {hasNextPage ? (
                        <TableRow>
                            <TableCell colSpan={5}>
                                <div
                                    ref={sentinelRef}
                                    className="flex justify-center py-2"
                                >
                                    {isFetchingNextPage ? (
                                        <Spinner className="size-4" />
                                    ) : null}
                                </div>
                            </TableCell>
                        </TableRow>
                    ) : null}
                </TableBody>
            </Table>
        </div>
    );
}

/** Same table shell with skeleton rows shaped like real rows. */
function PullsTableSkeleton() {
    return (
        <Table containerClassName="min-h-0 flex-1" variant="card">
            <PullsTableHead />
            <TableBody>
                {Array.from({ length: 8 }, (_, index) => (
                    <TableRow key={index}>
                        <TableCell className="font-medium">
                            <Skeleton
                                className="h-4"
                                style={{
                                    width: `${62 - (index % 3) * 12}%`,
                                }}
                            />
                        </TableCell>
                        <TableCell>
                            <Skeleton className="h-5 w-16 rounded-full" />
                        </TableCell>
                        <TableCell>
                            <div className="flex gap-1">
                                <Skeleton className="h-5 w-14 rounded-full" />
                                {index % 2 === 0 ? (
                                    <Skeleton className="h-5 w-20 rounded-full" />
                                ) : null}
                            </div>
                        </TableCell>
                        <TableCell className="font-mono">
                            <Skeleton className="h-4 w-6" />
                        </TableCell>
                        <TableCell>
                            <div className="flex min-h-5 items-center justify-end gap-1.5">
                                <Skeleton className="h-3 w-16" />
                                <Skeleton className="size-5 rounded-full" />
                            </div>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}
