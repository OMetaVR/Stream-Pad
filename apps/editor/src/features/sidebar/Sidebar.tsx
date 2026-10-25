// Left pane: profiles (drag to reorder, double-click or F2 to rename, right-click for more) + Settings.

import { DndContext, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Copy, Download, Pencil, Plus, Settings, Trash2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { getBackend } from "@/lib/backend";
import { cn } from "@/lib/cn";
import { useHotkey } from "@/lib/hotkeys";
import type { Profile } from "@/lib/types";
import { useDaemon } from "@/state/daemon";
import { editor, useEditor, useRequest } from "@/state/store";
import { Button, ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger, IconButton, RefIcon, ScrollArea, toast, Tooltip } from "@/ui";

function exportProfile(p: Profile) {
  const blob = new Blob([JSON.stringify(p, null, 2)], { type: "application/json" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = `${(p.name || "profile").replace(/[^\w-]+/g, "-")}.streampad.json`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

function ProfileRow({ p, index, renaming, setRenaming }: { p: Profile; index: number; renaming: boolean; setRenaming(id: string | null): void }) {
  const selected = useEditor((s) => s.selection.profileId === p.id);
  const count = useEditor((s) => Object.keys(s.config!.profiles[index]?.keys ?? {}).length);
  const active = useDaemon((s) => (s.state ? s.state.profiles[s.state.profileIndex]?.id === p.id : false));
  const connected = useDaemon((s) => s.status.kind === "connected");
  // A profile without its own colour uses the default accent — not the live accent, which follows the
  // selected profile.
  const defaultAccent = useEditor((s) => s.config!.settings.overlay.accent);
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: p.id, disabled: renaming });
  const [draft, setDraft] = useState(p.name);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (renaming) {
      setDraft(p.name);
      requestAnimationFrame(() => input.current?.select());
    }
  }, [renaming, p.name]);

  const commit = () => {
    const name = draft.trim() || "Untitled";
    if (name !== p.name) editor.renameProfile(p.id, name);
    setRenaming(null);
  };

  return (
    <li ref={setNodeRef} style={{ transform: CSS.Translate.toString(transform), transition }} className={cn(isDragging && "relative z-10 opacity-80")}>
      <ContextMenu onOpenChange={(o) => o && editor.selectProfile(p.id)}>
        <ContextMenuTrigger asChild>
          <div
            {...attributes}
            {...listeners}
            role="button"
            tabIndex={0}
            onClick={() => editor.selectProfile(p.id)}
            onDoubleClick={() => setRenaming(p.id)}
            onKeyDown={(e) => {
              if (renaming) return;
              if (e.key === "Enter" || e.key === " ") editor.selectProfile(p.id);
              if (e.key === "F2") setRenaming(p.id);
            }}
            aria-current={selected ? "true" : undefined}
            className={cn(
              "group flex h-9 w-full cursor-default items-center gap-2.5 rounded-sm px-2 text-left text-base transition-quick",
              selected ? "bg-control text-fg" : "text-muted hover:bg-control/60 hover:text-fg",
            )}
          >
            <span className="size-2 shrink-0 rounded-full" style={{ background: p.color ?? defaultAccent }} />
            {p.icon ? <RefIcon iconRef={p.icon} size={15} className="shrink-0" /> : null}
            {renaming ? (
              <input
                ref={input}
                value={draft}
                onChange={(e) => setDraft(e.currentTarget.value)}
                onBlur={commit}
                onKeyDown={(e) => {
                  e.stopPropagation();
                  if (e.key === "Enter") commit();
                  if (e.key === "Escape") setRenaming(null);
                }}
                onPointerDown={(e) => e.stopPropagation()}
                aria-label="Profile name"
                className="h-6 min-w-0 flex-1 rounded-[4px] bg-raised px-1.5 text-fg outline-none ring-1 ring-accent-line"
              />
            ) : (
              <span className="min-w-0 flex-1 truncate">{p.name || "Untitled"}</span>
            )}
            {!renaming && <span className="text-xs tabular-nums text-faint opacity-0 transition-quick group-hover:opacity-100">{count}</span>}
            {!renaming && connected && (
              <Tooltip content={active ? "Active on the pad" : "Make active on the pad"}>
                <button
                  type="button"
                  aria-label={active ? "Active on the pad" : `Switch the pad to ${p.name}`}
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={(e) => {
                    e.stopPropagation();
                    if (!active) void getBackend().daemonRequest("setProfile", { id: p.id }).catch((err) => toast.error("Couldn't switch profile", { description: String(err) }));
                  }}
                  className="flex size-5 items-center justify-center"
                >
                  <span className={cn("size-1.5 rounded-full transition-quick", active ? "bg-success" : "bg-transparent ring-1 ring-line-strong opacity-0 group-hover:opacity-100")} />
                </button>
              </Tooltip>
            )}
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem icon={Pencil} shortcut="f2" onSelect={() => setRenaming(p.id)}>
            Rename
          </ContextMenuItem>
          <ContextMenuItem icon={Copy} onSelect={() => editor.duplicateProfile(p.id)}>
            Duplicate
          </ContextMenuItem>
          <ContextMenuItem icon={Download} onSelect={() => exportProfile(p)}>
            Export…
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem icon={Trash2} destructive onSelect={() => editor.deleteProfile(p.id)}>
            Delete
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
    </li>
  );
}

export function Sidebar() {
  const profiles = useEditor((s) => s.config!.profiles);
  const selected = useEditor((s) => s.selection.profileId);
  const [renaming, setRenaming] = useState<string | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));
  useRequest("renameProfile", (id) => setRenaming(typeof id === "string" ? id : selected));
  useHotkey("f2", () => (selected ? void setRenaming(selected) : false), { description: "Rename profile" });
  useHotkey("mod+n", () => void editor.addProfile(), { description: "New profile" });

  const onDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return;
    editor.reorderProfiles(
      profiles.findIndex((p) => p.id === e.active.id),
      profiles.findIndex((p) => p.id === e.over!.id),
    );
  };

  return (
    <aside className="flex h-full min-h-0 flex-col border-r border-line bg-panel">
      <div className="flex h-11 items-center justify-between pl-4 pr-2">
        <span className="text-sm font-medium text-muted">Profiles</span>
        <IconButton
          icon={Plus}
          label="New profile"
          shortcut="mod+n"
          onClick={() => {
            const id = editor.addProfile();
            if (id) setRenaming(id);
          }}
        />
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
          <SortableContext items={profiles.map((p) => p.id)} strategy={verticalListSortingStrategy}>
            <ul className="flex flex-col gap-px px-2 pb-2">
              {profiles.map((p, i) => (
                <ProfileRow key={p.id} p={p} index={i} renaming={renaming === p.id} setRenaming={setRenaming} />
              ))}
            </ul>
          </SortableContext>
        </DndContext>
        <p className="px-4 pt-1 text-xs leading-relaxed text-faint">
          Numpad <span className="text-muted">+</span> / <span className="text-muted">−</span> cycle through these in order. Drag to reorder.
        </p>
      </ScrollArea>
      <div className="border-t border-line p-2">
        <Button variant="ghost" icon={Settings} className="w-full justify-start" onClick={() => editor.openSettings()}>
          Settings
        </Button>
      </div>
    </aside>
  );
}
