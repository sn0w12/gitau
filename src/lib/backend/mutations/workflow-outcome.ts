import type { WorkflowOutcome } from "@/lib/backend/protocol";

export interface WorkflowSummary {
    message: string;
    /** Non-empty when the operation stopped on merge conflicts. */
    conflictPaths: string[];
}

/**
 * Turns a backend WorkflowOutcome into toast-ready copy. Pull/push flows can
 * surface several of these variants; every case must render something
 * actionable rather than a bare success/failure binary.
 */
export function summarizeWorkflowOutcome(
    outcome: WorkflowOutcome
): WorkflowSummary {
    switch (outcome.outcome) {
        case "alreadyUpToDate":
            return { message: "Already up to date", conflictPaths: [] };
        case "fastForwarded":
            return {
                message: "Fast-forwarded successfully",
                conflictPaths: [],
            };
        case "merged":
            return { message: "Merged successfully", conflictPaths: [] };
        case "conflicted":
            return {
                message:
                    outcome.paths.length === 1
                        ? "1 conflicted file needs resolving"
                        : `${outcome.paths.length} conflicted files need resolving`,
                conflictPaths: outcome.paths,
            };
        case "started":
            return {
                message: `Started (${outcome.totalSteps} steps remaining)`,
                conflictPaths: [],
            };
        case "progressed":
            return {
                message: `${outcome.remaining} steps remaining`,
                conflictPaths: [],
            };
        case "finished":
            return { message: "Operation finished", conflictPaths: [] };
        case "aborted":
            return { message: "Operation aborted", conflictPaths: [] };
    }
}
