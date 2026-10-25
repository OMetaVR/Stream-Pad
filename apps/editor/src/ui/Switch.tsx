import { Switch as S } from "radix-ui";
import { cn } from "@/lib/cn";
import { useFieldControl } from "./Field";

export interface SwitchProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  size?: "sm" | "md";
  disabled?: boolean;
  id?: string;
  "aria-label"?: string;
  className?: string;
}

export function Switch({ checked, onCheckedChange, size = "md", disabled, className, ...aria }: SwitchProps) {
  const field = useFieldControl(aria);
  const track = size === "sm" ? "h-4 w-7" : "h-5 w-9";
  const thumb = size === "sm" ? "size-3 data-[state=checked]:translate-x-3" : "size-4 data-[state=checked]:translate-x-4";
  return (
    <S.Root
      {...field}
      aria-label={aria["aria-label"]}
      checked={checked}
      onCheckedChange={onCheckedChange}
      disabled={disabled}
      className={cn(
        "inline-flex shrink-0 items-center rounded-full border border-line bg-control p-[1px] transition-base",
        "hover:border-line-strong data-[state=checked]:border-transparent data-[state=checked]:bg-accent",
        "disabled:opacity-45",
        track,
        className,
      )}
    >
      <S.Thumb
        className={cn(
          "block translate-x-0 rounded-full bg-fg shadow-[0_1px_2px_rgba(0,0,0,0.35)] transition-transform duration-(--sp-dur) ease-out",
          "data-[state=checked]:bg-accent-fg",
          thumb,
        )}
      />
    </S.Root>
  );
}
