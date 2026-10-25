import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

// tailwind-merge must know the token names from styles/index.css, otherwise e.g. `text-2xs` (a size)
// and `text-muted` (a colour) would be treated as conflicting and one of them dropped.
const merge = extendTailwindMerge({
  override: {
    theme: {
      color: [
        "transparent", "current", "white", "black",
        "bg", "panel", "raised", "control", "control-hover", "control-active",
        "line-subtle", "line", "line-strong", "fg", "muted", "faint",
        "accent", "accent-fg", "accent-hover", "accent-soft", "accent-softer", "accent-line", "accent-text",
        "danger", "danger-soft", "success", "success-soft", "warning", "warning-soft",
        "keybed", "keycap", "keycap-skirt", "keycap-highlight",
      ],
      text: ["2xs", "xs", "sm", "base", "md", "lg", "xl", "2xl"],
      radius: ["sm", "md", "lg", "xl"],
      shadow: ["raised", "popover", "dialog"],
    },
  },
});

/** Join class names (clsx) and resolve Tailwind conflicts so a later `className` wins. */
export function cn(...inputs: ClassValue[]): string {
  return merge(clsx(inputs));
}
