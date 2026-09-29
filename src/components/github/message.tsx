import {
    Ellipsis,
    Link,
    Pen,
    SquareMinus,
    TextQuote,
    Trash,
} from "lucide-react";
import { useState } from "react";

import { useConfirm } from "@/contexts/confirm-context";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";
import { toastError } from "@/lib/toast-error";
import { formatRelativeDate, cn } from "@/lib/utils";

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
    const copy = useCopyToClipboard();
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
                        <MenuItem onClick={() => copy.copyToClipboard(link)}>
                            <Link />
                            Copy Link
                        </MenuItem>
                    ) : null}
                    <MenuItem onClick={() => copy.copyToClipboard(text)}>
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

const ABSOLUTE_MENU_CLASS =
    "absolute end-2.5 top-2.5 z-10 opacity-0 transition-opacity focus-within:opacity-100 group-hover/message:opacity-100";

/** The avatar, name, and timestamp row that opens a message. */
function MessageIdentity({
    author,
    avatarUrl,
    initial,
    createdAt,
    actionLabel,
    menu,
}: {
    author?: string;
    avatarUrl?: string;
    initial: string;
    createdAt?: string;
    actionLabel: string;
    menu: React.ReactNode;
}) {
    return (
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
            {menu}
        </FrameHeader>
    );
}

export function TimelineMessage({
    text,
    author,
    avatarUrl,
    createdAt,
    actionLabel = "opened on",
    link,
    owner,
    repo,
    canQuote = false,
    onQuote,
    canEdit = false,
    onSave,
    canDelete = false,
    onDelete,
    deleteTitle = "Delete this comment?",
    deleteDescription = "The comment is removed from the thread. This cannot be undone.",
    showAuthor = true,
}: {
    text: string;
    author?: string;
    avatarUrl?: string;
    createdAt?: string;
    actionLabel?: string;
    /** False when the caller already renders its own identity row, such as a
     * review comment carrying a path and line. */
    showAuthor?: boolean;
    /** Web URL of the message; hides Copy Link when absent. */
    link?: string;
    /** Repository of the thread, so `#123` references link out. */
    owner?: string;
    repo?: string;
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

    const menu = showMenu ? (
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
    ) : null;

    return (
        <Frame
            className={cn(
                "ui-selectable text-sm",
                // With no identity row the menu has no header to sit in, so
                // it floats over the body and appears on hover.
                !showAuthor && "group/message relative"
            )}
        >
            {showAuthor ? (
                <MessageIdentity
                    author={author}
                    avatarUrl={avatarUrl}
                    initial={initial}
                    createdAt={createdAt}
                    actionLabel={actionLabel}
                    menu={menu}
                />
            ) : (
                <div className={ABSOLUTE_MENU_CLASS}>{menu}</div>
            )}
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
                    <CustomMarkdown owner={owner} repo={repo}>
                        {text}
                    </CustomMarkdown>
                )}
            </FramePanel>
        </Frame>
    );
}

export function MessageSpacer({ className }: { className?: string }) {
    return <div className={cn("ml-4.5 h-2 w-0.5 bg-muted", className)} />;
}
