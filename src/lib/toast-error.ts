import { toastManager } from "@/components/ui/toast";

export interface ToastErrorAction {
    label: string;
    onClick: () => void;
}

export function toastError(
    title: string,
    error: unknown,
    action?: ToastErrorAction
): void {
    const id = toastManager.add({
        title,
        description: error instanceof Error ? error.message : String(error),
        type: "error",
        actionProps: action && {
            children: action.label,
            onClick: () => {
                toastManager.close(id);
                action.onClick();
            },
        },
    });
}
