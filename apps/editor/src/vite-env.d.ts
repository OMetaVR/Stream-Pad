/// <reference types="vite/client" />

declare module "virtual:lucide-catalog" {
  /** [kebab-case icon name, space-separated search tags] for every lucide icon StreamPad ships. */
  const catalog: ReadonlyArray<readonly [name: string, tags: string]>;
  export default catalog;
}

/** Injected by the Tauri shell before any page script runs (src-tauri/src/window.rs). */
interface StreamPadBoot {
  version: string;
  dataDir: string;
  shareDir: string | null;
  home: string;
  configFile: string;
}

interface Window {
  __STREAMPAD__?: StreamPadBoot;
}
