// Backend used automatically outside Tauri (`bun run dev` in a browser). Config + imported covers persist
// in localStorage; a simulated daemon (lib/mock/daemon.ts) answers IPC and emits events, so every screen
// is usable and screenshot-able without the real system. Not shipped behaviour — development only.

import type { AssetKind, Backend } from "./backend";
import { DaemonChannel } from "./daemon-channel";
import { compileMock } from "./mock/compile";
import { MockDaemon } from "./mock/daemon";
import { MOCK_APPS, MOCK_KDE_SHORTCUTS } from "./mock/fixtures";
import { sampleConfig } from "./mock/sample-config";
import type { Config, DaemonEvent, DeviceInfo, KeyId, OutputInfo, WindowInfo } from "./types";

const CONFIG_KEY = "streampad.mock.config";
const COVERS_KEY = "streampad.mock.covers";
const AUTOSTART_KEY = "streampad.mock.autostart";
const FAKE_HOME = "/home/streamer";
const IMAGE_TYPES = ["image/png", "image/jpeg", "image/webp", "image/gif", "image/svg+xml"];

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Controls for the dev-only Simulate panel (src/dev/SimulatePanel.tsx). */
export interface MockControls {
  pressKey(key: KeyId | "Toggle", pressed: boolean): void;
  startDaemon(): void;
  stopDaemon(): void;
  isRunning(): boolean;
  /** Replace the stored config with the sample profiles and reload the page. */
  resetConfig(): void;
  /** Deliver a daemon event as if the daemon had sent it (the promo page drives traces/runs on its own clock). */
  emit(event: DaemonEvent): void;
}

let controls: MockControls | null = null;

export function mockControls(): MockControls | null {
  return controls;
}

function readJson<T>(key: string): T | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? (JSON.parse(raw) as T) : null;
  } catch {
    return null;
  }
}

function extensionFor(type: string): string {
  return { "image/png": "png", "image/jpeg": "jpg", "image/webp": "webp", "image/gif": "gif", "image/svg+xml": "svg" }[type] ?? "png";
}

async function sha256Hex(bytes: ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function readDataUrl(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("cannot read file"));
    reader.readAsDataURL(file);
  });
}

export function createMockBackend(): Backend {
  const channel = new DaemonChannel();
  let config: Config = readJson<Config>(CONFIG_KEY) ?? sampleConfig();
  /** AssetRef → data URL for covers imported during development. */
  const covers = new Map<string, string>(Object.entries(readJson<Record<string, string>>(COVERS_KEY) ?? {}));
  /** Fake absolute path → File chosen through pickImage(). */
  const picked = new Map<string, File>();

  function assetUrl(ref: string): string | null {
    if (ref.startsWith("lucide:")) {
      const name = ref.slice("lucide:".length);
      return /^[a-z0-9-]+$/.test(name) ? `/__share/icons/lucide/${name}.svg` : null;
    }
    if (ref.startsWith("/mock/")) return ref;
    const imported = covers.get(ref);
    if (imported) return imported;
    if (ref.startsWith("covers/")) return `/mock/${ref}`;
    return null; // other files are outside the browser's reach
  }

  const daemon = new MockDaemon(channel, () => config, assetUrl);
  daemon.start();

  function persistCovers() {
    try {
      localStorage.setItem(COVERS_KEY, JSON.stringify(Object.fromEntries(covers)));
    } catch {
      // Quota exceeded: imported covers stay available until reload.
    }
  }

  async function importCoverFile(file: File, kind: AssetKind = "cover"): Promise<string> {
    if (!IMAGE_TYPES.includes(file.type)) throw new Error("not a supported image (use PNG, JPEG, WebP, GIF or SVG)");
    const ref = `${kind === "icon" ? "icons" : "covers"}/${await sha256Hex(await file.arrayBuffer())}.${extensionFor(file.type)}`;
    if (!covers.has(ref)) {
      covers.set(ref, await readDataUrl(file));
      persistCovers();
    }
    return ref;
  }

  controls = {
    pressKey: (key, pressed) => daemon.pressKey(key, pressed),
    startDaemon: () => daemon.start(),
    stopDaemon: () => daemon.stop("Stopped from the Simulate panel"),
    isRunning: () => daemon.running,
    emit: (event) => channel.pushEvent(event),
    resetConfig() {
      localStorage.removeItem(CONFIG_KEY);
      localStorage.removeItem(COVERS_KEY);
      location.reload();
    },
  };

  const request = <T,>(method: string, params?: unknown) => daemon.request(method, params) as Promise<T>;

  return {
    isMock: true,

    async loadConfig() {
      await wait(120);
      return structuredClone(config);
    },
    async saveConfig(next) {
      await wait(60);
      try {
        localStorage.setItem(CONFIG_KEY, JSON.stringify(next));
      } catch {
        throw new Error("Browser storage is full — remove some imported covers");
      }
      config = structuredClone(next);
      daemon.configChanged();
    },
    configPath: async () => `${FAKE_HOME}/.config/streampad/config.json`,

    pickImage() {
      return new Promise((resolve) => {
        const input = document.createElement("input");
        input.type = "file";
        input.accept = IMAGE_TYPES.join(",");
        input.addEventListener("change", () => {
          const file = input.files?.[0];
          if (!file) return resolve(null);
          const path = `${FAKE_HOME}/Pictures/${file.name}`;
          picked.set(path, file);
          resolve(path);
        });
        input.addEventListener("cancel", () => resolve(null));
        input.click();
      });
    },
    async importCover(absPath, kind) {
      if (absPath.startsWith("/mock/")) return absPath;
      const file = picked.get(absPath);
      if (!file) throw new Error(`cannot read ${absPath} from the browser`);
      return importCoverFile(file, kind);
    },
    importCoverFile,
    async importImageUrl(url, kind) {
      if (!/^https?:\/\//.test(url.trim())) throw new Error("enter an http(s) link to an image");
      const res = await fetch(url).catch(() => null);
      if (!res?.ok) throw new Error("couldn't download the image");
      const blob = await res.blob();
      return importCoverFile(new File([blob], url.split("/").pop() || "image", { type: blob.type }), kind);
    },
    assetUrl,

    async listApps() {
      await wait(150);
      return MOCK_APPS;
    },
    async listKdeShortcuts() {
      await wait(120);
      return MOCK_KDE_SHORTCUTS;
    },
    listOutputs: () => request<OutputInfo[]>("listOutputs"),
    listWindows: () => request<WindowInfo[]>("listWindows"),
    listDevices: () => request<DeviceInfo[]>("listDevices"),

    async compileMacro(m) {
      await wait(30);
      return compileMock(m);
    },
    async testMacro(m, key) {
      const r = await request<{ runId: number }>("runMacro", { macro: m, key });
      return r.runId;
    },
    async cancelRun(runId) {
      await request("cancelRun", { runId });
    },

    daemonStatus: () => channel.getStatus(),
    onDaemonStatus: (cb) => channel.onStatus(cb),
    onDaemonEvent: (cb) => channel.onEvent(cb),
    daemonRequest: request,
    async startDaemon() {
      daemon.start(900);
    },

    async getAutostart() {
      return localStorage.getItem(AUTOSTART_KEY) === "1";
    },
    async setAutostart(on) {
      localStorage.setItem(AUTOSTART_KEY, on ? "1" : "0");
    },
    async openPath(path) {
      console.info(`[mock] open ${path}`);
    },
    appVersion: () => "0.1.0 (browser mock)",
  };
}
