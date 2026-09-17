import {
    createContext,
    useCallback,
    useContext,
    useMemo,
    useRef,
    useState,
} from "react";
import type { ReactNode } from "react";

export type AppCommandAction = () => void | Promise<void>;

export interface AppCommandActions {
    newRepository: AppCommandAction;
    addRepository: AppCommandAction;
    cloneRepository: AppCommandAction;
}

type RegisteredActions = Partial<AppCommandActions>;

interface AppCommandContextValue {
    actions: RegisteredActions;
    register: <K extends keyof AppCommandActions>(
        key: K,
        action: AppCommandActions[K]
    ) => () => void;
    invoke: <K extends keyof AppCommandActions>(key: K) => void;
}

const AppCommandContext = createContext<AppCommandContextValue | null>(null);

export function AppCommandProvider({ children }: { children: ReactNode }) {
    const [actions, setActions] = useState<RegisteredActions>({});
    const actionsRef = useRef(actions);
    // oxlint-disable-next-line react/refs
    actionsRef.current = actions;

    const register = useCallback(
        <K extends keyof AppCommandActions>(
            key: K,
            action: AppCommandActions[K]
        ) => {
            setActions((current) => ({ ...current, [key]: action }));
            return () => {
                setActions((current) => {
                    if (current[key] !== action) return current;
                    const next = { ...current };
                    delete next[key];
                    return next;
                });
            };
        },
        []
    );

    const invoke = useCallback(<K extends keyof AppCommandActions>(key: K) => {
        void actionsRef.current[key]?.();
    }, []);

    const value = useMemo(
        () => ({ actions, register, invoke }),
        [actions, register, invoke]
    );
    return (
        <AppCommandContext.Provider value={value}>
            {children}
        </AppCommandContext.Provider>
    );
}

export function useAppCommands(): AppCommandContextValue {
    const context = useContext(AppCommandContext);
    if (!context)
        throw new Error(
            "useAppCommands must be used within AppCommandProvider"
        );
    return context;
}
