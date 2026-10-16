import { useSelector } from "@tanstack/react-store";
import { getTauriVersion, getVersion } from "@tauri-apps/api/app";
import { arch, platform, version as osVersion } from "@tauri-apps/plugin-os";
import { useEffect, useState } from "react";

import type { UpdateState } from "@/lib/updates/update-controller";
import { updaterController } from "@/lib/updates/update-manager";
import { formatRelativeDate } from "@/lib/utils";

import { FramePanel } from "../ui/frame";
import { Separator } from "../ui/separator";

const PLATFORM_LABELS: Record<string, string> = {
    windows: "Windows",
    macos: "macOS",
    linux: "Linux",
    freebsd: "FreeBSD",
    dragonfly: "DragonFly BSD",
    netbsd: "NetBSD",
    openbsd: "OpenBSD",
    solaris: "Solaris",
    android: "Android",
    ios: "iOS",
};

const ARCH_LABELS: Record<string, string> = {
    x86: "x86",
    x86_64: "x64",
    arm: "ARM",
    aarch64: "ARM64",
    mips: "MIPS",
    mips64: "MIPS64",
    powerpc: "PowerPC",
    powerpc64: "PowerPC64",
    riscv64: "RISC-V 64",
    s390x: "s390x",
    sparc64: "SPARC64",
};

interface ReleaseInfo {
    version: string;
    tauriVersion: string;
    platform: string;
    osVersion: string;
    arch: string;
}

interface ReleaseRow {
    label: string;
    value: string;
}

async function readReleaseInfo(): Promise<ReleaseInfo> {
    const [version, tauriVersion] = await Promise.all([
        getVersion(),
        getTauriVersion(),
    ]);
    const os = platform();
    const cpu = arch();
    return {
        version,
        tauriVersion,
        platform: PLATFORM_LABELS[os] ?? os,
        osVersion: osVersion(),
        arch: ARCH_LABELS[cpu] ?? cpu,
    };
}

/** A prerelease suffix marks the preview updater channel. */
function releaseChannel(version: string): "Stable" | "Preview" {
    return version.includes("-") ? "Preview" : "Stable";
}

function updateStatusLabel(state: UpdateState): string {
    switch (state.status) {
        case "idle":
            return "Not checked";
        case "checking":
            return "Checking…";
        case "none":
            return "Up to date";
        case "available":
            return `Version ${state.info.version} available`;
        case "downloading":
            return `Downloading ${state.info.version}…`;
        case "installing":
            return "Installing…";
        case "failed":
            return "Check failed";
    }
}

function formatBuildDate(iso: string): string {
    const date = new Date(iso);
    if (Number.isNaN(date.getTime())) return iso;
    return date.toLocaleDateString(undefined, {
        year: "numeric",
        month: "short",
        day: "numeric",
    });
}

export function ReleaseInfoCard() {
    const [info, setInfo] = useState<ReleaseInfo | null>(null);
    const updateState = useSelector(updaterController.store, (state) => state);
    const checkedAt = useSelector(
        updaterController.checkedAtStore,
        (value) => value
    );

    useEffect(() => {
        let active = true;
        readReleaseInfo()
            .then((next) => {
                if (active) setInfo(next);
            })
            .catch(() => {
                if (active) setInfo(null);
            });
        return () => {
            active = false;
        };
    }, []);

    const rows: ReleaseRow[] = [
        { label: "Version", value: info?.version ?? "Unavailable" },
    ];
    if (info) {
        rows.push({ label: "Channel", value: releaseChannel(info.version) });
    }
    rows.push(
        { label: "Commit", value: __BUILD_COMMIT__ },
        { label: "Built", value: formatBuildDate(__BUILD_DATE__) }
    );
    if (info) {
        rows.push(
            { label: "Tauri", value: info.tauriVersion },
            {
                label: "Platform",
                value: info.osVersion
                    ? `${info.platform} ${info.osVersion}`
                    : info.platform,
            },
            { label: "Architecture", value: info.arch }
        );
    }
    rows.push({ label: "Updates", value: updateStatusLabel(updateState) });
    if (checkedAt !== null) {
        rows.push({
            label: "Last checked",
            value: formatRelativeDate(checkedAt),
        });
    }

    return (
        <FramePanel data-slot="settings-release">
            <h3 className="pb-1 text-xl font-semibold">Release</h3>
            <Separator className="mb-2" />
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
                {rows.map((row) => (
                    <div key={row.label} className="contents">
                        <dt className="text-muted-foreground">{row.label}</dt>
                        <dd className="tabular-nums">{row.value}</dd>
                    </div>
                ))}
            </dl>
        </FramePanel>
    );
}
