// The hero: a faithful numpad for the selected profile. Click / arrow keys select, Enter picks an action,
// drop images for covers, drag keys onto keys to swap (Alt = copy), right-click for more.

import { ClipboardPaste, Copy, Eye, ImagePlus, ImageOff, Lock, MousePointerClick, Palette, Scissors, Trash2 } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { getBackend } from "@/lib/backend";
import { cn } from "@/lib/cn";
import { useHotkey } from "@/lib/hotkeys";
import { GLOBAL_ACTION_INFO, globalOf } from "@/lib/model";
import { KEY_IDS, KEY_INFO, type KeyId } from "@/lib/types";
import { useDaemon, useDaemonEvent, useKeyPress } from "@/state/daemon";
import { useConfig, useSelectedProfile } from "@/state/selectors";
import { editor, useEditor } from "@/state/store";
import {
  Button,
  ColorPicker,
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
  Popover,
  PopoverContent,
  PopoverTrigger,
  RefIcon,
  toast,
  Tooltip,
} from "@/ui";
import { CoverImage } from "../inspector/coverImage";
import { importDroppedImage } from "../inspector/CoverPicker";
import { IconPicker } from "../inspector/IconPicker";

const UNIT = 76;
const GAP = 10;
const PAD = 16;
const KEY_MIME = "application/x-streampad-key";

/** Spatial neighbour for arrow navigation, honouring spans. */
function neighbour(from: KeyId, dx: number, dy: number): KeyId {
  const g = KEY_INFO[from].grid;
  let col = dx ? (dx > 0 ? g.col + g.colSpan : g.col - 1) : g.col;
  let row = dy ? (dy > 0 ? g.row + g.rowSpan : g.row - 1) : g.row;
  for (let step = 0; step < 5; step++) {
    if (col < 0 || col > 3 || row < 0 || row > 4) return from;
    const hit = KEY_IDS.find((k) => {
      const h = KEY_INFO[k].grid;
      return col >= h.col && col < h.col + h.colSpan && row >= h.row && row < h.row + h.rowSpan;
    });
    if (hit && hit !== from) return hit;
    col += dx;
    row += dy;
  }
  return from;
}

