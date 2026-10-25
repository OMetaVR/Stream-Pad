// Icon picker popover: saved icons, every lucide icon (searchable by name and tags), installed apps' icons,
// and your own images (upload / link). Uploaded and linked icons are saved for reuse automatically; any other
// icon can be starred.

import fuzzysort from "fuzzysort";
import { Ban, Link2, Search, Star, Upload, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { cn } from "@/lib/cn";
import { loadLucideCatalog, type LucideCatalogEntry } from "@/lib/lucide";
import { editor, useEditor } from "@/state/store";
import { Button, Input, Popover, PopoverContent, PopoverTrigger, RefIcon, ScrollArea, Segmented, toast } from "@/ui";
import { useApps } from "../macro/data";
import { importDroppedImage } from "./CoverPicker";
import { UrlImport } from "./UrlImport";

const SUGGESTED = ["play", "pause", "skip-forward", "skip-back", "volume-2", "volume-x", "mic", "mic-off", "camera", "video", "radio", "music", "globe", "app-window", "monitor", "layers", "zap", "star", "heart", "flame", "gamepad-2", "message-circle", "bell", "clapperboard", "scissors", "eye", "lock", "sparkles", "keyboard", "terminal", "folder", "coffee"];

type Tab = "saved" | "icons" | "apps";

const EMPTY: string[] = [];

function useSavedIcons() {
  const saved = useEditor((s) => s.config?.savedIcons ?? EMPTY);
  const toggle = (ref: string, on?: boolean) =>
    editor.updateConfig(
      (c) => {
        const list = (c.savedIcons ??= []);
        const i = list.indexOf(ref);
        const want = on ?? i < 0;
        if (want && i < 0) list.unshift(ref);
        if (!want && i >= 0) list.splice(i, 1);
      },
      { coalesce: false },
    );
  return { saved, toggle };
}

function Cell({ iconRef, selected, saved, onPick, onStar, removable }: { iconRef: string; selected: boolean; saved: boolean; onPick(): void; onStar(): void; removable?: boolean }) {
  const lucide = iconRef.startsWith("lucide:");
  return (
    <div className="group relative">
      <button
        type="button"
        title={lucide ? iconRef.slice(7) : "Your icon"}
        onClick={onPick}
        className={cn("flex aspect-square w-full items-center justify-center rounded-sm text-muted transition-quick hover:bg-control hover:text-fg", selected && "bg-accent-soft text-accent-text")}
      >
        <RefIcon iconRef={iconRef} size={lucide ? 18 : 24} />
      </button>
      <button
        type="button"
        aria-label={removable ? "Remove from saved" : saved ? "Unsave" : "Save for later"}
        onClick={onStar}
        className={cn(
          "absolute -right-0.5 -top-0.5 flex size-4 items-center justify-center rounded-full bg-raised text-faint opacity-0 ring-1 ring-line transition-quick hover:text-fg group-hover:opacity-100",
          saved && !removable && "text-warning opacity-100",
        )}
      >
        {removable ? <X size={10} /> : <Star size={9} fill={saved ? "currentColor" : "none"} />}
      </button>
    </div>
  );
}

export function IconPicker({ value, onChange, trigger }: { value: string | null; onChange(v: string | null): void; trigger: ReactNode }) {
  const { saved, toggle } = useSavedIcons();
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<Tab>(saved.length ? "saved" : "icons");
  const [q, setQ] = useState("");
  const [catalog, setCatalog] = useState<LucideCatalogEntry[]>([]);
  const apps = useApps([]);
  const file = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open && !catalog.length) void loadLucideCatalog().then(setCatalog);
  }, [open, catalog.length]);

  const icons = useMemo(() => {
    if (!q.trim()) return SUGGESTED.concat(catalog.map((c) => c.name).filter((n) => !SUGGESTED.includes(n))).slice(0, 240);
    return fuzzysort.go(q, catalog, { keys: ["name", "tags"], limit: 240, threshold: 0.3 }).map((r) => r.obj.name);
  }, [q, catalog]);
  const appList = useMemo(() => {
    const withIcons = apps.filter((a) => a.iconPath);
    return q.trim() ? fuzzysort.go(q, withIcons, { keys: ["name", "id"], limit: 160 }).map((r) => r.obj) : withIcons.slice().sort((a, b) => a.name.localeCompare(b.name));
  }, [q, apps]);
  const savedList = useMemo(() => (q.trim() ? saved.filter((r) => r.toLowerCase().includes(q.trim().toLowerCase())) : saved), [q, saved]);

  const pick = (v: string | null) => {
    onChange(v);
    setOpen(false);
  };
  const imported = (ref: string) => {
    toggle(ref, true);
    pick(ref);
  };

  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        if (o) setTab(saved.length ? "saved" : "icons");
      }}
    >
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent padding="none" align="start" className="w-[360px]">
        <div className="flex flex-col gap-2 border-b border-line p-2">
          <Input autoFocus icon={Search} placeholder={tab === "apps" ? "Search apps" : tab === "saved" ? "Search saved icons" : "Search 2,000+ icons"} value={q} onChange={(e) => setQ(e.currentTarget.value)} />
          <div className="flex items-center justify-between gap-2">
            <Segmented
              size="sm"
              value={tab}
              onValueChange={(v) => setTab(v)}
              options={[
                { value: "saved", label: `Saved${saved.length ? ` · ${saved.length}` : ""}` },
                { value: "icons", label: "Icons" },
                { value: "apps", label: "Apps" },
              ]}
            />
            <Button size="sm" variant="ghost" icon={Ban} onClick={() => pick(null)}>
              None
            </Button>
          </div>
        </div>
        <ScrollArea className="h-72">
          {tab === "saved" && (
            <div className="p-2">
              {savedList.length > 0 ? (
                <div className="grid grid-cols-8 gap-0.5">
                  {savedList.map((ref) => (
                    <Cell key={ref} iconRef={ref} selected={value === ref} saved removable onPick={() => pick(ref)} onStar={() => toggle(ref, false)} />
                  ))}
                </div>
              ) : (
                <p className="px-4 py-8 text-center text-sm text-faint">Icons you upload, link or star show up here.</p>
              )}
            </div>
          )}
          {tab === "icons" && (
            <div className="grid grid-cols-8 gap-0.5 p-2">
              {icons.map((name) => {
                const ref = `lucide:${name}`;
                return <Cell key={name} iconRef={ref} selected={value === ref} saved={saved.includes(ref)} onPick={() => pick(ref)} onStar={() => toggle(ref)} />;
              })}
              {icons.length === 0 && <p className="col-span-8 py-8 text-center text-sm text-faint">{catalog.length ? "No icons match." : "Loading…"}</p>}
            </div>
          )}
          {tab === "apps" && (
            <div className="grid grid-cols-7 gap-1 p-2">
              {appList.map((a) => (
                <div key={a.id} className="group relative">
                  <button
                    type="button"
                    title={a.name}
                    onClick={() => pick(a.iconPath)}
                    className={cn("flex aspect-square w-full items-center justify-center rounded-sm transition-quick hover:bg-control", value === a.iconPath && "bg-accent-soft")}
                  >
                    {a.iconUrl && <img src={a.iconUrl} alt="" className="size-7 object-contain" />}
                  </button>
                  <button
                    type="button"
                    aria-label="Save for later"
                    onClick={() => a.iconPath && toggle(a.iconPath)}
                    className={cn(
                      "absolute -right-0.5 -top-0.5 flex size-4 items-center justify-center rounded-full bg-raised text-faint opacity-0 ring-1 ring-line hover:text-fg group-hover:opacity-100",
                      a.iconPath && saved.includes(a.iconPath) && "text-warning opacity-100",
                    )}
                  >
                    <Star size={9} fill={a.iconPath && saved.includes(a.iconPath) ? "currentColor" : "none"} />
                  </button>
                </div>
              ))}
              {appList.length === 0 && <p className="col-span-7 py-8 text-center text-sm text-faint">No app icons found.</p>}
            </div>
          )}
        </ScrollArea>
        <div className="flex items-center gap-1.5 border-t border-line p-2">
          <span className="mr-auto text-xs text-faint">Your own image:</span>
          <Button size="sm" variant="ghost" icon={Upload} onClick={() => file.current?.click()}>
            Upload
          </Button>
          <UrlImport
            kind="icon"
            onImported={imported}
            trigger={
              <Button size="sm" variant="ghost" icon={Link2}>
                Link
              </Button>
            }
          />
          <input
            ref={file}
            type="file"
            accept="image/png,image/jpeg,image/webp,image/gif,image/svg+xml"
            className="hidden"
            onChange={async (e) => {
              const input = e.currentTarget;
              const ref = await importDroppedImage(input.files, "icon");
              input.value = "";
              if (ref) {
                imported(ref);
                toast.success("Icon saved", { description: "Find it again under Saved." });
              }
            }}
          />
        </div>
      </PopoverContent>
    </Popover>
  );
}
