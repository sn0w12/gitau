import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";

import {
    Combobox,
    ComboboxEmpty,
    ComboboxInput,
    ComboboxItem,
    ComboboxList,
    ComboboxPopup,
} from "@/components/ui/combobox";
import { useAppServices } from "@/contexts/services-context";
import { systemFontsQuery } from "@/lib/backend/queries/font-queries";
import { fontChoices, fontFamilyStack } from "@/lib/fonts";

export function FontControl({
    value,
    fallback,
    onChange,
}: {
    value: string;
    fallback: string;
    onChange: (value: string) => void;
}) {
    const { backend } = useAppServices();
    const { data: systemFonts } = useQuery(systemFontsQuery({ backend }));
    const families = useMemo(
        () => fontChoices(systemFonts ?? [], value),
        [systemFonts, value]
    );

    return (
        <Combobox
            items={families}
            value={value}
            onValueChange={(next) => {
                if (typeof next === "string") onChange(next);
            }}
        >
            <ComboboxInput
                aria-label="Font family"
                className="w-56"
                placeholder="Search fonts..."
                showTrigger
                style={{ fontFamily: fontFamilyStack(value, fallback) }}
                triggerProps={{ "aria-label": "Open font list" }}
            />
            <ComboboxPopup className="w-72">
                <ComboboxEmpty>No fonts found.</ComboboxEmpty>
                <ComboboxList>
                    {(family: string) => (
                        <ComboboxItem key={family} value={family}>
                            <span
                                className="block truncate"
                                style={{
                                    fontFamily: fontFamilyStack(
                                        family,
                                        fallback
                                    ),
                                }}
                            >
                                {family}
                            </span>
                        </ComboboxItem>
                    )}
                </ComboboxList>
            </ComboboxPopup>
        </Combobox>
    );
}
