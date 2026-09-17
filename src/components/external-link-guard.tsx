"use client";

import { useEffect } from "react";

const EXTERNAL_PROTOCOLS = new Set(["http:", "https:", "mailto:", "tel:"]);

function isModifiedClick(event: MouseEvent) {
    return (
        event.defaultPrevented ||
        event.button !== 0 ||
        event.metaKey ||
        event.ctrlKey ||
        event.shiftKey ||
        event.altKey
    );
}

function shouldOpenExternally(anchor: HTMLAnchorElement) {
    const href = anchor.getAttribute("href");

    if (!href || href.startsWith("#") || anchor.hasAttribute("download")) {
        return false;
    }

    try {
        const url = new URL(anchor.href, window.location.href);

        if (!EXTERNAL_PROTOCOLS.has(url.protocol)) {
            return false;
        }

        if (url.protocol === "mailto:" || url.protocol === "tel:") {
            return true;
        }

        return url.origin !== window.location.origin;
    } catch {
        return false;
    }
}

async function openExternalLink(href: string) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(href);
}

export function ExternalLinkGuard() {
    useEffect(() => {
        function handleClick(event: MouseEvent) {
            if (isModifiedClick(event)) {
                return;
            }

            const target = event.target;

            if (!(target instanceof Element)) {
                return;
            }

            const anchor = target.closest("a[href]");

            if (!(anchor instanceof HTMLAnchorElement)) {
                return;
            }

            if (!shouldOpenExternally(anchor)) {
                return;
            }

            event.preventDefault();
            void openExternalLink(anchor.href);
        }

        document.addEventListener("click", handleClick, true);

        return () => {
            document.removeEventListener("click", handleClick, true);
        };
    }, []);

    return null;
}
