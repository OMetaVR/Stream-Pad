import { Tooltip as T } from "radix-ui";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Kbd } from "./Kbd";

export interface TooltipProps {
  /** The trigger; must accept a ref (any DOM element or kit control). */
  children: ReactNode;
  content: ReactNode;
  /** Shortcut shown after the text, e.g. "mod+z". */
  shortcut?: string;
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
  /** Disable without unmounting the trigger. */
  disabled?: boolean;
  delayDuration?: number;
}

/** Wrap the app once (App.tsx does). */
export const TooltipProvider = ({ children }: { children: ReactNode }) => (
  <T.Provider delayDuration={450} skipDelayDuration={250}>
    {children}
  </T.Provider>
);

export function Tooltip({ children, content, shortcut, side = "top", align = "center", disabled, delayDuration }: TooltipProps) {
  if (disabled || (!content && !shortcut)) return <>{children}</>;
  return (
    <T.Root delayDuration={delayDuration}>
      <T.Trigger asChild>{children}</T.Trigger>
      <T.Portal>
        <T.Content
          side={side}
          align={align}
          sideOffset={6}
          collisionPadding={8}
          className={cn(
            "z-50 flex max-w-72 items-center gap-2 rounded-sm bg-control-active px-2 py-1 text-sm text-fg shadow-popover",
            "origin-(--radix-tooltip-content-transform-origin) data-[state=closed]:animate-fade-out data-[state=delayed-open]:animate-pop-in",
          )}
        >
          <span>{content}</span>
          {shortcut && <Kbd combo={shortcut} />}
        </T.Content>
      </T.Portal>
    </T.Root>
  );
}
