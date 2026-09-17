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
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import type { CommitSummary } from "@/lib/backend/protocol";

/**
 * Edits the HEAD commit message. Always mounted and driven by `open`; the
 * form lives inside the portal body, so it remounts fresh on every open.
 */
export function AmendCommitDialog({
    commit,
    open,
    onSubmit,
    onClose,
}: {
    commit: CommitSummary;
    open: boolean;
    onSubmit: (message: string) => Promise<boolean>;
    onClose: () => void;
}) {
    return (
        <Dialog
            open={open}
            onOpenChange={(next) => {
                if (!next) onClose();
            }}
        >
            <DialogPopup data-testid="amend-commit-dialog">
                <AmendCommitDialogBody
                    commit={commit}
                    onSubmit={onSubmit}
                    onClose={onClose}
                />
            </DialogPopup>
        </Dialog>
    );
}

function AmendCommitDialogBody({
    commit,
    onSubmit,
    onClose,
}: {
    commit: CommitSummary;
    onSubmit: (message: string) => Promise<boolean>;
    onClose: () => void;
}) {
    const [message, setMessage] = useState(commit.message);
    const [submitting, setSubmitting] = useState(false);

    const canSubmit = message.trim().length > 0 && !submitting;

    const handleSubmit = async () => {
        if (!canSubmit) return;
        setSubmitting(true);
        const ok = await onSubmit(message.trim());
        setSubmitting(false);
        if (ok) onClose();
    };

    return (
        <>
            <DialogHeader>
                <DialogTitle>Amend commit</DialogTitle>
                <DialogDescription>
                    Rewrites the message of{" "}
                    <span className="font-mono">{commit.id.slice(0, 7)}</span>.
                    The branch moves to the replacement commit.
                </DialogDescription>
            </DialogHeader>
            <DialogPanel className="flex flex-col gap-3">
                <div className="flex flex-col gap-1.5">
                    <Label htmlFor="amend-message">Message</Label>
                    <Textarea
                        id="amend-message"
                        value={message}
                        onChange={(event) => setMessage(event.target.value)}
                        rows={4}
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
                    Amend commit
                </Button>
            </DialogFooter>
        </>
    );
}
