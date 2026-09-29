import { useParams } from "@tanstack/react-router";
import { useSelector } from "@tanstack/react-store";
import { X } from "lucide-react";
import { Fragment, useRef } from "react";

import { ChecksSection } from "@/components/github/checks";
import { TimelineCommit } from "@/components/github/commits";
import { TimelineEvent } from "@/components/github/event";
import {
    ConversationInput,
    type ConversationInputHandle,
} from "@/components/github/input";
import { MessageSpacer, TimelineMessage } from "@/components/github/message";
import {
    MERGE_METHOD_LABEL,
    PullChangesBlock,
    PullHeaderActions,
} from "@/components/github/pull-actions";
import {
    ReviewSummary,
    TimelineReview,
    TimelineReviewComment,
} from "@/components/github/review";
import {
    issueStatusOf,
    pullRequestStatusOf,
    StatusBadge,
} from "@/components/github/status-badge";
import {
    AvatarStack,
    ComposerSkeleton,
    EventSkeleton,
    LabelList,
    MessageSkeleton,
    PullSidebarSkeleton,
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
import { toastManager } from "@/components/ui/toast";
import { useConfirm } from "@/contexts/confirm-context";
import { useGithubAccount } from "@/hooks/github/use-github-account";
import {
    useCreateIssueComment,
    useDeleteIssueComment,
    useGithubCoords,
    useRepoPermissions,
    useUpdateIssueComment,
} from "@/hooks/github/use-github-issues";
import {
    useMergePullRequest,
    usePullRequest,
    useUpdatePullRequest,
} from "@/hooks/github/use-github-pull-requests";
import {
    useActiveTabHistory,
    useActiveTabRouter,
} from "@/hooks/tabs/use-active-tab-router";
import type {
    GithubCheckRun,
    GithubIssueComment,
    GithubIssueEvent,
    GithubPullRequestCommit,
    GithubPullRequestDetail,
    GithubPullRequestReview,
    GithubPullRequestReviewComment,
    GithubWorkflowRun,
    MergePullRequestBody,
    PullRequestMergeMethod,
} from "@/lib/backend/protocol";
import {
    buildPullTimeline,
    isTimelineMessage,
    timelineKey,
    timelineParticipants,
    type TimelineItem,
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
    pull,
    owner,
    repo,
    actions,
}: {
    item: TimelineItem;
    last: boolean;
    pull: GithubPullRequestDetail;
    owner: string;
    repo: string;
    actions: MessageActions;
}) {
    if (item.kind === "body") {
        return (
            <TimelineMessage
                text={pull.body || "_No description_"}
                author={pull.author.login}
                avatarUrl={pull.author.avatarUrl || undefined}
                createdAt={pull.createdAt}
                link={pull.htmlUrl || undefined}
                owner={owner}
                repo={repo}
                canQuote={actions.canQuote}
                onQuote={actions.onQuote}
                canEdit={actions.canEdit(pull.author.login)}
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
    if (item.kind === "commit") {
        return <TimelineCommit commit={item.commit} last={last} />;
    }
    if (item.kind === "review") {
        return (
            <TimelineReview
                review={item.review}
                comments={item.comments}
                owner={owner}
                repo={repo}
                canQuote={actions.canQuote}
                onQuote={actions.onQuote}
            />
        );
    }
    if (item.kind === "reviewComment") {
        return (
            <TimelineReviewComment
                comment={item.comment}
                owner={owner}
                repo={repo}
                canQuote={actions.canQuote}
                onQuote={actions.onQuote}
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

function PullSidebar({
    owner,
    repo,
    pull,
    participants,
    reviews,
    checks,
    workflows,
    checksLoading,
}: {
    owner: string;
    repo: string;
    pull: GithubPullRequestDetail;
    participants: ReturnType<typeof timelineParticipants>;
    reviews: GithubPullRequestReview[];
    checks: GithubCheckRun[];
    workflows: GithubWorkflowRun[];
    checksLoading: boolean;
}) {
    return (
        <Frame
            className="flex h-fit w-full flex-col"
            data-testid="pull-sidebar"
        >
            <PullChangesBlock pull={pull} />
            <ChecksSection
                owner={owner}
                repo={repo}
                checks={checks}
                workflows={workflows}
                isLoading={checksLoading}
            />
            <SidebarBlock label="Reviewers">
                <AvatarStack users={pull.assignees} />
            </SidebarBlock>
            <SidebarBlock label="Labels">
                <LabelList labels={pull.labels} />
            </SidebarBlock>
            <SidebarBlock label="Reviews">
                <ReviewSummary reviews={reviews} />
            </SidebarBlock>
            <SidebarBlock label="Participants">
                <AvatarStack users={participants} />
            </SidebarBlock>
        </Frame>
    );
}

function PullRequestContent({
    owner,
    repo,
    pull,
    commits,
    comments,
    events,
    reviews,
    reviewComments,
    checks,
    workflows,
    checksLoading,
    signedIn,
    viewerLogin,
    canPush,
    goBack,
}: {
    owner: string;
    repo: string;
    pull: GithubPullRequestDetail;
    commits: GithubPullRequestCommit[];
    comments: GithubIssueComment[];
    events: GithubIssueEvent[];
    reviews: GithubPullRequestReview[];
    reviewComments: GithubPullRequestReviewComment[];
    checks?: GithubCheckRun[];
    workflows?: GithubWorkflowRun[];
    checksLoading: boolean;
    signedIn: boolean;
    viewerLogin: string | null;
    canPush: boolean;
    goBack: () => void;
}) {
    const createComment = useCreateIssueComment();
    const updateComment = useUpdateIssueComment();
    const deleteComment = useDeleteIssueComment();
    const updatePull = useUpdatePullRequest();
    const merge = useMergePullRequest();
    const { confirm } = useConfirm();
    const composerRef = useRef<ConversationInputHandle>(null);

    const status = pullRequestStatusOf(pull);
    const isOpen = issueStatusOf(pull.state) === "open";
    const timeline = buildPullTimeline(pull, {
        commits,
        comments,
        events,
        reviews,
        reviewComments,
    });
    const participants = timelineParticipants(
        [pull.author, ...pull.assignees],
        comments,
        reviews.map((review) => review.author)
    );

    // Authors edit their own messages; repo push access moderates the rest.
    const canEdit = (authorLogin: string) =>
        signedIn &&
        (authorLogin.toLowerCase() === (viewerLogin ?? "").toLowerCase() ||
            canPush);
    const actions: MessageActions = {
        canQuote: signedIn,
        onQuote: (text) => composerRef.current?.insertQuote(text),
        canEdit,
        onSaveBody: async (body) => {
            await updatePull.updatePull(owner, repo, pull.number, { body });
        },
        onSaveComment: async (commentId, body) => {
            await updateComment.updateComment(
                owner,
                repo,
                commentId,
                pull.number,
                body
            );
        },
        onDeleteComment: async (commentId) => {
            await deleteComment.deleteComment(
                owner,
                repo,
                commentId,
                pull.number
            );
        },
    };

    const handleComment = async (body: string) => {
        try {
            await createComment.createComment(owner, repo, pull.number, body);
        } catch (error) {
            toastError("Could not post comment", error);
            throw error;
        }
    };

    const handleToggleState = async () => {
        try {
            await updatePull.updatePull(owner, repo, pull.number, {
                state: isOpen ? "closed" : "open",
            });
        } catch (error) {
            toastError(
                isOpen
                    ? "Could not close pull request"
                    : "Could not reopen pull request",
                error
            );
        }
    };

    const handleToggleDraft = async () => {
        try {
            await updatePull.updatePull(owner, repo, pull.number, {
                draft: !pull.draft,
            });
        } catch (error) {
            toastError("Could not change draft state", error);
        }
    };

    const handleMerge = async (method: PullRequestMergeMethod) => {
        const result = await confirm({
            title: `Merge pull request #${pull.number}?`,
            description: `${MERGE_METHOD_LABEL[method]}: ${pull.head.ref} into ${pull.base.ref}. This writes to the remote and cannot be undone from here.`,
            confirmText: "Merge",
        });
        if (!result.confirmed) return;
        const body: MergePullRequestBody = { mergeMethod: method };
        try {
            const outcome = await merge.mergePull(
                owner,
                repo,
                pull.number,
                body
            );
            if (!outcome.merged) {
                toastManager.add({
                    title: "GitHub declined the merge",
                    description: outcome.message,
                    type: "error",
                });
                return;
            }
            toastManager.add({ title: "Pull request merged", type: "success" });
        } catch (error) {
            toastError("Could not merge pull request", error);
        }
    };

    const mutating =
        createComment.pending ||
        updateComment.pending ||
        deleteComment.pending ||
        updatePull.pending;

    return (
        <div className="container flex h-full min-h-0 flex-col px-1 py-2">
            <header className="ui-selectable flex items-center justify-between">
                <div className="flex items-center gap-2">
                    <span className="truncate font-display text-3xl tracking-tight">
                        {pull.title}
                    </span>
                    <span className="self-end pb-0.5 font-mono text-muted-foreground">
                        #{pull.number}
                    </span>
                    <StatusBadge status={status} className="mb-1.5 self-end" />
                </div>
                <div className="flex items-center gap-2">
                    <PullHeaderActions
                        pull={pull}
                        canPush={canPush}
                        onMerge={(method) => void handleMerge(method)}
                        onToggleDraft={() => void handleToggleDraft()}
                        pending={merge.pending || updatePull.pending}
                    />
                    <Button
                        variant="outline"
                        size="icon"
                        onClick={goBack}
                        aria-label="Back to pull requests"
                    >
                        <X />
                    </Button>
                </div>
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
                                        pull={pull}
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
                                    isOpen
                                        ? "Close pull request"
                                        : "Reopen pull request"
                                }
                                onToggleState={() => void handleToggleState()}
                                owner={owner}
                                repo={repo}
                                onSubmit={handleComment}
                                inputRef={composerRef}
                            />
                        ) : null}
                    </ScrollArea>
                </div>
                <div className="flex flex-col gap-1">
                    <PullSidebar
                        owner={owner}
                        repo={repo}
                        pull={pull}
                        participants={participants}
                        reviews={reviews}
                        checks={checks ?? []}
                        workflows={workflows ?? []}
                        checksLoading={checksLoading}
                    />
                </div>
            </div>
        </div>
    );
}

function usePullParams() {
    const params = useParams({ strict: false });
    const repoId = Number(params.repoId);
    const pullNumber = Number(params.pullId);
    const valid =
        Number.isInteger(repoId) &&
        repoId > 0 &&
        Number.isInteger(pullNumber) &&
        pullNumber > 0;
    return { repoId, pullNumber, valid };
}

function usePullRouteData(repoId: number, pullNumber: number, valid: boolean) {
    const entry = useSelector(repositoryStore, (state) => {
        if (!valid) return undefined;
        for (const candidate of state.entries.values()) {
            if (candidate.repoId === repoId) return candidate;
        }
        return undefined;
    });
    const account = useGithubAccount();
    const { coords } = useGithubCoords(valid ? repoId : undefined);
    const number = valid ? pullNumber : null;
    const thread = usePullRequest(
        coords?.owner ?? null,
        coords?.repo ?? null,
        number
    );
    const permissions = useRepoPermissions(
        coords?.owner ?? null,
        coords?.repo ?? null
    );
    return { entry, account, coords, permissions, thread };
}

export function PullRequestPage() {
    const { repoId, pullNumber, valid } = usePullParams();
    const router = useActiveTabRouter();
    const history = useActiveTabHistory();
    const { entry, account, coords, permissions, thread } = usePullRouteData(
        repoId,
        pullNumber,
        valid
    );
    const {
        detail,
        comments,
        events,
        commits,
        reviews,
        reviewComments,
        checks,
        workflows,
    } = thread;

    const goBack = () => {
        if (history.canGoBack) {
            history.back();
        } else {
            router?.navigate({
                to: `/repo/${repoId}`,
                search: { view: "pulls" },
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
                    </div>
                    <div className="flex items-center gap-2">
                        <Skeleton className="h-8 w-28 rounded-lg" />
                        <Skeleton className="h-8 w-24 rounded-lg" />
                        <Button variant="outline" size="icon" onClick={goBack}>
                            <X />
                        </Button>
                    </div>
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
                        <PullSidebarSkeleton />
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
                        <EmptyTitle>Could not load pull request</EmptyTitle>
                        <EmptyDescription>
                            {detail.error instanceof Error
                                ? detail.error.message
                                : "The pull request could not be found."}
                        </EmptyDescription>
                    </EmptyHeader>
                    <Button variant="outline" size="sm" onClick={goBack}>
                        Back to pull requests
                    </Button>
                </Empty>
            </div>
        );
    }

    return (
        <PullRequestContent
            owner={coords?.owner ?? ""}
            repo={coords?.repo ?? ""}
            pull={detail.data}
            commits={commits.data ?? []}
            comments={comments.data ?? []}
            events={events.data ?? []}
            reviews={reviews.data ?? []}
            reviewComments={reviewComments.data ?? []}
            checks={checks.data}
            workflows={workflows.data}
            checksLoading={checks.isLoading}
            signedIn={account.data != null}
            viewerLogin={account.data?.login ?? null}
            canPush={permissions.data?.push === true}
            goBack={goBack}
        />
    );
}
