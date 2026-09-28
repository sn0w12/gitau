import { useParams } from "@tanstack/react-router";
import { useSelector } from "@tanstack/react-store";
import { X } from "lucide-react";
import { Fragment, useRef } from "react";

import { TimelineEvent } from "@/components/github/event";
import {
    ConversationInput,
    type ConversationInputHandle,
} from "@/components/github/input";
import { MessageSpacer, TimelineMessage } from "@/components/github/message";
import { issueStatusOf, StatusBadge } from "@/components/github/status-badge";
import {
    AvatarStack,
    ComposerSkeleton,
    EventSkeleton,
    IssueSidebarSkeleton,
    LabelList,
    MessageSkeleton,
    SidebarBlock,
} from "@/components/github/thread-chrome";
import { Button } from "@/components/ui/button";
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyTitle,
} from "@/components/ui/empty";
import { Frame } from "@/components/ui/frame";
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
} from "@/lib/backend/protocol";
import {
    buildIssueTimeline,
    isTimelineMessage,
    timelineKey,
    timelineParticipants,
    type IssueTimelineItem,
} from "@/lib/github/timeline";
import { toastError } from "@/lib/toast-error";
import { repositoryStore } from "@/stores/repository-store";

interface MessageActions {
    canQuote: boolean;
    onQuote: (text: string) => void;
    canEdit: (authorLogin: string) => boolean;
    onSaveBody: (body: string) => Promise<void>;
    onSaveComment: (commentId: number, body: string) => Promise<void>;
    onDeleteComment: (commentId: number) => Promise<void>;
}

function TimelineRow({
    item,
    last,
    issue,
    owner,
    repo,
    actions,
}: {
    item: IssueTimelineItem;
    last: boolean;
    issue: GithubIssueDetail;
    owner: string;
    repo: string;
    actions: MessageActions;
}) {
    if (item.kind === "body") {
        return (
            <TimelineMessage
                text={issue.body || "_No description_"}
                author={issue.author.login}
                avatarUrl={issue.author.avatarUrl || undefined}
                createdAt={issue.createdAt}
                link={issue.htmlUrl || undefined}
                owner={owner}
                repo={repo}
                canQuote={actions.canQuote}
                onQuote={actions.onQuote}
                canEdit={actions.canEdit(issue.author.login)}
                onSave={actions.onSaveBody}
            />
        );
    }
    if (item.kind === "comment") {
        const comment = item.comment;
        return (
            <TimelineMessage
                text={comment.body}
                author={comment.author.login}
                avatarUrl={comment.author.avatarUrl || undefined}
                createdAt={comment.createdAt}
                actionLabel="commented on"
                link={comment.htmlUrl || undefined}
                owner={owner}
                repo={repo}
                canQuote={actions.canQuote}
                onQuote={actions.onQuote}
                canEdit={actions.canEdit(comment.author.login)}
                onSave={(body) => actions.onSaveComment(comment.id, body)}
                canDelete={actions.canEdit(comment.author.login)}
                onDelete={() => actions.onDeleteComment(comment.id)}
            />
        );
    }
    const event = item.event;
    return (
        <TimelineEvent
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
    participants: ReturnType<typeof timelineParticipants>;
}) {
    return (
        <Frame className="flex h-fit w-full flex-col">
            <SidebarBlock label="Assignees">
                <AvatarStack users={issue.assignees} />
            </SidebarBlock>
            <SidebarBlock label="Labels">
                <LabelList labels={issue.labels} />
            </SidebarBlock>
            <SidebarBlock label="Participants">
                <AvatarStack users={participants} />
            </SidebarBlock>
        </Frame>
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
    const composerRef = useRef<ConversationInputHandle>(null);
    const mutating =
        createComment.pending || updateIssue.pending || updateComment.pending;
    const open = issueStatusOf(issue.state) === "open";
    const timeline = buildIssueTimeline(issue, { comments, events });
    const participants = timelineParticipants(issue.participants, comments);

    const handleComment = async (body: string) => {
        try {
            await createComment.createComment(owner, repo, issue.number, body);
        } catch (error) {
            toastError("Could not post comment", error);
            throw error;
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
            await updateIssue.updateIssue(owner, repo, issue.number, { body });
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
                <Button
                    variant="outline"
                    size="icon"
                    onClick={goBack}
                    aria-label="Back to issues"
                >
                    <X />
                </Button>
            </header>
            <div className="mt-2 grid min-h-0 flex-1 grid-cols-3">
                <div className="col-span-2 flex min-h-0 w-full flex-col">
                    <ScrollArea scrollFade scrollbarGutter className="pr-1">
                        {timeline.map((item, index) => {
                            const prev = index > 0 ? timeline[index - 1] : null;
                            const stacked =
                                isTimelineMessage(item) &&
                                prev !== null &&
                                isTimelineMessage(prev);
                            return (
                                <Fragment key={timelineKey(item)}>
                                    {stacked ? <MessageSpacer /> : null}
                                    <TimelineRow
                                        item={item}
                                        last={index === timeline.length - 1}
                                        issue={issue}
                                        owner={owner}
                                        repo={repo}
                                        actions={actions}
                                    />
                                </Fragment>
                            );
                        })}
                        <MessageSpacer />
                        {signedIn ? (
                            <ConversationInput
                                pending={mutating}
                                stateLabel={
                                    open ? "Close issue" : "Reopen issue"
                                }
                                owner={owner}
                                repo={repo}
                                onSubmit={handleComment}
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
            <div className="container flex h-full min-h-0 flex-col px-1 py-2">
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
                <div className="mt-2 grid min-h-0 flex-1 grid-cols-3">
                    <div className="col-span-2 flex min-h-0 w-full flex-col">
                        <ScrollArea scrollFade scrollbarGutter className="pr-1">
                            <MessageSkeleton lines={3} />
                            <EventSkeleton />
                            <MessageSkeleton lines={2} />
                            <MessageSpacer />
                            <ComposerSkeleton />
                        </ScrollArea>
                    </div>
                    <div className="flex flex-col gap-1">
                        <IssueSidebarSkeleton />
                    </div>
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
