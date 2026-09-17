import * as React from "react";

import { Spinner } from "@/components/ui/spinner";

/** Invisible for `delay` ms so fast loads never flash a spinner. */
export function DelayedSpinner({
    delay = 150,
    className,
}: {
    delay?: number;
    className?: string;
}) {
    const [visible, setVisible] = React.useState(false);

    React.useEffect(() => {
        const timer = window.setTimeout(() => setVisible(true), delay);
        return () => window.clearTimeout(timer);
    }, [delay]);

    if (!visible) return null;
    return <Spinner className={className} />;
}
