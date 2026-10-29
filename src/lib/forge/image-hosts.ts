/**
 * Whether an image may be loaded directly. A host matches a suffix exactly or
 * as a subdomain.
 */
export function isTrustedImageSource(
    source: string,
    hosts: readonly string[]
): boolean {
    let url: URL;
    try {
        url = new URL(source);
    } catch {
        return false;
    }
    if (url.protocol !== "https:") return false;
    const host = url.hostname.toLowerCase();
    return hosts.some((entry) => {
        const suffix = entry.toLowerCase();
        return host === suffix || host.endsWith(`.${suffix}`);
    });
}