function Keycap({ keyId, profileId }: { keyId: KeyId; profileId: string }) {
  const config = useConfig();
  const binding = useEditor((s) => s.config!.profiles.find((p) => p.id === profileId)?.keys[keyId]);
  const selected = useEditor((s) => s.selection.keyId === keyId);
  const clipboard = useEditor((s) => s.clipboard);
  const { pressed, seq } = useKeyPress(keyId);
  const [flash, setFlash] = useState(false);
  const [dropping, setDropping] = useState(false);
  const info = KEY_INFO[keyId];
  const global = globalOf(config.settings, keyId);
  const cover = !global && binding?.cover ? getBackend().assetUrl(binding.cover) : null;
  const label = global ? GLOBAL_ACTION_INFO[global.tap].label : binding?.label;
  const icon = global ? `lucide:${GLOBAL_ACTION_INFO[global.tap].icon}` : binding?.icon;
  const empty = !global && !binding?.macro && !binding?.label && !binding?.icon && !cover;

  useEffect(() => {
    if (!seq) return;
    setFlash(true);
    const t = setTimeout(() => setFlash(false), 320);
    return () => clearTimeout(t);
  }, [seq]);

  const cap = (
    <button
      type="button"
      data-key={keyId}
      onClick={() => editor.selectKey(keyId)}
      onDoubleClick={() => !global && editor.request("actionPicker", { profileId, keyId })}
      draggable={!global}
      onDragStart={(e) => {
        e.dataTransfer.setData(KEY_MIME, keyId);
        e.dataTransfer.effectAllowed = "copyMove";
      }}
      onDragOver={(e) => {
        if (global) return;
        if (e.dataTransfer.types.includes("Files") || e.dataTransfer.types.includes(KEY_MIME)) {
          e.preventDefault();
          e.dataTransfer.dropEffect = e.altKey || e.dataTransfer.types.includes("Files") ? "copy" : "move";
          setDropping(true);
        }
      }}
      onDragLeave={() => setDropping(false)}
      onDrop={async (e) => {
        e.preventDefault();
        setDropping(false);
        const from = e.dataTransfer.getData(KEY_MIME) as KeyId;
        if (from && from !== keyId) {
          if (e.altKey) editor.copyBindingTo(profileId, from, keyId);
          else editor.swapBindings(profileId, from, keyId);
          return;
        }
        const ref = await importDroppedImage(e.dataTransfer.files);
        if (ref) {
          editor.updateBinding(profileId, keyId, (b) => void ((b.cover = ref), (b.coverCrop = null)), { coalesce: false });
          editor.selectKey(keyId);
        }
      }}
      aria-pressed={selected}
      aria-label={`${info.name}${label ? `: ${label}` : ""}`}
      style={{
        gridColumn: `${info.grid.col + 1} / span ${info.grid.colSpan}`,
        gridRow: `${info.grid.row + 1} / span ${info.grid.rowSpan}`,
        ...(binding?.color && !global ? { background: `color-mix(in srgb, ${binding.color} 26%, var(--sp-keycap))` } : null),
      }}
      className={cn(
        "group relative flex flex-col items-center justify-center gap-1 overflow-hidden rounded-[11px] bg-keycap pt-3 text-fg outline-none transition-[transform,box-shadow,filter] duration-100",
        "shadow-[inset_0_1px_0_var(--sp-keycap-highlight),0_3px_0_var(--sp-keycap-skirt)] hover:brightness-110",
        (pressed || flash) && "translate-y-0.5 shadow-[inset_0_1px_0_var(--sp-keycap-highlight),0_1px_0_var(--sp-keycap-skirt),0_0_0_3px_var(--sp-accent-soft)]",
        selected && "ring-2 ring-accent ring-offset-2 ring-offset-(--sp-keybed)",
        dropping && "ring-2 ring-accent/60",
        "focus-visible:ring-2 focus-visible:ring-accent",
      )}
    >
      {cover && <CoverImage src={cover} crop={binding?.coverCrop ?? null} />}
      <span className={cn("absolute left-2 top-1.5 flex items-baseline gap-1 leading-none", cover && "rounded-[4px] bg-black/55 px-1 py-0.5")}>
        <span className={cn("text-sm font-semibold", cover ? "text-white" : empty ? "text-faint" : "text-muted")}>{info.legend.main}</span>
        {info.legend.sub && !cover && <span className="text-2xs text-faint">{info.legend.sub}</span>}
      </span>
      {global && <Lock size={11} className="absolute right-2 top-2 text-faint" aria-hidden />}
      {!cover && icon && <RefIcon iconRef={icon} size={22} className={global ? "text-accent-text" : "text-fg"} />}
      {!cover && label && <span className="line-clamp-2 max-w-full px-2 text-center text-xs leading-tight text-muted">{label}</span>}
      {!cover && empty && selected && <span className="text-2xs text-faint">Enter to set up</span>}
    </button>
  );

  if (global) {
    return (
      <Tooltip content={`Global: ${GLOBAL_ACTION_INFO[global.tap].description.toLowerCase()} — change in Settings › Global keys`}>{cap}</Tooltip>
    );
  }
  return (
    <ContextMenu onOpenChange={(o) => o && editor.selectKey(keyId)}>
      <ContextMenuTrigger asChild>{cap}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem icon={MousePointerClick} shortcut="enter" onSelect={() => editor.request("actionPicker", { profileId, keyId })}>
          {binding?.macro ? "Add a step…" : "Choose an action…"}
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem icon={Copy} shortcut="mod+c" disabled={!binding} onSelect={() => editor.copyBinding(profileId, keyId)}>
          Copy
        </ContextMenuItem>
        <ContextMenuItem icon={ClipboardPaste} shortcut="mod+v" disabled={!clipboard} onSelect={() => editor.pasteBinding(profileId, keyId)}>
          Paste
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem
          icon={ImagePlus}
          onSelect={async () => {
            const path = await getBackend().pickImage();
            if (!path) return;
            const ref = path.startsWith("data:") ? path : await getBackend().importCover(path);
            editor.updateBinding(profileId, keyId, (b) => void ((b.cover = ref), (b.coverCrop = null)), { coalesce: false });
          }}
        >
          Set cover…
        </ContextMenuItem>
        {binding?.cover && (
          <ContextMenuItem icon={ImageOff} onSelect={() => editor.updateBinding(profileId, keyId, (b) => void ((b.cover = null), (b.coverCrop = null)), { coalesce: false })}>
            Remove cover
          </ContextMenuItem>
        )}
        <ContextMenuSeparator />
        <ContextMenuItem icon={Trash2} destructive shortcut="delete" disabled={!binding} onSelect={() => editor.clearBinding(profileId, keyId)}>
          Clear key
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  );
}

