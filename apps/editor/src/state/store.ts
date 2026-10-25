// The editor store: the loaded Config plus undo/redo history, selection, view routing, the binding
// clipboard and a tiny request channel between features. Autosave lives in ./autosave.ts.
//
// Read with the hook:          const profile = useEditor((s) => findProfile(s.config!, s.selection.profileId));
// Write through the actions:   editor.updateBinding(profileId, "KP4", (b) => { b.label = "Mute"; });
//
// Every config mutation goes through `edit()`, which records one undo step. Rapid edits that share a
// coalesce key (typing into one field, dragging a slider) merge into a single step.

import { produce, type Draft } from "immer";
import { useEffect, useRef } from "react";
import { create } from "zustand";
import { getBackend } from "@/lib/backend";
import { deepClone, emptyBinding, findProfile, isBindingEmpty, isGlobalKey, newId, newProfile, uniqueName } from "@/lib/model";
import type { Binding, Config, KeyId, Profile, Settings } from "@/lib/types";
import { toast } from "@/ui/Toaster";

// ---------------------------------------------------------------------------------------------------------
// Types

export type SettingsSection = "general" | "globals" | "overlay" | "devices" | "scripting" | "about";

export type View =
  | { kind: "main" }
  | { kind: "settings"; section?: SettingsSection }
  | { kind: "flow"; profileId: string; keyId: KeyId }
  | { kind: "script"; profileId: string; keyId: KeyId };

export type LoadState = { kind: "loading" } | { kind: "ready" } | { kind: "error"; message: string };

export type SaveStatus = "idle" | "saving" | "saved" | "error";

export interface Selection {
  profileId: string | null;
  keyId: KeyId | null;
}

export interface HistoryOptions {
  /** Edits with the same key in quick succession merge into one undo step; `false` never merges. */
  coalesce?: string | false;
  /** Short description of the change ("Rename profile"). */
  label?: string;
}

export interface HistoryEntry {
  /** Config before the change. */
  config: Config;
  /** Selection before the change (restored on undo). */
  selection: Selection;
  label: string;
  coalesce: string | false;
  startedAt: number;
  at: number;
}

/** Names used with `editor.request()` / `useRequest()`. Features may add their own string names. */
export type RequestName = "actionPicker" | "iconPicker" | "renameProfile" | "focusLabel" | (string & {});

export interface EditorState {
  config: Config | null;
  load: LoadState;
  save: { status: SaveStatus; error: string | null; savedAt: number | null };
  past: HistoryEntry[];
  future: HistoryEntry[];
  selection: Selection;
  view: View;
  clipboard: Binding | null;
  requests: Partial<Record<RequestName, { n: number; payload?: unknown }>>;
}

/** Merge window between two edits with the same coalesce key, and the longest single merged burst. */
const COALESCE_MS = 900;
const COALESCE_MAX_MS = 6000;
const HISTORY_LIMIT = 200;
const LAST_PROFILE_KEY = "streampad.editor.lastProfile";

export const useEditor = create<EditorState>()(() => ({
  config: null,
  load: { kind: "loading" },
  save: { status: "idle", error: null, savedAt: null },
  past: [],
  future: [],
  selection: { profileId: null, keyId: null },
  view: { kind: "main" },
  clipboard: null,
  requests: {},
}));

const get = useEditor.getState;
const set = useEditor.setState;

// ---------------------------------------------------------------------------------------------------------
// Core edit + history

function validSelection(sel: Selection, config: Config): Selection {
  const profileId = findProfile(config, sel.profileId)?.id ?? config.profiles[0]?.id ?? null;
  return { profileId, keyId: sel.keyId };
}

/**
 * Apply `recipe` to a draft of the config and record an undo step. Returns false (and records nothing)
 * when nothing is loaded or the recipe made no change. `select` may move the selection in the same step.
 */
