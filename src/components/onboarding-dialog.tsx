import Scritto from "@scritto/react";
import {
    Check,
    ChevronLeft,
    ChevronRight,
    Download,
    FolderOpen,
    FolderPlus,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { startTransition, useId, useState } from "react";

import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogContent,
    DialogDescription,
    DialogHeader,
    DialogPanel,
    DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTab } from "@/components/ui/tabs";
import type { AppCommandActions } from "@/contexts/app-command-context";
import { useAppCommands } from "@/contexts/app-command-context";
import { useSetting } from "@/hooks/settings/use-setting";
import { EASE_SNAPPY } from "@/lib/motion";
import type { ThemeMode } from "@/lib/settings/settings.generated";
import { cn } from "@/lib/utils";

const STEPS = [
    {
        id: "theme",
        title: "Pick a theme",
        description:
            "Gitau follows your system by default. Keep it automatic, or lock in light or dark.",
    },
    {
        id: "editor",
        title: "Choose your editor",
        description:
            "Gitau opens changed files in this editor. Pick one you already use, or set your own command.",
    },
    {
        id: "start",
        title: "Add a repository",
        description:
            "Open, clone, or create a repository to start working. You can also do this any time from the sidebar.",
    },
] as const;

type StepId = (typeof STEPS)[number]["id"];
type NavigationDirection = 1 | -1;

const STEP_VARIANTS = {
    initial: (direction: NavigationDirection) => ({
        opacity: 0,
        x: direction * 16,
        filter: "blur(4px)",
    }),
    animate: {
        opacity: 1,
        x: 0,
        filter: "blur(0px)",
    },
    exit: (direction: NavigationDirection) => ({
        opacity: 0,
        x: direction * -16,
        filter: "blur(4px)",
    }),
};

