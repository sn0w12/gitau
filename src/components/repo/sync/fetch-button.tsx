import { useSelector } from "@tanstack/react-store";
import {
    ArrowDown,
    ArrowUp,
    CloudUpload,
    RefreshCw,
    Upload,
} from "lucide-react";
import {
    motion,
    useAnimationFrame,
    useMotionValue,
    useSpring,
    type MotionValue,
} from "motion/react";
import { useEffect, useMemo, useRef, useState } from "react";

import { PublishToGitHubDialog } from "@/components/repo/dialogs/publish-dialog";
import { ForcePushDialog } from "@/components/repo/sync/force-push-dialog";
import {
    useRemotes,
    useRepoListing,
    useRepositorySnapshot,
} from "@/hooks/repositories/use-repository-queries";
import { useSyncActions } from "@/hooks/repositories/use-sync-actions";
import { BORDER_GRADIENT, REPO_TOOLBAR_TRIGGER_CLASS } from "@/lib/constants";
import { deriveFetchAction } from "@/lib/repositories/fetch-action";
import { cn, formatRelativeDate } from "@/lib/utils";
import { fetchStore } from "@/stores/fetch-store";

const RELATIVE_TIME_TICK_MS = 30_000;

const TRIGGER_EXTRAS =
    "text-left disabled:pointer-events-none disabled:opacity-64";

