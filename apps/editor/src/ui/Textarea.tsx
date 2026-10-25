import { useLayoutEffect, useRef, type ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { useFieldControl } from "./Field";
import { controlSurface } from "./Input";

export interface TextareaProps extends ComponentProps<"textarea"> {
  /** Grow with the content up to `maxRows` (default 8). */
  autosize?: boolean;
  maxRows?: number;
  mono?: boolean;
}

export function Textarea({ autosize, maxRows = 8, mono, rows = 3, className, ref, ...props }: TextareaProps) {
  const field = useFieldControl(props);
  const inner = useRef<HTMLTextAreaElement | null>(null);

  useLayoutEffect(() => {
    const el = inner.current;
    if (!autosize || !el) return;
    const style = getComputedStyle(el);
    const px = (v: string) => parseFloat(v) || 0;
    const border = px(style.borderTopWidth) + px(style.borderBottomWidth);
    const limit = (px(style.lineHeight) || 20) * maxRows + px(style.paddingTop) + px(style.paddingBottom) + border;
    el.style.height = "auto";
    // border-box sizing: scrollHeight covers content + padding, so add the borders back.
    el.style.height = `${Math.min(el.scrollHeight + border, limit)}px`;
  }, [autosize, maxRows, props.value]);

  return (
    <textarea
      rows={rows}
      {...props}
      {...field}
      ref={(el) => {
        inner.current = el;
        if (typeof ref === "function") ref(el);
        else if (ref) ref.current = el;
      }}
      className={cn(
        controlSurface,
        "block w-full resize-none px-2.5 py-1.5 text-base outline-none",
        mono && "font-code leading-5",
        autosize && "overflow-y-auto",
        className,
      )}
    />
  );
}
