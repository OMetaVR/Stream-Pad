// Light/dark theme (follows the system unless the user picks one) and the runtime accent colour.
// The preference is an editor UI setting, stored in localStorage — not part of config.json.

import { useSyncExternalStore } from "react";

export type ThemePreference = "system" | "dark" | "light";
export type ResolvedTheme = "dark" | "light";

const STORAGE_KEY = "streampad.theme";
const DEFAULT_ACCENT = "#8b7cf6";
const media = typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: light)") : null;
const listeners = new Set<() => void>();

let preference: ThemePreference = readPreference();

function readPreference(): ThemePreference {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return v === "dark" || v === "light" ? v : "system";
  } catch {
    return "system";
  }
}

export function resolveTheme(p: ThemePreference = preference): ResolvedTheme {
  if (p !== "system") return p;
  return media?.matches ? "light" : "dark";
}

function apply() {
  document.documentElement.dataset.theme = resolveTheme();
  listeners.forEach((l) => l());
}

/** Apply the stored preference and follow system changes. Call once before the first render. */
export function initTheme(): void {
  apply();
  media?.addEventListener("change", () => preference === "system" && apply());
}

export function setThemePreference(p: ThemePreference): void {
  preference = p;
  try {
    if (p === "system") localStorage.removeItem(STORAGE_KEY);
    else localStorage.setItem(STORAGE_KEY, p);
  } catch {
    // Storage unavailable (private mode): the choice still applies for this session.
  }
  apply();
}

function subscribe(cb: () => void) {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

/** `{ preference, resolved, setPreference }`, re-rendering on change. */
export function useTheme() {
  const pref = useSyncExternalStore(subscribe, () => preference);
  const resolved = useSyncExternalStore(subscribe, () => resolveTheme());
  return { preference: pref, resolved, setPreference: setThemePreference };
}

// ---------------------------------------------------------------------------------------------------------
// Accent

const HEX = /^#(?:[0-9a-f]{3}|[0-9a-f]{6})$/i;

export function isHexColor(s: string | null | undefined): s is string {
  return !!s && HEX.test(s);
}

/** `#abc` → `#aabbcc`, lower-case. Returns null for anything that is not a hex colour. */
export function normalizeHex(s: string): string | null {
  const v = s.trim().toLowerCase();
  const withHash = v.startsWith("#") ? v : `#${v}`;
  if (!HEX.test(withHash)) return null;
  if (withHash.length === 4) {
    return "#" + [...withHash.slice(1)].map((c) => c + c).join("");
  }
  return withHash;
}

function luminance(hex: string): number {
  const n = normalizeHex(hex) ?? DEFAULT_ACCENT;
  const channel = (i: number) => {
    const c = parseInt(n.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
}

/** Text colour with the better WCAG contrast on `hex`. */
export function readableOn(hex: string): "#ffffff" | "#0c0d10" {
  const l = luminance(hex);
  const onWhite = 1.05 / (l + 0.05);
  const onDark = (l + 0.05) / (luminance("#0c0d10") + 0.05);
  return onWhite >= onDark ? "#ffffff" : "#0c0d10";
}

/** Set the UI accent (selection states, primary buttons, focus rings). null → the default violet. */
export function applyAccent(color: string | null | undefined): void {
  const accent = (color && normalizeHex(color)) || DEFAULT_ACCENT;
  const root = document.documentElement.style;
  root.setProperty("--sp-accent", accent);
  root.setProperty("--sp-accent-fg", readableOn(accent));
}
