import {
    Tooltip,
    TooltipTrigger,
    TooltipContent,
    TooltipPrimitive,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

export function ExternalLink({
    href,
    children,
    className,
    align,
    side,
    sideOffset,
}: {
    href?: string;
    children: React.ReactNode;
    className?: string;
    align?: TooltipPrimitive.Positioner.Props["align"];
    side?: TooltipPrimitive.Positioner.Props["side"];
    sideOffset?: TooltipPrimitive.Positioner.Props["sideOffset"];
}) {
    return (
        <Tooltip>
            <TooltipTrigger
                render={
                    <a
                        href={href}
                        target="_blank"
                        rel="noreferrer"
                        className={cn(
                            "underline-offset-2 hover:text-foreground hover:underline",
                            className
                        )}
                    >
                        {children}
                    </a>
                }
            />
            <TooltipContent align={align} side={side} sideOffset={sideOffset}>
                {href}
            </TooltipContent>
        </Tooltip>
    );
}
