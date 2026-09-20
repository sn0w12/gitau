import { useParams } from "@tanstack/react-router";
import { useSelector } from "@tanstack/react-store";
import { X } from "lucide-react";
import { Fragment, useRef } from "react";

import { IssueEvent } from "@/components/issues/event";
import { IssueInput, type IssueInputHandle } from "@/components/issues/input";
import { IssueMessage, MessageSpacer } from "@/components/issues/message";
import { issueStatusOf, StatusBadge } from "@/components/issues/status-badge";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyTitle,
} from "@/components/ui/empty";
import { Frame, FrameHeader, FramePanel } from "@/components/ui/frame";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { useGithubAccount } from "@/hooks/github/use-github-account";
import {
    useCreateIssueComment,
    useDeleteIssueComment,
    useGithubCoords,
    useIssue,
    useRepoPermissions,
    useUpdateIssue,
    useUpdateIssueComment,
} from "@/hooks/github/use-github-issues";
import {
    useActiveTabHistory,
    useActiveTabRouter,
} from "@/hooks/tabs/use-active-tab-router";
import type {
    GithubIssueComment,
    GithubIssueDetail,
    GithubIssueEvent,
    GithubUser,
} from "@/lib/backend/protocol";
import { toastError } from "@/lib/toast-error";
import { repositoryStore } from "@/stores/repository-store";

function initials(login: string): string {
    return login.slice(0, 1).toUpperCase() || "?";
}

function AvatarStack({ users }: { users: GithubUser[] }) {
    if (users.length === 0) {
        return <span className="text-sm text-muted-foreground">None</span>;
    }
    return (
        <div className="flex -space-x-[0.4rem]">
            {users.map((user) => (
                <Avatar key={user.login} className="size-6 ring-2 ring-card">
                    <AvatarImage src={user.avatarUrl || undefined} />
                    <AvatarFallback>{initials(user.login)}</AvatarFallback>
                </Avatar>
            ))}
        </div>
    );
}

type TimelineItem =
    | { kind: "body"; createdAt: string; comment: null; event: null }
    | {
          kind: "comment";
          createdAt: string;
          comment: GithubIssueComment;
          event: null;
      }
    | {
          kind: "event";
          createdAt: string;
          comment: null;
          event: GithubIssueEvent;
      };

const HIDDEN_EVENT_KINDS = new Set([
    "subscribed",
    "unsubscribed",
    "mentioned",
    "referenced",
    "cross-referenced",
]);

function buildTimeline(
    issue: GithubIssueDetail,
    comments: GithubIssueComment[],
    events: GithubIssueEvent[]
): TimelineItem[] {
    return [
        {
            kind: "body",
            createdAt: issue.createdAt,
            comment: null,
            event: null,
        } as TimelineItem,
        ...comments.map((comment): TimelineItem => ({
            kind: "comment",
            createdAt: comment.createdAt,
            comment,
            event: null,
        })),
        ...events
            .filter((event) => !HIDDEN_EVENT_KINDS.has(event.kind))
            .map((event): TimelineItem => ({
                kind: "event",
                createdAt: event.createdAt,
                comment: null,
                event,
            })),
    ].sort((a, b) => a.createdAt.localeCompare(b.createdAt));
}

function isMessage(item: TimelineItem): boolean {
    return item.kind === "body" || item.kind === "comment";
}

interface MessageActions {
    canQuote: boolean;
    onQuote: (text: string) => void;
    canEdit: (authorLogin: string) => boolean;
    onSaveBody: (body: string) => Promise<void>;
    onSaveComment: (commentId: number, body: string) => Promise<void>;
    onDeleteComment: (commentId: number) => Promise<void>;
}

function TimelineBody({
    issue,
    actions,
}: {
    issue: GithubIssueDetail;
    actions: MessageActions;
}) {
    return (
        <IssueMessage
            text={issue.body || "_No description_"}
            author={issue.author.login}
            avatarUrl={issue.author.avatarUrl || undefined}
            createdAt={issue.createdAt}
            link={issue.htmlUrl || undefined}
            canQuote={actions.canQuote}
            onQuote={actions.onQuote}
            canEdit={actions.canEdit(issue.author.login)}
            onSave={actions.onSaveBody}
        />
    );
}

function TimelineComment({
    comment,
    actions,
}: {
    comment: GithubIssueComment;
    actions: MessageActions;
}) {
    return (
        <IssueMessage
            text={comment.body}
            author={comment.author.login}
            avatarUrl={comment.author.avatarUrl || undefined}
            createdAt={comment.createdAt}
            actionLabel="commented on"
            link={comment.htmlUrl || undefined}
            canQuote={actions.canQuote}
            onQuote={actions.onQuote}
            canEdit={actions.canEdit(comment.author.login)}
            onSave={(body) => actions.onSaveComment(comment.id, body)}
            canDelete={actions.canEdit(comment.author.login)}
            onDelete={() => actions.onDeleteComment(comment.id)}
        />
    );
}

