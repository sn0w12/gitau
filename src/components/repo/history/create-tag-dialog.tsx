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
import { Textarea } from "@/components/ui/textarea";
import type { TagFormInput } from "@/hooks/repositories/use-commit-actions";
import type { CommitSummary } from "@/lib/backend/protocol";

/**
 * Creates a tag pointing at the commit: lightweight without an annotation,
 * annotated with one. Always mounted and driven by `open`; the form lives
 * inside the portal body, so it remounts fresh on every open.
 */
export function CreateTagDialog({
    commit,
    open,
    onSubmit,
    onClose,
}: {
    commit: CommitSummary;
    open: boolean;
    onSubmit: (input: TagFormInput) => Promise<boolean>;
    onClose: () => void;
}) {
    return (
        <Dialog
            open={open}
            onOpenChange={(next) => {
                if (!next) onClose();
            }}
        >
            <DialogPopup data-testid="create-tag-dialog">
                <CreateTagDialogBody
                    commit={commit}
                    onSubmit={onSubmit}
                    onClose={onClose}
                />
            </DialogPopup>
        </Dialog>
    );
}

function CreateTagDialogBody({
    commit,
    onSubmit,
    onClose,
}: {
    commit: CommitSummary;
    onSubmit: (input: TagFormInput) => Promise<boolean>;
    onClose: () => void;
}) {
    const [name, setName] = useState("");
    const [message, setMessage] = useState("");
    const [submitting, setSubmitting] = useState(false);

    const canSubmit = name.trim().length > 0 && !submitting;

    const handleSubmit = async () => {
        if (!canSubmit) return;
        setSubmitting(true);
        const ok = await onSubmit({
            name: name.trim(),
            message: message.trim() || undefined,
        });
        setSubmitting(false);
        if (ok) onClose();
    };

    return (
        <>
            <DialogHeader>
                <DialogTitle>Create tag</DialogTitle>
                <DialogDescription>
                    Tags{" "}
                    <span className="font-mono">{commit.id.slice(0, 7)}</span>.
                    Leave the annotation empty for a lightweight tag.
                </DialogDescription>
            </DialogHeader>
            <DialogPanel className="flex flex-col gap-3">
                <div className="flex flex-col gap-1.5">
                    <Label htmlFor="create-tag-name">Name</Label>
                    <Input
                        id="create-tag-name"
                        value={name}
                        onChange={(event) => setName(event.target.value)}
                        placeholder="v1.2.0"
                        autoFocus
                    />
                </div>
                <div className="flex flex-col gap-1.5">
                    <Label htmlFor="create-tag-message">
                        Annotation (optional)
                    </Label>
                    <Textarea
                        id="create-tag-message"
                        value={message}
                        onChange={(event) => setMessage(event.target.value)}
                        rows={3}
                        placeholder="Release notes"
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
                    Create tag
                </Button>
            </DialogFooter>
        </>
    );
}
