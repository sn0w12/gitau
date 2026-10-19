import ScrittoBase from "@scritto/react";
import type { ComponentProps } from "react";
import { useLayoutEffect, useRef } from "react";

type ScrittoProps = ComponentProps<typeof ScrittoBase>;

const GLYPH_LAYER_RULE = ".char { will-change: transform, opacity, filter; }";
const layeredRoots = new WeakSet<ShadowRoot>();

function keepGlyphLayers(host: Element | null) {
    const root = host?.shadowRoot;
    if (!root || layeredRoots.has(root)) return;

    layeredRoots.add(root);
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(GLYPH_LAYER_RULE);
    root.adoptedStyleSheets = [...root.adoptedStyleSheets, sheet];
}

/**
 * Scritto rolls each glyph with `transform` and clears the inline style when
 * the roll ends. The glyph then loses its composited layer and is rasterized
 * again against the parent, shifting its edges: the snap at the end of every
 * roll. Keeping the glyphs composited for the host's lifetime removes that
 * second rasterization. The rule has to be adopted by the shadow root because
 * `will-change` does not inherit across a shadow boundary.
 */
export function Scritto(props: ScrittoProps) {
    const holder = useRef<HTMLSpanElement>(null);

    useLayoutEffect(() => {
        keepGlyphLayers(holder.current?.querySelector("scritto-text") ?? null);
    }, []);

    return (
        <span ref={holder} className="contents">
            <ScrittoBase {...props} />
        </span>
    );
}
