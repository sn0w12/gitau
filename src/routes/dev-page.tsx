"use no memo";

import { useQuery } from "@tanstack/react-query";
import { useSelector } from "@tanstack/react-store";
import {
    FlaskConical,
    FolderPlus,
    Download,
    PlayCircle,
    RotateCcw,
} from "lucide-react";
import { useRef, useState } from "react";

import { CustomMarkdown } from "@/components/github/markdown";
import { CloneRepoDialog } from "@/components/repo/dialogs/clone-dialog";
import { CloneProgressPanel } from "@/components/repo/dialogs/clone-progress-panel";
import { NewRepoDialog } from "@/components/repo/dialogs/new-repo-dialog";
import { Button } from "@/components/ui/button";
import { Frame, FramePanel } from "@/components/ui/frame";
import { Input } from "@/components/ui/input";
import { Progress } from "@/components/ui/progress";
import { useAppServices } from "@/contexts/services-context";
import type { ClonePhase, CloneProgress } from "@/lib/backend/protocol";
import {
    gitignoreTemplatesQuery,
    licensesQuery,
} from "@/lib/backend/queries/template-queries";
import { operationStore } from "@/stores/operation-store";
import { repositoryStore } from "@/stores/repository-store";

/**
 * Developer-only playground: exercises dialogs, streams simulated clone
 * progress through the real panel component, probes IPC endpoints, and
 * dumps live store state. Not registered outside dev builds.
 */
export function DevPage() {
    return (
        <div className="flex size-full flex-col gap-6 overflow-y-auto p-6">
            <header className="space-y-1">
                <h1 className="flex items-center gap-2 text-2xl font-semibold tracking-tight">
                    <FlaskConical className="size-6" />
                    Dev playground
                </h1>
                <p className="text-sm text-muted-foreground">
                    Mock features without touching real repositories. This page
                    only exists in dev builds.
                </p>
            </header>

            <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
                <DialogsCard />
                <ProgressSimulatorCard />
                <BackendProbesCard />
                <StoresCard />
                <div className="xl:col-span-2">
                    <MarkdownCard />
                </div>
            </div>
        </div>
    );
}

function Card({
    title,
    children,
}: {
    title: string;
    children: React.ReactNode;
}) {
    return (
        <Frame>
            <FramePanel className="flex flex-col gap-3 p-4">
                <h2 className="text-sm font-semibold text-muted-foreground">
                    {title}
                </h2>
                {children}
            </FramePanel>
        </Frame>
    );
}

function DialogsCard() {
    const [dialog, setDialog] = useState<null | "new" | "clone">(null);

    const devCreated = (repoPath: string) => {
        window.setTimeout(
            () => window.alert(`[dev] created/cloned: ${repoPath}`),
            0
        );
    };

    return (
        <Card title="Repository dialogs">
            <div className="flex gap-2">
                <Button variant="secondary" onClick={() => setDialog("new")}>
                    <FolderPlus />
                    New repo
                </Button>
                <Button variant="secondary" onClick={() => setDialog("clone")}>
                    <Download />
                    Clone
                </Button>
            </div>
            <p className="text-xs text-muted-foreground">
                The clone dialog morphs between form and progress; a destination
                picked here persists into the hidden lastRepositoryDirectory
                setting.
            </p>
            <NewRepoDialog
                open={dialog === "new"}
                onClose={() => setDialog(null)}
                onCreated={(repoPath) => {
                    setDialog(null);
                    devCreated(repoPath);
                }}
            />
            <CloneRepoDialog
                open={dialog === "clone"}
                onClose={() => setDialog(null)}
                onCloned={(repoPath) => {
                    setDialog(null);
                    devCreated(repoPath);
                }}
            />
        </Card>
    );
}

const PHASE_SEQUENCE: Array<{
    phase: ClonePhase;
    steps: number;
}> = [
    { phase: "counting", steps: 2 },
    { phase: "receiving", steps: 30 },
    { phase: "resolving", steps: 12 },
    { phase: "checkingOut", steps: 2 },
];

