import { ScrollArea as SA } from "radix-ui";
import type { ReactNode, Ref } from "react";
import { cn } from "@/lib/cn";

export interface ScrollAreaProps {
  children: ReactNode;
  /** Size the root (it must have a bounded height/width to scroll). */
  className?: string;
  viewportClassName?: string;
  viewportRef?: Ref<HTMLDivElement>;
  orientation?: "vertical" | "horizontal" | "both";
}

const bar =
  "flex touch-none select-none p-[3px] transition-opacity duration-(--sp-dur) data-[state=hidden]:opacity-0 " +
  "data-[orientation=vertical]:w-2.5 data-[orientation=horizontal]:h-2.5 data-[orientation=horizontal]:flex-col";
const thumb = "relative flex-1 rounded-full bg-(--sp-scrollbar) hover:bg-(--sp-scrollbar-hover)";

/** Overlay scrollbars that appear on hover/scroll, matching the token colours. */
export function ScrollArea({ children, className, viewportClassName, viewportRef, orientation = "vertical" }: ScrollAreaProps) {
  return (
    <SA.Root type="hover" scrollHideDelay={500} className={cn("relative overflow-hidden", className)}>
      {/* Radix wraps content in a display:table div, which breaks truncation; force block layout. */}
      <SA.Viewport ref={viewportRef} className={cn("size-full rounded-[inherit] [&>div]:block!", viewportClassName)}>
        {children}
      </SA.Viewport>
      {orientation !== "horizontal" && (
        <SA.Scrollbar orientation="vertical" className={bar}>
          <SA.Thumb className={thumb} />
        </SA.Scrollbar>
      )}
      {orientation !== "vertical" && (
        <SA.Scrollbar orientation="horizontal" className={bar}>
          <SA.Thumb className={thumb} />
        </SA.Scrollbar>
      )}
      <SA.Corner />
    </SA.Root>
  );
}
