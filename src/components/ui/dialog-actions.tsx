import { CornerDownLeft } from "lucide-react";
import type React from "react";
import type { ReactNode } from "react";

import { Button } from "@/components/ui/button";
import { DialogClose, DialogFooter } from "@/components/ui/dialog";
import { Form } from "@/components/ui/form";
import { Kbd } from "@/components/ui/kbd";

export function DialogForm({
    onSubmit,
    children,
}: {
    onSubmit: () => void;
    children: React.ReactNode;
}): React.ReactElement {
    return (
        <Form
            className="contents"
            onSubmit={(event) => {
                event.preventDefault();
                onSubmit();
            }}
        >
            {children}
        </Form>
    );
}

/**
 * Standard footer actions. Without `onSubmit` the submit button relies on an
 * ancestor `DialogForm` for submission, so form dialogs can put it inside the
 * same form as their fields.
 */
export function DialogActions({
    cancelLabel = "Cancel",
    cancelDisabled,
    onCancel,
    submitLabel,
    submitVariant = "info",
    submitDisabled,
    submitLoading,
    onSubmit,
}: {
    cancelLabel?: ReactNode;
    cancelDisabled?: boolean;
    onCancel?: () => void;
    submitLabel?: ReactNode;
    submitVariant?: "info" | "destructive";
    submitDisabled?: boolean;
    submitLoading?: boolean;
    onSubmit?: () => void;
}): React.ReactElement {
    const footer = (
        <DialogFooter>
            {onCancel ? (
                <Button
                    type="button"
                    variant="secondary"
                    onClick={onCancel}
                    disabled={cancelDisabled}
                >
                    {cancelLabel} <Kbd>Esc</Kbd>
                </Button>
            ) : (
                <DialogClose render={<Button variant="secondary" />}>
                    {cancelLabel} <Kbd>Esc</Kbd>
                </DialogClose>
            )}
            {submitLabel ? (
                <Button
                    type="submit"
                    variant={submitVariant}
                    disabled={submitDisabled}
                    loading={submitLoading}
                >
                    {submitLabel}{" "}
                    <Kbd className="text-dark">
                        <CornerDownLeft />
                    </Kbd>
                </Button>
            ) : null}
        </DialogFooter>
    );

    return onSubmit ? (
        <DialogForm onSubmit={onSubmit}>{footer}</DialogForm>
    ) : (
        footer
    );
}
