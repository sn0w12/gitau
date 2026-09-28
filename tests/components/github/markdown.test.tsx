// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, fireEvent, render } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CustomMarkdown } from "@/components/github/markdown";
import { AppServicesContext } from "@/contexts/services-context";
import type { HighlightedSnippet } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

const PLAIN: HighlightedSnippet = {
    highlighted: false,
    spansByLine: [],
    styles: [],
};

function renderMarkdown(source: string, owner?: string, repo?: string) {
    const backend = {
        highlight: {
            code: async (): Promise<Result<HighlightedSnippet>> => ({
                ok: true,
                value: PLAIN,
            }),
        },
    } as unknown as BackendClient;
    const services = {
        backend,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
    };
    function Wrapper({ children }: { children: ReactNode }) {
        return (
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    {children}
                </QueryClientProvider>
            </AppServicesContext.Provider>
        );
    }
    return render(
        <Wrapper>
            <CustomMarkdown owner={owner} repo={repo}>
                {source}
            </CustomMarkdown>
        </Wrapper>
    );
}

function hrefs(container: HTMLElement): (string | null)[] {
    return [...container.querySelectorAll("a")].map((anchor) =>
        anchor.getAttribute("href")
    );
}

afterEach(() => {
    cleanup();
});

beforeEach(() => {
    seedSettingsForTests({});
});

describe("Safe HTML", () => {
    it("renders void elements with no children and no React warning", () => {
        const warnings: unknown[][] = [];
        const spy = vi
            .spyOn(console, "error")
            .mockImplementation((...args: unknown[]) => {
                warnings.push(args);
            });
        try {
            for (const source of [
                "one<br />two",
                "one<br>two",
                "one<wbr />two",
                "one<hr />two",
                'x<img src="https://github.com/a.png" alt="a" />y',
            ]) {
                const view = renderMarkdown(source);
                for (const tag of ["br", "wbr", "hr", "img"]) {
                    const element = view.container.querySelector(tag);
                    if (element === null) continue;
                    expect(element.childNodes, source).toHaveLength(0);
                }
                // React refuses children on an intrinsic void element, so a
                // regression here shows up as a console error, not as markup.
                const joined = JSON.stringify(warnings);
                expect(joined, source).not.toContain("void element");
            }
        } finally {
            spy.mockRestore();
        }
    });

    it("renders a void element inline with the text around it", () => {
        const view = renderMarkdown("one<br />two");
        const br = view.container.querySelector("br");
        expect(br).not.toBeNull();
        expect(br?.innerHTML).toBe("");
        expect(view.container.textContent).toBe("onetwo");
    });

    it("renders inline html as real elements, not as markup text", () => {
        const view = renderMarkdown("a <b>bold</b> and <i>italic</i>");
        expect(view.container.querySelector("b")?.textContent).toBe("bold");
        expect(view.container.querySelector("i")?.textContent).toBe("italic");
        expect(view.container.textContent).not.toContain("<b>");
    });

    it("renders a dependabot details block as a disclosure", () => {
        // Opened, so the collapsed-by-default body is in the DOM; the point is
        // that it arrived as markdown rather than as literal tags.
        const view = renderMarkdown(
            [
                "<details open>",
                "<summary>Release notes</summary>",
                "",
                "- Fixed a thing",
                "",
                "</details>",
            ].join("\n")
        );
        const text = view.container.textContent ?? "";
        expect(text).toContain("Release notes");
        expect(text).toContain("Fixed a thing");
        expect(text).not.toContain("<details>");
        expect(text).not.toContain("<summary>");
        expect(view.container.querySelector("li")?.textContent).toBe(
            "Fixed a thing"
        );
    });

    it("collapses a details body by default", () => {
        const view = renderMarkdown(
            "<details>\n<summary>Release notes</summary>\n\n- Fixed\n\n</details>"
        );
        expect(view.container.textContent).toContain("Release notes");
        expect(view.container.textContent).not.toContain("Fixed");
    });

    it("toggles a details body when the summary is clicked", () => {
        const view = renderMarkdown(
            "<details>\n<summary>Release notes</summary>\n\n- Fixed\n\n</details>"
        );
        const trigger = view.container.querySelector<HTMLElement>(
            '[data-slot="collapsible-trigger"]'
        );
        expect(trigger).not.toBeNull();
        expect(trigger?.getAttribute("aria-expanded")).toBe("false");

        fireEvent.click(trigger as Element);
        expect(trigger?.getAttribute("aria-expanded")).toBe("true");
        expect(view.container.textContent).toContain("Fixed");

        fireEvent.click(trigger as Element);
        expect(trigger?.getAttribute("aria-expanded")).toBe("false");
        expect(view.container.textContent).not.toContain("Fixed");
    });

    it("starts open when the details block carries the open attribute", () => {
        const view = renderMarkdown(
            "<details open>\n<summary>Release notes</summary>\n\n- Fixed\n\n</details>"
        );
        expect(view.container.textContent).toContain("Fixed");
        const trigger = view.container.querySelector<HTMLElement>(
            '[data-slot="collapsible-trigger"]'
        );
        expect(trigger?.getAttribute("aria-expanded")).toBe("true");
    });

    it("drops script and its body", () => {
        const view = renderMarkdown("before <script>alert(1)</script> after");
        expect(view.container.textContent).toBe("before  after");
        expect(view.container.querySelector("script")).toBeNull();
    });

    it("drops an inline event handler", () => {
        const view = renderMarkdown('<b onclick="steal()">text</b>');
        const bold = view.container.querySelector("b");
        expect(bold?.getAttribute("onclick")).toBeNull();
        expect(bold?.textContent).toBe("text");
    });
});

