import { useId, useState } from "react";

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
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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
    const checkboxId = useId();

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
            <DialogPanel className="flex flex-col gap-3">
                <div className="flex flex-col gap-1.5">
                    <Label htmlFor="stash-message">Message</Label>
                    <Input
                        id="stash-message"
                        value={message}
                        onChange={(event) => setMessage(event.target.value)}
                        placeholder="Optional"
                        autoFocus
                    />
                </div>
                <Label htmlFor={checkboxId}>
                    <Checkbox
                        id={checkboxId}
                        checked={includeUntracked}
                        onCheckedChange={(checked) =>
                            setIncludeUntracked(checked === true)
                        }
                    />
                    Include untracked files
                </Label>
            </DialogPanel>
            <DialogFooter>
                <DialogClose render={<Button variant="ghost" />}>
                    Cancel
                </DialogClose>
                <Button
                    loading={push.isPending}
                    onClick={() => void handleSubmit()}
                >
                    Stash
                </Button>
            </DialogFooter>
        </>
    );
}
