import { useQueryClient } from "@tanstack/react-query";
import { FolderGit2, GitBranch } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogClose,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogPanel,
    DialogPopup,
    DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTab } from "@/components/ui/tabs";
import {
    useCreateBranchMutation,
    useCreateWorktreeMutation,
} from "@/lib/backend/mutations/repository-mutations";
import { repositoryKeys } from "@/lib/backend/queries/query-keys";
import { toastError } from "@/lib/toast-error";
import { cn } from "@/lib/utils";

/** Conservative git refname subset: no whitespace or glob/revision chars. */
function isValidBranchName(name: string): boolean {
    if (
        name.length === 0 ||
        name.startsWith("-") ||
        name.startsWith("/") ||
        name.endsWith(".") ||
        name.endsWith("/") ||
        name.includes("..") ||
        name.includes("//") ||
        name.includes("@{")
    ) {
        return false;
    }
    return /^[^\s~^:?*[\\]+$/.test(name);
}

type CreationTarget = "branch" | "worktree";

/**
 * Creates either a branch (checked out immediately) or a linked worktree.
 * Always mounted and driven by `open`; the form lives inside the portal body,
 * so it remounts fresh on every open.
 */
export function CreateCheckoutDialog({
    repoId,
    existingBranches,
    open,
    onClose,
}: {
    repoId: number;
    existingBranches: readonly string[];
    open: boolean;
    onClose: () => void;
}) {
    return (
        <Dialog
            open={open}
            onOpenChange={(next) => {
                if (!next) onClose();
            }}
        >
            <DialogPopup data-testid="create-checkout-dialog">
                <CreateCheckoutForm
                    repoId={repoId}
                    existingBranches={existingBranches}
                    onClose={onClose}
                />
            </DialogPopup>
        </Dialog>
    );
}

function CreateCheckoutForm({
    repoId,
    existingBranches,
    onClose,
}: {
    repoId: number;
    existingBranches: readonly string[];
    onClose: () => void;
}) {
    const createBranch = useCreateBranchMutation(repoId);
    const createWorktree = useCreateWorktreeMutation(repoId);
    const queryClient = useQueryClient();

    const [target, setTarget] = useState<CreationTarget>("branch");
    const [name, setName] = useState("");
    const [startPoint, setStartPoint] = useState("");
    const [path, setPath] = useState("");
    const [submitting, setSubmitting] = useState(false);

    const trimmed = name.trim();
    const nameTaken =
        !isValidBranchName(trimmed) || existingBranches.includes(trimmed);
    const canSubmit = trimmed.length > 0 && !nameTaken && !submitting;

    const handleSubmit = async () => {
        if (!canSubmit) return;
        setSubmitting(true);
        try {
            if (target === "branch") {
                await createBranch.mutateAsync({
                    name: trimmed,
                    startPoint: startPoint.trim() || undefined,
                });
                // Creating from HEAD moves the checkout, so the listing's
                // ahead/behind data is stale for the new branch.
                queryClient.removeQueries({
                    queryKey: repositoryKeys.listing(repoId),
                });
            } else {
                await createWorktree.mutateAsync({
                    name: trimmed,
                    startPoint: startPoint.trim() || undefined,
                    path: path.trim() || undefined,
                });
            }
            onClose();
        } catch (error) {
            toastError(
                target === "branch"
                    ? "Could not create branch"
                    : "Could not create worktree",
                error
            );
        } finally {
            setSubmitting(false);
        }
    };

    return (
        <>
            <DialogHeader>
                <DialogTitle>New checkout</DialogTitle>
                <DialogDescription>
                    {target === "branch"
                        ? "Creates the branch and checks it out here."
                        : "Adds a linked checkout beside this repository."}
                </DialogDescription>
            </DialogHeader>
            <DialogPanel
                className="p-0"
                onKeyDown={(event) => {
                    if (event.key === "Enter") {
                        event.preventDefault();
                        void handleSubmit();
                    }
                }}
            >
                <div className="flex flex-col gap-2 px-6 pb-6">
                    <Tabs
                        value={target}
                        onValueChange={(value) => {
                            if (value === "branch" || value === "worktree") {
                                setTarget(value);
                            }
                        }}
                    >
                        <TabsList className="w-full">
                            <TabsTab value="branch" className="flex-1">
                                <GitBranch />
                                Branch
                            </TabsTab>
                            <TabsTab value="worktree" className="flex-1">
                                <FolderGit2 />
                                Worktree
                            </TabsTab>
                        </TabsList>
                    </Tabs>
                    <Field>
                        <FieldLabel htmlFor="create-checkout-name">
                            Name
                        </FieldLabel>
                        <Input
                            id="create-checkout-name"
                            value={name}
                            onChange={(event) => setName(event.target.value)}
                            placeholder="feature/thing"
                            aria-label="Name"
                            autoFocus
                        />
                        {trimmed.length > 0 && nameTaken && (
                            <p className="text-xs text-destructive">
                                That name is invalid or already exists locally.
                            </p>
                        )}
                    </Field>
                    <Field>
                        <FieldLabel htmlFor="create-checkout-start">
                            Start point
                        </FieldLabel>
                        <Input
                            id="create-checkout-start"
                            value={startPoint}
                            onChange={(event) =>
                                setStartPoint(event.target.value)
                            }
                            placeholder={
                                target === "branch"
                                    ? "Defaults to HEAD"
                                    : "HEAD"
                            }
                            aria-label="Start point"
                        />
                    </Field>
                    <div
                        className={cn(
                            "grid transition-[grid-template-rows,opacity,margin] duration-200 ease-snappy",
                            // Collapsed, the row is 0fr but the parent's gap-3
                            // would still reserve 12px, so cancel it out.
                            target === "worktree"
                                ? "mt-0 grid-rows-[1fr] opacity-100"
                                : "-mt-3 grid-rows-[0fr] opacity-0"
                        )}
                    >
                        <div className="min-h-0 overflow-hidden">
                            <Field>
                                <FieldLabel htmlFor="create-checkout-path">
                                    Path
                                </FieldLabel>
                                <Input
                                    id="create-checkout-path"
                                    value={path}
                                    onChange={(event) =>
                                        setPath(event.target.value)
                                    }
                                    placeholder="Defaults to a folder named after the worktree"
                                    aria-label="Path"
                                    tabIndex={
                                        target === "worktree" ? undefined : -1
                                    }
                                />
                            </Field>
                        </div>
                    </div>
                </div>
            </DialogPanel>
            <DialogFooter>
                <DialogClose render={<Button variant="ghost" />}>
                    Cancel
                </DialogClose>
                <Button
                    disabled={!canSubmit}
                    loading={submitting}
                    onClick={() => void handleSubmit()}
                >
                    {target === "branch" ? "Create branch" : "Create worktree"}
                </Button>
            </DialogFooter>
        </>
    );
}