/**
 * Toolbar sync button, mirroring GitHub Desktop's PushPullButton: fetch when
 * in sync, pull when behind (including diverged), push when ahead, publish a
 * branch without upstream, publish the repo without a remote. Pull, push, and
 * publish buttons carry a "Fetch" dropdown, and the subtitle is always the
 * last-fetch time.
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
    const [forcePushTarget, setForcePushTarget] = useState<null | {
        branchName: string;
        upstreamTarget: string | undefined;
        ahead: number;
        behind: number;
    }>(null);
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

    const target = remoteName
        ? `${remoteName.charAt(0).toUpperCase()}${remoteName.slice(1)}`
        : "";
    const fetchedLine = backgroundFetching
        ? "Fetching in the background…"
        : lastFetchedAt
          ? `Last fetched ${formatRelativeDate(lastFetchedAt)}`
          : "Never fetched";

    let body: React.ReactNode;
    if (!action) {
        body = (
            <div
                className={cn(
                    REPO_TOOLBAR_TRIGGER_CLASS,
                    TRIGGER_EXTRAS,
                    BORDER_GRADIENT
                )}
            />
        );
    } else if (action.kind === "noRemote") {
        body = (
            <button
                data-testid="publish-repository-button"
                onClick={() => setPublishOpen(true)}
                className={cn(
                    REPO_TOOLBAR_TRIGGER_CLASS,
                    TRIGGER_EXTRAS,
                    BORDER_GRADIENT
                )}
            >
                <ButtonContent
                    icon={<Upload className="size-7" strokeWidth="1.5px" />}
                    label="Publish repository"
                    subtitle="Publish this repository to GitHub"
                />
            </button>
        );
    } else if (action.kind === "detachedHead") {
        body = (
            <button
                disabled
                data-testid="detached-head-button"
                className={cn(
                    REPO_TOOLBAR_TRIGGER_CLASS,
                    TRIGGER_EXTRAS,
                    BORDER_GRADIENT
                )}
            >
                <ButtonContent
                    icon={<Upload className="size-7" strokeWidth="1.5px" />}
                    label="Publish branch"
                    subtitle="Cannot publish detached HEAD"
                />
            </button>
        );
    } else if (action.kind === "fetch") {
        body = (
            <button
                disabled={busy}
                data-testid="fetch-button"
                onClick={() => void handleActivate(action)}
                className={cn(
                    REPO_TOOLBAR_TRIGGER_CLASS,
                    TRIGGER_EXTRAS,
                    BORDER_GRADIENT
                )}
            >
                <ButtonContent
                    icon={<SpinnerIcon busy={busy} />}
                    label={`Fetch ${target}`}
                    subtitle={fetchedLine}
                />
            </button>
        );
    } else if (action.kind === "publishBranch") {
        body = (
            <button
                disabled={busy}
                data-testid="fetch-button"
                onClick={() => void handleActivate(action)}
                className={cn(
                    REPO_TOOLBAR_TRIGGER_CLASS,
                    TRIGGER_EXTRAS,
                    BORDER_GRADIENT
                )}
            >
                <ButtonContent
                    icon={
                        <SpinnerIcon
                            busy={busy}
                            idleIcon={
                                <CloudUpload
                                    className="size-7"
                                    strokeWidth="1.5px"
                                />
                            }
                        />
                    }
                    label="Publish branch"
                    subtitle="Publish this branch to GitHub"
                />
            </button>
        );
    } else if (action.kind === "pull") {
        body = (
            <button
                disabled={busy}
                data-testid="fetch-button"
                onClick={() => void handleActivate(action)}
                className={cn(
                    REPO_TOOLBAR_TRIGGER_CLASS,
                    TRIGGER_EXTRAS,
                    BORDER_GRADIENT
                )}
            >
                <ButtonContent
                    icon={
                        <SpinnerIcon
                            busy={busy}
                            idleIcon={
                                <ArrowDown
                                    className="size-7"
                                    strokeWidth="1.5px"
                                />
                            }
                        />
                    }
                    label={`Pull ${target}`}
                    subtitle={fetchedLine}
                    trailing={
                        <AheadBehind
                            ahead={action.ahead}
                            behind={action.behind}
                        />
                    }
                />
            </button>
        );
    } else {
        body = (
            <button
                disabled={busy}
                data-testid="fetch-button"
                onClick={() => void handleActivate(action)}
                className={cn(
                    REPO_TOOLBAR_TRIGGER_CLASS,
                    TRIGGER_EXTRAS,
                    BORDER_GRADIENT
                )}
            >
                <ButtonContent
                    icon={
                        <SpinnerIcon
                            busy={busy}
                            idleIcon={
                                <ArrowUp
                                    className="size-7"
                                    strokeWidth="1.5px"
                                />
                            }
                        />
                    }
                    label={`Push ${target}`}
                    subtitle={fetchedLine}
                    trailing={
                        <AheadBehind
                            ahead={action.ahead}
                            behind={action.behind}
                        />
                    }
                />
            </button>
        );
    }

    return (
        <>
            {body}
            <PublishToGitHubDialog
                open={publishOpen}
                repoId={repoId}
                repoPath={repoPath}
                onClose={() => setPublishOpen(false)}
            />
            {forcePushTarget !== null && (
                <ForcePushDialog
                    repoId={repoId}
                    branchName={forcePushTarget.branchName}
                    upstreamTarget={forcePushTarget.upstreamTarget}
                    ahead={forcePushTarget.ahead}
                    behind={forcePushTarget.behind}
                    open
                    onClose={() => setForcePushTarget(null)}
                    onConfirm={async () => {
                        setForcePushTarget(null);
                        await sync.push({ force: true });
                    }}
                />
            )}
        </>
    );

    async function handleActivate(
        resolved: Exclude<typeof action, null>
    ): Promise<void> {
        switch (resolved.kind) {
            case "fetch":
                return sync.fetch();
            case "pull":
                return sync.pull();
            case "push":
                return sync.push({});
            case "publishBranch":
                return sync.push({ setUpstream: true });
            case "noRemote":
                return setPublishOpen(true);
            case "detachedHead":
                return;
        }
    }
}

/** The compact `N↑ M↓` counts graphic GitHub Desktop renders on pull/push. */
function AheadBehind({ ahead, behind }: { ahead: number; behind: number }) {
    if (ahead === 0 && behind === 0) return null;
    return (
        <span className="flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground">
            {ahead > 0 && (
                <span className="flex items-center gap-0.5">
                    {ahead}
                    <ArrowUp className="size-3.5" />
                </span>
            )}
            {behind > 0 && (
                <span className="flex items-center gap-0.5">
                    {behind}
                    <ArrowDown className="size-3.5" />
                </span>
            )}
        </span>
    );
}

// Degrees per second the spinner settles into. The spring below chases a target
// advancing at this rate, so the icon winds up from rest on start and coasts to a
// stop on finish instead of snapping between static and full speed.
const SPIN_SPEED_DEG_PER_SEC = 360;
const SPIN_SPRING = { stiffness: 120, damping: 16, mass: 1 } as const;
const ICON_SLOT_CLASS = "absolute inset-0 flex items-center justify-center";
const ICON_SWAP_TRANSITION = { duration: 0.18, ease: "easeOut" } as const;
const BLURRED = "blur(4px)";
const SHARP = "blur(0px)";

