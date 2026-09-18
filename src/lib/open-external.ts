/**
 * Opens an external URL in the system browser, mirroring the global
 * ExternalLinkGuard. The opener is imported lazily so unit tests and
 * non-Tauri contexts never pay for it up front.
 */
export async function openExternal(href: string): Promise<void> {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(href);
}