export function OnboardingDialog() {
    const { value: isCompleted, setValue: setIsCompleted } =
        useSetting("onboardingComplete");
    const [activeTab, setActiveTab] = useState<StepId>(STEPS[0].id);
    const [direction, setDirection] = useState<NavigationDirection>(1);

    const stepIndex = STEPS.findIndex((step) => step.id === activeTab);
    const step = STEPS[stepIndex];
    const isFirst = stepIndex === 0;
    const isLast = stepIndex === STEPS.length - 1;

    const goTo = (index: number) => {
        const next = STEPS[index];
        if (!next || index === stepIndex) return;

        startTransition(() => {
            setDirection(index > stepIndex ? 1 : -1);
            setActiveTab(next.id);
        });
    };

    return (
        <Dialog open={!isCompleted}>
            <DialogContent
                showCloseButton={false}
                className="h-[80vh] max-h-120 w-[90vw] max-w-3xl"
            >
                <Button
                    variant="ghost"
                    size="sm"
                    className="absolute end-3 top-3"
                    onClick={() => setIsCompleted(true)}
                >
                    Skip
                </Button>
                <DialogHeader className="items-center text-center">
                    <DialogTitle className="ui-selectable font-display text-4xl font-normal text-balance">
                        <Scritto value={step.title} />
                    </DialogTitle>
                    <DialogDescription className="ui-selectable mx-auto max-w-md text-balance">
                        <AnimatePresence
                            mode="wait"
                            initial={false}
                            custom={direction}
                        >
                            <motion.span
                                key={step.id}
                                className="block"
                                variants={STEP_VARIANTS}
                                initial="initial"
                                animate="animate"
                                exit="exit"
                                transition={{
                                    duration: 0.3,
                                    ease: EASE_SNAPPY,
                                }}
                            >
                                {step.description}
                            </motion.span>
                        </AnimatePresence>
                    </DialogDescription>
                </DialogHeader>
                <DialogPanel fill className="h-full">
                    <Tabs
                        className="mx-auto flex h-full w-full max-w-2xl flex-col"
                        value={activeTab}
                        onValueChange={(value) => {
                            if (typeof value === "string") {
                                goTo(
                                    STEPS.findIndex((step) => step.id === value)
                                );
                            }
                        }}
                    >
                        <motion.div
                            layout
                            className="flex min-h-0 flex-1 flex-col justify-center gap-6 py-2"
                            transition={{ duration: 0.2, ease: EASE_SNAPPY }}
                        >
                            <AnimatePresence
                                mode="wait"
                                initial={false}
                                custom={direction}
                            >
                                {activeTab === "theme" && (
                                    <motion.div
                                        key="theme"
                                        className="flex-none"
                                        variants={STEP_VARIANTS}
                                        initial="initial"
                                        animate="animate"
                                        exit="exit"
                                        transition={{
                                            duration: 0.2,
                                            ease: EASE_SNAPPY,
                                        }}
                                    >
                                        <ThemeStep />
                                    </motion.div>
                                )}
                                {activeTab === "editor" && (
                                    <motion.div
                                        key="editor"
                                        className="flex-none"
                                        variants={STEP_VARIANTS}
                                        initial="initial"
                                        animate="animate"
                                        exit="exit"
                                        transition={{
                                            duration: 0.2,
                                            ease: EASE_SNAPPY,
                                        }}
                                    >
                                        <EditorStep />
                                    </motion.div>
                                )}
                                {activeTab === "start" && (
                                    <motion.div
                                        key="start"
                                        className="flex-none"
                                        variants={STEP_VARIANTS}
                                        initial="initial"
                                        animate="animate"
                                        exit="exit"
                                        transition={{
                                            duration: 0.2,
                                            ease: EASE_SNAPPY,
                                        }}
                                    >
                                        <StartStep />
                                    </motion.div>
                                )}
                            </AnimatePresence>
                        </motion.div>
                        <div className="grid grid-cols-[1fr_auto_1fr] items-center gap-4 pt-4">
                            <Button
                                variant="ghost"
                                className={cn(
                                    "justify-self-start",
                                    isFirst && "invisible"
                                )}
                                disabled={isFirst}
                                onClick={() => goTo(stepIndex - 1)}
                            >
                                <ChevronLeft /> Back
                            </Button>
                            <TabsList
                                variant="dot"
                                className="justify-self-center"
                            >
                                {STEPS.map((item) => (
                                    <TabsTab
                                        key={item.id}
                                        value={item.id}
                                        aria-label={item.title}
                                        className="size-2 rounded-full bg-muted-foreground/72 p-0 sm:size-2"
                                    />
                                ))}
                            </TabsList>
                            <Button
                                className="justify-self-end"
                                onClick={
                                    isLast
                                        ? () => setIsCompleted(true)
                                        : () => goTo(stepIndex + 1)
                                }
                            >
                                {isLast ? "Finish" : "Continue"}
                                {!isLast && <ChevronRight />}
                            </Button>
                        </div>
                    </Tabs>
                </DialogPanel>
            </DialogContent>
        </Dialog>
    );
}

const THEME_OPTIONS: { value: ThemeMode; label: string }[] = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
];

function ThemeStep() {
    const { value: theme, setValue: setTheme } = useSetting("theme");

    return (
        <div className="grid grid-cols-3 gap-3">
            {THEME_OPTIONS.map((option) => {
                const selected = theme === option.value;
                return (
                    <button
                        key={option.value}
                        type="button"
                        aria-pressed={selected}
                        onClick={() => setTheme(option.value)}
                        className={cn(
                            "flex cursor-pointer flex-col gap-2 rounded-xl border p-1.5 text-left transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring",
                            selected
                                ? "border-primary/40 bg-accent"
                                : "border-border hover:border-primary/24 hover:bg-accent/48"
                        )}
                    >
                        <div className="overflow-hidden rounded-lg">
                            <AppPreview mode={option.value} />
                        </div>
                        <span className="flex items-center px-1 pb-1 text-sm font-medium">
                            {option.label}
                            {selected && (
                                <Check className="ms-auto size-4 text-primary" />
                            )}
                        </span>
                    </button>
                );
            })}
        </div>
    );
}