function edit(
  recipe: (draft: Draft<Config>) => void,
  history: Required<HistoryOptions>,
  select?: (config: Config) => Partial<Selection>,
): boolean {
  const s = get();
  if (!s.config) return false;
  const next = produce(s.config, recipe);
  if (next === s.config) return false;
  const now = Date.now();
  const top = s.past[s.past.length - 1];
  const merge =
    history.coalesce !== false &&
    top !== undefined &&
    top.coalesce === history.coalesce &&
    now - top.at < COALESCE_MS &&
    now - top.startedAt < COALESCE_MAX_MS;
  const past = merge
    ? [...s.past.slice(0, -1), { ...top, at: now }]
    : [
        ...s.past,
        { config: s.config, selection: s.selection, label: history.label, coalesce: history.coalesce, startedAt: now, at: now },
      ].slice(-HISTORY_LIMIT);
  const selection = select ? { ...s.selection, ...select(next) } : s.selection;
  set({ config: next, past, future: [], selection: validSelection(selection, next) });
  return true;
}

function opts(o: HistoryOptions | undefined, label: string, coalesce: string | false): Required<HistoryOptions> {
  return { label: o?.label ?? label, coalesce: o?.coalesce ?? coalesce };
}

function undo(): boolean {
  const s = get();
  const entry = s.past[s.past.length - 1];
  if (!entry || !s.config) return false;
  const now = Date.now();
  set({
    config: entry.config,
    selection: validSelection(entry.selection, entry.config),
    past: s.past.slice(0, -1),
    future: [...s.future, { ...entry, config: s.config, selection: s.selection, coalesce: false, startedAt: now, at: now }],
  });
  return true;
}

function redo(): boolean {
  const s = get();
  const entry = s.future[s.future.length - 1];
  if (!entry || !s.config) return false;
  const now = Date.now();
  set({
    config: entry.config,
    selection: validSelection(entry.selection, entry.config),
    future: s.future.slice(0, -1),
    past: [...s.past, { ...entry, config: s.config, selection: s.selection, coalesce: false, startedAt: now, at: now }],
  });
  return true;
}

// ---------------------------------------------------------------------------------------------------------
// Loading

function rememberProfile(id: string | null) {
  try {
    if (id) localStorage.setItem(LAST_PROFILE_KEY, id);
  } catch {
    // ignore: only a convenience
  }
}

async function load(): Promise<void> {
  set({ load: { kind: "loading" } });
  try {
    const config = await getBackend().loadConfig();
    let last: string | null = null;
    try {
      last = localStorage.getItem(LAST_PROFILE_KEY);
    } catch {
      last = null;
    }
    set({
      config,
      load: { kind: "ready" },
      past: [],
      future: [],
      selection: validSelection({ profileId: last, keyId: null }, config),
      save: { status: "idle", error: null, savedAt: null },
    });
  } catch (e) {
    set({ load: { kind: "error", message: e instanceof Error ? e.message : String(e) } });
  }
}

useEditor.subscribe((s, prev) => {
  if (s.selection.profileId !== prev.selection.profileId) rememberProfile(s.selection.profileId);
});

// ---------------------------------------------------------------------------------------------------------
// Profiles

function addProfile(options: { name?: string; after?: string; select?: boolean } = {}): string | null {
  const config = get().config;
  if (!config) return null;
  const profile = newProfile(uniqueName(options.name?.trim() || "New profile", config.profiles.map((p) => p.name)));
  const ok = edit(
    (c) => {
      const i = options.after ? c.profiles.findIndex((p) => p.id === options.after) : -1;
      c.profiles.splice(i >= 0 ? i + 1 : c.profiles.length, 0, profile);
    },
    opts(undefined, "Add profile", false),
    options.select === false ? undefined : () => ({ profileId: profile.id }),
  );
  return ok ? profile.id : null;
}

function duplicateProfile(id: string): string | null {
  const config = get().config;
  const source = config && findProfile(config, id);
  if (!config || !source) return null;
  const copy: Profile = {
    ...deepClone(source),
    id: newId("p"),
    name: uniqueName(`${source.name} copy`, config.profiles.map((p) => p.name)),
  };
  edit(
    (c) => {
      c.profiles.splice(c.profiles.findIndex((p) => p.id === id) + 1, 0, copy);
    },
    opts(undefined, "Duplicate profile", false),
    () => ({ profileId: copy.id }),
  );
  return copy.id;
}