function ProgressSimulatorCard() {
    const [progress, setProgress] = useState<CloneProgress | null>(null);
    const [playing, setPlaying] = useState(false);
    const timer = useRef<number | null>(null);

    const stop = () => {
        if (timer.current !== null) {
            window.clearInterval(timer.current);
            timer.current = null;
        }
        setPlaying(false);
    };

    const run = () => {
        stop();
        let step = 0;
        const totalObjects = 8_421;
        const totalBytes = 96 * 1024 * 1024;

        const sampleFor = (step: number): CloneProgress | null => {
            let cursor = 0;
            for (const section of PHASE_SEQUENCE) {
                if (step < cursor + section.steps) {
                    const t = (step - cursor) / section.steps;
                    switch (section.phase) {
                        case "counting":
                            return {
                                phase: "counting",
                                progress: 0,
                                objectsReceived: 0,
                                objectsTotal: null,
                                receivedBytes: 0,
                            };
                        case "receiving":
                            return {
                                phase: "receiving",
                                progress: 0.05 + 0.85 * t,
                                objectsReceived: Math.floor(totalObjects * t),
                                objectsTotal: totalObjects,
                                receivedBytes: Math.floor(totalBytes * t),
                            };
                        case "resolving":
                            return {
                                phase: "resolving",
                                progress: 0.9 + 0.09 * t,
                                objectsReceived: totalObjects,
                                objectsTotal: totalObjects,
                                receivedBytes: totalBytes,
                            };
                        case "checkingOut":
                            return null;
                    }
                }
                cursor += section.steps;
            }
            return null;
        };

        setPlaying(true);
        timer.current = window.setInterval(() => {
            step += 1;
            const sample = sampleFor(step);
            if (
                !sample &&
                step > PHASE_SEQUENCE.reduce((a, s) => a + s.steps, 0)
            ) {
                stop();
                return;
            }
            setProgress(sample);
        }, 90);
    };

    const reset = () => {
        stop();
        setProgress(null);
    };

    const barPercent =
        progress && progress.progress > 0 ? progress.progress * 100 : null;

    return (
        <Card title="Clone progress simulator">
            <div className="flex gap-2">
                <Button onClick={run} disabled={playing}>
                    <PlayCircle />
                    Run
                </Button>
                <Button variant="ghost" onClick={reset}>
                    <RotateCcw />
                    Reset
                </Button>
            </div>

            <div className="rounded-lg border border-input p-3">
                <p className="mb-2 truncate text-xs text-muted-foreground">
                    https://github.com/octocat/simulated-repo.git
                </p>
                <CloneProgressPanel progress={progress} />
            </div>

            <p className="text-xs text-muted-foreground">
                Raw store value:{" "}
                <code className="font-mono">
                    {barPercent === null
                        ? "indeterminate"
                        : `${barPercent.toFixed(1)}%`}
                </code>
            </p>
            <div aria-hidden="true">
                <Progress value={barPercent ?? null} />
            </div>
        </Card>
    );
}

function BackendProbesCard() {
    const { backend } = useAppServices();
    const templates = useQuery(gitignoreTemplatesQuery({ backend }));
    const licenses = useQuery(licensesQuery({ backend }));

    const probeResult = licenses.data
        ? `${templates.data?.length ?? "?"} gitignore presets, ${
              licenses.data.length
          } licenses served by the backend`
        : "loading...";

    return (
        <Card title="Backend probes">
            <p className="text-sm">{probeResult}</p>
            <ul className="max-h-40 space-y-0.5 overflow-y-auto text-xs text-muted-foreground">
                {(templates.data ?? []).map((t) => (
                    <li key={t.id} className="font-mono">
                        {t.id} - {t.label}
                    </li>
                ))}
            </ul>
        </Card>
    );
}

