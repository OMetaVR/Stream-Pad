import { Check, Pipette } from "lucide-react";
import { useEffect, useState } from "react";
import { cn } from "@/lib/cn";
import { normalizeHex, readableOn } from "@/lib/theme";
import { useFieldControl } from "./Field";
import { Input } from "./Input";
import { Popover, PopoverContent, PopoverTrigger } from "./Popover";

/** Curated accents that read well on both themes. */
export const SWATCHES = [
  "#8b7cf6", "#6d7cff", "#4aa8ff", "#22c3d6", "#2ec4a5", "#3fcf8e", "#a3d94a", "#f5c84c",
  "#f5a524", "#ff8a4c", "#ff7a59", "#f0626b", "#f06aa8", "#c86ff0", "#8b8d98", "#e9e9ef",
];

export interface ColorPickerProps {
  value: string | null;
  onChange: (value: string | null) => void;
  /** Offer a "none" choice that sets null (e.g. "use the default accent"). */
  allowNone?: boolean;
  noneLabel?: string;
  swatches?: string[];
  /** Show the hex value next to the swatch in the trigger. */
  showValue?: boolean;
  size?: "sm" | "md";
  disabled?: boolean;
  id?: string;
  "aria-label"?: string;
  className?: string;
}

function Swatch({ color, size }: { color: string | null; size: number }) {
  return color ? (
    <span className="block rounded-full shadow-[inset_0_0_0_1px_rgba(0,0,0,0.25)]" style={{ width: size, height: size, background: color }} />
  ) : (
    <span className="block rounded-full border border-dashed border-faint" style={{ width: size, height: size }} />
  );
}

export function ColorPicker({
  value,
  onChange,
  allowNone,
  noneLabel = "Default",
  swatches = SWATCHES,
  showValue,
  size = "md",
  disabled,
  className,
  ...aria
}: ColorPickerProps) {
  const field = useFieldControl(aria);
  const [hex, setHex] = useState(value ?? "");
  useEffect(() => setHex(value ?? ""), [value]);

  const commitHex = () => {
    const n = normalizeHex(hex);
    if (n) onChange(n);
    else setHex(value ?? "");
  };

  return (
    <Popover>
      <PopoverTrigger
        {...field}
        aria-label={aria["aria-label"] ?? (value ? `Colour ${value}` : noneLabel)}
        disabled={disabled}
        className={cn(
          "inline-flex items-center gap-2 rounded-sm border border-line bg-raised shadow-raised transition-quick hover:border-line-strong",
          "disabled:opacity-50 data-[state=open]:border-accent-line",
          size === "sm" ? "h-7 px-1.5" : "h-8 px-2",
          className,
        )}
      >
        <Swatch color={value} size={size === "sm" ? 14 : 16} />
        {showValue && <span className="font-code text-sm text-muted">{value ?? noneLabel}</span>}
      </PopoverTrigger>
      <PopoverContent className="w-[228px]" align="start">
        <div className="grid grid-cols-8 gap-1.5" role="group" aria-label="Colours">
          {swatches.map((c) => (
            <button
              key={c}
              type="button"
              aria-label={c}
              aria-pressed={value?.toLowerCase() === c}
              onClick={() => onChange(c)}
              className="relative flex size-5 items-center justify-center rounded-full transition-quick hover:scale-110"
              style={{ background: c }}
            >
              {value?.toLowerCase() === c && <Check size={12} strokeWidth={3} style={{ color: readableOn(c) }} />}
            </button>
          ))}
        </div>
        <div className="mt-3 flex items-center gap-1.5">
          <Input
            size="sm"
            mono
            value={hex}
            placeholder="#8b7cf6"
            aria-label="Hex colour"
            spellCheck={false}
            onChange={(e) => setHex(e.currentTarget.value)}
            onBlur={commitHex}
            onKeyDown={(e) => e.key === "Enter" && commitHex()}
            wrapperClassName="flex-1"
          />
          <label
            className="relative flex size-7 shrink-0 cursor-default items-center justify-center rounded-sm border border-line bg-raised text-muted hover:text-fg"
            title="Custom colour"
          >
            <Pipette size={14} aria-hidden />
            <input
              type="color"
              aria-label="Custom colour"
              value={normalizeHex(value ?? "") ?? "#8b7cf6"}
              onChange={(e) => onChange(e.currentTarget.value)}
              className="absolute inset-0 opacity-0"
            />
          </label>
        </div>
        {allowNone && (
          <button
            type="button"
            onClick={() => onChange(null)}
            aria-pressed={value === null}
            disabled={value === null}
            className="mt-2 flex h-7 w-full items-center gap-2 rounded-sm px-2 text-sm text-muted transition-quick hover:bg-control hover:text-fg disabled:opacity-60 disabled:hover:bg-transparent"
          >
            <Swatch color={null} size={14} />
            {value === null ? `${noneLabel} (current)` : `Reset to ${noneLabel.toLowerCase()}`}
          </button>
        )}
      </PopoverContent>
    </Popover>
  );
}
