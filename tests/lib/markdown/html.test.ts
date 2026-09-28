import type { InlineNode } from "@tanstack/markdown";
import { parseMarkdown } from "@tanstack/markdown";
import { renderHtml } from "@tanstack/markdown/html";
import { describe, expect, it } from "vitest";

import { DETAILS_TAG, safeHtmlExtension } from "@/lib/markdown/html";

function parse(source: string) {
    return parseMarkdown(source, {
        allowHtml: true,
        extensions: [safeHtmlExtension()],
    });
}

function inline(source: string) {
    const doc = parse(source);
    const first = doc.children[0];
    if (first?.type !== "paragraph") {
        throw new Error(`expected a paragraph, got ${first?.type}`);
    }
    return first.children;
}

function tagOf(node: InlineNode | undefined): string | undefined {
    return node !== undefined && "tagName" in node ? node.tagName : undefined;
}

function findTag(nodes: readonly InlineNode[], name: string) {
    return nodes.find((node) => tagOf(node) === name);
}

function collectTypes(nodes: unknown[], found: string[] = []): string[] {
    for (const node of nodes as Record<string, unknown>[]) {
        if (typeof node?.type === "string") found.push(node.type);
        for (const key of ["children", "items", "header", "rows"]) {
            const value = node?.[key];
            if (Array.isArray(value)) {
                for (const entry of value) {
                    if (Array.isArray(entry)) collectTypes(entry, found);
                    else if (entry && typeof entry === "object") {
                        collectTypes([entry], found);
                    }
                }
            }
        }
    }
    return found;
}

describe("void elements", () => {
    /** The html renderer writes an open tag, the children, then a close tag,
     * so a void element would otherwise come out as `<br></br>`. */
    it("never emits a closing tag in the html rendering", () => {
        const sources = [
            "one<br />two",
            "one<br>two",
            "one<hr />two",
            "one<wbr />two",
            'x<img src="https://github.com/a.png" alt="a" width="8" />y',
        ];
        for (const source of sources) {
            const html = renderHtml(source, {
                allowHtml: true,
                extensions: [safeHtmlExtension()],
            });
            for (const tag of ["br", "hr", "wbr", "img"]) {
                expect(html, source).not.toContain(`</${tag}>`);
            }
        }
    });

    it("writes a void element with its attributes and no content", () => {
        const html = renderHtml(
            'a<img src="https://github.com/a.png" alt="a" />b',
            {
                allowHtml: true,
                extensions: [safeHtmlExtension()],
            }
        );
        expect(html).toBe(
            '<p>a<img src="https://github.com/a.png" alt="a">b</p>'
        );
    });

    it("escapes an attribute value written as raw markup", () => {
        const html = renderHtml(
            '<img src="https://x/a.png" title=\'a" onload="x\' />',
            {
                allowHtml: true,
                extensions: [safeHtmlExtension()],
            }
        );
        expect(html).toContain("&quot;");
        expect(html).not.toContain('onload="x"');
    });

    it("leaves a non-void element to the default renderer", () => {
        const html = renderHtml("a <b>bold</b> c", {
            allowHtml: true,
            extensions: [safeHtmlExtension()],
        });
        expect(html).toContain("<b>bold</b>");
    });

    it("gives a void element no children in the tree", () => {
        for (const source of ["one<br />two", "one<hr />two"]) {
            const br =
                findTag(inline(source), "br") ?? findTag(inline(source), "hr");
            if (br?.type !== "inlineComponent") {
                throw new Error(`no void element in ${source}`);
            }
            expect(br.children).toEqual([]);
        }
    });

    /** A void element must not swallow the prose around it. */
    it("keeps following text outside the void element", () => {
        const nodes = inline("<br />after");
        expect(nodes[0]).toMatchObject({ tagName: "br" });
        expect(nodes[1]).toMatchObject({ type: "text", value: "after" });
    });
});