type SpinPhase = "idle" | "spinning" | "settling";

/**
 * Fetch has no separate idle icon: the same `RefreshCw` stays mounted and only
 * its rotation is driven. Pull and push pass an `idleIcon`; the two glyphs
 * cross-fade through opacity and blur, and the handoff back waits for the spin
 * to finish its turn (`settling` -> `idle`) so the icon never swaps mid-spin.
 */
function SpinnerIcon({
    busy,
    idleIcon,
}: {
    busy: boolean;
    idleIcon?: React.ReactNode;
}) {
    const target = useMotionValue(0);
    const rotate = useSpring(target, SPIN_SPRING);
    const [phase, setPhase] = useState<SpinPhase>(busy ? "spinning" : "idle");
    const [wasBusy, setWasBusy] = useState(busy);
    const hasCustomIdle = idleIcon !== undefined;
    const showSpinner = !hasCustomIdle || phase !== "idle";

    // Track the busy edge during render rather than in an effect, so the phase
    // follows the prop without a second render pass.
    if (busy !== wasBusy) {
        setWasBusy(busy);
        setPhase((current) => {
            if (busy) return "spinning";
            return current === "spinning" ? "settling" : current;
        });
    }

    // Finish the turn the spin is partway through so the swap back to the idle
    // glyph lands on a completed rotation. `ceil` only moves forward, so it can
    // never rewind the way a nearest-multiple snap can.
    useEffect(() => {
        if (phase !== "settling") return;
        const angle = target.get();
        const nextTurn = Math.ceil(angle / 360) * 360;
        if (nextTurn !== angle) target.set(nextTurn);
    }, [phase, target]);

    return (
        <span className="relative size-7 shrink-0">
            {phase !== "idle" && (
                <SpinDriver
                    target={target}
                    rotate={rotate}
                    spinning={phase === "spinning"}
                    onSettled={() => setPhase("idle")}
                />
            )}
            <motion.span
                aria-hidden={hasCustomIdle && !showSpinner ? true : undefined}
                animate={
                    hasCustomIdle
                        ? {
                              opacity: showSpinner ? 1 : 0,
                              filter: showSpinner ? SHARP : BLURRED,
                          }
                        : undefined
                }
                className={ICON_SLOT_CLASS}
                style={{ rotate }}
                transition={ICON_SWAP_TRANSITION}
            >
                <RefreshCw
                    className="size-7"
                    strokeWidth="1.5px"
                    data-testid={busy ? "fetch-spinner" : undefined}
                />
            </motion.span>
            {hasCustomIdle && (
                <motion.span
                    aria-hidden={showSpinner ? true : undefined}
                    animate={{
                        opacity: showSpinner ? 0 : 1,
                        filter: showSpinner ? BLURRED : SHARP,
                    }}
                    className={ICON_SLOT_CLASS}
                    transition={ICON_SWAP_TRANSITION}
                >
                    {idleIcon}
                </motion.span>
            )}
        </span>
    );
}

/**
 * Advances the spin target while spinning, then watches the spring until it
 * settles. Mounted only while the spin is active, so the animation frame that
 * keeps rescheduling itself is gone once the icon is idle.
 */
function SpinDriver({
    target,
    rotate,
    spinning,
    onSettled,
}: {
    target: MotionValue<number>;
    rotate: MotionValue<number>;
    spinning: boolean;
    onSettled: () => void;
}) {
    const settledRef = useRef(false);

    useAnimationFrame((_, delta) => {
        if (spinning) {
            settledRef.current = false;
            target.set(target.get() + (SPIN_SPEED_DEG_PER_SEC * delta) / 1000);
            return;
        }

        if (settledRef.current) return;
        const close = Math.abs(rotate.get() - target.get()) < 0.5;
        const slow = Math.abs(rotate.getVelocity()) < 8;
        if (close && slow) {
            settledRef.current = true;
            onSettled();
        }
    });

    return null;
}

function ButtonContent({
    icon,
    label,
    subtitle,
    trailing,
}: {
    icon: React.ReactNode;
    label: string;
    subtitle?: string;
    trailing?: React.ReactNode;
}) {
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
