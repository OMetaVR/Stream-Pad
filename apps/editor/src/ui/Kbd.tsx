import type { ReactNode } from "react";
import { comboKeys } from "@/lib/combo";
import { cn } from "@/lib/cn";

export interface KbdProps {
  /** A combo ("mod+shift+z", "ctrl+alt+t") rendered as one keycap per key… */
  combo?: string;
  /** …or literal content for a single keycap. */
  children?: ReactNode;
  size?: "sm" | "md";
  className?: string;
}

const cap =
  "inline-flex items-center justify-center rounded-[5px] border border-line bg-keycap font-sans font-medium text-muted " +
  "shadow-[inset_0_1px_0_var(--sp-keycap-highlight),0_1px_0_var(--sp-keycap-skirt)]";

/** Keycap-styled key hint, sharing the numpad's keycap look. */
export function Kbd({ combo, children, size = "sm", className }: KbdProps) {
  const sizing = size === "sm" ? "h-[18px] min-w-[18px] px-1 text-2xs" : "h-[22px] min-w-[22px] px-1.5 text-xs";
  if (combo === undefined) return <kbd className={cn(cap, sizing, className)}>{children}</kbd>;
  const keys = comboKeys(combo);
  return (
    <kbd className={cn("inline-flex items-center gap-0.5", className)} aria-label={keys.join(" ")}>
      {keys.map((k, i) => (
        <kbd key={i} className={cn(cap, sizing)} aria-hidden>
          {k}
        </kbd>
      ))}
    </kbd>
  );
}
