import type { LucideIcon } from "lucide-react";
import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { buttonVariants, type ButtonVariant } from "./Button";
import { Spinner } from "./Spinner";
import { Tooltip } from "./Tooltip";

export interface IconButtonProps extends Omit<ComponentProps<"button">, "children"> {
  icon: LucideIcon;
  /** Accessible name, also shown as the tooltip. Required: an icon alone is not a label. */
  label: string;
  /** Shortcut shown in the tooltip ("mod+z"). */
  shortcut?: string;
  variant?: ButtonVariant;
  size?: "xs" | "sm" | "md";
  /** Toggle-button state (sets aria-pressed and the pressed look). */
  active?: boolean;
  loading?: boolean;
  tooltipSide?: "top" | "right" | "bottom" | "left";
  /** Hide the tooltip (e.g. when a visible label sits next to the button). */
  noTooltip?: boolean;
}

const sizes = { xs: "size-6 rounded-[5px]", sm: "size-7 rounded-sm", md: "size-8 rounded-sm" };
const iconSizes = { xs: 14, sm: 15, md: 16 };

export function IconButton({
  icon: Icon,
  label,
  shortcut,
  variant = "ghost",
  size = "sm",
  active,
  loading,
  tooltipSide = "top",
  noTooltip,
  disabled,
  className,
  type = "button",
  ...props
}: IconButtonProps) {
  const button = (
    <button
      type={type}
      aria-label={label}
      aria-pressed={active}
      disabled={disabled || loading}
      className={cn(
        "inline-flex shrink-0 items-center justify-center transition-quick disabled:pointer-events-none disabled:opacity-40",
        buttonVariants[variant],
        sizes[size],
        active && "bg-accent-soft text-accent-text hover:bg-accent-soft hover:text-accent-text",
        className,
      )}
      {...props}
    >
      {loading ? <Spinner size={iconSizes[size]} /> : <Icon size={iconSizes[size]} aria-hidden />}
    </button>
  );
  return (
    <Tooltip content={label} shortcut={shortcut} side={tooltipSide} disabled={noTooltip}>
      {button}
    </Tooltip>
  );
}
