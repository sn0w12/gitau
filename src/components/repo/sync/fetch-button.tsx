import { useSelector } from "@tanstack/react-store";
import { ArrowDown, ArrowUp, CloudUpload, Upload } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import type { ReactNode } from "react";
import { useEffect, useMemo, useState } from "react";

import { PublishRepositoryDialog } from "@/components/repo/dialogs/publish-dialog";
import { SpinnerIcon } from "@/components/repo/sync/spinner-icon";
import {
    ToolbarTrigger,
    ToolbarTriggerFrame,
} from "@/components/repo/toolbar-trigger";
import { Badge } from "@/components/ui/badge";
import {
    useRemotes,
    useRepoListing,
    useRepositorySnapshot,
} from "@/hooks/repositories/use-repository-queries";
import { useSyncActions } from "@/hooks/repositories/use-sync-actions";
import { REPO_TOOLBAR_TRIGGER_CLASS } from "@/lib/constants";
import { EASE_SNAPPY } from "@/lib/motion";
import {
    deriveFetchAction,
    type FetchAction,
} from "@/lib/repositories/fetch-action";
import { formatRelativeDate } from "@/lib/utils";
import { fetchStore } from "@/stores/fetch-store";

const RELATIVE_TIME_TICK_MS = 30_000;
const ICON_PROPS = { className: "size-7", strokeWidth: "1.5px" } as const;

/**
 * Toolbar sync button, mirroring GitHub Desktop's PushPullButton: fetch when
 * in sync, pull when behind (including diverged), push when ahead, publish a
 * branch without upstream, publish the repo without a remote. The subtitle is
 * always the last-fetch time.
 */
export function RepoFetchButton({
    repoId,
    repoPath,
}: {
    repoId: number;
    repoPath: string;
}) {
    const snapshot = useRepositorySnapshot(repoId);
    const listing = useRepoListing(repoId);
    const remotes = useRemotes(repoId);
    const sync = useSyncActions(repoId, repoPath);

    const lastFetchedAt = useSelector(
        fetchStore,
        (state) => state.lastFetchedByPath[repoPath]
    );
    const backgroundFetching = useSelector(
        fetchStore,
        (state) => state.inFlightByPath[repoPath] === true
    );
    const busy = sync.busy || backgroundFetching;
    const [publishOpen, setPublishOpen] = useState(false);
    // Re-render periodically so "X minutes ago" stays honest.
    const [, setTick] = useState(0);
    useEffect(() => {
        if (!lastFetchedAt) return;
        const timer = setInterval(
            () => setTick((n) => n + 1),
            RELATIVE_TIME_TICK_MS
        );
        return () => clearInterval(timer);
    }, [lastFetchedAt]);

    const head = snapshot.data?.head;
    const remoteName = sync.remoteName;
    const action = useMemo(() => {
        if (!head) return null;
        return deriveFetchAction(
            head,
            listing.data?.branches ?? [],
            remotes.data ?? []
        );
    }, [head, listing.data?.branches, remotes.data]);

    const remoteLabel = remoteName
        ? `${remoteName.charAt(0).toUpperCase()}${remoteName.slice(1)}`
        : "";
    const fetchedLine = backgroundFetching
        ? "Fetching in the background…"
        : lastFetchedAt
          ? `Last fetched ${formatRelativeDate(lastFetchedAt)}`
          : "Never fetched";

    const spec = action
        ? resolveButtonSpec(action, {
              remoteLabel,
              fetchedLine,
              busy,
              sync,
              onPublishRepository: () => setPublishOpen(true),
          })
        : null;

    return (
        <>
            {spec ? (
                <ToolbarTrigger
                    data-testid={spec.testId}
                    disabled={spec.disabled}
                    onClick={spec.onClick}
                >
                    <ButtonContent {...spec.content} />
                </ToolbarTrigger>
            ) : (
                <ToolbarTriggerFrame>
                    <div className={REPO_TOOLBAR_TRIGGER_CLASS} />
                </ToolbarTriggerFrame>
            )}
            <PublishRepositoryDialog
                open={publishOpen}
                repoId={repoId}
                repoPath={repoPath}
                onClose={() => setPublishOpen(false)}
            />
        </>
    );
}

type SyncActions = ReturnType<typeof useSyncActions>;

interface ButtonSpec {
    testId: string;
    disabled?: boolean;
    onClick?: () => void;
    content: {
        icon: ReactNode;
        label: string;
        subtitle?: string;
        trailing?: ReactNode;
    };
}