function StoresCard() {
    const entries = useSelector(repositoryStore, (state) => [
        ...state.entries.values(),
    ]);
    const operations = useSelector(operationStore, (state) => [
        ...state.operations.values(),
    ]);

    return (
        <Card title="Live stores">
            <section className="space-y-1">
                <h3 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
                    Repositories ({entries.length})
                </h3>
                {entries.map((entry) => (
                    <p key={entry.path} className="truncate font-mono text-xs">
                        {entry.repoId ?? "-"} {entry.path}
                        {entry.lastError ? ` (${entry.lastError})` : ""}
                    </p>
                ))}
                {entries.length === 0 ? (
                    <p className="text-xs text-muted-foreground">empty</p>
                ) : null}
            </section>
            <section className="space-y-1">
                <h3 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
                    Operations ({operations.length})
                </h3>
                {operations.map((op) => (
                    <p key={op.operationId} className="font-mono text-xs">
                        #{op.operationId} {op.kind} {op.phase}
                    </p>
                ))}
                {operations.length === 0 ? (
                    <p className="text-xs text-muted-foreground">none</p>
                ) : null}
            </section>
        </Card>
    );
}

interface MarkdownSample {
    title: string;
    source: string;
}

const MARKDOWN_SAMPLES: MarkdownSample[] = [
    {
        title: "Headings",
        source: "# Heading level 1\n\n## Heading level 2\n\n### Heading level 3\n\n#### Heading level 4",
    },
    {
        title: "Setext headings",
        source: "Setext level 1\n===============\n\nSetext level 2\n---------------",
    },
    {
        title: "Paragraphs and inline styles",
        source: "A paragraph of plain text.\n\n**bold**, *italic*, _underscore italic_, ~~strikethrough~~, `inline code`, and text\\ with a backslash escape.",
    },
    {
        title: "Hard line breaks",
        source: "A backslash before the newline breaks the line.\\\nThe next line starts here.",
    },
    {
        title: "Escapes",
        source: "\\*not bold\\*, \\_not italic\\_, \\# not a reference, \\[not a link\\]",
    },
    {
        title: "Unordered and nested lists",
        source: "- first\n- second\n    - nested one\n    - nested two\n- third",
    },
    {
        title: "Ordered lists",
        source: "1. one\n2. two\n3. three\n\n7) starts at seven",
    },
    {
        title: "Task lists",
        source: "- [x] done\n- [ ] pending\n- [X] also done",
    },
    {
        title: "Blockquotes",
        source: "> A quoted line.\n> Still the same quote.\n\n> > A nested quote.",
    },
    {
        title: "Thematic break",
        source: "above the rule\n\n---\n\nbelow the rule",
    },
    {
        title: "Fenced code with a language",
        source: '```rust\nfn main() {\n    println!("hi");\n}\n```',
    },
    {
        title: "Fenced code without a language",
        source: "```\nno highlighting here\n```",
    },
    {
        title: "Fence metadata",
        source: "```rust file=src/main.rs\nlet x = 1;\n```\n\n```js title=entry.js\nconst x = 1;\n```",
    },
    {
        title: "Tables",
        source: "| Left | Center | Right |\n| :--- | :----: | ----: |\n| a | b | c |\n| longer cell | x | y |",
    },
    {
        title: "Links",
        source: '[Inline link](https://example.com), [relative link](/repo/1), [reference link][docs], and [shortcut].\n\n[docs]: https://example.com/docs "Reference title"\n[shortcut]: https://example.com/shortcut',
    },
    {
        title: "Autolinks",
        source: "Bare https://example.com/docs, a www.example.com host, an ada@example.com address, and the <https://example.com/angle> form. Trailing punctuation stays outside: https://example.com.",
    },
    {
        title: "Mentions",
        source: "cc @ada-lovelace, or ping the @octocat/hello-world team.",
    },
    {
        title: "Issue references",
        source: "Fixes #123, tracked in octo/tools#456, and a glued word#7 stays text.",
    },
    {
        title: "Alerts",
        source: "> [!NOTE]\n> Something worth knowing.\n\n> [!TIP] Use the toolbar\n> An optional hint with its own title.\n\n> [!WARNING]\n> Something risky.\n\n> [!CAUTION]\n> Something destructive.\n\n> [!CUSTOM]\n> An unknown kind stays a plain quote.",
    },
    {
        title: "Footnotes",
        source: "A claim that needs a source.[^1]\n\n[^1]: The footnote body.",
    },
    {
        title: "Images",
        source: "![Tracked host](https://github.com/images/modules/logos_page/Octicons-155.png)\n\n![Untracked host](https://example.com/image.png)",
    },
    {
        title: "Safe HTML",
        source: [
            "Dependabot wraps its release notes in a disclosure:",
            "",
            "<details>",
            "<summary>Release notes</summary>",
            "",
            "## 19.2.7",
            "",
            "- Fixed a thing",
            "- Fixed another",
            "",
            "</details>",
            "",
            "Inline html is allowlisted: <b>bold</b>, <i>italic</i>, <kbd>Ctrl</kbd>, <sub>sub</sub>, and a",
            'break<br />here. <img src="https://github.com/images/modules/logos_page/Octicons-163.png" width="32" alt="octicon" />',
            "",
            "Escaped tags stay literal: \\<b>not bold\\</b>",
        ].join("\n"),
    },
    {
        title: "Unsafe HTML is dropped",
        source: [
            "A <script>alert(1)</script> tag and its body disappear, as do event",
            "handlers and script urls:",
            "",
            '<b onclick="steal()">not clickable</b>',
            "",
            "[not a link](javascript:alert(1))",
        ].join("\n"),
    },
    {
        title: "Unsupported syntax",
        source: ":tada: shortcodes, ^superscripts^, ==highlights== and a two-space hard break all stay literal.",
    },
];

