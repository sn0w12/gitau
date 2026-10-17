import { useSelector } from "@tanstack/react-store";
import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";

import { useAppServices } from "@/contexts/services-context";
import { useActiveConnection } from "@/hooks/forge/use-forge-account";
import { CloneSessionController } from "@/lib/repositories/clone-session";
import { joinRepoPath } from "@/lib/repositories/create-repository";
import {
    lastRepositoryDirectory,
    rememberRepositoryDirectory,
} from "@/lib/repositories/destination-memory";

import { Button } from "../../ui/button";
import {
    Dialog,
    DialogClose,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogPanel,
    DialogPopup,
    DialogTitle,
} from "../../ui/dialog";
import { Field, FieldLabel } from "../../ui/field";
import { Form } from "../../ui/form";
import { Input } from "../../ui/input";
import { CloneProgressPanel } from "./clone-progress-panel";
import { DestinationInput } from "./destination-input";

const SWAP_SPRING = { type: "spring", stiffness: 420, damping: 34 } as const;

/**
 * Clones a remote repository into a new folder. The body morphs between the
 * URL form and the live transfer view, so starting or cancelling a clone
 * resizes the dialog smoothly instead of snapping.
 */
export function CloneRepoDialog({
    open,
    onClose,
    onCloned,
}: {
    open: boolean;
    onClose: () => void;
    onCloned: (repoPath: string) => void;
}) {
    const { backend } = useAppServices();
    const connection = useActiveConnection();
    const [controller] = useState(() => new CloneSessionController(backend));
    const session = useSelector(controller.store, (state) => state);

    const [parentDirectory, setParentDirectory] = useState(
        lastRepositoryDirectory
    );
    const [url, setUrl] = useState("");
    const [folderName, setFolderName] = useState("");
    const [nameTouched, setNameTouched] = useState(false);

    // Fresh form for the next open; field-only so a cancelled note survives.
    // The session itself returns to idle on close and after a successful
    // clone.
    const resetFields = () => {
        setParentDirectory(lastRepositoryDirectory());
        setUrl("");
        setFolderName("");
        setNameTouched(false);
    };

    const resetForm = () => {
        controller.reset();
        resetFields();
    };

    const handleClose = () => {
        // Closing mid-clone must not leave an invisible transfer running.
        if (running) void controller.cancel();
        resetForm();
        onClose();
    };

    const trimmedUrl = url.trim();
    const trimmedFolder = folderName.trim();
    const running = session.status === "running";
    const canClone =
        isLikelyUrl(trimmedUrl) && !!trimmedFolder && !!parentDirectory;

    const handleChangeUrl = (next: string) => {
        setUrl(next);
        if (!nameTouched) setFolderName(repoSlugFromUrl(next));
    };

    const handleClone = () => {
        if (!canClone) return;
        rememberRepositoryDirectory(parentDirectory);
        void controller
            .start({
                url: normalizeUrl(trimmedUrl, connection?.host),
                destination: joinRepoPath(parentDirectory, trimmedFolder),
            })
            .then((outcome) => {
                resetFields();
                if (outcome.status === "completed") {
                    // The parent closes the dialog itself on success, so
                    // handleClose never runs and the session would stay
                    // terminal; a later start() would then be refused.
                    controller.reset();
                    onCloned(outcome.repoPath);
                }
            });
    };

    const lastError =
        session.status === "failed" ? (session.error?.message ?? null) : null;
    const wasCancelled = session.status === "cancelled";

    return (
        <Dialog open={open} onOpenChange={(next) => !next && handleClose()}>
            <DialogPopup data-testid="clone-repo-dialog">
                <DialogHeader>
                    <DialogTitle>
                        {running ? "Cloning repository" : "Clone repository"}
                    </DialogTitle>
                    <DialogDescription className="truncate">
                        {running
                            ? session.url || trimmedUrl
                            : trimmedUrl ||
                              "Downloads a remote repository into a new folder."}
                    </DialogDescription>
                </DialogHeader>

                <Form
                    className="contents"
                    onSubmit={(event) => {
                        event.preventDefault();
                        handleClone();
                    }}
                >
                    <DialogPanel className="p-0">
                        <motion.div
                            layout
                            className="px-6 pb-4"
                            transition={SWAP_SPRING}
                        >
                            <AnimatePresence mode="popLayout" initial={false}>
                                {running ? (
                                    <motion.section
                                        key="progress"
                                        className="w-full py-2"
                                        initial={{ opacity: 0, y: 8 }}
                                        animate={{ opacity: 1, y: 0 }}
                                        exit={{ opacity: 0, y: -8 }}
                                        transition={{ duration: 0.16 }}
                                    >
                                        <CloneProgressPanel
                                            progress={session.progress}
                                        />
                                    </motion.section>
                                ) : (
                                    <motion.section
                                        key="form"
                                        className="flex w-full flex-col gap-3"
                                        layout
                                        initial={{ opacity: 0, y: -4 }}
                                        animate={{ opacity: 1, y: 0 }}
                                        exit={{ opacity: 0, y: -6 }}
                                        transition={{ duration: 0.16 }}
                                    >
                                        <Field>
                                            <FieldLabel>
                                                Repository URL
                                            </FieldLabel>
                                            <Input
                                                type="text"
                                                data-testid="clone-url"
                                                placeholder="https://git.example.com/owner/repo.git"
                                                value={url}
                                                onChange={(event) =>
                                                    handleChangeUrl(
                                                        event.target.value
                                                    )
                                                }
                                                autoFocus
                                            />
                                        </Field>
                                        <Field>
                                            <FieldLabel>Destination</FieldLabel>
                                            <DestinationInput
                                                actionLabel="Will clone into"
                                                folderPlaceholder="repo"
                                                parentDirectory={
                                                    parentDirectory
                                                }
                                                folderName={folderName}
                                                onChangeParentDirectory={
                                                    setParentDirectory
                                                }
                                                onChangeFolderName={(next) => {
                                                    setNameTouched(true);
                                                    setFolderName(next);
                                                }}
                                            />
                                        </Field>
                                        {lastError ? (
                                            <p
                                                className="text-sm text-destructive"
                                                role="alert"
                                            >
                                                {lastError}
                                            </p>
                                        ) : wasCancelled ? (
                                            <p
                                                className="text-sm text-muted-foreground"
                                                role="status"
                                            >
                                                The previous clone was
                                                cancelled.
                                            </p>
                                        ) : null}
                                    </motion.section>
                                )}
                            </AnimatePresence>
                        </motion.div>
                    </DialogPanel>

                    <DialogFooter>
                        {running ? (
                            <>
                                <Button
                                    variant="outline"
                                    onClick={() => void controller.cancel()}
                                >
                                    Cancel clone
                                </Button>
                            </>
                        ) : (
                            <>
                                <DialogClose
                                    render={<Button variant="ghost" />}
                                >
                                    Cancel
                                </DialogClose>
                                <Button
                                    type="submit"
                                    disabled={!canClone}
                                    variant="info"
                                    data-testid="clone-submit"
                                >
                                    Clone
                                </Button>
                            </>
                        )}
                    </DialogFooter>
                </Form>
            </DialogPopup>
        </Dialog>
    );
}

function repoSlugFromUrl(url: string): string {
    const trimmed = url.trim();
    if (!trimmed) return "";
    try {
        const parsed = new URL(
            trimmed.includes("://") ? trimmed : `https://${trimmed}`
        );
        const segment = parsed.pathname.split("/").filter(Boolean).pop() ?? "";
        return segment.replace(/\.git$/i, "");
    } catch {
        return "";
    }
}

function isLikelyUrl(value: string): boolean {
    if (/^[\w.-]+\/[\w.-]+$/.test(value)) return true; // owner/repo shorthand
    try {
        const parsed = new URL(
            value.includes("://") ? value : `https://${value}`
        );
        return (
            !!parsed.hostname &&
            parsed.pathname.split("/").filter(Boolean).length >= 1
        );
    } catch {
        return false;
    }
}

/**
 * `owner/repo` is shorthand, not a URL: it is expanded against the host of
 * the active connection, since that is the account the clone will use.
 * GitHub stays the fallback for a signed-out or unknown host, because the
 * shorthand is its own convention.
 */
function normalizeUrl(value: string, host: string | undefined): string {
    return /^[\w.-]+\/[\w.-]+$/.test(value)
        ? `https://${host ?? "github.com"}/${value}.git`
        : value;
}
