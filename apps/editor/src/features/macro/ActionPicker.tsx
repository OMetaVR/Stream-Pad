// Command-palette style picker for "what should this key do?" — fuzzy search, categories, recents.

import fuzzysort from "fuzzysort";
import { Search } from "lucide-react";
import { useMemo, useState } from "react";
import { cn } from "@/lib/cn";
import { Dialog, DialogContent, Kbd, RefIcon, ScrollArea } from "@/ui";
import { ACTION_DEFS, CATEGORIES, categoryColor, nodeDef, type Category, type NodeDef } from "./catalog";

const RECENT_KEY = "streampad.recentActions";

function recents(): string[] {
  try {
    return (JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]") as string[]).filter((k) => ACTION_DEFS.some((d) => d.kind === k));
  } catch {
    return [];
  }
}

function remember(kind: string) {
  const list = [kind, ...recents().filter((k) => k !== kind)].slice(0, 5);
  localStorage.setItem(RECENT_KEY, JSON.stringify(list));
}

interface Props {
  open: boolean;
  onOpenChange(open: boolean): void;
  onPick(kind: string): void;
  title?: string;
  /** Hide flow-control nodes (the step list has no branches). */
  linearOnly?: boolean;
}

export function ActionPicker({ open, onOpenChange, onPick, title = "Choose an action", linearOnly }: Props) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const [category, setCategory] = useState<Category | null>(null);

  const defs = useMemo(() => ACTION_DEFS.filter((d) => !linearOnly || !["flow.if", "flow.repeat", "flow.stop"].includes(d.kind)), [linearOnly]);

  const sections = useMemo<{ title: string; items: NodeDef[] }[]>(() => {
    const q = query.trim();
    if (q) {
      const hits = fuzzysort.go(q, defs, { keys: ["title", "description", "category", "keywords"], threshold: 0.2 }).map((r) => r.obj);
      return [{ title: "Results", items: hits }];
    }
    if (category) return [{ title: category, items: defs.filter((d) => d.category === category) }];
    const recent = recents().map(nodeDef).filter((d) => defs.includes(d));
    const groups = CATEGORIES.map((c) => ({ title: c.id as string, items: defs.filter((d) => d.category === c.id) })).filter((g) => g.items.length);
    return recent.length ? [{ title: "Recent", items: recent }, ...groups] : groups;
  }, [query, category, defs]);

  const flat = sections.flatMap((s) => s.items);
  const pick = (d: NodeDef | undefined) => {
    if (!d) return;
    remember(d.kind);
    onPick(d.kind);
    onOpenChange(false);
  };

  let index = -1;
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        onOpenChange(o);
        if (o) {
          setQuery("");
          setActive(0);
          setCategory(null);
        }
      }}
    >
      <DialogContent size="md" hideClose bare title={title} className="overflow-hidden">
        <div className="flex items-center gap-2 border-b border-line px-4">
          <Search size={16} className="text-faint" />
          <input
            autoFocus
            value={query}
            onChange={(e) => {
              setQuery(e.currentTarget.value);
              setActive(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setActive((a) => Math.min(a + 1, flat.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActive((a) => Math.max(a - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                pick(flat[active]);
              } else if (e.key === "Backspace" && !query && category) {
                setCategory(null);
              }
            }}
            placeholder={category ? `Search ${category}…` : "What should this key do?"}
            className="h-12 flex-1 bg-transparent text-md text-fg outline-none placeholder:text-faint"
          />
          <Kbd>Esc</Kbd>
        </div>
        {!query && (
          <div className="flex flex-wrap gap-1 border-b border-line px-3 py-2">
            {CATEGORIES.filter((c) => defs.some((d) => d.category === c.id)).map((c) => (
              <button
                key={c.id}
                type="button"
                onClick={() => {
                  setCategory((cur) => (cur === c.id ? null : c.id));
                  setActive(0);
                }}
                className={cn(
                  "flex h-6 items-center gap-1.5 rounded-full border px-2.5 text-xs transition-quick",
                  category === c.id ? "border-accent-line bg-accent-soft text-accent-text" : "border-line text-muted hover:text-fg",
                )}
              >
                <span className="size-1.5 rounded-full" style={{ background: c.color }} />
                {c.id}
              </button>
            ))}
          </div>
        )}
        <ScrollArea className="h-[min(420px,60vh)]">
          <div className="p-2">
            {flat.length === 0 && <p className="px-3 py-10 text-center text-sm text-faint">No actions match “{query}”.</p>}
            {sections.map((s) => (
              <div key={s.title} className="mb-1">
                <div className="px-2 pb-1 pt-2 text-xs font-medium text-faint">{s.title}</div>
                {s.items.map((d) => {
                  index++;
                  const i = index;
                  return (
                    <button
                      key={`${s.title}:${d.kind}`}
                      type="button"
                      onMouseMove={() => setActive(i)}
                      onClick={() => pick(d)}
                      ref={(el) => {
                        if (i === active) el?.scrollIntoView({ block: "nearest" });
                      }}
                      className={cn("flex w-full items-center gap-3 rounded-md px-2 py-2 text-left", i === active && "bg-control")}
                    >
                      <span
                        className="flex size-8 shrink-0 items-center justify-center rounded-md"
                        style={{ background: `color-mix(in srgb, ${categoryColor(d.category)} 16%, transparent)`, color: categoryColor(d.category) }}
                      >
                        <RefIcon iconRef={`lucide:${d.icon}`} size={16} />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block text-base font-medium text-fg">{d.title}</span>
                        <span className="block truncate text-sm text-muted">{d.description}</span>
                      </span>
                      {i === active && <Kbd>Enter</Kbd>}
                    </button>
                  );
                })}
              </div>
            ))}
          </div>
        </ScrollArea>
      </DialogContent>
    </Dialog>
  );
}
