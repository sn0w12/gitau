import { createStore } from "@tanstack/store";

interface ConnectionState {
    /**
     * The connection the account-scoped pages scope to, or null to follow
     * the first live connection.
     */
    activeKey: string | null;
}

export const connectionStore = createStore<ConnectionState>({
    activeKey: null,
});

/** Identifies a connection in the store. Mirrors the backend's token id. */
export function connectionKey(provider: string, host: string): string {
    return `${provider}:${host}`;
}

export function setActiveConnection(provider: string, host: string): void {
    const next = connectionKey(provider, host);
    connectionStore.setState((state) =>
        state.activeKey === next ? state : { ...state, activeKey: next }
    );
}

export function clearActiveConnection(): void {
    connectionStore.setState((state) =>
        state.activeKey === null ? state : { ...state, activeKey: null }
    );
}