/** Re-insert a deleted profile (the delete toast's Undo). */
function restoreProfile(profile: Profile, index: number): void {
  edit(
    (c) => {
      if (c.profiles.some((p) => p.id === profile.id)) return;
      c.profiles.splice(Math.min(index, c.profiles.length), 0, profile);
    },
    opts(undefined, "Restore profile", false),
    () => ({ profileId: profile.id }),
  );
}

/**
 * Delete a profile (never the last one). Shows an undo toast by default — StreamPad uses undo instead of
 * confirmation dialogs for deletes.
 */
function deleteProfile(id: string, options: { toast?: boolean } = {}): boolean {
  const config = get().config;
  if (!config || config.profiles.length <= 1) return false;
  const index = config.profiles.findIndex((p) => p.id === id);
  const profile = config.profiles[index];
  if (!profile) return false;
  const neighbour = config.profiles[index + 1] ?? config.profiles[index - 1];
  const wasSelected = get().selection.profileId === id;
  const ok = edit(
    (c) => {
      c.profiles.splice(index, 1);
    },
    opts(undefined, "Delete profile", false),
    wasSelected ? () => ({ profileId: neighbour?.id ?? null }) : undefined,
  );
  if (ok && options.toast !== false) {
    toast({
      title: `Deleted “${profile.name || "Untitled"}”`,
      action: { label: "Undo", onClick: () => restoreProfile(profile, index) },
    });
  }
  return ok;
}

function renameProfile(id: string, name: string, history?: HistoryOptions): boolean {
  return edit(
    (c) => {
      const p = c.profiles.find((x) => x.id === id);
      if (p) p.name = name;
    },
    opts(history, "Rename profile", `rename:${id}`),
  );
}

function updateProfile(id: string, fn: (draft: Draft<Profile>) => void, history?: HistoryOptions): boolean {
  return edit(
    (c) => {
      const p = c.profiles.find((x) => x.id === id);
      if (p) fn(p);
    },
    opts(history, "Edit profile", `profile:${id}`),
  );
}

function reorderProfiles(from: number, to: number): boolean {
  return edit(
    (c) => {
      if (from === to || from < 0 || to < 0 || from >= c.profiles.length || to >= c.profiles.length) return;
      const [moved] = c.profiles.splice(from, 1);
      c.profiles.splice(to, 0, moved!);
    },
    opts(undefined, "Reorder profiles", false),
  );
}

// ---------------------------------------------------------------------------------------------------------
// Bindings

function canBind(config: Config, profileId: string, keyId: KeyId): boolean {
  return !!findProfile(config, profileId) && !isGlobalKey(config.settings, keyId);
}

/** Replace a key's binding (`null` clears it). Refuses global keys. */
function setBinding(profileId: string, keyId: KeyId, binding: Binding | null, history?: HistoryOptions): boolean {
  const config = get().config;
  if (!config || !canBind(config, profileId, keyId)) return false;
  return edit(
    (c) => {
      const p = c.profiles.find((x) => x.id === profileId)!;
      if (binding === null || isBindingEmpty(binding)) delete p.keys[keyId];
      else p.keys[keyId] = binding;
    },
    opts(history, binding ? "Set key" : "Clear key", false),
  );
}

/** Edit a key's binding in place (created empty if missing; dropped again if the edit leaves it empty). */
function updateBinding(
  profileId: string,
  keyId: KeyId,
  fn: (draft: Draft<Binding>) => void,
  history?: HistoryOptions,
): boolean {
  const config = get().config;
  if (!config || !canBind(config, profileId, keyId)) return false;
  return edit(
    (c) => {
      const p = c.profiles.find((x) => x.id === profileId)!;
      const b = p.keys[keyId] ?? emptyBinding();
      fn(b);
      if (isBindingEmpty(b)) delete p.keys[keyId];
      else p.keys[keyId] = b;
    },
    opts(history, "Edit key", `binding:${profileId}:${keyId}`),
  );
}

