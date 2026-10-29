// Backend implementation inside the Tauri shell. Every method maps onto a command in src-tauri/src
// (see main.rs for the list) or onto a daemon request forwarded by src-tauri/src/daemon.rs.

import { convertFileSrc, invoke, type InvokeArgs, type InvokeOptions } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppEntry, Backend, CompileResult, DaemonStatus, KdeShortcut, UpdateInfo, UpdateProgress } from "./backend";
import { DaemonChannel, toError } from "./daemon-channel";
import type { Config, DaemonEvent, DeviceInfo, KeyId, Macro, OutputInfo, WindowInfo } from "./types";

const STATUS_EVENT = "streampad:daemon-status";
const DAEMON_EVENT = "streampad:daemon-event";
const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "gif", "svg"];

async function call<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
  try {
    return await invoke<T>(cmd, args, options);
  } catch (e) {
    throw toError(e);
  }
}

function bootInfo(): StreamPadBoot {
  const boot = window.__STREAMPAD__;
  if (!boot) throw new Error("StreamPad boot info missing — the window must be created by src-tauri/src/window.rs");
  return boot;
}

export function createTauriBackend(): Backend {
  const boot = bootInfo();
  const channel = new DaemonChannel();
  const urls = new Map<string, string | null>();
  // Imported images are content-addressed, so re-importing an image that went missing yields the same ref and
  // path. The webview caches the failed load per URL, so each import gets a fresh URL (`?v=`; the asset
  // protocol ignores the query).
  const versions = new Map<string, number>();
  const imported = (ref: string) => {
    versions.set(ref, (versions.get(ref) ?? 0) + 1);
    urls.delete(ref);
    return ref;
  };

  void (async () => {
    let sawStatusEvent = false;
    await listen<DaemonStatus>(STATUS_EVENT, (e) => {
      sawStatusEvent = true;
      channel.setStatus(e.payload);
    });
    await listen<DaemonEvent>(DAEMON_EVENT, (e) => channel.pushEvent(e.payload));
    const initial = await call<DaemonStatus>("daemon_status");
    // A status event that raced ahead of the snapshot is newer; keep it.
    if (!sawStatusEvent) channel.setStatus(initial);
  })().catch((e) => {
    console.error("daemon status wiring failed", e);
    channel.setStatus({ kind: "stopped", reason: toError(e).message });
  });

  /** AssetRef / IconRef → absolute path (docs/SPEC.md §3.2), or null if it cannot be one. */
  function resolvePath(ref: string): string | null {
    if (ref.startsWith("lucide:")) {
      const name = ref.slice("lucide:".length);
      return boot.shareDir && /^[a-z0-9-]+$/.test(name) ? `${boot.shareDir}/icons/lucide/${name}.svg` : null;
    }
    if (ref.startsWith("/")) return ref;
    if (ref.startsWith("~/")) return `${boot.home}/${ref.slice(2)}`;
    if (/^[a-z][a-z0-9+.-]*:/i.test(ref)) return null; // URLs are not AssetRefs
    return ref ? `${boot.dataDir}/${ref}` : null;
  }

  function assetUrl(ref: string): string | null {
    const cached = urls.get(ref);
    if (cached !== undefined) return cached;
    const path = resolvePath(ref);
    const v = versions.get(ref);
    const url = path ? convertFileSrc(path) + (v ? `?v=${v}` : "") : null;
    urls.set(ref, url);
    return url;
  }

  async function daemonRequest<T = unknown>(method: string, params?: unknown): Promise<T> {
    return call<T>("daemon_request", { method, params: params ?? null });
  }

  return {
    isMock: false,

    loadConfig: () => call<Config>("load_config"),
    saveConfig: (config) => call<void>("save_config", { config }),
    configPath: () => call<string>("config_path"),

    async pickImage() {
      const picked = await open({
        title: "Choose an image",
        multiple: false,
        directory: false,
        filters: [{ name: "Images", extensions: IMAGE_EXTENSIONS }],
      });
      return typeof picked === "string" ? picked : null;
    },
    importCover: (absPath, kind = "cover") => call<string>("import_cover", { path: absPath, kind }).then(imported),
    async importCoverFile(file, kind = "cover") {
      const bytes = new Uint8Array(await file.arrayBuffer());
      return call<string>("import_cover_bytes", bytes, { headers: { "x-file-name": encodeURIComponent(file.name), "x-asset-kind": kind } }).then(imported);
    },
    importImageUrl: (url, kind = "cover") => call<string>("import_image_url", { url, kind }).then(imported),
    assetUrl,

    async listApps() {
      const apps = await call<Omit<AppEntry, "iconUrl">[]>("list_apps");
      return apps.map((a) => ({ ...a, iconUrl: a.iconPath ? assetUrl(a.iconPath) : null }));
    },
    listKdeShortcuts: () => call<KdeShortcut[]>("list_kde_shortcuts"),
    listOutputs: () => daemonRequest<OutputInfo[]>("listOutputs"),
    listWindows: () => daemonRequest<WindowInfo[]>("listWindows"),
    listDevices: () => daemonRequest<DeviceInfo[]>("listDevices"),

    async compileMacro(m: Macro): Promise<CompileResult> {
      try {
        return await call<CompileResult>("compile_macro", { macroDef: m });
      } catch (e) {
        return { ok: false, error: toError(e).message };
      }
    },
    async testMacro(m: Macro, key?: KeyId) {
      const r = await daemonRequest<{ runId: number }>("runMacro", key ? { macro: m, key } : { macro: m });
      return r.runId;
    },
    async cancelRun(runId) {
      await daemonRequest("cancelRun", { runId });
    },

    daemonStatus: () => channel.getStatus(),
    onDaemonStatus: (cb) => channel.onStatus(cb),
    onDaemonEvent: (cb) => channel.onEvent(cb),
    daemonRequest,
    startDaemon: () => call<void>("start_daemon"),

    getAutostart: () => call<boolean>("get_autostart"),
    setAutostart: (on) => call<void>("set_autostart", { on }),
    openPath: (path) => call<void>("open_path", { path }),
    appVersion: () => boot.version,

    checkForUpdate: () => call<UpdateInfo>("check_update"),
    async installUpdate(onProgress) {
      const unlisten = await listen<UpdateProgress>("streampad:update-progress", (e) => onProgress?.(e.payload));
      try {
        return await call<string>("install_update");
      } finally {
        unlisten();
      }
    },
    restartEditor: () => call<void>("restart_editor"),
  };
}
