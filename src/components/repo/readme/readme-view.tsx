import { ChevronLeft, FileText } from "lucide-react";
import {
    type ReactNode,
    useCallback,
    useEffect,
    useRef,
    useState,
} from "react";

import { CustomMarkdown } from "@/components/forge/markdown";
import { Button } from "@/components/ui/button";
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
} from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import {
    useReadmeText,
    useRepoReadme,
} from "@/hooks/repositories/use-repository-queries";

const MARKDOWN_EXTENSION = /\.(md|markdown|mdown|mkd|mkdn)$/i;

function isMarkdownPath(path: string): boolean {
    return MARKDOWN_EXTENSION.test(path);
}

function directoryOf(path: string): string {
    return path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
}

function ReadmeFrame({
    linked,
    onBack,
    children,
}: {
    linked: boolean;
    onBack: () => void;
    children: ReactNode;
}) {
    return (
        <div className="relative flex min-h-0 flex-1 flex-col">
            {linked && (
                <Button
                    aria-label="Back"
                    variant="outline"
                    size="icon-sm"
                    onClick={onBack}
                    className="absolute start-1.25 top-1.25 z-10"
                >
                    <ChevronLeft />
                </Button>
            )}
            {children}
        </div>
    );
}

function ReadmeMessage({
    title,
    description,
}: {
    title: string;
    description: string;
}) {
    return (
        <Empty className="min-h-0 flex-1">
            <EmptyHeader>
                <EmptyMedia variant="icon">
                    <FileText />
                </EmptyMedia>
                <EmptyTitle>{title}</EmptyTitle>
                <EmptyDescription>{description}</EmptyDescription>
            </EmptyHeader>
        </Empty>
    );
}

export function ReadmeView({ repoId }: { repoId: number }) {
    const readme = useRepoReadme(repoId);
    const [trail, setTrail] = useState<string[]>([]);
    const anchorRef = useRef<string | null>(null);
    const viewportRef = useRef<HTMLDivElement>(null);

    const rootPath = readme.data?.path ?? "";
    const currentPath = trail.length > 0 ? trail[trail.length - 1] : rootPath;
    const linked = trail.length > 0;
    const linkedMarkdown = linked && isMarkdownPath(currentPath);
    const linkedText = useReadmeText(
        repoId,
        linkedMarkdown ? currentPath : undefined
    );

    const openRelative = useCallback(
        (path: string, fragment: string | undefined) => {
            if (path === currentPath) {
                if (fragment) {
                    document
                        .getElementById(fragment)
                        ?.scrollIntoView?.({ block: "start" });
                } else {
                    viewportRef.current?.scrollTo?.({ top: 0 });
                }
                return;
            }
            setTrail((current) => {
                if (path === rootPath) return [];
                const existing = current.indexOf(path);
                if (existing !== -1) return current.slice(0, existing + 1);
                return [...current, path];
            });
            anchorRef.current = fragment ?? null;
        },
        [rootPath, currentPath]
    );

    const goBack = useCallback(() => {
        setTrail((current) => current.slice(0, -1));
        anchorRef.current = null;
    }, []);

    useEffect(() => {
        viewportRef.current?.scrollTo?.({ top: 0 });
    }, [currentPath]);

    useEffect(() => {
        const anchor = anchorRef.current;
        if (!anchor) return;
        if (linkedMarkdown && linkedText.isPending) return;
        anchorRef.current = null;
        document.getElementById(anchor)?.scrollIntoView?.({ block: "start" });
    }, [currentPath, linkedMarkdown, linkedText.isPending, linkedText.data]);

    if (readme.isPending) {
        return (
            <div className="flex min-h-0 flex-1 items-center justify-center">
                <Spinner className="size-6 text-muted-foreground" />
            </div>
        );
    }

    if (readme.isError) {
        return (
            <Empty className="min-h-0 flex-1">
                <EmptyHeader>
                    <EmptyTitle>Could not read the README</EmptyTitle>
                    <EmptyDescription>
                        The repository could not be read.
                    </EmptyDescription>
                </EmptyHeader>
            </Empty>
        );
    }

    if (!readme.data) {
        return (
            <Empty className="min-h-0 flex-1">
                <EmptyHeader>
                    <EmptyMedia variant="icon">
                        <FileText />
                    </EmptyMedia>
                    <EmptyTitle>No README</EmptyTitle>
                    <EmptyDescription>
                        Add a README.md at the repository root to see it here.
                    </EmptyDescription>
                </EmptyHeader>
            </Empty>
        );
    }

    if (linked && !linkedMarkdown) {
        return (
            <ReadmeFrame linked onBack={goBack}>
                <ReadmeMessage
                    title="Cannot preview this file"
                    description="Only Markdown documents open from the README."
                />
            </ReadmeFrame>
        );
    }

    if (linkedMarkdown && linkedText.isPending) {
        return (
            <ReadmeFrame linked onBack={goBack}>
                <div className="flex min-h-0 flex-1 items-center justify-center">
                    <Spinner className="size-6 text-muted-foreground" />
                </div>
            </ReadmeFrame>
        );
    }

    if (
        linkedMarkdown &&
        (linkedText.isError ||
            linkedText.data === null ||
            linkedText.data === undefined)
    ) {
        return (
            <ReadmeFrame linked onBack={goBack}>
                <ReadmeMessage
                    title="Cannot read this document"
                    description="The file is missing, too large, or not UTF-8 text."
                />
            </ReadmeFrame>
        );
    }

    const content = linkedMarkdown
        ? (linkedText.data ?? "")
        : readme.data.content;

    return (
        <ReadmeFrame linked={linked} onBack={goBack}>
            <ScrollArea
                className="min-h-0 flex-1"
                viewportRef={viewportRef}
                scrollFade
                overscrollContain
                scrollbarGutter
            >
                <article className="ui-selectable mx-auto w-full max-w-3xl px-6 py-8">
                    <CustomMarkdown
                        imageRepoId={repoId}
                        imageBasePath={directoryOf(currentPath)}
                        onOpenRelative={openRelative}
                    >
                        {content}
                    </CustomMarkdown>
                </article>
            </ScrollArea>
        </ReadmeFrame>
    );
}
