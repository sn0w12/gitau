import { Markdown, type MarkdownComponents } from "@tanstack/markdown/react";
import {
    Children,
    createContext,
    isValidElement,
    useContext,
    useMemo,
} from "react";

import { useForgeProviders } from "@/hooks/forge/use-forge-account";
import { useRepoImage } from "@/hooks/repositories/use-repository-queries";
import { isTrustedImageSource } from "@/lib/forge/image-hosts";
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

const InsideLinkContext = createContext(false);
const ImageHostsContext = createContext<readonly string[]>([]);

interface LocalImageSource {
    repoId: number;
    basePath: string;
}

const LocalImagesContext = createContext<LocalImageSource | null>(null);

const ABSOLUTE_SRC = /^[a-z][a-z0-9+.-]*:/i;

function isAbsoluteSrc(src: string): boolean {
    return ABSOLUTE_SRC.test(src) || src.startsWith("//");
}

/**
 * Resolves a markdown image source against the repository root, or returns
 * `null` for an absolute source or one that climbs out of the worktree.
 */
function repoImagePath(src: string, basePath: string): string | null {
    let decoded = src;
    try {
        decoded = decodeURIComponent(src);
    } catch {
        decoded = src;
    }
    if (isAbsoluteSrc(decoded)) return null;
    const path = decoded.split(/[?#]/, 1)[0] ?? "";
    const segments = path.startsWith("/")
        ? []
        : basePath.split("/").filter(Boolean);
    for (const part of path.split("/")) {
        if (part === "" || part === ".") continue;
        if (part === "..") {
            if (segments.length === 0) return null;
            segments.pop();
            continue;
        }
        segments.push(part);
    }
    return segments.length > 0 ? segments.join("/") : null;
}

function LocalImage({
    repoId,
    path,
    alt,
    title,
    width,
    height,
}: {
    repoId: number;
    path: string | null;
    alt?: string;
    title?: string;
    width?: string | number;
    height?: string | number;
}) {
    const image = useRepoImage(repoId, path ?? undefined);
    if (!path || !image.data) {
        return alt ? (
            <span className="text-muted-foreground">{alt}</span>
        ) : null;
    }
    return (
        <img
            src={image.data}
            alt={alt ?? ""}
            title={title}
            width={width}
            height={height}
            className="max-w-full rounded-md"
        />
    );
}

const components = {
    a(props) {
        const href = linkHref(props.href);
        const external = /^https?:\/\//i.test(href || "");
        // The heading anchor arrives with its own classes; merge so the
        // hover-reveal styling survives the link color.
        const className = cn("text-info hover:text-info/70", props.className);
        return (
            <InsideLinkContext value={true}>
                {external ? (
                    <ExternalLink
                        {...props}
                        href={href}
                        className={className}
                    />
                ) : (
                    <a {...props} href={href} className={className} />
                )}
            </InsideLinkContext>
        );
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
                    "mt-3 mb-1 border-b pb-1 font-display text-5xl font-extralight first:mt-0",
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
                    "mt-3 mb-1 border-b pb-1 text-xl font-semibold first:mt-0",
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
                    "mt-2 mb-1 text-lg font-semibold first:mt-0",
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
    // GitHub leaves a task list item unmarked so the checkbox reads as the
    // only bullet. The list keeps its gutter for every other item, so a task
    // item cancels that padding and pulls itself back by the same amount.
    // The hidden input `Checkbox` always renders is the signal, which also
    // covers a checkbox the renderer never reached.
    li(props) {
        return (
            <li
                {...props}
                className={cn(
                    "has-[input[type=checkbox]]:list-none",
                    "has-[input[type=checkbox]]:pl-0",
                    "has-[input[type=checkbox]]:ms-[-1.5rem]",
                    props.className
                )}
            />
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
        const title =
            "data-code-title" in props &&
            typeof props["data-code-title"] === "string" &&
            props["data-code-title"].length > 0
                ? props["data-code-title"]
                : "data-filename" in props &&
                    typeof props["data-filename"] === "string" &&
                    props["data-filename"].length > 0
                  ? props["data-filename"]
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
            <CodeBlock
                language={lang}
                text={text}
                title={title}
                className="my-2"
            />
        );
    },
    // The renderer wraps a fence with `file=`/`title=` info in a
    // `<figure><figcaption>`; CodeBlock renders the title in its own header,
    // so the wrapper is dropped rather than shown as loose text above it.
    figure({ children }) {
        return <>{children}</>;
    },
    figcaption() {
        return null;
    },
    code(props) {
        return (
            <code
                {...props}
                className="rounded inline-code-bg [box-decoration-break:clone] px-0.5 font-mono [-webkit-box-decoration-break:clone]"
            />
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
        return <TableHead {...props} className="h-7 whitespace-normal" />;
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
        const insideLink = useContext(InsideLinkContext);
        const imageHosts = useContext(ImageHostsContext);
        const localImages = useContext(LocalImagesContext);
        const src = props.src || "";
        if (isTrustedImageSource(src, imageHosts)) {
            return <img {...props} className="max-w-full rounded-md" />;
        }
        const localPath = localImages
            ? repoImagePath(src, localImages.basePath)
            : null;
        if (localImages && !isAbsoluteSrc(src)) {
            return (
                <LocalImage
                    repoId={localImages.repoId}
                    path={localPath}
                    alt={props.alt}
                    title={props.title}
                    width={props.width}
                    height={props.height}
                />
            );
        }
        const href = linkHref(src);
        if (!href) return null;
        const label = props.alt || src;
        // An anchor cannot contain another anchor, so an image that already
        // sits inside a link degrades to its alt text.
        if (insideLink) return <>{label}</>;
        return (
            <ExternalLink href={href} className="text-info hover:text-info/70">
                {label}
            </ExternalLink>
        );
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
    imageRepoId,
    imageBasePath,
}: {
    children: React.ComponentProps<typeof Markdown>["children"];
    /** Repository the message belongs to; `#123` only links when both are set. */
    owner?: string;
    repo?: string;
    /** Repository whose worktree serves relative `<img>` sources. */
    imageRepoId?: number;
    /** Directory of the rendered document, used to resolve relative images. */
    imageBasePath?: string;
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
    const providers = useForgeProviders();
    // A body's own provider is not known at this layer, and the app trusts
    // every registered provider's image hosts, so the union covers all of
    // them.
    const imageHosts = useMemo(
        () => (providers.data ?? []).flatMap((provider) => provider.imageHosts),
        [providers.data]
    );
    const localImages = useMemo(
        () =>
            imageRepoId === undefined
                ? null
                : { repoId: imageRepoId, basePath: imageBasePath ?? "" },
        [imageRepoId, imageBasePath]
    );
    return (
        <ImageHostsContext value={imageHosts}>
            <LocalImagesContext value={localImages}>
                <div className="min-w-0 break-words">
                    <Markdown
                        extensions={extensions}
                        components={components}
                        // Required so the parser can tell real html from escaped
                        // `\<b>` text. It is safe because `safeHtmlExtension`
                        // rewrites every html node before the renderer sees one,
                        // so the renderer's innerHTML branch stays unreachable.
                        allowHtml
                    >
                        {typeof children === "string"
                            ? normalizeHtmlBlocks(children)
                            : children}
                    </Markdown>
                </div>
            </LocalImagesContext>
        </ImageHostsContext>
    );
}
