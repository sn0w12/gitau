import {
    Bold,
    Code,
    Heading,
    Italic,
    LayoutList,
    Link,
    List,
    ListOrdered,
    TextQuote,
} from "lucide-react";
import { useImperativeHandle, useRef, useState } from "react";

import { Button } from "../ui/button";
import { Frame, FrameHeader, FramePanel } from "../ui/frame";
import { Tabs, TabsContent, TabsList, TabsTab } from "../ui/tabs";
import { Textarea } from "../ui/textarea";
import { Toolbar, ToolbarSeparator } from "../ui/toolbar";
import {
    Tooltip,
    TooltipPopup,
    TooltipProvider,
    TooltipTrigger,
} from "../ui/tooltip";
import { CustomMarkdown } from "./markdown";

/** Wraps the selection with `before`/`after`, restoring the selection
 * around the (possibly placeholder) content. */
function wrapSelection(
    element: HTMLTextAreaElement,
    before: string,
    after: string,
    placeholder: string,
    setValue: (next: string) => void
) {
    const { selectionStart, selectionEnd, value } = element;
    const selected = value.slice(selectionStart, selectionEnd) || placeholder;
    const next =
        value.slice(0, selectionStart) +
        before +
        selected +
        after +
        value.slice(selectionEnd);
    setValue(next);
    const start = selectionStart + before.length;
    const end = start + selected.length;
    requestAnimationFrame(() => {
        element.focus();
        element.setSelectionRange(start, end);
    });
}

/** Prefixes every selected line (or the current line) with `prefix`. */
function prefixLines(
    element: HTMLTextAreaElement,
    prefix: string,
    placeholder: string,
    setValue: (next: string) => void
) {
    const { selectionStart, selectionEnd, value } = element;
    const blockStart = value.lastIndexOf("\n", selectionStart - 1) + 1;
    const blockEnd =
        selectionEnd >= value.length
            ? value.length
            : value.indexOf("\n", selectionEnd);
    const end = blockEnd === -1 ? value.length : blockEnd;
    const block = value.slice(blockStart, end) || placeholder;
    const prefixed = block
        .split("\n")
        .map((line) => `${prefix}${line}`)
        .join("\n");
    const next = value.slice(0, blockStart) + prefixed + value.slice(end);
    setValue(next);
    requestAnimationFrame(() => {
        element.focus();
        element.setSelectionRange(blockStart, blockStart + prefixed.length);
    });
}

export interface IssueInputHandle {
    /** Appends a `> ` quoted block and focuses the composer. */
    insertQuote: (text: string) => void;
}