function resolveButtonSpec(
    action: FetchAction,
    state: {
        remoteLabel: string;
        fetchedLine: string;
        busy: boolean;
        sync: SyncActions;
        onPublishRepository: () => void;
    }
): ButtonSpec {
    const { remoteLabel, fetchedLine, busy, sync, onPublishRepository } = state;
    const spinner = (idleIcon?: ReactNode) => (
        <SpinnerIcon busy={busy} idleIcon={idleIcon} />
    );
    const counts = (ahead: number, behind: number) => (
        <AheadBehind ahead={ahead} behind={behind} />
    );

    switch (action.kind) {
        case "noRemote":
            return {
                testId: "publish-repository-button",
                onClick: onPublishRepository,
                content: {
                    icon: <Upload {...ICON_PROPS} />,
                    label: "Publish repository",
                    subtitle: "Create a remote and push this branch",
                },
            };
        case "detachedHead":
            return {
                testId: "detached-head-button",
                disabled: true,
                content: {
                    icon: <Upload {...ICON_PROPS} />,
                    label: "Publish branch",
                    subtitle: "Cannot publish detached HEAD",
                },
            };
        case "fetch":
            return {
                testId: "fetch-button",
                disabled: busy,
                onClick: () => void sync.fetch(),
                content: {
                    icon: spinner(),
                    label: `Fetch ${remoteLabel}`,
                    subtitle: fetchedLine,
                },
            };
        case "publishBranch":
            return {
                testId: "fetch-button",
                disabled: busy,
                onClick: () => void sync.push({ setUpstream: true }),
                content: {
                    icon: spinner(<CloudUpload {...ICON_PROPS} />),
                    label: "Publish branch",
                    subtitle: "Push this branch and set its upstream",
                },
            };
        case "pull":
            return {
                testId: "fetch-button",
                disabled: busy,
                onClick: () => void sync.pull(),
                content: {
                    icon: spinner(<ArrowDown {...ICON_PROPS} />),
                    label: `Pull ${remoteLabel}`,
                    subtitle: fetchedLine,
                    trailing: counts(action.ahead, action.behind),
                },
            };
        case "push":
            return {
                testId: "fetch-button",
                disabled: busy,
                onClick: () => void sync.push({}),
                content: {
                    icon: spinner(<ArrowUp {...ICON_PROPS} />),
                    label: `Push ${remoteLabel}`,
                    subtitle: fetchedLine,
                    trailing: counts(action.ahead, action.behind),
                },
            };
    }
}

const BADGE_TRANSITION = { duration: 0.18, ease: EASE_SNAPPY } as const;
const BADGE_ANIMATION = {
    initial: { opacity: 0, scale: 0.6 },
    animate: { opacity: 1, scale: 1 },
    exit: { opacity: 0, scale: 0.6 },
    transition: BADGE_TRANSITION,
} as const;

/** The compact `N↑ M↓` counts graphic GitHub Desktop renders on pull/push. */
function AheadBehind({ ahead, behind }: { ahead: number; behind: number }) {
    const counts = [
        { key: "ahead", count: ahead, icon: ArrowUp },
        { key: "behind", count: behind, icon: ArrowDown },
    ].filter((entry) => entry.count > 0);

    return (
        <span className="flex shrink-0 items-center gap-1.5 text-sm text-muted-foreground">
            <AnimatePresence initial={false}>
                {counts.map(({ key, count, icon: Icon }) => (
                    <motion.span key={key} {...BADGE_ANIMATION}>
                        <Badge className="gap-0.5 pr-[1px]" size="sm">
                            <span className="mt-[1px]">{count}</span>
                            <Icon className="size-3" />
                        </Badge>
                    </motion.span>
                ))}
            </AnimatePresence>
        </span>
    );
}

function ButtonContent({
    icon,
    label,
    subtitle,
    trailing,
}: ButtonSpec["content"]) {
    const firstLabel = label.split(" ")[0];
    const restLabel = label.slice(firstLabel.length);

    return (
        <span className="flex flex-1 items-center gap-3.5">
            {icon}
            <span className="flex min-w-0 flex-1 flex-col">
                <span className="hidden truncate text-xs text-muted-foreground lg:block">
                    {subtitle}
                </span>
                <span className="truncate pr-1.5 font-semibold lg:pr-0">
                    <span>{firstLabel}</span>
                    <span className="hidden lg:inline">{restLabel}</span>
                </span>
            </span>
            {trailing}
        </span>
    );
}
