// The ONLY way UI code talks to the outside world. Two implementations:
//   - Tauri (src/lib/backend.tauri.ts) — real app; Rust commands in src-tauri/src/commands.rs
//   - Mock  (src/lib/backend.mock.ts)  — used automatically outside Tauri (plain browser, `bun run dev`), keeps
//     state in localStorage and simulates a daemon (state/key/run/log/trace events) so every screen is usable.
// `getBackend()` picks one. Keep this interface the contract between editor features.

import type { Config, DaemonEvent, DaemonState, DeviceInfo, KeyId, Macro, OutputInfo, WindowInfo } from "./types";

export type AssetKind = "cover" | "icon";

export interface AppEntry {
  /** Desktop file id without ".desktop", e.g. "org.kde.dolphin". Used as `app.launch` param. */
  id: string;
  name: string;
  /** Generic name / comment for search, e.g. "File Manager". */
  description: string;
  /** Absolute path of the best icon file (png/svg), or null. */
  iconPath: string | null;
  /** URL usable in <img src> for iconPath (asset protocol / data URL in mock), or null. */
  iconUrl: string | null;
  categories: string[];
  /** Matching hints for "focus if running" (StartupWMClass etc.), lower-case. */
  wmClass: string | null;
}

export interface KdeShortcut {
  component: string;        // "kwin"
  componentName: string;    // "KWin"
  name: string;             // "Overview"
  friendlyName: string;     // "Toggle Overview"
  keys: string[];           // current key sequences, display form
}

export type CompileResult =
  | { ok: true; code: string }
  | { ok: false; error: string; nodeId?: string };

export type DaemonStatus =
  | { kind: "connected"; state: DaemonState }
  | { kind: "connecting" }
  | { kind: "stopped"; reason?: string };

export interface Backend {
  readonly isMock: boolean;

  // ---- config -------------------------------------------------------------------------------------------
  /** Loads config.json (creating the default config if missing). */
  loadConfig(): Promise<Config>;
  /** Atomic write of config.json; the daemon hot-reloads it. */
  saveConfig(config: Config): Promise<void>;
  /** Absolute path of the config file (for "Open config folder"). */
  configPath(): Promise<string>;

  // ---- assets -------------------------------------------------------------------------------------------
  /** Native file picker for images (png/jpg/jpeg/webp/gif/svg). Returns an absolute path or null. */
  pickImage(): Promise<string | null>;
  /** Copy an image into the data dir (content-hashed) and return its AssetRef ("covers/<hash>.<ext>",
   *  or "icons/<hash>.<ext>" for kind "icon"). */
  importCover(absPath: string, kind?: AssetKind): Promise<string>;
  /** Import from a dropped browser File (drag & drop from the file manager). */
  importCoverFile(file: File, kind?: AssetKind): Promise<string>;
  /** Download an image from an http(s) URL into the data dir; returns its AssetRef. */
  importImageUrl(url: string, kind?: AssetKind): Promise<string>;
  /** AssetRef / IconRef ("lucide:play", "covers/x.gif", "/abs/path.png") → URL for <img src>. Sync + cached. */
  assetUrl(ref: string): string | null;

  // ---- discovery ----------------------------------------------------------------------------------------
  listApps(): Promise<AppEntry[]>;
  listKdeShortcuts(): Promise<KdeShortcut[]>;
  listOutputs(): Promise<OutputInfo[]>;
  listWindows(): Promise<WindowInfo[]>;
  listDevices(): Promise<DeviceInfo[]>;

  // ---- macros -------------------------------------------------------------------------------------------
  compileMacro(m: Macro): Promise<CompileResult>;
  /** Run a macro now via the daemon (unsaved macros allowed). Resolves with the runId. */
  testMacro(m: Macro, key?: KeyId): Promise<number>;
  cancelRun(runId: number): Promise<void>;

  // ---- daemon -------------------------------------------------------------------------------------------
  daemonStatus(): DaemonStatus;
  onDaemonStatus(cb: (s: DaemonStatus) => void): () => void;
  onDaemonEvent(cb: (e: DaemonEvent) => void): () => void;
  /** Raw IPC request (docs/SPEC.md §4.1). Rejects with Error(message) on daemon error / not connected. */
  daemonRequest<T = unknown>(method: string, params?: unknown): Promise<T>;
  /** Start the background service (systemd user unit, else spawn streampadd). */
  startDaemon(): Promise<void>;

  // ---- system -------------------------------------------------------------------------------------------
  getAutostart(): Promise<boolean>;
  setAutostart(on: boolean): Promise<void>;
  openPath(path: string): Promise<void>;
  appVersion(): string;
}

let instance: Backend | null = null;

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function initBackend(): Promise<Backend> {
  if (instance) return instance;
  if (isTauri()) {
    const m = await import("./backend.tauri");
    instance = m.createTauriBackend();
  } else {
    const m = await import("./backend.mock");
    instance = m.createMockBackend();
  }
  return instance;
}

/** Synchronous accessor; only valid after `initBackend()` resolved (main.tsx awaits it before rendering). */
export function getBackend(): Backend {
  if (!instance) throw new Error("backend not initialised — call initBackend() first");
  return instance;
}
