import { FramePanel } from "@/components/ui/frame";
import { Separator } from "@/components/ui/separator";

import { ExternalLink } from "../external-link";
import { TooltipProvider } from "../ui/tooltip";

interface AttributionEntry {
    name: string;
    author: string;
    license: string;
    licenseUrl: string;
    sourceUrl?: string;
    usage: string;
}

const ATTRIBUTIONS: AttributionEntry[] = [
    {
        name: "Cat icons (curled, sitting, stretching)",
        author: "Vectordoodle",
        license: "CC Attribution",
        licenseUrl: "https://creativecommons.org/licenses/by/4.0/legalcode",
        sourceUrl: "https://vectordoodle.gumroad.com/l/FOCLd?ref=svgrepo.com",
        usage: "Empty state illustrations",
    },
];

export function AttributionsCard() {
    return (
        <FramePanel data-slot="settings-attributions">
            <h3 className="pb-1 text-xl font-semibold">Attributions</h3>
            <Separator className="mb-2" />
            <div className="space-y-4">
                <TooltipProvider>
                    {ATTRIBUTIONS.map((entry) => (
                        <div key={entry.name} className="space-y-1 text-sm">
                            <div className="font-medium">{entry.name}</div>
                            <div className="text-muted-foreground">
                                by{" "}
                                {entry.sourceUrl ? (
                                    <ExternalLink href={entry.sourceUrl}>
                                        {entry.author}
                                    </ExternalLink>
                                ) : (
                                    entry.author
                                )}
                            </div>
                            <div className="text-muted-foreground">
                                <ExternalLink href={entry.licenseUrl}>
                                    {entry.license}
                                </ExternalLink>
                                {entry.usage ? ` · ${entry.usage}` : ""}
                            </div>
                        </div>
                    ))}
                </TooltipProvider>
            </div>
        </FramePanel>
    );
}
