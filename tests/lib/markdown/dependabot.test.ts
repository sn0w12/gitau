import { parseMarkdown } from "@tanstack/markdown";
import { describe, expect, it } from "vitest";

import { githubMarkdownExtension } from "@/lib/markdown/github";
import {
    DETAILS_TAG,
    normalizeHtmlBlocks,
    safeHtmlExtension,
} from "@/lib/markdown/html";

// The body of sn0w12/Akari#334, verbatim. Note there is no blank line before
// `<details>`, and the release notes are raw html with no blank lines either.
const DEPENDABOT_BODY = `Bumps [react](https://github.com/facebook/react/tree/HEAD/packages/react) and [@types/react](https://github.com/DefinitelyTyped/DefinitelyTyped/tree/HEAD/types/react). These dependencies needed to be updated together.
Updates \`react\` from 19.2.4 to 19.2.7
<details>
<summary>Release notes</summary>
<p><em>Sourced from <a href="https://github.com/facebook/react/releases">react's releases</a>.</em></p>
<blockquote>
<h2>19.2.7 (June 1st, 2026)</h2>
<h2>React Server Components</h2>
<ul>
<li>Fixed missing <code>FormData</code> entries in Server Actions which regressed in 19.2.6
(<a href="https://redirect.github.com/facebook/react/pull/36566">#36566</a> by <a href="https://github.com/unstubbable"><code>@unstubbable</code></a>)</li>
</ul>
</blockquote>
</details>
<details>
<summary>Commits</summary>
<ul>
<li><a href="https://github.com/react/react/commit/6117d7c"><code>6117d7c</code></a> Version 19.2.7</li>
</ul>
</details>
<br />

> **Note**
> Automatic rebases have been disabled.
`;

interface AnyNode {
    type: string;
    tagName?: string;
    properties?: Record<string, string>;
    children?: AnyNode[];
    depth?: number;
}

function parseDependabotBody() {
    return parseMarkdown(normalizeHtmlBlocks(DEPENDABOT_BODY), {
        allowHtml: true,
        extensions: [safeHtmlExtension()],
    }) as unknown as { children: AnyNode[] };
}

function types(nodes: AnyNode[]): string[] {
    return nodes.map((node) => {
        if (node.tagName) return `${node.type}:${node.tagName}`;
        if (node.type === "heading") return `heading:${node.depth}`;
        return node.type;
    });
}

function findAll(nodes: AnyNode[], type: string): AnyNode[] {
    const found: AnyNode[] = [];
    for (const node of nodes) {
        if (node.type === type) found.push(node);
        if (node.children) found.push(...findAll(node.children, type));
    }
    return found;
}

describe("normalizeHtmlBlocks", () => {
    it("lets a details block interrupt a paragraph", () => {
        const lines = normalizeHtmlBlocks(
            "a sentence\n<details>\nx\n</details>"
        ).split("\n");
        expect(lines[1]).toBe("");
        expect(lines[2]).toBe("<details>");
    });

    it("leaves a block that already follows a blank line alone", () => {
        expect(normalizeHtmlBlocks("a\n\n<details>\nx\n</details>")).toBe(
            "a\n\n<details>\nx\n</details>"
        );
    });

    it("does not touch inline html mid-paragraph", () => {
        expect(normalizeHtmlBlocks("a <b>bold</b> line")).toBe(
            "a <b>bold</b> line"
        );
    });
});

describe("a dependabot body", () => {
    it("turns each details block into a disclosure", () => {
        const doc = parseDependabotBody();
        const details = findAll(doc.children, "component");
        expect(details).toHaveLength(2);
        expect(details.every((node) => node.tagName === DETAILS_TAG)).toBe(
            true
        );
        expect(
            details.map((node) => node.properties?.["data-summary"])
        ).toEqual(["Release notes", "Commits"]);
    });

    /** The release notes are one unbroken html run, so the tags have to be
     * paired and lifted back into real blocks. */
    it("keeps the release notes as a blockquote with headings and a list", () => {
        const doc = parseDependabotBody();
        const details = findAll(doc.children, "component")[0];
        // The release notes are one blockquote, not a flat run of siblings.
        expect(types(details?.children ?? [])).toEqual([
            "paragraph",
            "blockquote",
        ]);

        const quote = findAll(details?.children ?? [], "blockquote")[0];
        const inner = types(quote?.children ?? []);
        expect(inner).toContain("heading:2");
        expect(inner).toContain("list");
    });

    it("does not leave a heading inside a paragraph", () => {
        const doc = parseDependabotBody();
        const bad = doc.children
            .flatMap((node) => node.children ?? [node])
            .filter((node) => node.type === "paragraph")
            .flatMap((node) => node.children ?? [])
            .filter(
                (node) =>
                    node.tagName !== undefined &&
                    [
                        "h1",
                        "h2",
                        "h3",
                        "ul",
                        "ol",
                        "blockquote",
                        "table",
                    ].includes(node.tagName)
            );
        expect(bad).toEqual([]);
    });

    it("leaves no html node behind", () => {
        const doc = parseDependabotBody();
        expect(findAll(doc.children, "html")).toEqual([]);
        expect(findAll(doc.children, "inlineHtml")).toEqual([]);
    });

    /** Dependabot links an upstream pull request and labels it with its
     * number, so `#36236` inside that anchor is a label, not a reference to
     * an issue in this repository. */
    it("does not re-link a number that is already a link label", () => {
        const doc = parseMarkdown(
            normalizeHtmlBlocks(
                '<li>See (<a href="https://redirect.github.com/facebook/react/pull/36236">#36236</a>)</li>'
            ),
            {
                allowHtml: true,
                // The same pair the app passes, so `#36236` has coordinates
                // to resolve against if the guard is not working.
                extensions: [
                    safeHtmlExtension(),
                    githubMarkdownExtension({ owner: "sn0w12", repo: "Akari" }),
                ],
            }
        ) as unknown as { children: AnyNode[] };
        const anchors = findAll(doc.children, "inlineComponent").filter(
            (node) => node.tagName === "a"
        );
        expect(anchors).toHaveLength(1);
        const anchor = anchors[0] as AnyNode;
        expect(anchor.properties?.href).toBe(
            "https://redirect.github.com/facebook/react/pull/36236"
        );
        // The label stays plain text: no link, no mention, no reference.
        expect(anchor.children).toEqual([{ type: "text", value: "#36236" }]);
    });
});