describe("safeHtmlExtension", () => {
    /** The renderer injects html with `dangerouslySetInnerHTML`, so the
     * extension's whole job is to make sure it never gets an html node. */
    it("leaves no html node anywhere in the tree", () => {
        const sources = [
            "<details><summary>S</summary>\n\nbody\n\n</details>",
            "a <b>bold <i>both</i></b> tail",
            "before <script>alert(1)</script> after",
            '<a href="javascript:alert(1)">click</a>',
            "<table>\n<tr><th>h</th></tr>\n<tr><td>x</td></tr>\n</table>",
            "| h |\n| - |\n| <b>x</b> |",
            "- item <b>bold</b>\n- second",
            "> quoted <img src='https://x/a.png' />",
            "# heading <b>bold</b>",
            "<p align='center'>centered</p>",
            "<svg><script>x</script></svg>",
            "<div onclick='x()'>text</div>",
        ];
        for (const source of sources) {
            const types = collectTypes(parse(source).children);
            expect(types, source).not.toContain("html");
            expect(types, source).not.toContain("inlineHtml");
        }
    });

    /** Dependabot wraps every release-notes block in this pair. */
    it("turns a details block into a disclosure and keeps the body as markdown", () => {
        const doc = parse(
            [
                "Bumps react.",
                "",
                "<details>",
                "<summary>Release notes</summary>",
                "",
                "> ## 19.2.7",
                ">",
                "> - Fixed a thing",
                "",
                "</details>",
            ].join("\n")
        );
        const details = doc.children.find((node) => node.type === "component");
        expect(details).toBeDefined();
        if (details?.type !== "component") return;
        expect(details.tagName).toBe(DETAILS_TAG);
        expect(details.properties?.["data-summary"]).toBe("Release notes");
        // The body must still be markdown, not literal tags.
        const kinds = details.children.map((child) => child.type);
        expect(kinds).toContain("blockquote");
        expect(JSON.stringify(details.children)).not.toContain("<h2");
    });

    it("strips markdown and html out of the summary label", () => {
        const doc = parse(
            "<details>\n<summary>**Changelog** and <b>notes</b></summary>\n\nbody\n\n</details>"
        );
        const details = doc.children[0];
        if (details?.type !== "component")
            throw new Error("expected a component");
        expect(details.properties?.["data-summary"]).toBe(
            "Changelog and notes"
        );
    });

    /** An unclosed pair must not swallow the rest of the body. */
    it("leaves an unclosed details block to the default html handling", () => {
        const doc = parse("<details>\n<summary>Notes</summary>\n\nstill open");
        expect(doc.children.some((node) => node.type === "component")).toBe(
            false
        );
        expect(doc.children).toHaveLength(2);
    });

    it("reads the open attribute", () => {
        const doc = parse(
            "<details open>\n<summary>Notes</summary>\n\nbody\n\n</details>"
        );
        const details = doc.children[0];
        if (details?.type !== "component")
            throw new Error("expected a component");
        expect(details.properties?.["data-open"]).toBe("open");
    });

    it("maps a void tag to its real element", () => {
        const nodes = inline("line<br />break");
        const br = findTag(nodes, "br");
        expect(br).toMatchObject({ type: "inlineComponent", tagName: "br" });
    });

    it("pairs open and close tags into a nested tree", () => {
        const nodes = inline("a <b>bold <i>both</i></b> tail");
        const bold = findTag(nodes, "b");
        expect(bold).toBeDefined();
        if (bold?.type !== "inlineComponent") return;
        const italic = findTag(bold.children, "i");
        expect(italic).toBeDefined();
    });

    it("keeps the text of an unclosed tag", () => {
        const nodes = inline("tail <b>unclosed");
        expect(JSON.stringify(nodes)).toContain("unclosed");
        expect(nodes.every((node) => node.type === "text")).toBe(true);
    });

    it("keeps the text of an inner tag that never closes inside its parent", () => {
        const nodes = inline("<b>outer <i>inner</b>");
        const bold = findTag(nodes, "b");
        if (bold?.type !== "inlineComponent") throw new Error("no bold");
        expect(findTag(bold.children, "i")).toBeUndefined();
        expect(JSON.stringify(bold.children)).toContain("inner");
    });

    it("parses a raw html table with a header row", () => {
        const doc = parse(
            [
                "<table>",
                "<tr><th>Package</th><th>Before</th><th>After</th></tr>",
                "<tr><td>react</td><td>19.2.4</td><td>19.2.7</td></tr>",
                "</table>",
            ].join("\n")
        );
        const table = doc.children[0];
        if (table?.type !== "table") throw new Error("expected a table");
        expect(table.header).toHaveLength(3);
        expect(table.rows).toHaveLength(1);
        expect(table.rows[0]?.[0]?.children[0]).toMatchObject({
            type: "text",
            value: "react",
        });
    });

    it("parses a raw html table with no header row", () => {
        const doc = parse("<table>\n<tr><td>a</td><td>b</td></tr>\n</table>");
        const table = doc.children[0];
        if (table?.type !== "table") throw new Error("expected a table");
        expect(table.header).toHaveLength(0);
        expect(table.rows).toHaveLength(1);
    });

    it("keeps an escaped angle-bracket tag as literal text", () => {
        const nodes = inline("escaped \\<b>bold\\</b> here");
        expect(nodes.every((node) => node.type === "text")).toBe(true);
    });

    describe("security", () => {
        it("drops script and its tag entirely", () => {
            const nodes = inline("before <script>alert(1)</script> after");
            const text = JSON.stringify(nodes);
            expect(text).not.toContain("script");
            expect(text).not.toContain("alert");
            expect(text).toContain("after");
        });

        it("drops event handlers and styles", () => {
            const nodes = inline(
                '<b onclick="steal()" style="position:fixed">x</b>'
            );
            const bold = findTag(nodes, "b");
            if (bold?.type !== "inlineComponent") throw new Error("no bold");
            expect(bold.properties).toEqual({});
        });

        it("rejects javascript, data, and vbscript urls on links", () => {
            for (const url of [
                "javascript:alert(1)",
                "data:text/html,<script>alert(1)</script>",
                "vbscript:msgbox(1)",
            ]) {
                const nodes = inline(`<a href="${url}">click</a>`);
                const link = findTag(nodes, "a");
                if (link?.type !== "inlineComponent")
                    throw new Error("no link");
                expect(link.properties).not.toHaveProperty("href");
            }
        });

        it("rejects a javascript url on an image", () => {
            const nodes = inline('<img src="javascript:alert(1)" />');
            const img = findTag(nodes, "img");
            if (img?.type !== "inlineComponent") throw new Error("no img");
            expect(img.properties).not.toHaveProperty("src");
        });

        it("keeps ordinary urls on links", () => {
            const nodes = inline('<a href="https://example.com/x">click</a>');
            const link = findTag(nodes, "a");
            if (link?.type !== "inlineComponent") throw new Error("no link");
            expect(link.properties?.href).toBe("https://example.com/x");
        });

        it("drops svg, iframe, and form", () => {
            const nodes = inline(
                "keep <svg><script>x</script></svg> and <iframe>y</iframe> and <form>z</form>"
            );
            const text = JSON.stringify(nodes);
            expect(text).not.toContain("svg");
            expect(text).not.toContain("iframe");
            expect(text).not.toContain("form");
            expect(text).toContain("keep");
            expect(text).toContain("and");
        });

        it("unwraps presentational tags but keeps their text", () => {
            const nodes = inline("<center>middle</center>");
            expect(nodes.map((node) => node.type)).toEqual(["text"]);
            expect(JSON.stringify(nodes)).toContain("middle");
        });

        it("keeps a bare angle bracket as text", () => {
            const nodes = inline("a < b and 3<4");
            expect(JSON.stringify(nodes)).toContain("<");
        });

        it("drops html comments bots leave in the body", () => {
            const nodes = inline(
                "Bumps the crate. <!-- finishing_touch_suggestion:docstrings -->"
            );
            const text = JSON.stringify(nodes);
            expect(text).not.toContain("finishing_touch");
            expect(text).toContain("Bumps the crate.");
        });
    });
});
