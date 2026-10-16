// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const tauri = vi.hoisted(() => ({
    getVersion: vi.fn(),
    getTauriVersion: vi.fn(),
    platform: vi.fn(),
    osVersion: vi.fn(),
    arch: vi.fn(),
}));

vi.mock("@tauri-apps/api/app", () => ({
    getVersion: tauri.getVersion,
    getTauriVersion: tauri.getTauriVersion,
}));

vi.mock("@tauri-apps/plugin-os", () => ({
    platform: tauri.platform,
    version: tauri.osVersion,
    arch: tauri.arch,
}));

import { ReleaseInfoCard } from "@/components/settings/release-info-card";
import { updaterController } from "@/lib/updates/update-manager";

describe("ReleaseInfoCard", () => {
    beforeEach(() => {
        vi.clearAllMocks();
        updaterController.store.setState(() => ({ status: "idle" }));
        updaterController.checkedAtStore.setState(() => null);
    });

    afterEach(() => {
        cleanup();
    });

    it("renders release, build, and update details", async () => {
        tauri.getVersion.mockResolvedValue("1.2.3");
        tauri.getTauriVersion.mockResolvedValue("2.4.5");
        tauri.platform.mockReturnValue("windows");
        tauri.osVersion.mockReturnValue("10.0.19045");
        tauri.arch.mockReturnValue("x86_64");
        updaterController.store.setState(() => ({ status: "none" }));
        updaterController.checkedAtStore.setState(() => Date.now());

        render(<ReleaseInfoCard />);

        expect(await screen.findByText("1.2.3")).toBeDefined();
        expect(screen.getByText("Stable")).toBeDefined();
        expect(screen.getByText("2.4.5")).toBeDefined();
        expect(screen.getByText("Windows 10.0.19045")).toBeDefined();
        expect(screen.getByText("x64")).toBeDefined();
        expect(screen.getByText("Up to date")).toBeDefined();
        expect(screen.getByText("now")).toBeDefined();
        expect(screen.getByText("Commit")).toBeDefined();
        expect(screen.getByText("Built")).toBeDefined();
    });

    it("marks a prerelease build as the preview channel", async () => {
        tauri.getVersion.mockResolvedValue("1.2.3-preview.4");
        tauri.getTauriVersion.mockResolvedValue("2.4.5");
        tauri.platform.mockReturnValue("macos");
        tauri.osVersion.mockReturnValue("15.0");
        tauri.arch.mockReturnValue("aarch64");

        render(<ReleaseInfoCard />);

        expect(await screen.findByText("Preview")).toBeDefined();
        expect(screen.getByText("ARM64")).toBeDefined();
        expect(screen.getByText("Not checked")).toBeDefined();
    });

    it("reports when the release APIs are unavailable", async () => {
        tauri.getVersion.mockRejectedValue(
            new Error("not running under tauri")
        );

        render(<ReleaseInfoCard />);

        expect(await screen.findByText("Unavailable")).toBeDefined();
    });
});
