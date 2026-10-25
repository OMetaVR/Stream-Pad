// TypeScript mirror of crates/streampad-core (config.rs, ipc.rs, keys.rs). Keep in sync — docs/SPEC.md §3–4.

export type KeyId =
  | "NumLock" | "KPSlash" | "KPAsterisk" | "KPMinus"
  | "KP7" | "KP8" | "KP9" | "KPPlus"
  | "KP4" | "KP5" | "KP6"
  | "KP1" | "KP2" | "KP3" | "KPEnter"
  | "KP0" | "KPDot";

export interface GridPos { col: number; row: number; colSpan: number; rowSpan: number }
export interface Legend { main: string; sub: string | null }

export const KEY_IDS: KeyId[] = [
  "NumLock", "KPSlash", "KPAsterisk", "KPMinus",
  "KP7", "KP8", "KP9", "KPPlus",
  "KP4", "KP5", "KP6",
  "KP1", "KP2", "KP3", "KPEnter",
  "KP0", "KPDot",
];

export const KEY_INFO: Record<KeyId, { name: string; grid: GridPos; legend: Legend }> = {
  NumLock:    { name: "Num Lock",     grid: { col: 0, row: 0, colSpan: 1, rowSpan: 1 }, legend: { main: "Num", sub: "Lock" } },
  KPSlash:    { name: "Numpad /",     grid: { col: 1, row: 0, colSpan: 1, rowSpan: 1 }, legend: { main: "/", sub: null } },
  KPAsterisk: { name: "Numpad *",     grid: { col: 2, row: 0, colSpan: 1, rowSpan: 1 }, legend: { main: "*", sub: null } },
  KPMinus:    { name: "Numpad −",     grid: { col: 3, row: 0, colSpan: 1, rowSpan: 1 }, legend: { main: "−", sub: null } },
  KP7:        { name: "Numpad 7",     grid: { col: 0, row: 1, colSpan: 1, rowSpan: 1 }, legend: { main: "7", sub: "Home" } },
  KP8:        { name: "Numpad 8",     grid: { col: 1, row: 1, colSpan: 1, rowSpan: 1 }, legend: { main: "8", sub: "↑" } },
  KP9:        { name: "Numpad 9",     grid: { col: 2, row: 1, colSpan: 1, rowSpan: 1 }, legend: { main: "9", sub: "PgUp" } },
  KPPlus:     { name: "Numpad +",     grid: { col: 3, row: 1, colSpan: 1, rowSpan: 2 }, legend: { main: "+", sub: null } },
  KP4:        { name: "Numpad 4",     grid: { col: 0, row: 2, colSpan: 1, rowSpan: 1 }, legend: { main: "4", sub: "←" } },
  KP5:        { name: "Numpad 5",     grid: { col: 1, row: 2, colSpan: 1, rowSpan: 1 }, legend: { main: "5", sub: null } },
  KP6:        { name: "Numpad 6",     grid: { col: 2, row: 2, colSpan: 1, rowSpan: 1 }, legend: { main: "6", sub: "→" } },
  KP1:        { name: "Numpad 1",     grid: { col: 0, row: 3, colSpan: 1, rowSpan: 1 }, legend: { main: "1", sub: "End" } },
  KP2:        { name: "Numpad 2",     grid: { col: 1, row: 3, colSpan: 1, rowSpan: 1 }, legend: { main: "2", sub: "↓" } },
  KP3:        { name: "Numpad 3",     grid: { col: 2, row: 3, colSpan: 1, rowSpan: 1 }, legend: { main: "3", sub: "PgDn" } },
  KPEnter:    { name: "Numpad Enter", grid: { col: 3, row: 3, colSpan: 1, rowSpan: 2 }, legend: { main: "Enter", sub: null } },
  KP0:        { name: "Numpad 0",     grid: { col: 0, row: 4, colSpan: 2, rowSpan: 1 }, legend: { main: "0", sub: "Ins" } },
  KPDot:      { name: "Numpad .",     grid: { col: 2, row: 4, colSpan: 1, rowSpan: 1 }, legend: { main: ".", sub: "Del" } },
};

// ---------------------------------------------------------------------------------------------------------
// config.json

export type GlobalAction =
  | "toggleOverlay" | "showOverlay" | "hideOverlay"
  | "nextProfile" | "prevProfile" | "firstProfile"
  | "toggleMacroMode" | "openEditor" | "none";

export interface GlobalBinding { tap: GlobalAction; hold: GlobalAction | null }

export interface DeviceSettings {
  mode: "auto" | "manual";
  include: string[];
  exclude: string[];
}

export interface OverlaySettings {
  edge: "bottom" | "top" | "left" | "right";
  align: "start" | "center" | "end";
  offset: number;
  monitor: string | null;
  scale: number;
  theme: "dark" | "light";
  accent: string;
  opacity: number;
  blur: boolean;
  showHeader: boolean;
  showLegends: boolean;
  legendStyle: "full" | "number";
  showLabels: boolean;
  animationMs: number;
  peekOnModeChange: boolean;
  peekOnProfileChange: boolean;
  peekMs: number;
}

