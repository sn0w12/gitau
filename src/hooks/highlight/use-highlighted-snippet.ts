import { useQuery } from "@tanstack/react-query";

import { useAppServices } from "@/contexts/services-context";
import { useSettingValue } from "@/hooks/settings/use-setting";
import { snippetQuery } from "@/lib/backend/queries/highlight-queries";

/** Fence tags with no grammar; the backend agrees and stays plain. */
const PLAIN_LANGUAGES = new Set(["", "plaintext", "plain", "text", "txt"]);

/**
 * Backend syntax highlighting for one code fence. Resolves plain
 * synchronously for unhighlightable tags; anything else upgrades from
 * plain text once the highlight query lands.
 */
export function useHighlightedSnippet(
    language: string | undefined,
    text: string | null
) {
    const { backend } = useAppServices();
    const lightTheme = useSettingValue("syntaxThemeLight");
    const darkTheme = useSettingValue("syntaxThemeDark");
    const lang = (language ?? "").trim().toLowerCase();
    const query = useQuery(
        snippetQuery(
            { backend },
            lang,
            text ?? "",
            lightTheme,
            darkTheme,
            text != null && text.length > 0 && !PLAIN_LANGUAGES.has(lang)
        )
    );
    return query;
}