function TimelineRow({
    item,
    last,
    issue,
    actions,
}: {
    item: TimelineItem;
    last: boolean;
    issue: GithubIssueDetail;
    actions: MessageActions;
}) {
    if (item.kind === "body")
        return <TimelineBody issue={issue} actions={actions} />;
    if (item.kind === "comment")
        return <TimelineComment comment={item.comment} actions={actions} />;
    const event = item.event;
    return (
        <IssueEvent
            last={last}
            kind={event.kind}
            actor={event.actor}
            avatarUrl={event.actorAvatarUrl || undefined}
            createdAt={event.createdAt}
            label={event.label}
            labelColor={event.labelColor}
            assignee={event.assignee}
        />
    );
}

function IssueSidebar({
    issue,
    participants,
}: {
    issue: GithubIssueDetail;
    participants: GithubUser[];
}) {
    return (
        <Frame className="flex h-fit w-full flex-col">
            <FramePanel className="flex flex-col p-2 pt-1">
                <span className="ui-selectable">Assignees</span>
                <AvatarStack users={issue.assignees} />
            </FramePanel>
            <FramePanel className="flex flex-col p-2 pt-1">
                <span className="ui-selectable">Labels</span>
                {issue.labels.length === 0 ? (
                    <div className="text-sm text-muted-foreground">None</div>
                ) : (
                    <div className="space-x-1">
                        {issue.labels.map((label) => (
                            <Badge key={label.name} variant="outline">
                                <span
                                    aria-hidden="true"
                                    className="size-1.5 rounded-full"
                                    style={
                                        label.color
                                            ? {
                                                  backgroundColor: `#${label.color}`,
                                              }
                                            : undefined
                                    }
                                />
                                {label.name}
                            </Badge>
                        ))}
                    </div>
                )}
            </FramePanel>
            <FramePanel className="flex flex-col p-2 pt-1">
                <span className="ui-selectable">Participants</span>
                <AvatarStack users={participants} />
            </FramePanel>
        </Frame>
    );
}

/** Message-shaped placeholder mirroring IssueMessage's Frame layout. */
function MessageSkeleton({ lines }: { lines: number }) {
    const widths = ["w-full", "w-11/12", "w-2/3"];
    return (
        <Frame className="ui-selectable">
            <FrameHeader className="flex flex-row items-center gap-1 px-2 py-1.5">
                <Skeleton className="size-6 rounded-full" />
                <Skeleton className="h-4 w-40" />
            </FrameHeader>
            <FramePanel className="px-3 py-2">
                <div className="flex flex-col gap-2">
                    {Array.from({ length: lines }, (_, index) => (
                        <Skeleton
                            key={index}
                            className={`h-4 ${widths[index % widths.length]}`}
                        />
                    ))}
                </div>
            </FramePanel>
        </Frame>
    );
}

/** Composer-shaped placeholder mirroring IssueInput's layout. */
function ComposerSkeleton() {
    return (
        <div className="w-full">
            <Frame>
                <FrameHeader className="flex flex-row justify-between px-2 py-2">
                    <div className="flex gap-3">
                        <Skeleton className="h-4 w-12" />
                        <Skeleton className="h-4 w-14" />
                    </div>
                    <div className="flex gap-1">
                        {[0, 1, 2, 3, 4].map((index) => (
                            <Skeleton
                                key={index}
                                className="size-6 rounded-md"
                            />
                        ))}
                    </div>
                </FrameHeader>
                <FramePanel className="px-3 py-2">
                    <Skeleton className="h-20 w-full" />
                </FramePanel>
            </Frame>
            <div className="flex justify-end gap-1 pt-2">
                <Skeleton className="h-8 w-24 rounded-lg" />
                <Skeleton className="h-8 w-20 rounded-lg" />
            </div>
        </div>
    );
}

