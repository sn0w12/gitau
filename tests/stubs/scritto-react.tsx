import type { ReactElement } from "react";

/**
 * Test stub for `@scritto/react`. The real package imports
 * `@scritto/core/ssr.css` at module scope and registers custom elements on
 * load, which breaks Vitest's node and jsdom module loading. No test asserts
 * the animation, so the stub renders the value as plain text.
 */
export default function Scritto({ value }: { value: string }): ReactElement {
    return <span>{value}</span>;
}
