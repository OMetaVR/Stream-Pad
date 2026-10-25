import type { LucideIcon } from "lucide-react";
import { Slot } from "radix-ui";
import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { Spinner } from "./Spinner";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

export interface ButtonProps extends ComponentProps<"button"> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** Leading icon. */
  icon?: LucideIcon;
  /** Trailing icon (menus, disclosure). */
  iconRight?: LucideIcon;
  /** Shows a spinner instead of the icon and disables the button. */
  loading?: boolean;
  /** Render the child element (e.g. <a>) with button styling. */
  asChild?: boolean;
}

const base =
  "relative inline-flex shrink-0 select-none items-center justify-center whitespace-nowrap font-medium transition-quick " +
  "disabled:pointer-events-none disabled:opacity-45 active:translate-y-px";

export const buttonVariants: Record<ButtonVariant, string> = {
  primary:
    "bg-accent text-accent-fg shadow-[inset_0_1px_0_rgba(255,255,255,0.18),0_1px_2px_rgba(0,0,0,0.25)] hover:bg-accent-hover",
  secondary: "border border-line bg-control text-fg shadow-raised hover:border-line-strong hover:bg-control-hover",
  ghost: "text-muted hover:bg-control hover:text-fg data-[state=open]:bg-control data-[state=open]:text-fg",
  danger: "bg-danger-soft text-danger hover:bg-danger hover:text-white",
};

export const buttonSizes: Record<ButtonSize, string> = {
  sm: "h-7 gap-1.5 rounded-sm px-2.5 text-sm",
  md: "h-8 gap-2 rounded-sm px-3 text-base",
  lg: "h-10 gap-2 rounded-md px-4 text-md",
};

export function Button({
  variant = "secondary",
  size = "md",
  icon: Icon,
  iconRight: IconRight,
  loading = false,
  asChild = false,
  disabled,
  className,
  children,
  type = "button",
  ...props
}: ButtonProps) {
  const classes = cn(base, buttonVariants[variant], buttonSizes[size], className);
  const iconSize = size === "lg" ? 16 : size === "md" ? 15 : 14;
  if (asChild) {
    return (
      <Slot.Root className={classes} {...props}>
        {children}
      </Slot.Root>
    );
  }
  return (
    <button type={type} disabled={disabled || loading} aria-busy={loading || undefined} className={classes} {...props}>
      {loading ? <Spinner size={iconSize} /> : Icon && <Icon size={iconSize} aria-hidden />}
      {children}
      {IconRight && <IconRight size={iconSize} aria-hidden className="-mr-0.5 opacity-70" />}
    </button>
  );
}
