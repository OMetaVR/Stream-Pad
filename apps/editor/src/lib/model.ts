// Pure helpers over the config model (no store, no IO).

import type { Binding, Config, GlobalAction, GlobalBinding, KeyId, Profile, Settings } from "./types";
import { KEY_IDS } from "./types";

/** Random short id with a prefix, same format as `streampad_core::new_id` (`p_3f9c2a71`). */
export function newId(prefix: string): string {
  const n = crypto.getRandomValues(new Uint32Array(1))[0]!;
  return `${prefix}_${n.toString(16).padStart(8, "0")}`;
}

export function emptyBinding(): Binding {
  return { label: "", icon: null, cover: null, coverCrop: null, color: null, macro: null, repeat: false, concurrency: "ignore" };
}

export function newProfile(name: string): Profile {
  return { id: newId("p"), name, color: null, icon: null, keys: {} };
}

/** True when a binding shows nothing and does nothing (safe to drop from the profile). */
export function isBindingEmpty(b: Binding | null | undefined): boolean {
  return !b || (!b.label && !b.icon && !b.cover && !b.color && !b.macro && !b.repeat && b.concurrency === "ignore");
}

/** The global bound to `key`, if it is active (tap ≠ "none"). Profile bindings on such keys are ignored. */
export function globalOf(settings: Settings, key: KeyId): GlobalBinding | null {
  const g = settings.globals[key];
  return g && g.tap !== "none" ? g : null;
}

export function isGlobalKey(settings: Settings, key: KeyId): boolean {
  return globalOf(settings, key) !== null;
}

/** Keys a profile can bind (all pad keys minus active globals), in grid order. */
export function bindableKeys(settings: Settings): KeyId[] {
  return KEY_IDS.filter((k) => !isGlobalKey(settings, k));
}

/** Mirrors `GlobalAction::presentation()` in config.rs — default key label + lucide icon — plus UI copy. */
export const GLOBAL_ACTION_INFO: Record<GlobalAction, { label: string; icon: string; description: string }> = {
  toggleOverlay: { label: "Show / Hide", icon: "eye", description: "Show or hide the on-screen pad" },
  showOverlay: { label: "Show", icon: "eye", description: "Show the on-screen pad" },
  hideOverlay: { label: "Hide", icon: "eye-off", description: "Hide the on-screen pad" },
  nextProfile: { label: "Next", icon: "chevron-down", description: "Switch to the next profile" },
  prevProfile: { label: "Prev", icon: "chevron-up", description: "Switch to the previous profile" },
  firstProfile: { label: "First", icon: "chevrons-up", description: "Switch to the first profile" },
  toggleMacroMode: { label: "Numpad", icon: "keyboard", description: "Turn macro mode off or on" },
  openEditor: { label: "Editor", icon: "settings-2", description: "Open this editor" },
  none: { label: "", icon: "circle-off", description: "Do nothing" },
};

export function findProfile(config: Config, id: string | null | undefined): Profile | undefined {
  return id ? config.profiles.find((p) => p.id === id) : undefined;
}

/** "Audio" → "Audio copy", then "Audio copy 2", … avoiding names already in `taken`. */
export function uniqueName(base: string, taken: Iterable<string>): string {
  const names = new Set(taken);
  if (!names.has(base)) return base;
  for (let i = 2; ; i++) {
    const candidate = `${base} ${i}`;
    if (!names.has(candidate)) return candidate;
  }
}

export function deepClone<T>(value: T): T {
  return structuredClone(value);
}
