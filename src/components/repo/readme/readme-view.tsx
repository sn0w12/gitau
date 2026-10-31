import { FileText } from "lucide-react";

import { CustomMarkdown } from "@/components/forge/markdown";
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
} from "@/components/ui/empty";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { useRepoReadme } from "@/hooks/repositories/use-repository-queries";

export function ReadmeView({ repoId }: { repoId: number }) {
    const readme = useRepoReadme(repoId);

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

    return (
        <ScrollArea
            className="min-h-0 flex-1"
            scrollFade
            overscrollContain
            scrollbarGutter
        >
            <article className="mx-auto w-full max-w-3xl px-6 py-8">
                <CustomMarkdown>{readme.data.content}</CustomMarkdown>
            </article>
        </ScrollArea>
    );
}