function IssueContent({
    owner,
    repo,
    issue,
    comments,
    events,
    signedIn,
    viewerLogin,
    goBack,
}: {
    owner: string;
    repo: string;
    issue: GithubIssueDetail;
    comments: GithubIssueComment[];
    events: GithubIssueEvent[];
    signedIn: boolean;
    viewerLogin: string | null;
    goBack: () => void;
}) {
    const createComment = useCreateIssueComment();
    const updateIssue = useUpdateIssue();
    const updateComment = useUpdateIssueComment();
    const deleteComment = useDeleteIssueComment();
    const permissions = useRepoPermissions(owner, repo);
    const composerRef = useRef<IssueInputHandle>(null);
    const mutating = createComment.pending || updateIssue.pending;
    const open = issueStatusOf(issue.state) === "open";
    const timeline = buildTimeline(issue, comments, events);
    // Comment authors repeat (one per comment) and overlap the backend
    // list, so merge order-preservingly by login.
    const participants = (() => {
        const seen = new Set<string>();
        return [...issue.participants, ...comments.map((c) => c.author)].filter(
            (user) => {
                const key = user.login.toLowerCase();
                if (key === "" || seen.has(key)) return false;
                seen.add(key);
                return true;
            }
        );
    })();

    const handleComment = async (body: string) => {
        try {
            await createComment.createComment(owner, repo, issue.number, body);
        } catch (error) {
            toastError("Could not post comment", error);
        }
    };

    const handleToggleState = async () => {
        try {
            await updateIssue.updateIssue(owner, repo, issue.number, {
                state: open ? "closed" : "open",
            });
        } catch (error) {
            toastError(
                open ? "Could not close issue" : "Could not reopen issue",
                error
            );
        }
    };

    // Authors edit their own messages; repo push access moderates the
    // rest. Signed-out viewers get no write actions at all.
    const canPush = permissions.data?.push === true;
    const canEdit = (authorLogin: string) =>
        signedIn &&
        (authorLogin.toLowerCase() === (viewerLogin ?? "").toLowerCase() ||
            canPush);
    const actions: MessageActions = {
        canQuote: signedIn,
        onQuote: (text) => composerRef.current?.insertQuote(text),
        canEdit,
        // Rejections propagate to the message, which toasts once.
        onSaveBody: async (body) => {
            await updateIssue.updateIssue(owner, repo, issue.number, {
                body,
            });
        },
        onSaveComment: async (commentId, body) => {
            await updateComment.updateComment(
                owner,
                repo,
                commentId,
                issue.number,
                body
            );
        },
        onDeleteComment: async (commentId) => {
            await deleteComment.deleteComment(
                owner,
                repo,
                commentId,
                issue.number
            );
        },
    };

    return (
        <div className="container flex h-full min-h-0 flex-col px-1 py-2">
            <header className="ui-selectable flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <span className="truncate font-display text-3xl tracking-tight">
                        {issue.title}
                    </span>
                    <span className="self-end pb-0.5 font-mono text-muted-foreground">
                        #{issue.number}
                    </span>
                    <StatusBadge
                        status={issueStatusOf(issue.state)}
                        className="mb-1.5 self-end"
                    />
                </div>
                <Button variant="outline" size="icon" onClick={goBack}>
                    <X />
                </Button>
            </header>
            <div className="mt-2 grid min-h-0 flex-1 grid-cols-3">
                <div className="col-span-2 flex min-h-0 w-full flex-col">
                    <ScrollArea scrollFade scrollbarGutter className="pr-1">
                        {timeline.map((item, index) => {
                            const prev = index > 0 ? timeline[index - 1] : null;
                            const stackedMessage =
                                isMessage(item) &&
                                prev !== null &&
                                isMessage(prev);
                            return (
                                <Fragment
                                    key={
                                        item.kind === "body"
                                            ? "body"
                                            : item.kind === "comment"
                                              ? `comment-${item.comment.id}`
                                              : `event-${item.event.id}`
                                    }
                                >
                                    {stackedMessage ? <MessageSpacer /> : null}
                                    <TimelineRow
                                        item={item}
                                        last={index === timeline.length - 1}
                                        issue={issue}
                                        actions={actions}
                                    />
                                </Fragment>
                            );
                        })}
                        <MessageSpacer />
                        {signedIn ? (
                            <IssueInput
                                pending={mutating}
                                stateLabel={
                                    open ? "Close issue" : "Reopen issue"
                                }
                                onSubmit={(body) => void handleComment(body)}
                                onToggleState={() => void handleToggleState()}
                                inputRef={composerRef}
                            />
                        ) : null}
                    </ScrollArea>
                </div>
                <IssueSidebar issue={issue} participants={participants} />
            </div>
        </div>
    );
}

function useIssueParams() {
    const params = useParams({ strict: false });
    const repoId = Number(params.repoId);
    const issueNumber = Number(params.issueId);
    const valid =
        Number.isInteger(repoId) &&
        repoId > 0 &&
        Number.isInteger(issueNumber) &&
        issueNumber > 0;
    return { repoId, issueNumber, valid };
}

