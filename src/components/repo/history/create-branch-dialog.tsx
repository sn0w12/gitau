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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { CommitSummary } from "@/lib/backend/protocol";

/**
 * Creates a branch starting at the commit without moving HEAD. Always
 * mounted and driven by `open`; the form lives inside the portal body, so
 * it remounts fresh on every open.
 */
export function CreateBranchDialog({
    commit,
    open,
    onSubmit,
    onClose,
}: {
    commit: CommitSummary;
    open: boolean;
    onSubmit: (name: string) => Promise<boolean>;
    onClose: () => void;
}) {
    return (
        <Dialog
            open={open}
            onOpenChange={(next) => {
                if (!next) onClose();
            }}
        >
            <DialogPopup data-testid="create-branch-dialog">
                <CreateBranchDialogBody
                    commit={commit}
                    onSubmit={onSubmit}
                    onClose={onClose}
                />
            </DialogPopup>
        </Dialog>
    );
}

function CreateBranchDialogBody({
    commit,
    onSubmit,
    onClose,
}: {
    commit: CommitSummary;
    onSubmit: (name: string) => Promise<boolean>;
    onClose: () => void;
}) {
    const [name, setName] = useState("");
    const [submitting, setSubmitting] = useState(false);

    const canSubmit = name.trim().length > 0 && !submitting;

    const handleSubmit = async () => {
        if (!canSubmit) return;
        setSubmitting(true);
        const ok = await onSubmit(name.trim());
        setSubmitting(false);
        if (ok) onClose();
    };

    return (
        <>
            <DialogHeader>
                <DialogTitle>Create branch</DialogTitle>
                <DialogDescription>
                    Starts at{" "}
                    <span className="font-mono">{commit.id.slice(0, 7)}</span>.
                    Your current branch stays checked out.
                </DialogDescription>
            </DialogHeader>
            <DialogPanel className="flex flex-col gap-3">
                <div className="flex flex-col gap-1.5">
                    <Label htmlFor="create-branch-name">Name</Label>
                    <Input
                        id="create-branch-name"
                        value={name}
                        onChange={(event) => setName(event.target.value)}
                        placeholder="feature/thing"
                        autoFocus
                    />
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
                    Create branch
                </Button>
            </DialogFooter>
        </>
    );
}
