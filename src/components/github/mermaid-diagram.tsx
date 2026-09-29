import { useStore } from "@tanstack/react-store";
import { TriangleAlert } from "lucide-react";
import mermaid from "mermaid";
import { useEffect, useId, useState } from "react";

import { CodeBlock } from "@/components/github/code-block";
import { Frame, FrameHeader, FramePanel } from "@/components/ui/frame";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { themeStore } from "@/stores/theme-store";

/** Mermaid's font has to be a real family name: it writes this straight into
 * the generated stylesheet, where a `var()` would not resolve. */
const FONT = '"Inter Variable", sans-serif';

/** Resolved values of the app's own design tokens.
 *
 * These are literals rather than reads of the CSS custom properties, because
 * `getComputedStyle` hands back the unresolved token stream for a custom
 * property: `--muted` resolves to `--alpha(var(--color-black) / 4%)`, which
 * only becomes a colour inside a `color:` declaration. Mermaid writes these
 * into svg attributes from script, where nothing would resolve them, and it
 * cannot take a `var()` reference either since its colour derivation needs a
 * concrete value.
 *
 * The values are the computed results of the `--*` tokens in `styles.css` for
 * the light and dark blocks, with the surfaces composited over the background
 * they sit on. */
const LIGHT: Record<string, string> = {
    background: "#ffffff",
    // Nodes sit on a card, so the fill is the muted overlay composited on
    // white: black at 4% over #ffffff.
    primaryColor: "#f5f5f5",
    primaryTextColor: "#262626",
    primaryBorderColor: "#e5e5e5",
    secondaryColor: "#fafafa",
    tertiaryColor: "#f5f5f5",
    lineColor: "#737373",
    textColor: "#262626",
    mainBkg: "#f5f5f5",
    nodeBorder: "#e5e5e5",
    clusterBkg: "#fafafa",
    clusterBorder: "#e5e5e5",
    edgeLabelBackground: "#ffffff",
    titleColor: "#171717",
    noteBkgColor: "#f5f5f5",
    noteTextColor: "#262626",
    noteBorderColor: "#e5e5e5",
    actorBkg: "#f5f5f5",
    actorBorder: "#e5e5e5",
    actorTextColor: "#262626",
    actorLineColor: "#d4d4d4",
    signalColor: "#737373",
    signalTextColor: "#262626",
    labelBoxBkgColor: "#f5f5f5",
    labelBoxBorderColor: "#e5e5e5",
    labelTextColor: "#262626",
    loopTextColor: "#262626",
    activationBkgColor: "#ebebeb",
    activationBorderColor: "#d4d4d4",
    sequenceNumberColor: "#a3a3a3",
    fontFamily: FONT,
    fontSize: "13px",
};

const DARK: Record<string, string> = {
    background: "#0a0a0a",
    // In dark the muted overlay is white at 4% over the background.
    primaryColor: "#1a1a1a",
    primaryTextColor: "#f5f5f5",
    primaryBorderColor: "#262626",
    secondaryColor: "#171717",
    tertiaryColor: "#1a1a1a",
    lineColor: "#a3a3a3",
    textColor: "#f5f5f5",
    mainBkg: "#1a1a1a",
    nodeBorder: "#262626",
    clusterBkg: "#171717",
    clusterBorder: "#262626",
    edgeLabelBackground: "#0a0a0a",
    titleColor: "#fafafa",
    noteBkgColor: "#1a1a1a",
    noteTextColor: "#f5f5f5",
    noteBorderColor: "#262626",
    actorBkg: "#1a1a1a",
    actorBorder: "#262626",
    actorTextColor: "#f5f5f5",
    actorLineColor: "#333333",
    signalColor: "#a3a3a3",
    signalTextColor: "#f5f5f5",
    labelBoxBkgColor: "#1a1a1a",
    labelBoxBorderColor: "#262626",
    labelTextColor: "#f5f5f5",
    loopTextColor: "#f5f5f5",
    activationBkgColor: "#262626",
    activationBorderColor: "#333333",
    sequenceNumberColor: "#737373",
    fontFamily: FONT,
    fontSize: "13px",
};

