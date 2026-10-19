import type { SettingsSection } from "@/lib/backend/protocol";
import {
    visibleSettings,
    type VisibleSettingDefinition,
} from "@/lib/settings/schema";

import { FramePanel } from "../ui/frame";
import { Separator } from "../ui/separator";
import { SettingField } from "./setting-field";

export function SettingsSectionCard({
    section,
    settings = visibleSettings(section),
    tabLabel,
}: {
    section: SettingsSection;
    settings?: VisibleSettingDefinition[];
    tabLabel?: string;
}) {
    if (settings.length === 0) return null;

    return (
        <FramePanel>
            {tabLabel && (
                <p className="pb-1 text-xs text-muted-foreground">{tabLabel}</p>
            )}
            <h3 className="pb-1 text-xl font-semibold">{section.title}</h3>
            <Separator className="mb-2" />
            <div className="space-y-4">
                {settings.map((definition) => (
                    <SettingField
                        key={definition.key}
                        definition={definition}
                    />
                ))}
            </div>
        </FramePanel>
    );
}
