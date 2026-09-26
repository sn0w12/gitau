import { Markdown, type MarkdownComponents } from "@tanstack/markdown/react";
import { Children, isValidElement } from "react";

import { ExternalLink } from "../external-link";
import { Checkbox } from "../ui/checkbox";
import { Frame, FramePanel } from "../ui/frame";
import { Separator } from "../ui/separator";
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from "../ui/table";
import { CodeBlock } from "./code-block";

function linkHref(href: string | undefined): string | undefined {
    const hasProtocol = /^[a-z][a-z0-9+.-]*:/i.test(href || "");
    const allowed = !hasProtocol || /^(https?:|mailto:)/i.test(href || "");
    return allowed ? href : undefined;
}

/**
 * Raw fence text out of the renderer's `<code>` element. The element is
 * still unresolved, so its props carry the source string directly.
 * Anything unexpected falls back to rendering the children as-is.
 */
function fenceTextOf(children: React.ReactNode): string | null {
    const elements = Children.toArray(children);
    if (elements.length !== 1) return null;
    const only = elements[0];
    if (!isValidElement(only)) return null;
    const inner = (only.props as { children?: unknown }).children;
    return typeof inner === "string" ? inner : null;
}

const components = {
    a(props) {
        const href = linkHref(props.href);
        const external = /^https?:\/\//i.test(href || "");
        if (external) {
            return (
                <ExternalLink
                    {...props}
                    href={href}
                    className="text-info hover:text-info/70"
                />
            );
        }
        return (
            <a
                {...props}
                href={href}
                className="text-info hover:text-info/70"
            />
        );
    },
    p(props) {
        return <p {...props} className="my-2 first:mt-0 last:mb-0" />;
    },
    h1(props) {
        return (
            <h1
                {...props}
                className="mt-3 mb-1 border-b pb-1 text-lg font-semibold first:mt-0"
            />
        );
    },
    h2(props) {
        return (
            <h2
                {...props}
                className="mt-3 mb-1 border-b pb-1 text-base font-semibold first:mt-0"
            />
        );
    },
    h3(props) {
        return <h3 {...props} className="mt-2 mb-1 font-semibold first:mt-0" />;
    },
    h4(props) {
        return <h4 {...props} className="mt-2 mb-1 font-semibold first:mt-0" />;
    },
    ul(props) {
        return <ul {...props} className="my-2 list-disc space-y-1 pr-0 pl-6" />;
    },
    ol(props) {
        return (
            <ol {...props} className="my-2 list-decimal space-y-1 pr-0 pl-6" />
        );
    },
    blockquote(props) {
        return (
            <blockquote
                {...props}
                className="my-2 border-l-2 border-muted-foreground/40 pl-2 text-muted-foreground"
            />
        );
    },
    pre(props) {
        const { children } = props;
        const lang =
            "data-lang" in props && typeof props["data-lang"] === "string"
                ? props["data-lang"]
                : undefined;
        const text = fenceTextOf(children);
        if (text === null) {
            return (
                <Frame className="my-2">
                    <FramePanel className="overflow-x-auto p-2 font-mono">
                        {children}
                    </FramePanel>
                </Frame>
            );
        }
        return <CodeBlock language={lang} text={text} />;
    },
    code(props) {
        return (
            <code
                {...props}
                className="rounded bg-muted px-1 py-0.5 font-mono"
            />
        );
    },
    table({ children }) {
        return <Table containerClassName="my-2">{children}</Table>;
    },
    thead(props) {
        return <TableHeader {...props} />;
    },
    tbody(props) {
        return <TableBody {...props} />;
    },
    tr(props) {
        return <TableRow {...props} />;
    },
    th(props) {
        return <TableHead {...props} className="whitespace-normal" />;
    },
    td(props) {
        return <TableCell {...props} className="whitespace-normal" />;
    },
    hr() {
        return <Separator className="my-3" />;
    },
    img(props) {
        return <img {...props} className="max-w-full rounded-md" />;
    },
    input(props) {
        if (props.type === "checkbox") {
            return (
                <Checkbox
                    checked={props.checked}
                    disabled
                    className="mr-1 align-middle"
                />
            );
        }
        return <input {...props} />;
    },
} satisfies MarkdownComponents;

export function CustomMarkdown({
    children,
}: {
    children: React.ComponentProps<typeof Markdown>["children"];
}) {
    return (
        <div className="min-w-0 break-words">
            <Markdown components={components}>{children}</Markdown>
        </div>
    );
}
