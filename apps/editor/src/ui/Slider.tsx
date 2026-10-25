import { Slider as S } from "radix-ui";
import { cn } from "@/lib/cn";
import { useFieldControl } from "./Field";

export interface SliderProps {
  value: number;
  /** Fires continuously while dragging (store edits coalesce into one undo step). */
  onValueChange: (value: number) => void;
  /** Fires once when the drag ends. */
  onValueCommit?: (value: number) => void;
  min: number;
  max: number;
  step?: number;
  /** Value readout on the right; return null to hide it. */
  format?: (value: number) => string | null;
  disabled?: boolean;
  className?: string;
  id?: string;
  "aria-label"?: string;
}

export function Slider({
  value,
  onValueChange,
  onValueCommit,
  min,
  max,
  step = 1,
  format = (v) => String(v),
  disabled,
  className,
  ...aria
}: SliderProps) {
  const field = useFieldControl(aria);
  const readout = format(value);
  return (
    <div className={cn("flex items-center gap-3", disabled && "opacity-50", className)}>
      <S.Root
        value={[value]}
        onValueChange={([v]) => v !== undefined && onValueChange(v)}
        onValueCommit={([v]) => v !== undefined && onValueCommit?.(v)}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        className="relative flex h-5 flex-1 touch-none select-none items-center"
      >
        <S.Track className="relative h-1 grow overflow-hidden rounded-full bg-control-active">
          <S.Range className="absolute h-full rounded-full bg-accent" />
        </S.Track>
        <S.Thumb
          {...field}
          aria-label={aria["aria-label"]}
          className={cn(
            "block size-3.5 rounded-full border border-black/20 bg-white shadow-[0_1px_3px_rgba(0,0,0,0.4)] transition-quick",
            "hover:scale-110 focus-visible:shadow-[0_0_0_4px_var(--sp-accent-soft)] focus-visible:outline-none",
          )}
        />
      </S.Root>
      {readout !== null && <span className="min-w-10 text-right text-sm tabular-nums text-muted">{readout}</span>}
    </div>
  );
}
