// Global keyboard shortcut registry. One window-level listener; features register handlers into it.
//
//   const off = registerHotkey({ combo: "mod+z", handler: () => undo(), description: "Undo" });
//   useHotkey("delete", () => clearKey(), { when: () => canClear }, [canClear]);
//
// Combos: "+"-separated, modifiers first — mod (= Ctrl), ctrl, shift, alt, meta — then a key as named by
// KeyboardEvent.key, lower-case ("z", "delete", "arrowleft", "escape", "enter", "f2", "/").
// Priority: the most recently registered matching hotkey runs first; a handler returning `false` passes
// the event on to the next one. Matching hotkeys call preventDefault() unless `preventDefault: false`.
// By default hotkeys do not fire while typing in inputs/textareas/CodeMirror (so their native undo, arrows,
// Delete … keep working), nor inside open dialogs/menus; opt in with `allowInInputs` / `allowInOverlays`.

import { useEffect, useRef } from "react";

export interface HotkeyOptions {
  /** Extra guard evaluated at key time. */
  when?: () => boolean;
  allowInInputs?: boolean;
  allowInOverlays?: boolean;
  /** Fire on auto-repeat (held key). Default false. */
  repeat?: boolean;
  preventDefault?: boolean;
  /** Human description, shown in shortcut help. */
  description?: string;
}

export interface Hotkey extends HotkeyOptions {
  combo: string;
  /** Return `false` to let lower-priority hotkeys handle the event. */
  handler: (e: KeyboardEvent) => void | boolean;
}

interface Entry {
  id: number;
  hotkey: Hotkey;
  combo: string;
}

const MODIFIERS = ["ctrl", "alt", "shift", "meta"] as const;
const ALIASES: Record<string, string> = {
  mod: "ctrl", control: "ctrl", cmd: "meta", super: "meta", win: "meta", option: "alt",
  esc: "escape", del: "delete", ins: "insert", return: "enter", space: " ",
  up: "arrowup", down: "arrowdown", left: "arrowleft", right: "arrowright", plus: "+",
};

let entries: Entry[] = [];
let nextId = 1;
let installed = false;

/** Canonical form: modifiers in fixed order, then the key — "ctrl+shift+z". */
export function normalizeCombo(combo: string): string {
  const raw = combo.toLowerCase().trim();
  const parts = raw.endsWith("++") ? [...raw.slice(0, -2).split("+"), "+"] : raw.split("+");
  const names = parts.map((p) => p.trim()).filter(Boolean).map((p) => ALIASES[p] ?? p);
  const mods = MODIFIERS.filter((m) => names.includes(m));
  const key = names.find((n) => !(MODIFIERS as readonly string[]).includes(n)) ?? "";
  return [...mods, key].join("+");
}

export function eventCombo(e: KeyboardEvent): string {
  const mods = MODIFIERS.filter((m) => e[`${m}Key` as const]);
  let key = e.key.toLowerCase();
  if (key === "spacebar") key = " ";
  // Shifted digits/symbols report the shifted character; bind those by their unshifted key.
  if (e.shiftKey && /^Digit\d$/.test(e.code)) key = e.code.slice(5);
  return [...mods, key].join("+");
}

export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (target.closest(".cm-editor, [contenteditable=''], [contenteditable='true']")) return true;
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  if (target instanceof HTMLInputElement) {
    return !["checkbox", "radio", "button", "submit", "reset", "range", "color", "file"].includes(target.type);
  }
  return false;
}

function inOverlay(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    !!target.closest("[role='dialog'], [role='alertdialog'], [role='menu'], [role='listbox'], [data-radix-popper-content-wrapper]")
  );
}

function onKeyDown(e: KeyboardEvent) {
  if (e.defaultPrevented || e.isComposing) return;
  const combo = eventCombo(e);
  const editable = isEditableTarget(e.target);
  const overlay = inOverlay(e.target);
  for (let i = entries.length - 1; i >= 0; i--) {
    const { hotkey, combo: wanted } = entries[i]!;
    if (wanted !== combo) continue;
    if (e.repeat && !hotkey.repeat) continue;
    if (editable && !hotkey.allowInInputs) continue;
    if (overlay && !hotkey.allowInOverlays) continue;
    if (hotkey.when && !hotkey.when()) continue;
    if (hotkey.handler(e) === false) continue;
    if (hotkey.preventDefault !== false) e.preventDefault();
    return;
  }
}

function install() {
  if (installed) return;
  installed = true;
  window.addEventListener("keydown", onKeyDown);
}

/** Register a hotkey; returns the unregister function. */
export function registerHotkey(hotkey: Hotkey): () => void {
  install();
  const id = nextId++;
  entries = [...entries, { id, hotkey, combo: normalizeCombo(hotkey.combo) }];
  return () => {
    entries = entries.filter((e) => e.id !== id);
  };
}

/** All registered hotkeys with a description, most recent first (for a shortcuts help sheet). */
export function listHotkeys(): { combo: string; description: string }[] {
  return entries
    .filter((e) => e.hotkey.description)
    .map((e) => ({ combo: e.hotkey.combo, description: e.hotkey.description! }))
    .reverse();
}

/**
 * React hook form. The handler and `when` are read through a ref, so they may close over fresh state
 * without re-registering; `deps` only matters for options other than handler/when.
 */
export function useHotkey(
  combo: string | string[],
  handler: (e: KeyboardEvent) => void | boolean,
  options: HotkeyOptions = {},
  deps: unknown[] = [],
): void {
  const ref = useRef({ handler, options });
  ref.current = { handler, options };
  const combos = Array.isArray(combo) ? combo : [combo];
  const key = combos.join("|");
  useEffect(() => {
    const { options: o } = ref.current;
    const offs = combos.map((c) =>
      registerHotkey({
        ...o,
        combo: c,
        when: () => ref.current.options.when?.() ?? true,
        handler: (e) => ref.current.handler(e),
      }),
    );
    return () => offs.forEach((off) => off());
  }, [key, ...deps]);
}
