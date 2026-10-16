import Scritto from "@scritto/react";
import { useHotkeyRecorder } from "@tanstack/react-hotkeys";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { Fragment } from "react";

import { AnimatedWidth } from "@/components/ui/animated-width";
import { BlurCrossfade } from "@/components/ui/blur-crossfade";
import { Button } from "@/components/ui/button";
import {
    InsetInput,
    InsetInputAddition,
    InsetInputValue,
} from "@/components/ui/inset-input";
import { KbdGroup } from "@/components/ui/kbd";
import { useHeldChord } from "@/hooks/use-held-chord";
import { displayToken, splitChord } from "@/lib/shortcuts";

type Preview = { key: string; node: ReactNode };

function Chord({ chord }: { chord: string }) {
    const parts = splitChord(chord);

    if (parts.length === 0) return null;

    return (
        <KbdGroup className="h-full items-center">
            {parts.map((part, index) => (
                <Fragment key={`${part}-${index}`}>
                    <span>{displayToken(part)}</span>
                    {index + 1 !== parts.length && <span>+</span>}
                </Fragment>
            ))}
        </KbdGroup>
    );
}

function Placeholder({ children }: { children: ReactNode }) {
    return (
        <span className="font-mono text-nowrap text-muted-foreground">
            {children}
        </span>
    );
}

function buildPreview(
    recording: boolean,
    heldChord: string,
    value: string
): Preview {
    if (recording) {
        if (heldChord) {
            return {
                key: `record:${heldChord}`,
                node: <Chord chord={heldChord} />,
            };
        }
        return {
            key: "record:none",
            node: <Placeholder>Press keys…</Placeholder>,
        };
    }
    if (value) {
        return { key: `value:${value}`, node: <Chord chord={value} /> };
    }
    return { key: "value:none", node: <Placeholder>Not set</Placeholder> };
}

/**
 * Shortcut editor backed by TanStack Hotkey's recorder: click edit, press a
 * chord, done. Backspace/Delete while recording clears the binding.
 */
export function ShortcutControl({
    value,
    onChange,
}: {
    value: string;
    onChange: (value: string) => void;
}) {
    const recorder = useHotkeyRecorder({
        onRecord: (hotkey) => {
            onChange(typeof hotkey === "string" ? hotkey.trim() : "");
        },
    });
    const heldChord = useHeldChord(recorder.isRecording);
    const preview = buildPreview(recorder.isRecording, heldChord, value);

    return (
        <InsetInput className="cursor-default">
            <InsetInputAddition align="inline-start" className="cursor-default">
                <Button
                    variant="ghost"
                    size="sm"
                    className="px-1"
                    onClick={() => {
                        if (recorder.isRecording) {
                            recorder.cancelRecording();
                            return;
                        }

                        recorder.startRecording();
                    }}
                >
                    <Scritto value={recorder.isRecording ? "Cancel" : "Edit"} />
                </Button>
            </InsetInputAddition>
            <InsetInputValue className="cursor-text px-2">
                <AnimatedWidth>
                    <BlurCrossfade valueKey={preview.key}>
                        {preview.node}
                    </BlurCrossfade>
                </AnimatedWidth>
            </InsetInputValue>
            <InsetInputAddition
                align="inline-end"
                className="-mr-2 cursor-default"
            >
                <Button
                    variant="destructive"
                    size="icon-sm"
                    aria-label="Clear shortcut"
                    onClick={() => onChange("")}
                    render={
                        <span role="button">
                            <X />
                        </span>
                    }
                />
            </InsetInputAddition>
        </InsetInput>
    );
}
