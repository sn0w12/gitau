import { Markdown, type MarkdownComponents } from "@tanstack/markdown/react";
import { Children, isValidElement, useMemo } from "react";

import { ALERT_TAG, githubMarkdownExtension } from "@/lib/markdown/github";
import {
    DETAILS_TAG,
    normalizeHtmlBlocks,
    safeHtmlExtension,
} from "@/lib/markdown/html";
import { cn } from "@/lib/utils";

import { ExternalLink } from "../external-link";
import { Checkbox } from "../ui/checkbox";
import { Frame, FramePanel } from "../ui/frame";
import { Kbd } from "../ui/kbd";
import { Separator } from "../ui/separator";
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from "../ui/table";
import { MarkdownAlert } from "./alert";
import { CodeBlock } from "./code-block";
import { MarkdownDetails } from "./html";
import { MermaidDiagram } from "./mermaid-diagram";

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

function isMermaid(lang: string | undefined): boolean {
    return (lang ?? "").trim().toLowerCase() === "mermaid";
}

const components = {
    a(props) {
        const href = linkHref(props.href);
        const external = /^https?:\/\//i.test(href || "");
        // The heading anchor arrives with its own classes; merge so the
        // hover-reveal styling survives the link color.
        const className = cn("text-info hover:text-info/70", props.className);
        if (external) {
            return (
                <ExternalLink {...props} href={href} className={className} />
            );
        }
        return <a {...props} href={href} className={className} />;
    },
    p(props) {
        return <p {...props} className="my-2 first:mt-0 last:mb-0" />;
    },
    // Headings merge the renderer's className: the footnotes heading ships
    // `sr-only`, which replacing the class list would un-hide.
    h1(props) {
        return (
            <h1
                {...props}
                className={cn(
                    "mt-3 mb-1 border-b pb-1 text-lg font-semibold first:mt-0",
                    props.className
                )}
            />
        );
    },
    h2(props) {
        return (
            <h2
                {...props}
                className={cn(
                    "mt-3 mb-1 border-b pb-1 text-base font-semibold first:mt-0",
                    props.className
                )}
            />
        );
    },
    h3(props) {
        return (
            <h3
                {...props}
                className={cn(
                    "mt-2 mb-1 font-semibold first:mt-0",
                    props.className
                )}
            />
        );
    },
    h4(props) {
        return (
            <h4
                {...props}
                className={cn(
                    "mt-2 mb-1 font-semibold first:mt-0",
                    props.className
                )}
            />
        );
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
        return isMermaid(lang) ? (
            <MermaidDiagram text={text} />
        ) : (
            <CodeBlock language={lang} text={text} />
        );
    },
    code(props) {
        return (
            <code {...props} className="rounded bg-muted px-0.5 font-mono" />
        );
    },
    table({ children }) {
        return (
            <Frame className="my-2 w-full">
                <Table variant="card">{children}</Table>
            </Frame>
        );
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
    /** Tags that only exist because a markdown body carried HTML. The rest
     * resolve to real elements with no mapping, which React escapes. */
    [DETAILS_TAG]: MarkdownDetails,
    // The renderer hands every component node an empty child list, and React
    // rejects children on an intrinsic void element outright, so a void tag
    // needs a wrapper that drops them.
    br({ children: _voidChildren, ...props }) {
        return <br {...props} />;
    },
    wbr({ children: _voidChildren, ...props }) {
        return <wbr {...props} />;
    },
    kbd(props) {
        return <Kbd {...props} />;
    },
    sub(props) {
        return <sub {...props} className="text-xs" />;
    },
    sup(props) {
        return <sup {...props} className="text-xs" />;
    },
    mark(props) {
        return <mark {...props} className="rounded-sm bg-warning/24 px-0.5" />;
    },
    small(props) {
        return <small {...props} className="text-muted-foreground" />;
    },
    s(props) {
        return <s {...props} />;
    },
    del(props) {
        return <del {...props} />;
    },
    ins(props) {
        return <ins {...props} className="no-underline" />;
    },
    img({ children: _voidChildren, ...props }) {
        const src = props.src || "";
        const trusted =
            /^https:\/\/(github\.com|githubusercontent\.com|githubassets\.com)\//i.test(
                src
            );
        if (!trusted) {
            const href = linkHref(src);
            if (!href) return null;
            return (
                <ExternalLink
                    href={href}
                    className="text-info hover:text-info/70"
                >
                    {props.alt || src}
                </ExternalLink>
            );
        }
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
    [ALERT_TAG]: MarkdownAlert,
} satisfies MarkdownComponents;

export function CustomMarkdown({
    children,
    owner,
    repo,
}: {
    children: React.ComponentProps<typeof Markdown>["children"];
    /** Repository the message belongs to; `#123` only links when both are set. */
    owner?: string;
    repo?: string;
}) {
    const extensions = useMemo(
        () => [
            // Sanitizing runs first so the GitHub text rules see the tags as
            // plain text they can autolink around, rather than as nodes.
            safeHtmlExtension(),
            githubMarkdownExtension(
                owner && repo ? { owner, repo } : undefined
            ),
        ],
        [owner, repo]
    );
    return (
        <div className="min-w-0 break-words">
            <Markdown
                extensions={extensions}
                components={components}
                // Required so the parser can tell real html from escaped
                // `\<b>` text. It is safe because `safeHtmlExtension`
                // rewrites every html node before the renderer sees one, so
                // the renderer's innerHTML branch stays unreachable.
                allowHtml
            >
                {typeof children === "string"
                    ? normalizeHtmlBlocks(children)
                    : children}
            </Markdown>
        </div>
    );
}