const CHOICE_CARD =
    "flex cursor-pointer items-center gap-3 rounded-xl border p-3 text-left transition-colors outline-none focus-visible:ring-2 focus-visible:ring-ring";
const CHOICE_CARD_SELECTED = "border-primary/40 bg-accent";
const CHOICE_CARD_IDLE =
    "border-border hover:border-primary/24 hover:bg-accent/48";
const CHOICE_TILE =
    "flex size-9 shrink-0 items-center justify-center rounded-lg border bg-muted";

const CUSTOM_EDITOR_ICON = (
    <svg
        viewBox="0 0 24 24"
        className="size-5"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
    >
        <rect x="3" y="4" width="18" height="16" rx="2" />
        <path d="m7 9 3 3-3 3" />
        <path d="M13 15h4" />
    </svg>
);

const EDITOR_PRESETS: {
    id: string;
    name: string;
    command: string;
    viewBox: string;
    color: string;
    path: string;
}[] = [
    {
        id: "vscode",
        name: "VS Code",
        command: "code",
        viewBox: "0 0 128 128",
        color: "#007ACC",
        path: "M90.767 127.126a7.968 7.968 0 0 0 6.35-.244l26.353-12.681a8 8 0 0 0 4.53-7.209V21.009a8 8 0 0 0-4.53-7.21L97.117 1.12a7.97 7.97 0 0 0-9.093 1.548l-50.45 46.026L15.6 32.013a5.328 5.328 0 0 0-6.807.302l-7.048 6.411a5.335 5.335 0 0 0-.006 7.888L20.796 64 1.74 81.387a5.336 5.336 0 0 0 .006 7.887l7.048 6.411a5.327 5.327 0 0 0 6.807.303l21.974-16.68 50.45 46.025a7.96 7.96 0 0 0 2.743 1.793Zm5.252-92.183L57.74 64l38.28 29.058V34.943Z",
    },
    {
        id: "cursor",
        name: "Cursor",
        command: "cursor",
        viewBox: "0 0 24 24",
        color: "currentColor",
        path: "M11.503.131 1.891 5.678a.84.84 0 0 0-.42.726v11.188c0 .3.162.575.42.724l9.609 5.55a1 1 0 0 0 .998 0l9.61-5.55a.84.84 0 0 0 .42-.724V6.404a.84.84 0 0 0-.42-.726L12.497.131a1.01 1.01 0 0 0-.996 0M2.657 6.338h18.55c.263 0 .43.287.297.515L12.23 22.918c-.062.107-.229.064-.229-.06V12.335a.59.59 0 0 0-.295-.51l-9.11-5.257c-.109-.063-.064-.23.061-.23",
    },
    {
        id: "zed",
        name: "Zed",
        command: "zed",
        viewBox: "0 0 24 24",
        color: "#084CCF",
        path: "M2.25 1.5a.75.75 0 0 0-.75.75v16.5H0V2.25A2.25 2.25 0 0 1 2.25 0h20.095c1.002 0 1.504 1.212.795 1.92L10.764 14.298h3.486V12.75h1.5v1.922a1.125 1.125 0 0 1-1.125 1.125H9.264l-2.578 2.578h11.689V9h1.5v9.375a1.5 1.5 0 0 1-1.5 1.5H5.185L2.562 22.5H21.75a.75.75 0 0 0 .75-.75V5.25H24v16.5A2.25 2.25 0 0 1 21.75 24H1.655C.653 24 .151 22.788.86 22.08L13.19 9.75H9.75v1.5h-1.5V9.375A1.125 1.125 0 0 1 9.375 8.25h5.314l2.625-2.625H5.625V15h-1.5V5.625a1.5 1.5 0 0 1 1.5-1.5h13.19L21.438 1.5z",
    },
    {
        id: "sublime",
        name: "Sublime Text",
        command: "subl",
        viewBox: "0 0 24 24",
        color: "#FF9800",
        path: "M20.953.004a.397.397 0 0 0-.18.017L3.225 5.585c-.175.055-.323.214-.402.398a.42.42 0 0 0-.06.22v5.726a.42.42 0 0 0 .06.22c.079.183.227.341.402.397l7.454 2.364-7.454 2.363c-.255.08-.463.374-.463.655v5.688c0 .282.208.444.463.363l17.55-5.565c.237-.075.426-.336.452-.6.003-.022.013-.04.013-.065V12.06c0-.281-.208-.575-.463-.656L13.4 9.065l7.375-2.339c.255-.08.462-.375.462-.656V.384c0-.211-.117-.355-.283-.38z",
    },
    {
        id: "neovim",
        name: "Neovim",
        command: "nvim",
        viewBox: "0 0 24 24",
        color: "#57A143",
        path: "M2.214 4.954v13.615L7.655 24V10.314L3.312 3.845 2.214 4.954zm4.999 17.98l-4.557-4.548V5.136l.59-.596 3.967 5.908v12.485zm14.573-4.457l-.862.937-4.24-6.376V0l5.068 5.092.034 13.385zM7.431.001l12.998 19.835-3.637 3.637L3.787 3.683 7.43 0z",
    },
];

