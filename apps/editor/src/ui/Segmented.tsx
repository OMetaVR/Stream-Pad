import type { LucideIcon } from "lucide-react";
import { motion } from "motion/react";
import { ToggleGroup } from "radix-ui";
import { useId } from "react";
import { cn } from "@/lib/cn";
import { useFieldControl } from "./Field";
import { Tooltip } from "./Tooltip";

export interface SegmentedOption<T extends string> {
  value: T;
  label?: string;
  icon?: LucideIcon;
  /** Tooltip, and the accessible name for icon-only options. */
  title?: string;
  disabled?: boolean;
}

export interface SegmentedProps<T extends string> {
  value: T;
  onValueChange: (value: T) => void;
  options: SegmentedOption<T>[];
  size?: "sm" | "md";
  /** Stretch segments to fill the width. */
  fill?: boolean;
  "aria-label"?: string;
  className?: string;
}

/** Single choice among 2–5 options, with a sliding selection pill. Arrow keys move between options. */
export function Segmented<T extends string>({
  value,
  onValueChange,
  options,
  size = "md",
  fill,
  className,
  ...aria
}: SegmentedProps<T>) {
  const group = useId();
  const field = useFieldControl(aria);
  return (
    <ToggleGroup.Root
      type="single"
      value={value}
      // Radix allows deselecting the active item; a segmented control always has a value.
      onValueChange={(v) => v && onValueChange(v as T)}
      aria-label={aria["aria-label"]}
      aria-labelledby={field["aria-labelledby"]}
      aria-describedby={field["aria-describedby"]}
      className={cn(
        "inline-flex items-center gap-0.5 rounded-sm border border-line bg-bg/60 p-0.5",
        size === "sm" ? "h-7" : "h-8",
        fill ? "flex w-full" : "w-fit self-start",
        className,
      )}
    >
      {options.map((o) => {
        const active = o.value === value;
        const item = (
          <ToggleGroup.Item
            key={o.value}
            value={o.value}
            disabled={o.disabled}
            aria-label={o.label ? undefined : o.title}
            className={cn(
              "relative inline-flex h-full items-center justify-center gap-1.5 rounded-[5px] px-2.5 font-medium transition-quick",
              size === "sm" ? "text-sm" : "text-base",
              active ? "text-fg" : "text-muted hover:text-fg",
              fill && "flex-1",
              "disabled:opacity-40",
            )}
          >
            {active && (
              <motion.span
                layoutId={`seg-${group}`}
                className="absolute inset-0 rounded-[5px] border border-line bg-control-active shadow-raised"
                transition={{ type: "spring", bounce: 0.15, duration: 0.3 }}
              />
            )}
            <span className="relative flex items-center gap-1.5">
              {o.icon && <o.icon size={size === "sm" ? 14 : 15} aria-hidden />}
              {o.label}
            </span>
          </ToggleGroup.Item>
        );
        return o.title && !o.label ? (
          <Tooltip key={o.value} content={o.title}>
            {item}
          </Tooltip>
        ) : (
          item
        );
      })}
    </ToggleGroup.Root>
  );
}
