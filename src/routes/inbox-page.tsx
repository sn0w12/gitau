"use no memo";

import {
    Bell,
    Check,
    CircleDot,
    GitCommitHorizontal,
    GitPullRequestArrow,
    Inbox,
    RotateCw,
    Tag,
} from "lucide-react";
import { useMemo, useState } from "react";

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
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Separator } from "@/components/ui/separator";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { useConfirm } from "@/contexts/confirm-context";
import { useGithubAccount } from "@/hooks/github/use-github-account";
import {
    useGithubNotifications,
    useMarkAllNotificationsRead,
    useMarkNotificationRead,
    useResolveSubjectUrl,
} from "@/hooks/github/use-github-inbox";
import { useLocalIssueRepoMap } from "@/hooks/github/use-github-issues";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import type { GithubNotification } from "@/lib/backend/protocol";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import { parseIssueUrl } from "@/lib/github/repo-coords";
import { openExternal } from "@/lib/open-external";
import { toastError } from "@/lib/toast-error";
import { formatRelativeDate } from "@/lib/utils";

type InboxFilter = "all" | "unread" | "mention" | "review_requested" | "assign";

const FILTER_OPTIONS: { value: InboxFilter; label: string }[] = [
    { value: "all", label: "All" },
    { value: "unread", label: "Unread" },
    { value: "mention", label: "Mentions" },
    { value: "review_requested", label: "Review requests" },
    { value: "assign", label: "Assignments" },
];

function matchesFilter(
    thread: GithubNotification,
    filter: InboxFilter
): boolean {
    switch (filter) {
        case "all":
            return true;
        case "unread":
            return thread.unread;
        default:
            return thread.reason === filter;
    }
}

function SubjectIcon({ type }: { type: string }) {
    switch (type) {
        case "PullRequest":
            return <GitPullRequestArrow />;
        case "Issue":
            return <CircleDot />;
        case "Commit":
            return <GitCommitHorizontal />;
        case "Release":
            return <Tag />;
        default:
            return <Bell />;
    }
}

function formatSubjectType(type: string): string {
    switch (type) {
        case "PullRequest":
            return "Pull request";
        case "CheckSuite":
            return "Check suite";
        case "Discussion":
            return "Discussion";
        default:
            return type;
    }
}

/**
 * Signed-in GitHub notifications, newest first, paged from the backend 100
 * threads at a time. Unread state updates in place on mark-read; the
 * sidebar badge reads the same cache. Only the list scrolls; the header
 * stays pinned.
 */
