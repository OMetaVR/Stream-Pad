import path from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";
import { devShareDir } from "./vite/dev-share.ts";
import { lucideCatalog } from "./vite/lucide-catalog.ts";

const root = path.dirname(fileURLToPath(import.meta.url));
const shareDir = process.env.STREAMPAD_SHARE_DIR || path.join(root, "../../build/share/streampad");
const tauriDevHost = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss(), lucideCatalog(), devShareDir(shareDir)],
  resolve: { alias: { "@": path.join(root, "src") } },
  clearScreen: false,
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  server: {
    // Feature engineers run in parallel: main → 5174, macro → 5175 (`bun run dev --port 5174`).
    port: 5173,
    strictPort: true,
    host: tauriDevHost || "127.0.0.1",
    hmr: tauriDevHost ? { protocol: "ws", host: tauriDevHost, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // WebKitGTK 2.4x+ (Tauri on Linux) is Safari-17 class.
    target: "safari16",
    chunkSizeWarningLimit: 4096,
  },
});