function useFitScale(ref: React.RefObject<HTMLDivElement | null>) {
  const [scale, setScale] = useState(1);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const w = 4 * UNIT + 3 * GAP + 2 * PAD;
    const h = 5 * UNIT + 4 * GAP + 2 * PAD;
    const ro = new ResizeObserver(([e]) => {
      const { width, height } = e!.contentRect;
      setScale(Math.max(0.6, Math.min(1.15, (width - 48) / w, (height - 48) / h)));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [ref]);
  return scale;
}

function Toolbar({ profileId }: { profileId: string }) {
  const profile = useSelectedProfile()!;
  const learn = useDaemon((s) => s.state?.learn ?? false);
  const defaultAccent = useEditor((s) => s.config!.settings.overlay.accent);
  const connected = useDaemon((s) => s.status.kind === "connected");
  const [name, setName] = useState(profile.name);
  useEffect(() => setName(profile.name), [profile.name, profile.id]);

  useDaemonEvent("learn", ({ key }) => {
    editor.selectKey(key);
    document.querySelector<HTMLElement>(`[data-key="${key}"]`)?.focus();
  });
  useEffect(() => {
    if (!learn) return;
    const off = () => void getBackend().daemonRequest("setLearn", { on: false }).catch(() => {});
    window.addEventListener("blur", off);
    return () => window.removeEventListener("blur", off);
  }, [learn]);

  const daemon = async (method: string, params: unknown, failure: string) => {
    try {
      await getBackend().daemonRequest(method, params);
    } catch (e) {
      toast.error(failure, { description: e instanceof Error ? e.message : String(e) });
    }
  };

  return (
    <div className="flex h-12 shrink-0 items-center gap-2 px-4">
      <IconPicker
        value={profile.icon}
        onChange={(icon) => editor.updateProfile(profileId, (p) => void (p.icon = icon))}
        trigger={
          <button type="button" aria-label="Profile icon" title="Profile icon" className="flex size-8 items-center justify-center rounded-sm text-muted transition-quick hover:bg-control hover:text-fg">
            {profile.icon ? <RefIcon iconRef={profile.icon} size={16} /> : <Palette size={16} />}
          </button>
        }
      />
      <input
        value={name}
        onChange={(e) => {
          setName(e.currentTarget.value);
          editor.renameProfile(profileId, e.currentTarget.value);
        }}
        onBlur={() => !name.trim() && editor.renameProfile(profileId, "Untitled")}
        onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        aria-label="Profile name"
        className="h-8 min-w-0 max-w-64 rounded-sm bg-transparent px-1.5 text-md font-semibold text-fg outline-none hover:bg-control/60 focus:bg-control"
        style={{ width: `${Math.max(6, name.length + 2)}ch` }}
      />
      <Popover>
        <PopoverTrigger asChild>
          <button type="button" aria-label="Profile colour" className="size-4 rounded-full ring-1 ring-line-strong transition-quick hover:scale-110" style={{ background: profile.color ?? defaultAccent }} />
        </PopoverTrigger>
        <PopoverContent className="w-64" align="start">
          <ColorPicker value={profile.color} allowNone onChange={(c) => editor.updateProfile(profileId, (p) => void (p.color = c))} />
        </PopoverContent>
      </Popover>
      <div className="flex-1" />
      <Button
        size="sm"
        variant={learn ? "primary" : "ghost"}
        icon={MousePointerClick}
        disabled={!connected}
        title="Press a key on your numpad (in macro mode) to select it here"
        onClick={() => void daemon("setLearn", { on: !learn }, "Couldn't start press-to-select")}
      >
        {learn ? "Press a key on your numpad…" : "Press to select"}
      </Button>
      <Button size="sm" variant="ghost" icon={Eye} disabled={!connected} onClick={() => void daemon("overlay", { action: "peek", preview: true, durationMs: 3000 }, "Couldn't show the overlay")}>
        Preview overlay
      </Button>
    </div>
  );
}

export function NumpadCanvas() {
  const profile = useSelectedProfile();
  const selection = useEditor((s) => s.selection);
  const stage = useRef<HTMLDivElement>(null);
  const scale = useFitScale(stage);
  const config = useConfig();

  const keyId = selection.keyId;
  const pid = profile?.id;
  const bound = !!(keyId && pid && !globalOf(config.settings, keyId));
  const move = (dx: number, dy: number) => () => {
    const next = keyId ? neighbour(keyId, dx, dy) : "KP5";
    editor.selectKey(next);
    document.querySelector<HTMLElement>(`[data-key="${next}"]`)?.focus();
  };
  useHotkey("arrowleft", move(-1, 0));
  useHotkey("arrowright", move(1, 0));
  useHotkey("arrowup", move(0, -1));
  useHotkey("arrowdown", move(0, 1));
  useHotkey("enter", () => (bound ? editor.request("actionPicker", { profileId: pid, keyId }) : false), { description: "Choose an action" });
  useHotkey(["delete", "backspace"], () => (bound ? void editor.clearBinding(pid!, keyId!) : false), { description: "Clear key" });
  useHotkey("mod+c", () => (bound ? void editor.copyBinding(pid!, keyId!) : false), { description: "Copy key" });
  useHotkey("mod+x", () => (bound ? void (editor.copyBinding(pid!, keyId!), editor.clearBinding(pid!, keyId!)) : false), { description: "Cut key" });
  useHotkey("mod+v", () => (bound ? void editor.pasteBinding(pid!, keyId!) : false), { description: "Paste key" });

  if (!profile) return null;
  return (
    <section className="flex h-full min-h-0 flex-col bg-bg" aria-label="Numpad">
      <Toolbar profileId={profile.id} />
      <div ref={stage} className="flex min-h-0 flex-1 items-center justify-center" onClick={(e) => e.target === e.currentTarget && editor.selectKey(null)}>
        <div style={{ transform: `scale(${scale})` }} className="transition-transform duration-200">
          <div
            className="grid rounded-xl border border-line bg-keybed shadow-raised"
            style={{ padding: PAD, gridTemplateColumns: `repeat(4, ${UNIT}px)`, gridTemplateRows: `repeat(5, ${UNIT}px)`, gap: GAP }}
          >
            {KEY_IDS.map((k) => (
              <Keycap key={k} keyId={k} profileId={profile.id} />
            ))}
          </div>
        </div>
      </div>
      <p className="flex shrink-0 items-center justify-center gap-1.5 pb-4 text-xs text-faint">
        <Scissors size={12} /> Drag a key onto another to swap (hold Alt to copy) · drop an image on a key to use it as the cover
      </p>
    </section>
  );
}