/** Mermaid paints colours onto svg elements as presentation attributes, which
 * the class-based rules in its own generated stylesheet cannot override, so a
 * themed diagram keeps the palette it was drawn with and ignores the theme.
 * Dropping those attributes lets the stylesheet drive every colour.
 *
 * `fill="none"` and `fill="url(#id)"` are kept: the first is how an open shape
 * stays open, the second is a gradient reference. The generated stylesheet
 * uses `fill:` with a colon, so it is never touched.
 */
function stripPaintAttributes(svg: string): string {
    return svg.replace(
        /\s(?:fill|stroke)="([^"]*)"/g,
        (match, value: string) => {
            if (value === "none" || value.startsWith("url(")) return match;
            return "";
        }
    );
}

function describe(cause: unknown): string {
    if (cause instanceof Error) {
        return cause.message.split("\n")[0] ?? cause.name;
    }
    return String(cause);
}

/**
 * A mermaid fence rendered as a diagram. The source is a fallback rather than
 * an error state: a diagram that will not parse is still worth reading as
 * text, and it stays copyable.
 */
export function MermaidDiagram({ text }: { text: string }) {
    const dark = useStore(themeStore, (state) => state.resolved === "dark");
    const [svg, setSvg] = useState<string | null>(null);
    const [error, setError] = useState<string | null>(null);
    const diagramId = useId();
    // Mermaid renders by id, so each diagram needs a stable, collision-free one.
    const renderId = `mermaid${diagramId.replaceAll(":", "")}`;

    useEffect(() => {
        let live = true;
        // Mermaid is a global singleton, so the config is applied and the
        // render awaited as one step. Reconfiguring per diagram would let a
        // second diagram on the same page repaint this one, since `render` is
        // async and reads the shared config when it runs.
        void (async () => {
            mermaid.initialize({
                startOnLoad: false,
                // Diagram text comes from a pull request body, so labels are
                // sanitised and no html is allowed through.
                securityLevel: "strict",
                // Only the base theme reads themeVariables; the rest ignore it.
                theme: "base",
                // Top-level keys, not theme variables. Left at mermaid's
                // defaults they come out as 16px trebuchet, which does not
                // match the app at all.
                fontFamily: FONT,
                fontSize: 13,
                themeVariables: dark ? DARK : LIGHT,
            });
            const result = await mermaid.render(renderId, text);
            if (live) setSvg(stripPaintAttributes(result.svg));
        })().catch((cause: unknown) => {
            console.error("mermaid render failed", cause);
            if (live) setError(describe(cause));
        });
        return () => {
            live = false;
        };
    }, [dark, renderId, text]);

    if (error !== null) {
        // The source is still the content, so it stays readable and
        // copyable; the reason sits above it rather than only in a console
        // nobody has open.
        return (
            <>
                <Frame className="my-2">
                    <FrameHeader className="flex flex-row items-center gap-1.5 px-2 py-1.5">
                        <TriangleAlert className="size-3.5 text-destructive" />
                        <span className="text-xs font-medium text-destructive">
                            Diagram could not be rendered
                        </span>
                    </FrameHeader>
                    <FramePanel className="px-3 py-1.5">
                        <p className="ui-selectable font-mono text-xs break-words text-muted-foreground">
                            {error}
                        </p>
                    </FramePanel>
                </Frame>
                <CodeBlock language="mermaid" text={text} />
            </>
        );
    }

    return (
        <Frame className="my-2">
            <FramePanel
                className={cn(
                    "overflow-auto p-3",
                    dark ? "bg-card" : "bg-card"
                )}
                data-testid="mermaid-diagram"
            >
                {svg === null ? (
                    <div className="flex flex-col gap-2" aria-busy="true">
                        <Skeleton className="h-32 w-full" />
                    </div>
                ) : (
                    <div
                        // Mermaid emits a complete, self-contained svg, and
                        // `securityLevel: strict` has already stripped
                        // anything scriptable from the source that produced it.
                        dangerouslySetInnerHTML={{ __html: svg }}
                    />
                )}
            </FramePanel>
        </Frame>
    );
}