function EditorStep() {
    const { value, setValue } = useSetting("editorCommand");
    const [custom, setCustom] = useState(
        () =>
            value !== "" &&
            !EDITOR_PRESETS.some((preset) => preset.command === value)
    );

    return (
        <div className="grid gap-3">
            <div className="grid grid-cols-2 gap-3">
                {EDITOR_PRESETS.map((preset) => {
                    const selected = !custom && value === preset.command;
                    return (
                        <button
                            key={preset.id}
                            type="button"
                            aria-pressed={selected}
                            onClick={() => {
                                setCustom(false);
                                setValue(preset.command);
                            }}
                            className={cn(
                                CHOICE_CARD,
                                selected
                                    ? CHOICE_CARD_SELECTED
                                    : CHOICE_CARD_IDLE
                            )}
                        >
                            <span className={CHOICE_TILE}>
                                <svg
                                    viewBox={preset.viewBox}
                                    className="size-5"
                                    aria-hidden="true"
                                >
                                    <path
                                        d={preset.path}
                                        fill={preset.color}
                                        fillRule="evenodd"
                                        clipRule="evenodd"
                                    />
                                </svg>
                            </span>
                            <span className="min-w-0 flex-1">
                                <span className="block text-sm font-medium">
                                    {preset.name}
                                </span>
                                <span className="block font-mono text-xs text-muted-foreground">
                                    {preset.command}
                                </span>
                            </span>
                            {selected && (
                                <Check className="size-4 shrink-0 text-primary" />
                            )}
                        </button>
                    );
                })}
                <button
                    type="button"
                    aria-pressed={custom}
                    onClick={() => setCustom(true)}
                    className={cn(
                        CHOICE_CARD,
                        custom ? CHOICE_CARD_SELECTED : CHOICE_CARD_IDLE
                    )}
                >
                    <span className={cn(CHOICE_TILE, "text-muted-foreground")}>
                        {CUSTOM_EDITOR_ICON}
                    </span>
                    <span className="min-w-0 flex-1">
                        <span className="block text-sm font-medium">
                            Custom
                        </span>
                        <span className="block font-mono text-xs text-muted-foreground">
                            your own command
                        </span>
                    </span>
                    {custom && (
                        <Check className="size-4 shrink-0 text-primary" />
                    )}
                </button>
            </div>
            <div className="space-y-1.5">
                <Input
                    value={value}
                    disabled={!custom}
                    placeholder="code --reuse-window"
                    aria-label="Editor command"
                    onChange={(event) => setValue(event.target.value)}
                />
                <p className="text-xs text-muted-foreground">
                    The command Gitau runs to open a file, for example the
                    editor's CLI name or a full path to its executable.
                </p>
            </div>
        </div>
    );
}

