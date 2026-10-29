/** Path part of a resolved location, without query string or hash. */
export function pathnameOf(href: string): string {
    const end = href.search(/[?#]/);
    return end === -1 ? href : href.slice(0, end);
}

/** True for the repo page and the nested issue and pull routes below it. */
export function isRepoPathname(pathname: string): boolean {
    return pathname === "/repo" || pathname.startsWith("/repo/");
}
