import {
    Ellipsis,
    Link,
    Pen,
    SquareMinus,
    TextQuote,
    Trash,
} from "lucide-react";
import { useState } from "react";

import { toastManager } from "@/components/ui/toast";
import { useConfirm } from "@/contexts/confirm-context";
import { toastError } from "@/lib/toast-error";
import { formatRelativeDate } from "@/lib/utils";

import { Avatar, AvatarFallback, AvatarImage } from "../ui/avatar";
import { Button } from "../ui/button";
import { Frame, FrameHeader, FramePanel } from "../ui/frame";
import {
    Menu,
    MenuGroup,
    MenuItem,
    MenuPopup,
    MenuSeparator,
    MenuTrigger,
} from "../ui/menu";
import { Textarea } from "../ui/textarea";
import { CustomMarkdown } from "./markdown";

async function copyText(text: string, title: string) {
    try {
        await navigator.clipboard.writeText(text);
        toastManager.add({ title, type: "success" });
    } catch (error) {
        toastError("Could not copy", error);
    }
}

interface MessageMenuState {
    link?: string;
    text: string;
    canQuote: boolean;
    onQuote?: (text: string) => void;
    showEdit: boolean;
    onEdit: () => void;
    showDelete: boolean;
    deleting: boolean;
    onDelete: () => void;
}

function MessageMenu({
    link,
    text,
    canQuote,
    onQuote,
    showEdit,
    onEdit,
    showDelete,
    deleting,
    onDelete,
}: MessageMenuState) {
    return (
        <Menu>
            <MenuTrigger
                render={
                    <Button variant="ghost" size="icon-sm">
                        <Ellipsis />
                    </Button>
                }
            />
            <MenuPopup>
                <MenuGroup>
                    {link !== undefined ? (
                        <MenuItem
                            onClick={() => void copyText(link, "Copied link")}
                        >
                            <Link />
                            Copy Link
                        </MenuItem>
                    ) : null}
                    <MenuItem
                        onClick={() => void copyText(text, "Copied markdown")}
                    >
                        <SquareMinus />
                        Copy Markdown
                    </MenuItem>
                    {canQuote && onQuote !== undefined ? (
                        <MenuItem onClick={() => onQuote(text)}>
                            <TextQuote />
                            Quote Reply
                        </MenuItem>
                    ) : null}
                </MenuGroup>
                {showEdit || showDelete ? (
                    <>
                        <MenuSeparator />
                        <MenuGroup>
                            {showEdit ? (
                                <MenuItem onClick={onEdit}>
                                    <Pen />
                                    Edit
                                </MenuItem>
                            ) : null}
                            {showDelete ? (
                                <MenuItem
                                    variant="destructive"
                                    disabled={deleting}
                                    onClick={() => void onDelete()}
                                >
                                    <Trash />
                                    Delete
                                </MenuItem>
                            ) : null}
                        </MenuGroup>
                    </>
                ) : null}
            </MenuPopup>
        </Menu>
    );
}

function MessageEditor({
    draft,
    onDraft,
    saving,
    onCancel,
    onSave,
}: {
    draft: string;
    onDraft: (next: string) => void;
    saving: boolean;
    onCancel: () => void;
    onSave: () => void;
}) {
    return (
        <div className="flex flex-col gap-2">
            <Textarea value={draft} onChange={(e) => onDraft(e.target.value)} />
            <div className="flex justify-end gap-1">
                <Button
                    variant="outline"
                    size="sm"
                    disabled={saving}
                    onClick={onCancel}
                >
                    Cancel
                </Button>
                <Button
                    size="sm"
                    disabled={draft.trim().length === 0 || saving}
                    loading={saving}
                    onClick={onSave}
                >
                    Save
                </Button>
            </div>
        </div>
    );
}

export function IssueMessage({
    text,
    author,
    avatarUrl,
    createdAt,
    actionLabel = "opened on",
    link,
    canQuote = false,
    onQuote,
    canEdit = false,
    onSave,
    canDelete = false,
    onDelete,
    deleteTitle = "Delete this comment?",
    deleteDescription = "The comment is removed from the issue. This cannot be undone.",
}: {
    text: string;
    author?: string;
    avatarUrl?: string;
    createdAt?: string;
    actionLabel?: string;
    /** Web URL of the message; hides Copy Link when absent. */
    link?: string;
    canQuote?: boolean;
    onQuote?: (text: string) => void;
    /** Edit/Delete reflect authorship and repo push access. */
    canEdit?: boolean;
    onSave?: (body: string) => Promise<void>;
    canDelete?: boolean;
    onDelete?: () => Promise<void>;
    deleteTitle?: string;
    deleteDescription?: string;
}) {
    const initial = (author ?? "?").slice(0, 1).toUpperCase();
    const [editing, setEditing] = useState(false);
    const [draft, setDraft] = useState(text);
    const [saving, setSaving] = useState(false);
    const [deleting, setDeleting] = useState(false);
    const { confirm } = useConfirm();

    const showEdit = canEdit && onSave !== undefined;
    const showDelete = canDelete && onDelete !== undefined;
    const showMenu =
        link !== undefined ||
        (canQuote && onQuote !== undefined) ||
        showEdit ||
        showDelete;

    const startEdit = () => {
        setDraft(text);
        setEditing(true);
    };

    const saveEdit = async () => {
        if (draft.trim().length === 0 || onSave === undefined) return;
        setSaving(true);
        try {
            await onSave(draft.trim());
            setEditing(false);
        } catch (error) {
            toastError("Could not save", error);
        } finally {
            setSaving(false);
        }
    };

    const deleteMessage = async () => {
        if (onDelete === undefined) return;
        const result = await confirm({
            title: deleteTitle,
            description: deleteDescription,
            confirmText: "Delete",
            variant: "destructive",
        });
        if (!result.confirmed) return;
        setDeleting(true);
        try {
            await onDelete();
        } catch (error) {
            toastError("Could not delete", error);
        } finally {
            setDeleting(false);
        }
    };

    return (
        <Frame className="ui-selectable text-sm">
            <FrameHeader className="flex flex-row items-center justify-between px-2 py-1.5">
                <div className="flex items-center gap-1">
                    <Avatar className="size-6">
                        <AvatarImage src={avatarUrl} />
                        <AvatarFallback>{initial}</AvatarFallback>
                    </Avatar>
                    <span>
                        {author ?? "Someone"}{" "}
                        {createdAt ? (
                            <span className="text-muted-foreground">
                                {actionLabel} {formatRelativeDate(createdAt)}
                            </span>
                        ) : null}
                    </span>
                </div>
                {showMenu ? (
                    <MessageMenu
                        link={link}
                        text={text}
                        canQuote={canQuote}
                        onQuote={onQuote}
                        showEdit={showEdit}
                        onEdit={startEdit}
                        showDelete={showDelete}
                        deleting={deleting}
                        onDelete={deleteMessage}
                    />
                ) : null}
            </FrameHeader>
            <FramePanel className="px-3 py-2">
                {editing ? (
                    <MessageEditor
                        draft={draft}
                        onDraft={setDraft}
                        saving={saving}
                        onCancel={() => setEditing(false)}
                        onSave={() => void saveEdit()}
                    />
                ) : (
                    <CustomMarkdown>{text}</CustomMarkdown>
                )}
            </FramePanel>
        </Frame>
    );
}

export function MessageSpacer() {
    return <div className="ml-4 h-2 w-0.5 bg-muted" />;
}
