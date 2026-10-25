// Dev-server only: serves the StreamPad share dir (lucide SVGs) at /__share/ so the mock backend's
// `assetUrl("lucide:…")` resolves in a plain browser, like the asset protocol does inside Tauri.
import { createReadStream, statSync } from "node:fs";
import path from "node:path";
import type { Plugin } from "vite";

const PREFIX = "/__share/";
const TYPES: Record<string, string> = { ".svg": "image/svg+xml", ".png": "image/png" };

export function devShareDir(shareDir: string): Plugin {
  const root = path.resolve(shareDir);
  return {
    name: "streampad:dev-share-dir",
    apply: "serve",
    configureServer(server) {
      server.middlewares.use((req, res, next) => {
        if (!req.url?.startsWith(PREFIX)) return next();
        const rel = decodeURIComponent(req.url.slice(PREFIX.length).split("?")[0] ?? "");
        const file = path.resolve(root, rel);
        if (!file.startsWith(root + path.sep)) return next();
        try {
          if (!statSync(file).isFile()) return next();
        } catch {
          res.statusCode = 404;
          return res.end();
        }
        res.setHeader("Content-Type", TYPES[path.extname(file)] ?? "application/octet-stream");
        res.setHeader("Cache-Control", "max-age=3600");
        createReadStream(file).pipe(res);
      });
    },
  };
}
