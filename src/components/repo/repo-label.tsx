import { cn } from "@/lib/utils";

export function getRepoInfo(repo: string) {
    const slash = repo.indexOf("/");
    if (slash === -1) return { owner: null, title: repo };

    return {
        owner: repo.slice(0, slash),
        title: repo.slice(slash + 1),
    };
}

export function RepoLabel({
    repo,
    className,
}: {
    repo: string;
    className?: string;
}) {
    const info = getRepoInfo(repo);
    if (!info.title) return null;

    return (
        <div className={cn("flex gap-0.5", className)}>
            {info.owner && (
                <>
                    <span className="text-muted-foreground">{info.owner}</span>
                    <span className="text-muted-foreground">/</span>
                </>
            )}
            <span>{info.title}</span>
        </div>
    );
}
