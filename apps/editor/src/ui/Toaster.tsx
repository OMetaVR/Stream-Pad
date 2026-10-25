import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, type ReactNode } from "react";
import { create } from "zustand";
import { cn } from "@/lib/cn";

export type ToastKind = "info" | "success" | "warning" | "error";

export interface ToastInput {
  title: string;
  description?: ReactNode;
  kind?: ToastKind;
  /** ms; default 4 s (6 s for errors and toasts with an action). `Infinity` stays until dismissed. */
  duration?: number;
  /** One action button, e.g. `{ label: "Undo", onClick: restore }`. Clicking it dismisses the toast. */
  action?: { label: string; onClick: () => void };
  /** Reusing an id replaces that toast instead of stacking a new one. */
  id?: string;
}

interface ToastItem extends Required<Pick<ToastInput, "title" | "kind" | "duration">> {
  id: string;
  description?: ReactNode;
  action?: ToastInput["action"];
}

const MAX_VISIBLE = 4;
let counter = 0;

const useToasts = create<{ items: ToastItem[] }>(() => ({ items: [] }));

function show(input: ToastInput): string {
  const id = input.id ?? `t${++counter}`;
  const kind = input.kind ?? "info";
  const item: ToastItem = {
    id,
    title: input.title,
    description: input.description,
    kind,
    action: input.action,
    duration: input.duration ?? (kind === "error" || input.action ? 6000 : 4000),
  };
  useToasts.setState((s) => {
    const rest = s.items.filter((t) => t.id !== id);
    return { items: [...rest, item].slice(-MAX_VISIBLE) };
  });
  return id;
}

function dismiss(id: string) {
  useToasts.setState((s) => ({ items: s.items.filter((t) => t.id !== id) }));
}

type Shortcut = (title: string, options?: Omit<ToastInput, "title" | "kind">) => string;

/**
 * Show a toast. `toast("Saved")`, `toast.error("Could not save", { description: e.message })`,
 * `toast({ title: "Deleted “Audio”", action: { label: "Undo", onClick: restore } })`. Returns the id.
 */
export const toast = Object.assign((input: string | ToastInput) => show(typeof input === "string" ? { title: input } : input), {
  info: ((title, o) => show({ ...o, title, kind: "info" })) as Shortcut,
  success: ((title, o) => show({ ...o, title, kind: "success" })) as Shortcut,
  warning: ((title, o) => show({ ...o, title, kind: "warning" })) as Shortcut,
  error: ((title, o) => show({ ...o, title, kind: "error" })) as Shortcut,
  dismiss,
});

const icons = {
  info: <Info size={16} className="text-muted" />,
  success: <CircleCheck size={16} className="text-success" />,
  warning: <TriangleAlert size={16} className="text-warning" />,
  error: <CircleAlert size={16} className="text-danger" />,
};

function ToastView({ item }: { item: ToastItem }) {
  const remaining = useRef(item.duration);
  const startedAt = useRef(Date.now());
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const start = () => {
    if (!Number.isFinite(remaining.current)) return;
    startedAt.current = Date.now();
    timer.current = setTimeout(() => dismiss(item.id), remaining.current);
  };
  const pause = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
    remaining.current = Math.max(1000, remaining.current - (Date.now() - startedAt.current));
  };

  useEffect(() => {
    remaining.current = item.duration;
    start();
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
    // Restart the countdown when a toast is replaced through a reused id.
  }, [item]);

  return (
    <motion.li
      layout
      initial={{ opacity: 0, y: 16, scale: 0.97 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.97, transition: { duration: 0.12 } }}
      transition={{ type: "spring", bounce: 0.18, duration: 0.35 }}
      onPointerEnter={pause}
      onPointerLeave={start}
      onFocus={pause}
      onBlur={start}
      role={item.kind === "error" ? "alert" : "status"}
      className={cn(
        "group pointer-events-auto flex w-[min(420px,calc(100vw-32px))] items-start gap-3 rounded-md bg-raised py-2.5 pl-3 pr-2 shadow-popover",
        item.kind === "error" && "shadow-[0_0_0_1px_color-mix(in_oklab,var(--sp-danger)_40%,transparent),var(--sp-shadow-popover)]",
      )}
    >
      <span className="mt-0.5 shrink-0">{icons[item.kind]}</span>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5 py-px">
        <p className="text-base font-medium text-fg">{item.title}</p>
        {item.description && <div className="text-sm text-muted selectable">{item.description}</div>}
      </div>
      {item.action && (
        <button
          type="button"
          onClick={() => {
            item.action!.onClick();
            dismiss(item.id);
          }}
          className="shrink-0 rounded-sm px-2 py-0.5 text-base font-medium text-accent-text hover:bg-accent-soft"
        >
          {item.action.label}
        </button>
      )}
      <button
        type="button"
        aria-label="Dismiss"
        onClick={() => dismiss(item.id)}
        className="mt-px shrink-0 rounded-[5px] p-0.5 text-faint opacity-0 transition-quick hover:text-fg focus-visible:opacity-100 group-hover:opacity-100"
      >
        <X size={14} />
      </button>
    </motion.li>
  );
}

/** Mounted once in App.tsx. Toasts stack at the bottom centre, newest at the bottom. */
export function Toaster() {
  const items = useToasts((s) => s.items);
  return (
    <ol aria-live="polite" className="pointer-events-none fixed inset-x-0 bottom-5 z-[60] flex flex-col items-center gap-2">
      <AnimatePresence initial={false}>
        {items.map((t) => (
          <ToastView key={t.id} item={t} />
        ))}
      </AnimatePresence>
    </ol>
  );
}
