import type { LucideIcon } from "lucide-react";
import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/cn";
import { useFieldControl } from "./Field";

/** Shared look of every text-like control (Input, Textarea, NumberInput, Select trigger). */
export const controlSurface =
  "rounded-sm border border-line bg-raised text-fg shadow-raised transition-quick " +
  "placeholder:text-faint hover:border-line-strong " +
  "focus-within:border-accent-line focus-within:shadow-[0_0_0_3px_var(--sp-accent-soft)] " +
  "aria-invalid:border-danger aria-[invalid=true]:border-danger";

export interface InputProps extends Omit<ComponentProps<"input">, "size"> {
  size?: "sm" | "md";
  /** Icon inside the left edge (e.g. Search). */
  icon?: LucideIcon;
  /** Content inside the right edge (a unit, a clear button, a Kbd). */
  trailing?: ReactNode;
  /** Monospace text (combos, ids, paths). */
  mono?: boolean;
  /** Class for the outer wrapper (width, margins). `className` styles the <input> itself. */
  wrapperClassName?: string;
}

export function Input({ size = "md", icon: Icon, trailing, mono, wrapperClassName, className, ...props }: InputProps) {
  const field = useFieldControl(props);
  return (
    <div
      className={cn(controlSurface, "flex items-center gap-2 px-2.5", size === "sm" ? "h-7" : "h-8", wrapperClassName)}
      aria-invalid={field["aria-invalid"]}
    >
      {Icon && <Icon size={14} className="-ml-0.5 text-faint" aria-hidden />}
      <input
        {...props}
        {...field}
        className={cn(
          "h-full min-w-0 flex-1 bg-transparent outline-none placeholder:text-faint",
          size === "sm" ? "text-sm" : "text-base",
          mono && "font-code",
          className,
        )}
      />
      {trailing && <div className="-mr-1 flex shrink-0 items-center gap-1 text-sm text-muted">{trailing}</div>}
    </div>
  );
}
