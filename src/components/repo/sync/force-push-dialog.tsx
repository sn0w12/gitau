import { useQuery } from "@tanstack/react-query";
import { TriangleAlert } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogPanel,
    DialogPopup,
    DialogTitle,
} from "@/components/ui/dialog";
import { useAppServices } from "@/contexts/services-context";
import { historyPageQuery } from "@/lib/backend/queries/repository-queries";

/**
 * One-step confirmation for a force push. Two walks against the backend show
 * exactly what gets rewritten: commits on the remote that the local branch
 * replaces, and the local commits replacing them. The walk from the upstream
 * tip needs the remote SHA carried in the branch listing; when it is absent
 * the dialog degrades to the plain ahead/behind counts.
 */
export function ForcePushDialog({
    repoId,
    branchName,
    upstreamTarget,
    ahead,
    behind,
    open,
    onClose,
    onConfirm,
}: {
    repoId: number;
    branchName: string;
    upstreamTarget: string | undefined;
    ahead: number;
    behind: number;
    open: boolean;
    onClose: () => void;
    onConfirm: () => Promise<void>;
}) {
    return (
        <Dialog
            open={open}
            onOpenChange={(next) => {
                if (!next) onClose();
            }}
        >
            <DialogPopup data-testid="force-push-dialog">
                <ForcePushDialogBody
                    repoId={repoId}
                    branchName={branchName}
                    upstreamTarget={upstreamTarget}
                    ahead={ahead}
                    behind={behind}
                    open={open}
                    onClose={onClose}
                    onConfirm={onConfirm}
                />
            </DialogPopup>
        </Dialog>
    );
}

function ForcePushDialogBody({
    repoId,
    branchName,
    upstreamTarget,
    ahead,
    behind,
    open,
    onClose,
    onConfirm,
}: {
    repoId: number;
    branchName: string;
    upstreamTarget: string | undefined;
    ahead: number;
    behind: number;
    open: boolean;
    onClose: () => void;
    onConfirm: () => Promise<void>;
}) {
    const { backend } = useAppServices();

    // Remote-only commits: walk from the upstream tip, hiding everything the
    // local branch still reaches. This is the loss a force push causes.
    const droppedQuery = useQuery({
        ...historyPageQuery(
            { backend },
            repoId,
            upstreamTarget
                ? {
                      revision: upstreamTarget,
                      excludeReachableFrom: ["HEAD"],
                      limit: 100,
                  }
                : { revision: upstreamTarget, limit: 1 }
        ),
        enabled: open && upstreamTarget !== undefined,
    });
    const localQuery = useQuery({
        ...historyPageQuery({ backend }, repoId, {
            revision: "HEAD",
            excludeReachableFrom: upstreamTarget ? [upstreamTarget] : undefined,
            limit: 20,
        }),
        enabled: open && upstreamTarget !== undefined,
    });

    const dropped = droppedQuery.data?.commits ?? [];
    const replacing = localQuery.data?.commits ?? [];
    const previewAvailable = upstreamTarget !== undefined;

    return (
        <>
            <DialogHeader>
                <DialogTitle className="flex items-center gap-2">
                    <TriangleAlert className="size-4 text-warning" />
                    Force push to {branchName}?
                </DialogTitle>
                <DialogDescription>
                    The remote branch has {behind} commit
                    {behind === 1 ? "" : "s"} you do not have
                    {ahead > 0 ? ` and you are ${ahead} ahead` : ""}. Force
                    pushing overwrites the remote branch.
                </DialogDescription>
            </DialogHeader>
            {previewAvailable ? (
                <DialogPanel className="flex max-h-80 flex-col gap-3 overflow-y-auto text-sm">
                    <ForcePushList
                        title={`Discarded from the remote (${dropped.length})`}
                        empty="The remote commits are already merged locally; nothing is lost."
                        commits={dropped}
                        loading={droppedQuery.isPending}
                        destructive
                    />
                    <ForcePushList
                        title={`Replacing it with (${replacing.length})`}
                        empty=""
                        commits={replacing}
                        loading={localQuery.isPending}
                    />
                </DialogPanel>
            ) : (
                <DialogPanel>
                    <p className="text-sm text-muted-foreground">
                        The remote branch tip is not known yet, so the commits
                        being overwritten cannot be listed.
                    </p>
                </DialogPanel>
            )}
            <DialogFooter>
                <Button variant="outline" onClick={onClose}>
                    Cancel
                </Button>
                <Button variant="destructive" onClick={() => void onConfirm()}>
                    Force push
                </Button>
            </DialogFooter>
        </>
    );
}

function ForcePushList({
    title,
    empty,
    commits,
    loading,
    destructive = false,
}: {
    title: string;
    empty: string;
    commits: Array<{ id: string; summaryLine: string }>;
    loading: boolean;
    destructive?: boolean;
}) {
    return (
        <div className="flex flex-col gap-1">
            <p
                className={
                    destructive
                        ? "font-semibold text-destructive"
                        : "font-semibold"
                }
            >
                {title}
            </p>
            {loading ? (
                <p className="text-muted-foreground">Loading…</p>
            ) : commits.length === 0 ? (
                <p className="text-muted-foreground">{empty}</p>
            ) : (
                <ul className="flex flex-col gap-0.5">
                    {commits.map((commit) => (
                        <li
                            key={commit.id}
                            className="flex items-baseline gap-2"
                        >
                            <code className="shrink-0 font-mono text-xs text-muted-foreground">
                                {commit.id.slice(0, 7)}
                            </code>
                            <span className="truncate">
                                {commit.summaryLine}
                            </span>
                        </li>
                    ))}
                </ul>
            )}
        </div>
    );
}
