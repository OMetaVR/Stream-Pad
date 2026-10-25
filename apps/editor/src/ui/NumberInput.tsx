import { ChevronDown, ChevronUp } from "lucide-react";
import { useEffect, useState, type KeyboardEvent } from "react";
import { cn } from "@/lib/cn";
import { useFieldControl } from "./Field";
import { controlSurface } from "./Input";

export interface NumberInputProps {
  value: number;
  /** Called with a clamped, step-rounded number on commit (blur, Enter, arrows, stepper buttons). */
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  /** Also sets the precision: step 1 → integers (what u32 config fields require), 0.05 → two decimals. */
  step?: number;
  /** Unit shown inside the right edge ("ms", "px", "%"). */
  suffix?: string;
  size?: "sm" | "md";
  disabled?: boolean;
  className?: string;
  id?: string;
  "aria-label"?: string;
  "aria-describedby"?: string;
}

function decimals(step: number): number {
  const s = String(step);
  return s.includes(".") ? s.length - s.indexOf(".") - 1 : 0;
}

/** Numeric field with steppers. ↑/↓ step (Shift ×10), Enter commits, Esc reverts. */
export function NumberInput({
  value,
  onChange,
  min = -Infinity,
  max = Infinity,
  step = 1,
  suffix,
  size = "md",
  disabled,
  className,
  ...aria
}: NumberInputProps) {
  const field = useFieldControl(aria);
  const [text, setText] = useState(String(value));
  const [editing, setEditing] = useState(false);

  useEffect(() => {
    if (!editing) setText(String(value));
  }, [value, editing]);

  const normalize = (n: number) => {
    const rounded = Number((Math.round(n / step) * step).toFixed(decimals(step)));
    return Math.min(max, Math.max(min, rounded));
  };

  const commit = (raw: string) => {
    const n = Number(raw.replace(",", ".").trim());
    if (raw.trim() === "" || !Number.isFinite(n)) {
      setText(String(value));
      return;
    }
    const next = normalize(n);
    setText(String(next));
    if (next !== value) onChange(next);
  };

  const nudge = (dir: 1 | -1, big = false) => {
    const next = normalize(value + dir * step * (big ? 10 : 1));
    setText(String(next));
    if (next !== value) onChange(next);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      nudge(e.key === "ArrowUp" ? 1 : -1, e.shiftKey);
    } else if (e.key === "Enter") {
      commit(text);
    } else if (e.key === "Escape") {
      setText(String(value));
      e.currentTarget.blur();
    }
  };

  const stepper = "flex h-1/2 w-5 items-center justify-center text-faint hover:text-fg disabled:opacity-40";
  return (
    <div
      className={cn(controlSurface, "flex items-center pl-2.5", size === "sm" ? "h-7" : "h-8", disabled && "opacity-50", className)}
      aria-invalid={field["aria-invalid"]}
    >
      <input
        {...field}
        aria-label={aria["aria-label"]}
        type="text"
        inputMode="decimal"
        role="spinbutton"
        aria-valuenow={value}
        aria-valuemin={Number.isFinite(min) ? min : undefined}
        aria-valuemax={Number.isFinite(max) ? max : undefined}
        disabled={disabled}
        value={text}
        onFocus={() => setEditing(true)}
        onBlur={(e) => {
          setEditing(false);
          commit(e.currentTarget.value);
        }}
        onChange={(e) => setText(e.currentTarget.value)}
        onKeyDown={onKeyDown}
        className={cn("h-full min-w-0 flex-1 bg-transparent tabular-nums outline-none", size === "sm" ? "text-sm" : "text-base")}
      />
      {suffix && <span className="pl-1 pr-1 text-sm text-faint">{suffix}</span>}
      <div className="flex h-full flex-col border-l border-line-subtle" aria-hidden>
        <button type="button" tabIndex={-1} disabled={disabled || value >= max} onClick={() => nudge(1)} className={stepper}>
          <ChevronUp size={12} />
        </button>
        <button type="button" tabIndex={-1} disabled={disabled || value <= min} onClick={() => nudge(-1)} className={stepper}>
          <ChevronDown size={12} />
        </button>
      </div>
    </div>
  );
}