describe("GitHub references", () => {
    it("links a user mention", () => {
        const view = renderMarkdown(
            "cc @ada-lovelace please",
            "gitau",
            "gitau"
        );
        expect(hrefs(view.container)).toEqual([
            "https://github.com/ada-lovelace",
        ]);
        const anchor = view.container.querySelector("a")!;
        expect(anchor.textContent).toBe("@ada-lovelace");
        // Mentions go through the shared external link treatment.
        expect(anchor.getAttribute("target")).toBe("_blank");
        expect(anchor.querySelector("svg")).not.toBeNull();
    });

    it("links a team mention through the org path", () => {
        const view = renderMarkdown("@gitau/core-team owns this");
        expect(hrefs(view.container)).toEqual([
            "https://github.com/orgs/gitau/teams/core-team",
        ]);
    });

    it("links a repository issue number", () => {
        const view = renderMarkdown("blocked by #42", "gitau", "gitau");
        expect(hrefs(view.container)).toEqual([
            "https://github.com/gitau/gitau/issues/42",
        ]);
        expect(view.container.querySelector("a")?.textContent).toBe("#42");
    });

    it("links a cross-repository issue number", () => {
        const view = renderMarkdown("see octo/tools#7", "gitau", "gitau");
        expect(hrefs(view.container)).toEqual([
            "https://github.com/octo/tools/issues/7",
        ]);
    });

    it("leaves a bare number as text without repository coordinates", () => {
        const view = renderMarkdown("blocked by #42");
        expect(hrefs(view.container)).toEqual([]);
        expect(view.container.textContent).toBe("blocked by #42");
    });

    it("keeps a reference glued to a word as text", () => {
        const view = renderMarkdown("issue#42 and C#9", "gitau", "gitau");
        expect(hrefs(view.container)).toEqual([]);
    });

    it("links mentions nested in emphasis", () => {
        const view = renderMarkdown("**cc @ada**", "gitau", "gitau");
        expect(hrefs(view.container)).toEqual(["https://github.com/ada"]);
    });

    it("does not link inside code spans", () => {
        const view = renderMarkdown("`@ada #42 https://x.dev`");
        expect(hrefs(view.container)).toEqual([]);
        expect(view.container.querySelector("code")?.textContent).toBe(
            "@ada #42 https://x.dev"
        );
    });
});

