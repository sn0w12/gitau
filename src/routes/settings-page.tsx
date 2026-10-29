import { X } from "lucide-react";
import { motion } from "motion/react";
import { useLayoutEffect, useRef, useState } from "react";

import { AttributionsCard } from "@/components/settings/attributions-card";
import { ReleaseInfoCard } from "@/components/settings/release-info-card";
import { Button } from "@/components/ui/button";
import { Frame, FramePanel } from "@/components/ui/frame";
import {
    InputGroup,
    InputGroupAddon,
    InputGroupInput,
} from "@/components/ui/input-group";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsList, TabsPanel, TabsTrigger } from "@/components/ui/tabs";
import {
    useSettingsSchema,
    useSettingsValues,
} from "@/hooks/settings/use-setting";
import { EASE_OUT } from "@/lib/motion";
import { visibleSettings } from "@/lib/settings/schema";
import { searchSettings } from "@/lib/settings/search";

import { SettingsSectionCard } from "../components/settings/settings-section";

const SEARCH_PANEL_ID = "__settings-search";

// Everything renders from the Rust-defined schema. The last tab, About, is
// hardcoded in the frontend and holds attributions and version info.
export function SettingsPage() {
    const [selectedTabId, setSelectedTabId] = useState<string | null>(null);
    const [query, setQuery] = useState("");
    const schema = useSettingsSchema();
    const values = useSettingsValues();
    const contentRef = useRef<HTMLDivElement>(null);
    const [contentHeight, setContentHeight] = useState<number>();

    useLayoutEffect(() => {
        const element = contentRef.current;
        if (!element) return;

        const updateHeight = () => {
            setContentHeight(element.scrollHeight);
        };

        updateHeight();

        const observer = new ResizeObserver(updateHeight);
        observer.observe(element);

        return () => observer.disconnect();
    }, [selectedTabId]);

    if (schema.tabs.length === 0) {
        return (
            <div className="flex h-full items-center justify-center p-6">
                <p className="text-sm text-muted-foreground">
                    No settings available.
                </p>
            </div>
        );
    }

    const aboutTab = { id: "about", label: "About" };
    const tabs = [...schema.tabs, aboutTab];
    const activeTabId = selectedTabId ?? tabs[0].id;
    const searchTerm = query.trim();
    const searching = searchTerm.length > 0;
    const searchResults = searching
        ? searchSettings(schema, values, searchTerm)
        : [];

    return (
        <ScrollArea className="h-full">
            <Tabs
                className="container h-full min-h-0 p-2"
                orientation="vertical"
                value={searching ? SEARCH_PANEL_ID : activeTabId}
                onValueChange={(value) => {
                    if (typeof value !== "string" || value === SEARCH_PANEL_ID)
                        return;
                    setQuery("");
                    setSelectedTabId(value);
                }}
                data-slot="settings-page"
            >
                <div className="sticky top-2 flex h-fit w-44 shrink-0 flex-col gap-1.5">
                    <TabsList
                        className="h-fit w-full p-1"
                        aria-label="Settings"
                    >
                        {tabs.map((tab) => (
                            <TabsTrigger
                                key={tab.id}
                                value={tab.id}
                                className="data-[orientation=vertical]:justify-center"
                            >
                                {tab.label}
                            </TabsTrigger>
                        ))}
                    </TabsList>
                    <InputGroup
                        className="rounded-lg bg-muted p-0.5 text-base font-medium ring-ring/24 transition-shadow placeholder:text-muted-foreground/72 has-focus-visible:ring-[3px] sm:text-sm"
                        unstyled
                    >
                        <InputGroupInput
                            type="search"
                            value={query}
                            onChange={(event) => setQuery(event.target.value)}
                            placeholder="Search settings..."
                            aria-label="Search settings"
                        />
                        <InputGroupAddon align="inline-end">
                            <Button
                                variant="ghost"
                                size="icon-xs"
                                onClick={() => setQuery("")}
                                disabled={query.length === 0}
                                className="opcaity-100 disabled:opacity-0"
                            >
                                <X />
                            </Button>
                        </InputGroupAddon>
                    </InputGroup>
                </div>
                <Frame className="ui-selectable w-full">
                    <motion.div
                        animate={{ height: contentHeight }}
                        transition={{
                            duration: 0.35,
                            ease: EASE_OUT,
                        }}
                        style={{ overflow: "hidden" }}
                    >
                        <div ref={contentRef}>
                            {schema.tabs.map((tab) => (
                                <TabsPanel
                                    key={tab.id}
                                    value={tab.id}
                                    className="min-h-0 space-y-1"
                                >
                                    {tab.sections.map((section) =>
                                        visibleSettings(section).length > 0 ? (
                                            <SettingsSectionCard
                                                key={section.id}
                                                section={section}
                                            />
                                        ) : null
                                    )}
                                </TabsPanel>
                            ))}
                            <TabsPanel
                                value={aboutTab.id}
                                className="min-h-0 space-y-1"
                            >
                                <ReleaseInfoCard />
                                <AttributionsCard />
                            </TabsPanel>
                            {searching && (
                                <TabsPanel
                                    value={SEARCH_PANEL_ID}
                                    className="min-h-0 space-y-1"
                                >
                                    {searchResults.length === 0 ? (
                                        <FramePanel>
                                            <p className="text-sm text-muted-foreground">
                                                {`No settings match "${searchTerm}".`}
                                            </p>
                                        </FramePanel>
                                    ) : (
                                        searchResults.map((group) => (
                                            <SettingsSectionCard
                                                key={`${group.tabId}/${group.section.id}`}
                                                section={group.section}
                                                settings={group.settings}
                                                tabLabel={group.tabLabel}
                                            />
                                        ))
                                    )}
                                </TabsPanel>
                            )}
                        </div>
                    </motion.div>
                </Frame>
            </Tabs>
        </ScrollArea>
    );
}