export function IssueInput({
    pending = false,
    stateLabel,
    onSubmit,
    onToggleState,
    inputRef,
}: {
    pending?: boolean;
    /** Label for the close/reopen button; hides it when absent. */
    stateLabel?: string;
    onSubmit: (body: string) => void;
    onToggleState?: () => void;
    inputRef?: React.Ref<IssueInputHandle>;
}) {
    const [value, setValue] = useState("");
    const areaRef = useRef<HTMLTextAreaElement>(null);

    useImperativeHandle(inputRef, () => ({
        insertQuote: (text: string) => {
            const quoted = text
                .split("\n")
                .map((line) => `> ${line}`)
                .join("\n");
            setValue((prev) =>
                prev.trim().length > 0
                    ? `${prev.trimEnd()}\n\n${quoted}\n`
                    : `${quoted}\n`
            );
            requestAnimationFrame(() => {
                const element = areaRef.current;
                if (element) {
                    element.focus();
                    element.setSelectionRange(
                        element.value.length,
                        element.value.length
                    );
                }
            });
        },
    }));

    const withArea = (apply: (element: HTMLTextAreaElement) => void) => () => {
        const element = areaRef.current;
        if (element) apply(element);
    };
    const canSubmit = value.trim().length > 0 && !pending;

    return (
        <Tabs className="w-full gap-0">
            <Frame>
                <FrameHeader className="flex flex-row justify-between px-2 py-0">
                    <TabsList variant="underline">
                        <TabsTab value="write">Write</TabsTab>
                        <TabsTab value="preview">Preview</TabsTab>
                    </TabsList>
                    <TooltipProvider>
                        <Toolbar className="border-0 bg-transparent">
                            <ToolbarButton
                                label="Heading"
                                icon={<Heading />}
                                onClick={withArea((element) =>
                                    prefixLines(
                                        element,
                                        "### ",
                                        "Heading",
                                        setValue
                                    )
                                )}
                            />
                            <ToolbarButton
                                label="Bold"
                                icon={<Bold />}
                                onClick={withArea((element) =>
                                    wrapSelection(
                                        element,
                                        "**",
                                        "**",
                                        "bold text",
                                        setValue
                                    )
                                )}
                            />
                            <ToolbarButton
                                label="Italic"
                                icon={<Italic />}
                                onClick={withArea((element) =>
                                    wrapSelection(
                                        element,
                                        "*",
                                        "*",
                                        "italic text",
                                        setValue
                                    )
                                )}
                            />
                            <ToolbarButton
                                label="Quote"
                                icon={<TextQuote />}
                                onClick={withArea((element) =>
                                    prefixLines(
                                        element,
                                        "> ",
                                        "Quote",
                                        setValue
                                    )
                                )}
                            />
                            <ToolbarButton
                                label="Code"
                                icon={<Code />}
                                onClick={withArea((element) =>
                                    wrapSelection(
                                        element,
                                        "`",
                                        "`",
                                        "code",
                                        setValue
                                    )
                                )}
                            />
                            <ToolbarButton
                                label="Link"
                                icon={<Link />}
                                onClick={withArea((element) =>
                                    wrapSelection(
                                        element,
                                        "[",
                                        "](url)",
                                        "link text",
                                        setValue
                                    )
                                )}
                            />
                            <ToolbarSeparator />
                            <ToolbarButton
                                label="Unordered List"
                                icon={<List />}
                                onClick={withArea((element) =>
                                    prefixLines(
                                        element,
                                        "- ",
                                        "List item",
                                        setValue
                                    )
                                )}
                            />
                            <ToolbarButton
                                label="Numbered List"
                                icon={<ListOrdered />}
                                onClick={withArea((element) =>
                                    prefixLines(
                                        element,
                                        "1. ",
                                        "List item",
                                        setValue
                                    )
                                )}
                            />
                            <ToolbarButton
                                label="Task List"
                                icon={<LayoutList />}
                                onClick={withArea((element) =>
                                    prefixLines(
                                        element,
                                        "- [ ] ",
                                        "Task",
                                        setValue
                                    )
                                )}
                            />
                        </Toolbar>
                    </TooltipProvider>
                </FrameHeader>
                <TabsContent value="write">
                    <Textarea
                        ref={areaRef}
                        value={value}
                        placeholder="Leave a comment"
                        onChange={(e) => setValue(e.target.value)}
                    />
                </TabsContent>
                <TabsContent value="preview" className="ui-selectable">
                    <FramePanel className="px-3 py-2">
                        {value.trim() ? (
                            <CustomMarkdown>{value}</CustomMarkdown>
                        ) : (
                            <span className="text-muted-foreground">
                                Nothing to preview
                            </span>
                        )}
                    </FramePanel>
                </TabsContent>
            </Frame>
            <div className="flex justify-end gap-1 pt-2">
                {stateLabel && onToggleState ? (
                    <Button
                        variant="outline"
                        disabled={pending}
                        onClick={onToggleState}
                    >
                        {stateLabel}
                    </Button>
                ) : null}
                <Button
                    variant="info"
                    disabled={!canSubmit}
                    loading={pending}
                    onClick={() => {
                        onSubmit(value.trim());
                        setValue("");
                    }}
                >
                    Comment
                </Button>
            </div>
        </Tabs>
    );
}

function ToolbarButton({
    label,
    icon,
    onClick,
}: {
    label: string;
    icon: React.ReactNode;
    onClick?: () => void;
}) {
    return (
        <Tooltip>
            <TooltipTrigger
                render={
                    <Button
                        aria-label={label}
                        size="icon-sm"
                        variant="ghost"
                        onClick={onClick}
                    >
                        {icon}
                    </Button>
                }
            />
            <TooltipPopup sideOffset={8}>{label}</TooltipPopup>
        </Tooltip>
    );
}
