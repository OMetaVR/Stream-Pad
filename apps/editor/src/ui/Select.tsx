import { Check, ChevronsUpDown, type LucideIcon } from "lucide-react";
import { Select as S } from "radix-ui";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { useFieldControl } from "./Field";
import { controlSurface } from "./Input";
import { menuContent, menuItem } from "./menu-styles";

export interface SelectOption<T extends string> {
  /** Must not be "" (Radix reserves it for "no value"). */
  value: T;
  label: string;
  icon?: LucideIcon;
  /** Second line in the list. */
  description?: string;
  disabled?: boolean;
}

export interface SelectProps<T extends string> {
  value: T | null;
  onValueChange: (value: T) => void;
  options: SelectOption<T>[];
  placeholder?: string;
  size?: "sm" | "md";
  disabled?: boolean;
  /** Custom rendering of the selected value in the trigger. */
  renderValue?: (option: SelectOption<T>) => ReactNode;
  className?: string;
  id?: string;
  "aria-label"?: string;
}

export function Select<T extends string>({
  value,
  onValueChange,
  options,
  placeholder = "Choose…",
  size = "md",
  disabled,
  renderValue,
  className,
  ...aria
}: SelectProps<T>) {
  const field = useFieldControl(aria);
  const current = options.find((o) => o.value === value);
  return (
    <S.Root value={value ?? undefined} onValueChange={(v) => onValueChange(v as T)} disabled={disabled}>
      <S.Trigger
        {...field}
        aria-label={aria["aria-label"]}
        className={cn(
          controlSurface,
          "flex items-center justify-between gap-2 px-2.5 text-left outline-none data-[placeholder]:text-faint",
          "focus-visible:border-accent-line focus-visible:shadow-[0_0_0_3px_var(--sp-accent-soft)] disabled:opacity-50",
          size === "sm" ? "h-7 text-sm" : "h-8 text-base",
          className,
        )}
      >
        <span className="flex min-w-0 items-center gap-2 truncate">
          {current ? (
            renderValue ? (
              renderValue(current)
            ) : (
              <>
                {current.icon && <current.icon size={14} className="text-muted" aria-hidden />}
                <span className="truncate">{current.label}</span>
              </>
            )
          ) : (
            <S.Value placeholder={placeholder} />
          )}
        </span>
        <S.Icon className="text-faint">
          <ChevronsUpDown size={14} />
        </S.Icon>
      </S.Trigger>
      <S.Portal>
        <S.Content
          position="popper"
          sideOffset={4}
          collisionPadding={8}
          className={cn(menuContent, "max-h-(--radix-select-content-available-height) min-w-(--radix-select-trigger-width)")}
        >
          <S.Viewport className="p-1">
            {options.map((o) => (
              <S.Item key={o.value} value={o.value} disabled={o.disabled} className={cn(menuItem, "pr-8")}>
                {o.icon && <o.icon size={15} className="text-muted" aria-hidden />}
                <div className="flex min-w-0 flex-col">
                  <S.ItemText>{o.label}</S.ItemText>
                  {o.description && <span className="truncate text-sm text-muted">{o.description}</span>}
                </div>
                <S.ItemIndicator className="absolute right-2 text-accent-text">
                  <Check size={14} />
                </S.ItemIndicator>
              </S.Item>
            ))}
          </S.Viewport>
        </S.Content>
      </S.Portal>
    </S.Root>
  );
}