describe("Autolinks", () => {
    it("links a bare URL and keeps sentence punctuation outside it", () => {
        const view = renderMarkdown("see https://example.com/docs.");
        expect(hrefs(view.container)).toEqual(["https://example.com/docs"]);
        expect(view.container.textContent).toBe(
            "see https://example.com/docs."
        );
    });

    it("keeps balanced parentheses inside a URL", () => {
        const view = renderMarkdown(
            "https://en.wikipedia.org/wiki/Foo_(bar) tail"
        );
        expect(hrefs(view.container)).toEqual([
            "https://en.wikipedia.org/wiki/Foo_(bar)",
        ]);
    });

    it("upgrades a www host to https", () => {
        const view = renderMarkdown("www.example.com");
        expect(hrefs(view.container)).toEqual(["https://www.example.com"]);
    });

    it("links a bare email as mailto", () => {
        const view = renderMarkdown("mail ada@example.com now");
        expect(hrefs(view.container)).toEqual(["mailto:ada@example.com"]);
        expect(view.container.textContent).toBe("mail ada@example.com now");
    });

    it("unwraps an angle-bracketed autolink", () => {
        const view = renderMarkdown(
            "<https://example.com/a> and <ada@example.com>"
        );
        expect(hrefs(view.container)).toEqual([
            "https://example.com/a",
            "mailto:ada@example.com",
        ]);
        expect(view.container.textContent).toBe(
            "https://example.com/a and ada@example.com"
        );
    });
});

describe("Hard breaks", () => {
    it("breaks after a trailing backslash", () => {
        const view = renderMarkdown("first line\\\nsecond line");
        expect(view.container.querySelectorAll("br")).toHaveLength(1);
    });
});

describe("Setext headings", () => {
    it("reads an underline as a level one heading", () => {
        const view = renderMarkdown("Release notes\n=============");
        const heading = view.container.querySelector("h1");
        expect(heading?.textContent).toBe("Release notes");
        expect(heading?.querySelector("a")).toBeNull();
    });

    it("reads a dash underline as a level two heading", () => {
        const view = renderMarkdown("Details\n-------");
        expect(view.container.querySelector("h2")?.textContent).toContain(
            "Details"
        );
    });

    it("joins the lines above the underline into one heading", () => {
        const view = renderMarkdown("Release\nnotes\n===");
        expect(view.container.querySelector("h1")?.textContent).toContain(
            "Release notes"
        );
    });

    it("leaves a table alone", () => {
        const view = renderMarkdown("| a | b |\n| --- | --- |\n| 1 | 2 |");
        expect(view.container.querySelector("h1")).toBeNull();
        expect(view.container.querySelector("table")).not.toBeNull();
    });

    it("reads a heading whose text holds a pipe", () => {
        const view = renderMarkdown("left | right\n===");
        expect(view.container.querySelector("h1")?.textContent).toContain(
            "left | right"
        );
    });

    it("keeps a list and its rule apart", () => {
        const view = renderMarkdown("- one\n\n---\n");
        expect(view.container.querySelector("ul")).not.toBeNull();
        expect(view.container.querySelector("h2")).toBeNull();
        expect(
            view.container.querySelector('[data-slot="separator"]')
        ).not.toBeNull();
    });
});

describe("Alerts", () => {
    it("renders an alert with its kind label and body", () => {
        const view = renderMarkdown("> [!NOTE]\n> Ship on Friday.");
        const alert = view.container.querySelector("[role='note']");
        expect(alert).not.toBeNull();
        expect(alert?.querySelector("svg")).not.toBeNull();
        expect(alert?.textContent).toContain("Note");
        expect(alert?.textContent).toContain("Ship on Friday.");
    });

    it("uses a custom title when one is given", () => {
        const view = renderMarkdown("> [!WARNING] Heads up\n> Careful.");
        const alert = view.container.querySelector("[role='note']");
        expect(alert?.textContent).toContain("Heads up");
        expect(alert?.textContent).not.toContain("Warning");
    });

    it("leaves an unknown kind as a blockquote", () => {
        const view = renderMarkdown("> [!CUSTOM]\n> Body.");
        expect(view.container.querySelector("[role='note']")).toBeNull();
        expect(view.container.querySelector("blockquote")).not.toBeNull();
    });
});

describe("CustomMarkdown", () => {
    it("marks fenced code for backend highlighting", () => {
        const view = renderMarkdown("```rust\nlet x = 1;\n```");
        expect(view.container.textContent).toContain("let x = 1;");
        // The spans carry theme colors as custom properties; this wrapper is
        // what applies them.
        expect(view.container.querySelector(".code-hl")).not.toBeNull();
    });
});