export function InboxPage() {
    const router = useActiveTabRouter();
    const account = useGithubAccount();
    const [filter, setFilter] = useState<InboxFilter>("all");
    const inbox = useGithubNotifications();
    const markRead = useMarkNotificationRead();
    const markAll = useMarkAllNotificationsRead();
    const [markingIds, setMarkingIds] = useState<ReadonlySet<string>>(
        () => new Set()
    );
    const { confirm } = useConfirm();
    // Open repos keyed by `owner/repo` for in-app issue links; threads
    // whose repo is not open fall back to the browser.
    const localIssueRepos = useLocalIssueRepoMap();

    const threads = useMemo(
        () => inbox.threads.filter((thread) => matchesFilter(thread, filter)),
        [inbox.threads, filter]
    );
    const groups = useMemo(() => {
        const byRepo = new Map<string, GithubNotification[]>();
        for (const thread of threads) {
            const key = thread.repoFullName || "Unknown repository";
            const list = byRepo.get(key);
            if (list) list.push(thread);
            else byRepo.set(key, [thread]);
        }
        return [...byRepo.entries()];
    }, [threads]);
    const unreadCount = useMemo(
        () => inbox.threads.filter((thread) => thread.unread).length,
        [inbox.threads]
    );
    const showScopeHint =
        inbox.error instanceof GitBackendError && !inbox.error.retryable;

    const handleMarkAll = async () => {
        const result = await confirm({
            title: "Mark all notifications read?",
            description:
                "Every GitHub notification is marked read, including threads on pages you have not loaded yet.",
            confirmText: "Mark all read",
        });
        if (!result.confirmed) return;
        try {
            await markAll.markAllRead();
        } catch (error) {
            toastError("Could not mark all read", error);
        }
    };

    const handleMarkRead = async (threadId: string) => {
        setMarkingIds((prev) => new Set(prev).add(threadId));
        try {
            await markRead.markRead(threadId);
        } catch (error) {
            toastError("Could not mark thread read", error);
        } finally {
            setMarkingIds((prev) => {
                const next = new Set(prev);
                next.delete(threadId);
                return next;
            });
        }
    };

    const loadMore = inbox.hasNextPage ? (
        <div className="flex justify-center py-3">
            <Button
                variant="outline"
                size="sm"
                disabled={inbox.isFetchingNextPage}
                loading={inbox.isFetchingNextPage}
                onClick={() => void inbox.fetchNextPage()}
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
                            Inbox
                        </FrameTitle>
                        <div className="ml-auto flex items-center gap-2">
                            <Button
                                variant="ghost"
                                size="icon-sm"
                                aria-label="Refresh inbox"
                                disabled={inbox.isFetching}
                                loading={inbox.isFetching}
                                onClick={() => void inbox.refetch()}
                            >
                                <RotateCw />
                            </Button>
                            <Select
                                value={filter}
                                onValueChange={(next) => {
                                    if (typeof next === "string")
                                        setFilter(next as InboxFilter);
                                }}
                                items={FILTER_OPTIONS}
                            >
                                <SelectTrigger className="w-36" size="sm">
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectPopup>
                                    {FILTER_OPTIONS.map((option) => (
                                        <SelectItem
                                            key={option.value}
                                            value={option.value}
                                        >
                                            {option.label}
                                        </SelectItem>
                                    ))}
                                </SelectPopup>
                            </Select>
                            <Button
                                variant="outline"
                                size="sm"
                                disabled={markAll.pending || unreadCount === 0}
                                loading={markAll.pending}
                                onClick={() => void handleMarkAll()}
                            >
                                Mark all read
                            </Button>
                        </div>
                    </div>
                    <FrameDescription className="text-sm text-muted-foreground">
                        GitHub notifications for the connected account.
                    </FrameDescription>
                </FrameHeader>
                <FramePanel data-testid="inbox-panel" className="min-h-0 p-2">
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
                                        Sign in to read notifications, review
                                        requests, and mentions.
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
                        ) : inbox.isLoading ? (
                            <div className="flex flex-col gap-2 py-2">
                                {[0, 1, 2, 3].map((index) => (
                                    <Skeleton
                                        key={index}
                                        className="h-14 w-full"
                                    />
                                ))}
                            </div>
                        ) : inbox.isError ? (
                            <div className="flex flex-col items-center gap-2 py-8 text-center">
                                <p
                                    className="text-sm text-destructive"
                                    role="alert"
                                >
                                    {inbox.error instanceof Error
                                        ? inbox.error.message
                                        : "Could not load notifications"}
                                </p>
                                {showScopeHint ? (
                                    <p className="max-w-sm text-sm text-muted-foreground">
                                        Tokens granted before the notifications
                                        permission existed cannot list the
                                        inbox. Sign out and back in on the
                                        Account page to grant it.
                                    </p>
                                ) : null}
                                <div className="flex items-center gap-2">
                                    <Button
                                        variant="outline"
                                        size="sm"
                                        onClick={() => void inbox.refetch()}
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
                        ) : threads.length === 0 ? (
                            <div className="flex flex-col">
                                <Empty>
                                    <EmptyMedia variant="icon">
                                        <Check />
                                    </EmptyMedia>
                                    <EmptyHeader>
                                        <EmptyTitle>All caught up</EmptyTitle>
                                        <EmptyDescription>
                                            {filter === "all"
                                                ? "No notifications on the loaded pages."
                                                : "Nothing matches this filter on the loaded pages."}
                                        </EmptyDescription>
                                    </EmptyHeader>
                                </Empty>
                                {loadMore}
                            </div>
                        ) : (
                            <div className="flex flex-col">
                                {groups.map(([repo, repoThreads]) => (
                                    <section key={repo}>
                                        <h2 className="px-2 pt-2 pb-1 text-sm font-semibold text-muted-foreground">
                                            {repo}
                                        </h2>
                                        {repoThreads.map((thread, index) => (
                                            <div key={thread.id}>
                                                {index > 0 ? (
                                                    <Separator />
                                                ) : null}
                                                <InboxRow
                                                    thread={thread}
                                                    marking={markingIds.has(
                                                        thread.id
                                                    )}
                                                    onMarkRead={() =>
                                                        void handleMarkRead(
                                                            thread.id
                                                        )
                                                    }
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

function InboxRow({
    thread,
    marking,
    onMarkRead,
    localIssueRepos,
}: {
    thread: GithubNotification;
    marking: boolean;
    onMarkRead: () => void;
    localIssueRepos: Map<string, number>;
}) {
    const router = useActiveTabRouter();
    const resolve = useResolveSubjectUrl();
    const [resolving, setResolving] = useState(false);

    // Issue threads whose repo is open resolve to the in-app issue page;
    // everything else keeps the external browser behavior.
    const issueTarget =
        thread.subjectType === "Issue"
            ? parseIssueUrl(thread.htmlUrl ?? thread.subjectUrl ?? undefined)
            : null;
    const inAppRepoId = issueTarget
        ? localIssueRepos.get(
              `${issueTarget.owner.toLowerCase()}/${issueTarget.repo.toLowerCase()}`
          )
        : undefined;

    const openResolved = async () => {
        if (!thread.subjectUrl || resolving) return;
        setResolving(true);
        try {
            const url = await resolve.resolve(thread.subjectUrl);
            const target =
                url ??
                (thread.repoFullName
                    ? `https://github.com/${thread.repoFullName}`
                    : null);
            if (target) await openExternal(target);
        } catch (error) {
            toastError("Could not open notification", error);
        } finally {
            setResolving(false);
        }
    };

    const openInApp = () => {
        if (inAppRepoId === undefined || !issueTarget) return;
        onMarkRead();
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
                <span className="truncate">
                    {thread.subjectTitle || "(no title)"}
                </span>
            </button>
        ) : thread.htmlUrl ? (
            <ExternalLink
                href={thread.htmlUrl}
                className="truncate font-medium"
            >
                {thread.subjectTitle || "(no title)"}
            </ExternalLink>
        ) : (
            <button
                type="button"
                disabled={resolving || !thread.subjectUrl}
                onClick={() => void openResolved()}
                className="flex min-w-0 cursor-pointer items-center gap-1 truncate text-left font-medium hover:underline disabled:pointer-events-none disabled:opacity-100"
            >
                <span className="truncate">
                    {thread.subjectTitle || "(no title)"}
                </span>
            </button>
        );
    return (
        <div className="flex items-center gap-2 px-2 py-2">
            <span
                aria-hidden={!thread.unread}
                className={
                    thread.unread
                        ? "size-2 shrink-0 rounded-full bg-primary"
                        : "size-2 shrink-0"
                }
            />
            <span className="shrink-0 text-muted-foreground [&_svg:not([class*='size-'])]:size-4">
                <SubjectIcon type={thread.subjectType} />
            </span>
            <div className="min-w-0 flex-1">
                <div className="truncate text-sm">{title}</div>
                <p className="truncate text-xs text-muted-foreground">
                    {formatSubjectType(thread.subjectType)}
                    {thread.updatedAt
                        ? ` · ${formatRelativeDate(thread.updatedAt)}`
                        : ""}
                </p>
            </div>
            <span className="flex size-8 shrink-0 items-center justify-center">
                {thread.unread ? (
                    <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label={`Mark ${thread.subjectTitle} read`}
                        loading={marking}
                        onClick={onMarkRead}
                    >
                        <Check />
                    </Button>
                ) : null}
            </span>
        </div>
    );
}
