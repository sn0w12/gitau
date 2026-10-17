import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { Form } from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { useStashPushMutation } from "@/lib/backend/mutations/repository-mutations";
import { toastError } from "@/lib/toast-error";

export function StashChangesDialog({
    repoId,
    open,
    paths,
    defaultIncludeUntracked = false,
    onClose,
}: {
    repoId: number;
    open: boolean;
    /** When set, only these paths are stashed; otherwise the whole tree. */
    paths?: string[];
    defaultIncludeUntracked?: boolean;
    onClose: () => void;
}) {
    return (
        <Dialog
            open={open}
            onOpenChange={(next) => {
                if (!next) onClose();
            }}
        >
            <DialogPopup data-testid="stash-changes-dialog">
                <StashChangesDialogBody
                    repoId={repoId}
                    paths={paths}
                    defaultIncludeUntracked={defaultIncludeUntracked}
                    onClose={onClose}
                />
            </DialogPopup>
        </Dialog>
    );
}

function StashChangesDialogBody({
    repoId,
    paths,
    defaultIncludeUntracked,
    onClose,
}: {
    repoId: number;
    paths?: string[];
    defaultIncludeUntracked: boolean;
    onClose: () => void;
}) {
    const push = useStashPushMutation(repoId);
    const [message, setMessage] = useState("");
    const [includeUntracked, setIncludeUntracked] = useState(
        defaultIncludeUntracked
    );

    const scoped = paths !== undefined;

    const handleSubmit = async () => {
        try {
            await push.mutateAsync({
                message: message.trim() || undefined,
                includeUntracked,
                paths,
            });
            onClose();
        } catch (error) {
            toastError("Could not stash changes", error);
        }
    };

    return (
        <>
            <DialogHeader>
                <DialogTitle>
                    {scoped ? "Stash changes" : "Stash all changes"}
                </DialogTitle>
                <DialogDescription>
                    {scoped
                        ? "Saves the selected changes on the stash and restores them to HEAD."
                        : "Saves your local changes on the stash and leaves a clean working tree."}
                </DialogDescription>
            </DialogHeader>
            <Form
                className="contents"
                onSubmit={(event) => {
                    event.preventDefault();
                    void handleSubmit();
                }}
            >
                <DialogPanel className="flex flex-col gap-3">
                    <Field>
                        <FieldLabel>Message</FieldLabel>
                        <Input
                            value={message}
                            onChange={(event) => setMessage(event.target.value)}
                            placeholder="Optional"
                            autoFocus
                        />
                    </Field>
                    <Field>
                        <FieldLabel>
                            <Checkbox
                                checked={includeUntracked}
                                onCheckedChange={(checked) =>
                                    setIncludeUntracked(checked === true)
                                }
                            />
                            Include untracked files
                        </FieldLabel>
                    </Field>
                </DialogPanel>
                <DialogFooter>
                    <DialogClose render={<Button variant="ghost" />}>
                        Cancel
                    </DialogClose>
                    <Button type="submit" loading={push.isPending}>
                        Stash
                    </Button>
                </DialogFooter>
            </Form>
        </>
    );
}
