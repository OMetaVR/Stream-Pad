// TypeScript mirror of the Rust `Default` impls in crates/streampad-core/src/config.rs. Keep in sync.

import type { Config, GlobalAction, GlobalBinding, KeyId, OverlaySettings, Settings } from "./types";
import { newProfile } from "./model";

export const CONFIG_VERSION = 1;

export function defaultGlobals(): Partial<Record<KeyId, GlobalBinding>> {
  return {
    KPEnter: { tap: "toggleOverlay", hold: "openEditor" },
    KPPlus: { tap: "nextProfile", hold: null },
    KPMinus: { tap: "prevProfile", hold: null },
  };
}

export function defaultOverlaySettings(): OverlaySettings {
  return {
    edge: "bottom",
    align: "end",
    offset: 0,
    monitor: null,
    scale: 1,
    theme: "dark",
    accent: "#8b7cf6",
    opacity: 0.94,
    blur: true,
    showHeader: true,
    showLegends: true,
    legendStyle: "full",
    showLabels: true,
    animationMs: 320,
    peekOnModeChange: true,
    peekOnProfileChange: true,
    peekMs: 1600,
  };
}

export function defaultSettings(): Settings {
  return {
    toggleKey: "ScrollLock",
    startInMacroMode: false,
    rememberState: true,
    unboundKeys: "block",
    holdMs: 600,
    globals: defaultGlobals(),
    devices: { mode: "auto", include: [], exclude: [] },
    overlay: defaultOverlaySettings(),
    toastOnError: true,
  };
}

export function defaultConfig(): Config {
  return { version: CONFIG_VERSION, settings: defaultSettings(), profiles: [newProfile("Default")], library: "", savedIcons: [] };
}

/** Numeric ranges enforced by `Config::normalize` (the daemon clamps out-of-range values on load). */
export const LIMITS = {
  holdMs: { min: 150, max: 5000 },
  overlayScale: { min: 0.5, max: 2 },
  overlayOpacity: { min: 0.3, max: 1 },
  animationMs: { min: 0, max: 2000 },
  peekMs: { min: 300, max: 10_000 },
} as const;

export const GLOBAL_ACTIONS: GlobalAction[] = [
  "toggleOverlay", "showOverlay", "hideOverlay",
  "nextProfile", "prevProfile", "firstProfile",
  "toggleMacroMode", "openEditor", "none",
];