function MarkdownSampleRow({
    sample,
    owner,
    repo,
}: {
    sample: MarkdownSample;
    owner: string;
    repo: string;
}) {
    return (
        <section className="space-y-1">
            <h3 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
                {sample.title}
            </h3>
            <div className="grid gap-2 lg:grid-cols-2">
                <pre className="ui-selectable overflow-x-auto rounded-md border bg-muted/40 p-2 font-mono text-xs whitespace-pre-wrap">
                    {sample.source}
                </pre>
                <div className="min-w-0 rounded-md border p-2 text-sm">
                    <CustomMarkdown owner={owner} repo={repo}>
                        {sample.source}
                    </CustomMarkdown>
                </div>
            </div>
        </section>
    );
}

function MarkdownCard() {
    const [owner, setOwner] = useState("octocat");
    const [repo, setRepo] = useState("Hello-World");

    return (
        <Card title="Markdown">
            <p className="text-xs text-muted-foreground">
                Every construct the comment renderer handles, source beside the
                result. The coordinates decide whether issue references link;
                clear a field to see them fall back to text.
            </p>
            <div className="flex flex-wrap items-end gap-2">
                <label className="flex flex-col gap-1">
                    <span className="text-xs text-muted-foreground">owner</span>
                    <Input
                        value={owner}
                        onChange={(event) => setOwner(event.target.value)}
                        className="h-7 w-36 font-mono text-xs"
                    />
                </label>
                <label className="flex flex-col gap-1">
                    <span className="text-xs text-muted-foreground">repo</span>
                    <Input
                        value={repo}
                        onChange={(event) => setRepo(event.target.value)}
                        className="h-7 w-36 font-mono text-xs"
                    />
                </label>
            </div>
            <div className="flex flex-col gap-4">
                {MARKDOWN_SAMPLES.map((sample) => (
                    <MarkdownSampleRow
                        key={sample.title}
                        sample={sample}
                        owner={owner}
                        repo={repo}
                    />
                ))}
            </div>
        </Card>
    );
}
