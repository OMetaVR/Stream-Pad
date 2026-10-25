// A popover with a search box and a keyboard-navigable list — shared by the app and KDE shortcut pickers.

import fuzzysort from "fuzzysort";
import { ChevronsUpDown, Search } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Input, Popover, PopoverContent, PopoverTrigger, ScrollArea } from "@/ui";

export interface PickerItem {
  id: string;
  title: string;
  subtitle?: string;
  group?: string;
  icon?: ReactNode;
  /** Extra search text. */
  keywords?: string;
}

interface Props {
  items: PickerItem[];
  value: string | null;
  onPick(item: PickerItem): void;
  /** Offer "Use <query>" for free-form values. */
  freeform?: (query: string) => PickerItem | null;
  trigger: ReactNode;
  placeholder: string;
  empty?: string;
}

export function SearchPicker({ items, value, onPick, freeform, trigger, placeholder, empty = "Nothing found" }: Props) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);

  const results = useMemo(() => {
    const q = query.trim();
    let list: PickerItem[] = q
      ? fuzzysort
          .go(q, items, { keys: ["title", "subtitle", "keywords"], limit: 60, threshold: 0.3 })
          .map((r) => r.obj)
      : items.slice(0, 200);
    const extra = q && freeform ? freeform(q) : null;
    if (extra && !list.some((i) => i.id === extra.id)) list = [...list, extra];
    return list;
  }, [items, query, freeform]);

  const pick = (item: PickerItem | undefined) => {
    if (!item) return;
    onPick(item);
    setOpen(false);
    setQuery("");
  };

  let lastGroup: string | undefined;
  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        setActive(0);
      }}
    >
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent padding="none" className="w-[min(340px,var(--radix-popover-content-available-width))]" align="start">
        <div className="border-b border-line p-2">
          <Input
            autoFocus
            icon={Search}
            placeholder={placeholder}
            value={query}
            onChange={(e) => {
              setQuery(e.currentTarget.value);
              setActive(0);
            }}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault();
                setActive((a) => Math.min(a + 1, results.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setActive((a) => Math.max(a - 1, 0));
              } else if (e.key === "Enter") {
                e.preventDefault();
                pick(results[active]);
              }
            }}
          />
        </div>
        <ScrollArea className="max-h-72">
          <ul className="p-1" role="listbox">
            {results.length === 0 && <li className="px-3 py-6 text-center text-sm text-faint">{empty}</li>}
            {results.map((item, i) => {
              const header = !query && item.group && item.group !== lastGroup ? item.group : null;
              lastGroup = item.group;
              return (
                <li key={item.id}>
                  {header && <div className="px-2 pb-1 pt-2 text-xs font-medium text-faint">{header}</div>}
                  <button
                    type="button"
                    role="option"
                    aria-selected={item.id === value}
                    onMouseEnter={() => setActive(i)}
                    onClick={() => pick(item)}
                    ref={(el) => {
                      if (i === active) el?.scrollIntoView({ block: "nearest" });
                    }}
                    className={cn(
                      "flex w-full items-center gap-2.5 rounded-sm px-2 py-1.5 text-left",
                      i === active ? "bg-control" : "",
                      item.id === value && "text-accent-text",
                    )}
                  >
                    {item.icon && <span className="flex size-6 shrink-0 items-center justify-center">{item.icon}</span>}
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-base text-fg">{item.title}</span>
                      {item.subtitle && <span className="block truncate text-xs text-muted">{item.subtitle}</span>}
                    </span>
                  </button>
                </li>
              );
            })}
          </ul>
        </ScrollArea>
      </PopoverContent>
    </Popover>
  );
}

/** The button that opens a SearchPicker, styled like an input. */
export function PickerButton({ children, placeholder, ...rest }: React.ComponentProps<"button"> & { placeholder?: string }) {
  return (
    <button
      type="button"
      {...rest}
      className="flex h-8 w-full min-w-0 items-center gap-2 rounded-sm border border-line bg-raised px-2 text-left text-base transition-quick hover:border-line-strong"
    >
      <span className="flex min-w-0 flex-1 items-center gap-2">{children ?? <span className="text-faint">{placeholder}</span>}</span>
      <ChevronsUpDown size={14} className="shrink-0 text-faint" />
    </button>
  );
}
