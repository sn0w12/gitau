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

import { CloneRepoDialog } from "@/components/repo/dialogs/clone-dialog";
import { CloneProgressPanel } from "@/components/repo/dialogs/clone-progress-panel";
import { NewRepoDialog } from "@/components/repo/dialogs/new-repo-dialog";
import { Button } from "@/components/ui/button";
import { Frame, FramePanel } from "@/components/ui/frame";
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