/** Swap two keys' bindings within a profile (drag a key onto another). */
function swapBindings(profileId: string, a: KeyId, b: KeyId): boolean {
  const config = get().config;
  if (!config || a === b || !canBind(config, profileId, a) || !canBind(config, profileId, b)) return false;
  return edit(
    (c) => {
      const p = c.profiles.find((x) => x.id === profileId)!;
      const [ba, bb] = [p.keys[a], p.keys[b]];
      if (bb) p.keys[a] = bb;
      else delete p.keys[a];
      if (ba) p.keys[b] = ba;
      else delete p.keys[b];
    },
    opts(undefined, "Swap keys", false),
    () => ({ keyId: b }),
  );
}

/** Copy one key's binding onto another key (Alt-drag). */
function copyBindingTo(profileId: string, from: KeyId, to: KeyId): boolean {
  const config = get().config;
  const source = config ? findProfile(config, profileId)?.keys[from] : undefined;
  return setBinding(profileId, to, source ? deepClone(source) : null, { label: "Copy key" });
}

/** Put a key's binding on the binding clipboard (and the system clipboard as JSON, best effort). */
function copyBinding(profileId: string, keyId: KeyId): boolean {
  const config = get().config;
  const binding = config && findProfile(config, profileId)?.keys[keyId];
  if (!binding) return false;
  set({ clipboard: deepClone(binding) });
  navigator.clipboard?.writeText(JSON.stringify(binding, null, 2)).catch(() => undefined);
  return true;
}

function pasteBinding(profileId: string, keyId: KeyId): boolean {
  const clip = get().clipboard;
  return !!clip && setBinding(profileId, keyId, deepClone(clip), { label: "Paste key" });
}

// ---------------------------------------------------------------------------------------------------------
// Settings + whole config

function updateSettings(fn: (draft: Draft<Settings>) => void, history?: HistoryOptions): boolean {
  return edit((c) => fn(c.settings), opts(history, "Change settings", "settings"));
}

function updateConfig(fn: (draft: Draft<Config>) => void, history?: HistoryOptions): boolean {
  return edit(fn, opts(history, "Edit", false));
}

// ---------------------------------------------------------------------------------------------------------
// Request channel

/** Ask whichever feature listens (see `useRequest`) to do something, e.g. open the action picker. */
function request(name: RequestName, payload?: unknown): void {
  set((s) => ({ requests: { ...s.requests, [name]: { n: (s.requests[name]?.n ?? 0) + 1, payload } } }));
}

/** Run `handler(payload)` whenever `editor.request(name, payload)` is called after mount. */
export function useRequest(name: RequestName, handler: (payload: unknown) => void): void {
  const ref = useRef(handler);
  ref.current = handler;
  useEffect(
    () =>
      useEditor.subscribe((s, prev) => {
        const cur = s.requests[name];
        if (cur && cur !== prev.requests[name]) ref.current(cur.payload);
      }),
    [name],
  );
}

// ---------------------------------------------------------------------------------------------------------
// Public action surface

export const editor = {
  load,
  undo,
  redo,

  updateConfig,
  updateSettings,

  addProfile,
  duplicateProfile,
  deleteProfile,
  restoreProfile,
  renameProfile,
  updateProfile,
  reorderProfiles,

  setBinding,
  updateBinding,
  clearBinding: (profileId: string, keyId: KeyId) => setBinding(profileId, keyId, null),
  swapBindings,
  copyBindingTo,
  copyBinding,
  pasteBinding,

  /** True when `keyId` carries an active global (profile bindings there are ignored and refused). */
  isGlobalKey: (keyId: KeyId): boolean => {
    const config = get().config;
    return !!config && isGlobalKey(config.settings, keyId);
  },

  selectProfile: (profileId: string) => set((s) => ({ selection: { ...s.selection, profileId } })),
  selectKey: (keyId: KeyId | null) => set((s) => ({ selection: { ...s.selection, keyId } })),

  setView: (view: View) => set({ view }),
  showMain: () => set({ view: { kind: "main" } }),
  openSettings: (section?: SettingsSection) => set({ view: { kind: "settings", section } }),
  openFlow: (profileId: string, keyId: KeyId) => set({ view: { kind: "flow", profileId, keyId } }),
  openScript: (profileId: string, keyId: KeyId) => set({ view: { kind: "script", profileId, keyId } }),

  request,
};

/** Internal: used by ./autosave.ts only. */
export function setSaveState(save: EditorState["save"]): void {
  set({ save });
}
