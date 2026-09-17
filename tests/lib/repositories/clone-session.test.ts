import { beforeEach, describe, expect, it } from "vitest";

import type { CloneEvent } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import { CloneSessionController } from "@/lib/repositories/clone-session";
import { operationStore } from "@/stores/operation-store";

function fakeBackend() {
    let onEvent: ((event: CloneEvent) => void) | null = null;
    let startValue: Result<number> | null = null;
    const cancelCalls: number[] = [];
    const startCalls: unknown[] = [];

    const backend = {
        remotes: {
            clone: async (
                _input: unknown,
                callback: (event: CloneEvent) => void
            ) => {
                startCalls.push(_input);
                onEvent = callback;
                return (
                    startValue ?? {
                        ok: true,
                        value: 7,
                    }
                );
            },
        },
        operations: {
            cancel: async (operationId: number) => {
                cancelCalls.push(operationId);
                return { ok: true, value: true } as Result<boolean>;
            },
        },
    } as unknown as BackendClient;

    return {
        backend,
        emit: (event: CloneEvent) => onEvent?.(event),
        setStartValue: (value: Result<number>) => {
            startValue = value;
        },
        cancelCalls,
        startCalls,
    };
}

const INPUT = { url: "https://example.test/a/b.git", destination: "/tmp/b" };
describe("CloneSessionController", () => {
    beforeEach(() => {
        // The operation store is process-global; start each case clean.
        operationStore.setState(() => ({ operations: new Map() }));
    });

    it("tracks progress and resolves completed with the repo path", async () => {
        const fake = fakeBackend();
        const controller = new CloneSessionController(fake.backend);

        const pending = controller.start(INPUT);
        // Interleave stream traffic with the in-flight start().
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }
        fake.emit({
            event: "progress",
            operationId: 7,
            phase: "receiving",
            progress: 0.5,
            objectsReceived: 10,
            objectsTotal: 20,
            receivedBytes: 2048,
        });
        expect(controller.store.state.status).toBe("running");
        expect(controller.store.state.progress?.objectsTotal).toBe(20);
        expect(controller.store.state.operationId).toBe(7);

        fake.emit({
            event: "completed",
            operationId: 7,
            repoPath: "/tmp/b",
        });
        const outcome = await pending;

        expect(outcome).toEqual({ status: "completed", repoPath: "/tmp/b" });
        expect(controller.store.state.status).toBe("completed");
        const record = operationStore.state.operations.get(7);
        expect(record?.phase).toBe("completed");
    });

    it("maps failed events to a typed error", async () => {
        const fake = fakeBackend();
        const controller = new CloneSessionController(fake.backend);
        const pending = controller.start(INPUT);
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }

        fake.emit({
            event: "failed",
            operationId: 7,
            code: "network",
            message: "connection reset",
        });
        const outcome = await pending;

        expect(outcome.status).toBe("failed");
        if (outcome.status === "failed") {
            expect(outcome.error.code).toBe("network");
            expect(outcome.error.message).toBe("connection reset");
        }
        expect(operationStore.state.operations.get(7)?.phase).toBe("failed");
    });

    it("cancels through the backend operation registry", async () => {
        const fake = fakeBackend();
        const controller = new CloneSessionController(fake.backend);
        const pending = controller.start(INPUT);
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }

        await controller.cancel();
        expect(fake.cancelCalls).toEqual([7]);

        fake.emit({ event: "cancelled", operationId: 7 });
        const outcome = await pending;
        expect(outcome.status).toBe("cancelled");
        expect(operationStore.state.operations.get(7)?.phase).toBe("cancelled");
    });

    it("survives a terminal event that races past invoke resolution", async () => {
        // The fake delivers the terminal event synchronously inside clone(),
        // before start() can register the operation id.
        let fire: ((event: CloneEvent) => void) | null = null;
        const backend = {
            remotes: {
                clone: async (
                    _input: unknown,
                    callback: (event: CloneEvent) => void
                ) => {
                    fire = callback;
                    fire({
                        event: "completed",
                        operationId: 3,
                        repoPath: "/r",
                    });
                    return { ok: true, value: 3 } as Result<number>;
                },
            },
            operations: { cancel: async () => ({ ok: true, value: true }) },
        } as unknown as BackendClient;

        const controller = new CloneSessionController(backend);
        const outcome = await controller.start(INPUT);
        expect(outcome).toEqual({ status: "completed", repoPath: "/r" });
    });

    it("ignores duplicate terminal events after settling", async () => {
        const fake = fakeBackend();
        const controller = new CloneSessionController(fake.backend);
        const pending = controller.start(INPUT);
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }

        fake.emit({ event: "cancelled", operationId: 7 });
        expect(await pending).toEqual({ status: "cancelled" });

        fake.emit({
            event: "failed",
            operationId: 7,
            code: "network",
            message: "late",
        });
        expect(controller.store.state.status).toBe("cancelled");
        expect(controller.store.state.error).toBeUndefined();
    });

    it("reports failures thrown by the start call itself", async () => {
        const fake = fakeBackend();
        fake.setStartValue({
            ok: false,
            error: new GitBackendError({
                code: "invalidInput",
                message: "bad url",
                detail: undefined,
                retryable: false,
            }),
        });

        const controller = new CloneSessionController(fake.backend);
        const outcome = await controller.start(INPUT);

        expect(outcome.status).toBe("failed");
        expect(controller.store.state.status).toBe("failed");
        // No operation ever began, so none is finished.
        expect(operationStore.state.operations.get(7)).toBeUndefined();
    });

    it("ignores a second start while a clone is running", async () => {
        const fake = fakeBackend();
        const controller = new CloneSessionController(fake.backend);
        void controller.start(INPUT);
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }

        const outcome = await controller.start(INPUT);
        expect(outcome.status).toBe("cancelled");
        expect(controller.store.state.status).toBe("running");
        // Only the first start reached the backend.
        expect(fake.startCalls.length).toBe(1);
    });

    it("cancel without a running clone is a no-op", async () => {
        const fake = fakeBackend();
        const controller = new CloneSessionController(fake.backend);

        // StrictMode's mount/unmount cycle cancels fresh controllers before
        // anything ran; that must be harmless and startable afterwards.
        await controller.cancel();
        expect(fake.cancelCalls).toEqual([]);

        const pending = controller.start(INPUT);
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }
        fake.emit({
            event: "completed",
            operationId: 7,
            repoPath: "/tmp/b",
        });
        const outcome = await pending;
        expect(outcome.status).toBe("completed");
    });

    it("reset returns to pristine idle and releases a pending start", async () => {
        const fake = fakeBackend();
        const controller = new CloneSessionController(fake.backend);
        const pending = controller.start(INPUT);
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }
        expect(controller.store.state.status).toBe("running");

        controller.reset();

        expect(controller.store.state.status).toBe("idle");
        expect(await pending).toEqual({ status: "cancelled" });

        // A fresh start works afterwards and re-seeds the store.
        const second = controller.start(INPUT);
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }
        fake.emit({
            event: "completed",
            operationId: 7,
            repoPath: "/tmp/b",
        });
        expect(await second).toEqual({
            status: "completed",
            repoPath: "/tmp/b",
        });
        expect(fake.startCalls.length).toBe(2);
    });

    it("cancels an in-flight clone when its dialog unmounts", async () => {
        const fake = fakeBackend();
        const controller = new CloneSessionController(fake.backend);
        void controller.start(INPUT);
        for (let i = 0; i < 5; i++) {
            await Promise.resolve();
        }

        await controller.cancel();
        await new Promise((resolve) => setTimeout(resolve, 0));
        expect(fake.cancelCalls).toEqual([7]);

        // The orphaned controller still folds in the terminal event; nothing
        // may throw or hang on it after the dialog is gone.
        fake.emit({
            event: "completed",
            operationId: 7,
            repoPath: "/tmp/b",
        });
        expect(controller.store.state.status).toBe("completed");
    });
});
