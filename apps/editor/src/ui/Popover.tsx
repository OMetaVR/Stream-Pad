import { Popover as P } from "radix-ui";
import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { floatingSurface } from "./menu-styles";

export const Popover = P.Root;
export const PopoverTrigger = P.Trigger;
export const PopoverAnchor = P.Anchor;
export const PopoverClose = P.Close;

export interface PopoverContentProps extends ComponentProps<typeof P.Content> {
  /** Padding preset; "none" for lists that manage their own. */
  padding?: "none" | "sm" | "md";
}

export function PopoverContent({ className, padding = "md", sideOffset = 6, collisionPadding = 8, ...props }: PopoverContentProps) {
  return (
    <P.Portal>
      <P.Content
        sideOffset={sideOffset}
        collisionPadding={collisionPadding}
        className={cn(
          floatingSurface,
          "origin-(--radix-popover-content-transform-origin)",
          { none: "", sm: "p-2", md: "p-3" }[padding],
          className,
        )}
        {...props}
      />
    </P.Portal>
  );
}
