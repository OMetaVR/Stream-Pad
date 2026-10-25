import { Check, Minus } from "lucide-react";
import { Checkbox as C } from "radix-ui";
import { useId, type ReactNode } from "react";
import { cn } from "@/lib/cn";
import { useFieldControl } from "./Field";

export interface CheckboxProps {
  checked: boolean | "indeterminate";
  onCheckedChange: (checked: boolean) => void;
  /** Clickable text next to the box. */
  label?: ReactNode;
  disabled?: boolean;
  id?: string;
  "aria-label"?: string;
  className?: string;
}

export function Checkbox({ checked, onCheckedChange, label, disabled, className, ...aria }: CheckboxProps) {
  const field = useFieldControl(aria);
  const ownId = useId();
  const id = field.id ?? ownId;
  const box = (
    <C.Root
      {...field}
      id={id}
      aria-label={aria["aria-label"]}
      checked={checked}
      onCheckedChange={(v) => onCheckedChange(v === true)}
      disabled={disabled}
      className={cn(
        "flex size-4 shrink-0 items-center justify-center rounded-[4px] border border-line-strong bg-raised transition-quick",
        "hover:border-muted data-[state=checked]:border-accent data-[state=checked]:bg-accent",
        "data-[state=indeterminate]:border-accent data-[state=indeterminate]:bg-accent disabled:opacity-45",
        !label && className,
      )}
    >
      <C.Indicator className="text-accent-fg">
        {checked === "indeterminate" ? <Minus size={12} strokeWidth={2.5} /> : <Check size={12} strokeWidth={2.5} />}
      </C.Indicator>
    </C.Root>
  );
  if (!label) return box;
  return (
    <div className={cn("flex items-center gap-2", disabled && "opacity-60", className)}>
      {box}
      <label htmlFor={id} className="text-base text-fg">
        {label}
      </label>
    </div>
  );
}