export interface Settings {
  toggleKey: string;
  startInMacroMode: boolean;
  rememberState: boolean;
  unboundKeys: "block" | "passthrough";
  holdMs: number;
  globals: Partial<Record<KeyId, GlobalBinding>>;
  devices: DeviceSettings;
  overlay: OverlaySettings;
  toastOnError: boolean;
}

/** "lucide:<kebab-name>" (monochrome, tinted) or an AssetRef (drawn as-is). */
export type IconRef = string;
/** Absolute path, or relative to $XDG_DATA_HOME/streampad/ (e.g. "covers/ab12.gif"). */
export type AssetRef = string;

export interface Profile {
  id: string;
  name: string;
  color: string | null;
  icon: IconRef | null;
  keys: Partial<Record<KeyId, Binding>>;
}

export type Concurrency = "ignore" | "restart" | "parallel";

/** A rectangle in normalized image coordinates (0–1 of the image's width / height). */
export interface CropRect { x: number; y: number; w: number; h: number }

export interface Binding {
  label: string;
  icon: IconRef | null;
  cover: AssetRef | null;
  /** Part of the cover shown on the key; null = centred "cover" fit. The cover file is never modified. */
  coverCrop: CropRect | null;
  color: string | null;
  macro: Macro | null;
  repeat: boolean;
  concurrency: Concurrency;
}

export interface GraphNode {
  id: string;
  kind: string;
  params: Record<string, unknown>;
  position: { x: number; y: number };
}

export interface GraphEdge {
  id: string;
  source: string;
  sourcePort: string;
  target: string;
}

export type Macro =
  | { type: "graph"; nodes: GraphNode[]; edges: GraphEdge[] }
  | { type: "script"; source: string };

export type Condition =
  | { type: "activeApp"; match: string }
  | { type: "activeTitle"; match: string }
  | { type: "appRunning"; match: string }
  | { type: "var"; name: string; op: "truthy" | "falsy" | "eq" | "neq"; value?: string }
  | { type: "profile"; profileId: string }
  | { type: "time"; from: string; to: string }
  | { type: "expr"; code: string };

export interface Config {
  version: 1;
  settings: Settings;
  profiles: Profile[];
  library: string;
  /** Icons saved for reuse (IconRefs), newest first. */
  savedIcons: string[];
}

// ---------------------------------------------------------------------------------------------------------
// IPC

export interface ProfileSummary { id: string; name: string; color: string | null; icon: string | null }

export interface RenderKey {
  label: string;
  icon: string | null;
  iconTint: boolean;
  cover: string | null;
  coverCrop: CropRect | null;
  color: string | null;
  kind: "macro" | "global" | "empty";
  global: GlobalAction | null;
  legend: Legend;
}

export interface RenderProfile {
  id: string;
  name: string;
  color: string | null;
  icon: string | null;
  keys: Record<KeyId, RenderKey>;
}

export interface DeviceInfo {
  id: string;
  name: string;
  path: string;
  phys: string | null;
  hasNumpad: boolean;
  eligible: boolean;
  grabbed: boolean;
  error: string | null;
}

export interface OutputInfo {
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  scale: number;
  primary: boolean;
  model: string | null;
}

export interface WindowInfo {
  id: string;
  title: string;
  appId: string;
  resourceClass: string;
  desktopFile: string;
  pid: number;
  output: string | null;
  active: boolean;
}

export interface DaemonState {
  version: string;
  macroMode: boolean;
  profileIndex: number;
  profiles: ProfileSummary[];
  profile: RenderProfile;
  overlay: {
    visible: boolean;
    pinned: boolean;
    preview: boolean;
    output: string | null;
    settings: OverlaySettings;
    accent: string;
  };
  learn: boolean;
  devices: DeviceInfo[];
  integrations: { kwin: boolean; layerShell: boolean | null };
  configError: string | null;
}

export type DaemonEvent =
  | { event: "state"; data: DaemonState }
  | { event: "key"; data: { key: KeyId; pressed: boolean; kind: "macro" | "global" | "empty" | "learn" } }
  | { event: "profile"; data: { index: number; direction: 1 | -1 } }
  | { event: "toast"; data: { id: number; text: string; kind: "info" | "success" | "error"; durationMs: number; icon?: string } }
  | { event: "learn"; data: { key: KeyId } }
  | { event: "run"; data: { runId: number; key: KeyId | null; status: "started" | "finished" | "failed" | "cancelled"; error?: string } }
  | { event: "trace"; data: { runId: number; nodeId: string } }
  | { event: "log"; data: { runId: number | null; level: "info" | "warn" | "error"; message: string } }
  | { event: "configError"; data: { message: string } };