const START_ACTIONS: {
    command: keyof AppCommandActions;
    icon: LucideIcon;
    title: string;
    description: string;
}[] = [
    {
        command: "addRepository",
        icon: FolderOpen,
        title: "Open a repository",
        description: "Pick a folder that is already a Git repository.",
    },
    {
        command: "cloneRepository",
        icon: Download,
        title: "Clone a repository",
        description: "Copy a remote repository onto your machine.",
    },
    {
        command: "newRepository",
        icon: FolderPlus,
        title: "Create a repository",
        description: "Start a new Git repository from scratch.",
    },
];

function StartStep() {
    const { invoke } = useAppCommands();
    const { setValue: setIsCompleted } = useSetting("onboardingComplete");

    const start = (command: keyof AppCommandActions) => {
        setIsCompleted(true);
        invoke(command);
    };

    return (
        <div className="grid gap-3">
            {START_ACTIONS.map((action) => (
                <button
                    key={action.command}
                    type="button"
                    onClick={() => start(action.command)}
                    className={cn(CHOICE_CARD, CHOICE_CARD_IDLE)}
                >
                    <span className={cn(CHOICE_TILE, "text-muted-foreground")}>
                        <action.icon className="size-5" />
                    </span>
                    <span className="min-w-0 flex-1">
                        <span className="block text-sm font-medium">
                            {action.title}
                        </span>
                        <span className="block text-sm text-muted-foreground">
                            {action.description}
                        </span>
                    </span>
                    <ChevronRight className="size-4 shrink-0 text-muted-foreground" />
                </button>
            ))}
        </div>
    );
}

// useId emits characters (":", "«") that break SVG url(#…) references.
function useSafeId(): string {
    return useId().replace(/[^a-zA-Z0-9]/g, "");
}

const BAR = 16;
const FILLET = 9;

function WindowMock() {
    const clipId = useSafeId();

    return (
        <g>
            <defs>
                <clipPath id={clipId}>
                    <rect x="0.5" y="0.5" width="159" height="99" rx="8" />
                </clipPath>
            </defs>
            <g clipPath={`url(#${clipId})`}>
                <rect
                    x="0"
                    y="0"
                    width="160"
                    height="100"
                    style={{ fill: "var(--sidebar)" }}
                />
                <path
                    d={`M${BAR} ${BAR + FILLET} A${FILLET} ${FILLET} 0 0 1 ${BAR + FILLET} ${BAR} H160 V100 H${BAR} Z`}
                    style={{ fill: "var(--background)" }}
                />
            </g>
            <rect
                x="0.5"
                y="0.5"
                width="159"
                height="99"
                rx="8"
                fill="none"
                style={{ stroke: "var(--sidebar-border)" }}
            />
        </g>
    );
}

function AppPreview({ mode }: { mode: ThemeMode }) {
    if (mode === "system") return <SystemPreview />;

    return (
        <div className={mode}>
            <svg
                viewBox="0 0 160 100"
                className="block h-auto w-full"
                aria-hidden="true"
            >
                <WindowMock />
            </svg>
        </div>
    );
}

function SystemPreview() {
    return (
        <div className="relative">
            <div className="dark">
                <svg
                    viewBox="0 0 160 100"
                    className="block h-auto w-full"
                    aria-hidden="true"
                >
                    <WindowMock />
                </svg>
            </div>
            <div
                className="light absolute inset-0"
                style={{ clipPath: "inset(0 50% 0 0)" }}
            >
                <svg
                    viewBox="0 0 160 100"
                    className="block h-auto w-full"
                    aria-hidden="true"
                >
                    <WindowMock />
                </svg>
            </div>
            <div className="absolute inset-y-0 left-1/2 w-px bg-border" />
        </div>
    );
}