function useIssueRouteData(
    repoId: number,
    issueNumber: number,
    valid: boolean
) {
    const entry = useSelector(repositoryStore, (state) => {
        if (!valid) return undefined;
        for (const candidate of state.entries.values()) {
            if (candidate.repoId === repoId) return candidate;
        }
        return undefined;
    });
    const account = useGithubAccount();
    const { coords } = useGithubCoords(valid ? repoId : undefined);
    const { detail, comments, events } = useIssue(
        coords?.owner ?? null,
        coords?.repo ?? null,
        valid ? issueNumber : null
    );
    return { entry, account, coords, detail, comments, events };
}

export function IssuePage() {
    const { repoId, issueNumber, valid } = useIssueParams();
    const router = useActiveTabRouter();
    const history = useActiveTabHistory();
    const { entry, account, coords, detail, comments, events } =
        useIssueRouteData(repoId, issueNumber, valid);

    const goBack = () => {
        if (history.canGoBack) {
            history.back();
        } else {
            router?.navigate({
                to: `/repo/${repoId}`,
                search: { view: "issues" },
            });
        }
    };

    if (!valid || !entry) {
        return (
            <div className="flex h-full items-center justify-center p-6">
                <p className="text-sm text-muted-foreground">
                    Repository not open in this session.
                </p>
            </div>
        );
    }

    if (account.isLoading || detail.isLoading) {
        return (
            <div className="container flex h-full min-h-0 flex-col p-1 pt-2">
                <header className="ui-selectable flex items-center justify-between">
                    <div className="flex items-center gap-2">
                        <Skeleton className="h-8 w-72" />
                        <Skeleton className="h-5 w-12 self-end" />
                        <Skeleton className="mb-1.5 h-5 w-16 self-end rounded-full" />
                    </div>
                    <Button variant="outline" size="icon" onClick={goBack}>
                        <X />
                    </Button>
                </header>
                <div className="mt-2 grid min-h-0 flex-1 grid-cols-3 gap-2">
                    <div className="col-span-2 flex min-h-0 w-full flex-col">
                        <ScrollArea>
                            <MessageSkeleton lines={3} />
                            <div className="ml-4 h-1.5 w-0.5 bg-muted" />
                            <div className="ui-selectable flex items-center gap-1 px-1.5 py-1">
                                <Skeleton className="size-6 rounded-full" />
                                <Skeleton className="size-6 rounded-full" />
                                <Skeleton className="h-4 w-48" />
                            </div>
                            <div className="ml-4 h-1.5 w-0.5 bg-muted" />
                            <MessageSkeleton lines={2} />
                            <MessageSpacer />
                            <ComposerSkeleton />
                        </ScrollArea>
                    </div>
                    <Frame className="flex h-fit w-full flex-col">
                        <FramePanel className="p-2 pt-1">
                            <span className="ui-selectable">Assignees</span>
                            <div className="flex -space-x-[0.4rem]">
                                <Skeleton className="size-6 rounded-full ring-2 ring-card" />
                                <Skeleton className="size-6 rounded-full ring-2 ring-card" />
                            </div>
                        </FramePanel>
                        <FramePanel className="p-2 pt-1">
                            <span className="ui-selectable">Labels</span>
                            <div className="flex gap-1">
                                <Skeleton className="h-5 w-14 rounded-full" />
                                <Skeleton className="h-5 w-20 rounded-full" />
                            </div>
                        </FramePanel>
                        <FramePanel className="p-2 pt-1">
                            <span className="ui-selectable">Participants</span>
                            <div className="flex -space-x-[0.4rem]">
                                <Skeleton className="size-6 rounded-full ring-2 ring-card" />
                                <Skeleton className="size-6 rounded-full ring-2 ring-card" />
                            </div>
                        </FramePanel>
                    </Frame>
                </div>
            </div>
        );
    }

    if (detail.isError || !detail.data) {
        return (
            <div className="container flex h-full items-center justify-center p-1">
                <Empty>
                    <EmptyHeader>
                        <EmptyTitle>Could not load issue</EmptyTitle>
                        <EmptyDescription>
                            {detail.error instanceof Error
                                ? detail.error.message
                                : "The issue could not be found."}
                        </EmptyDescription>
                    </EmptyHeader>
                    <Button variant="outline" size="sm" onClick={goBack}>
                        Back to issues
                    </Button>
                </Empty>
            </div>
        );
    }

    return (
        <IssueContent
            owner={coords?.owner ?? ""}
            repo={coords?.repo ?? ""}
            issue={detail.data}
            comments={comments.data ?? []}
            events={events.data ?? []}
            signedIn={account.data != null}
            viewerLogin={account.data?.login ?? null}
            goBack={goBack}
        />
    );
}
