#!/usr/bin/env node
// Capture the promo (apps/editor/promo.html) frame by frame, headless.
//
//   node promo/capture.mjs                       # all 450 frames → promo/build/frames/0000.png …
//   node promo/capture.mjs --from 60 --to 200 --step 5 [--out DIR] [--port 5190]
//
// Starts its own Vite dev server for apps/editor (strictPort) unless --url is given, opens promo.html at
// 1280×720 (deviceScaleFactor 1), awaits window.promo.seek(n) for every frame and screenshots it. Files the
// page asks for under /__promo/ are served from the repo (overlay frames, the app icon).

import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const here = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(here, "..");
const EDITOR = path.join(ROOT, "apps/editor");

const args = process.argv.slice(2);
const opt = (name, def) => {
  const i = args.indexOf(`--${name}`);
  return i >= 0 ? args[i + 1] : def;
};
const flag = (name) => args.includes(`--${name}`);
const port = Number(opt("port", "5190"));
const from = Number(opt("from", "0"));
const to = Number(opt("to", "449"));
const step = Math.max(1, Number(opt("step", "1")));
const outDir = path.resolve(opt("out", path.join(here, "build/frames")));
const externalUrl = opt("url", null);

const CHROMES = [
  process.env.PROMO_CHROME,
  path.join(process.env.HOME ?? "", ".cache/ms-playwright/chromium-1223/chrome-linux64/chrome"),
  "/usr/bin/google-chrome-stable",
].filter(Boolean);
const executablePath = CHROMES.find((p) => existsSync(p));
if (!executablePath) throw new Error("no Chromium found (set PROMO_CHROME)");

const ROUTES = {
  "/__promo/icon.svg": path.join(ROOT, "packaging/icons/streampad.svg"),
};
const routeFile = (pathname) => {
  if (ROUTES[pathname]) return ROUTES[pathname];
  if (pathname.startsWith("/__promo/overlay/")) return path.join(here, "build/overlay", pathname.slice("/__promo/overlay/".length));
  return null;
};
const MIME = { ".png": "image/png", ".json": "application/json", ".svg": "image/svg+xml" };

async function waitForServer(url, ms = 30000) {
  const t0 = Date.now();
  for (;;) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch {}
    if (Date.now() - t0 > ms) throw new Error(`dev server did not come up at ${url}`);
    await new Promise((r) => setTimeout(r, 200));
  }
}

let server = null;
let browser = null;
async function cleanup() {
  await browser?.close().catch(() => {});
  if (server && server.exitCode === null) {
    server.kill("SIGTERM");
    await new Promise((r) => setTimeout(r, 300));
    if (server.exitCode === null) server.kill("SIGKILL");
  }
}
process.on("SIGINT", () => void cleanup().then(() => process.exit(130)));
process.on("SIGTERM", () => void cleanup().then(() => process.exit(143)));

try {
  let base = externalUrl;
  if (!base) {
    base = `http://127.0.0.1:${port}`;
    server = spawn(path.join(EDITOR, "node_modules/.bin/vite"), ["--port", String(port), "--strictPort", "--host", "127.0.0.1"], {
      cwd: EDITOR,
      stdio: ["ignore", "pipe", "pipe"],
      env: { ...process.env, BROWSER: "none" },
    });
    server.stderr.on("data", (d) => process.stderr.write(`[vite] ${d}`));
    server.on("exit", (code) => code && code !== 0 && console.error(`[vite] exited with ${code}`));
    console.log(`vite pid ${server.pid} on :${port}`);
    await waitForServer(`${base}/promo.html`);
  }

  browser = await chromium.launch({
    executablePath,
    headless: true,
    args: ["--hide-scrollbars", "--font-render-hinting=none", "--disable-lcd-text", "--force-color-profile=srgb", "--disable-gpu-vsync"],
  });
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1, colorScheme: "dark", reducedMotion: "no-preference" });
  await context.route("**/__promo/**", async (route) => {
    const file = routeFile(new URL(route.request().url()).pathname);
    if (!file || !existsSync(file)) return route.fulfill({ status: 404, body: "not found" });
    return route.fulfill({ status: 200, body: readFileSync(file), contentType: MIME[path.extname(file)] ?? "application/octet-stream", headers: { "cache-control": "no-store" } });
  });
  const page = await context.newPage();
  page.on("console", (m) => (m.type() === "error" || m.type() === "warning") && console.log(`[page ${m.type()}] ${m.text()}`));
  page.on("pageerror", (e) => console.log(`[page error] ${e.message}`));
  await page.goto(`${base}/promo.html`, { waitUntil: "load" });
  await page.waitForFunction(() => window.promo || window.promoError, null, { timeout: 60000 });
  const err = await page.evaluate(() => window.promoError);
  if (err) throw new Error(`promo failed to boot:\n${err}`);
  const info = await page.evaluate(() => ({ frames: window.promo.frames, overlay: window.promo.overlay }));
  console.log(`promo ready: ${info.frames} frames, overlay ${info.overlay ? "real frames" : "placeholder"}`);

  if (!flag("keep")) rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  const t0 = Date.now();
  const last = Math.min(to, info.frames - 1);
  let n = 0;
  for (let f = from; f <= last; f += step) {
    await page.evaluate((i) => window.promo.seek(i), f);
    await page.screenshot({ path: path.join(outDir, `${String(f).padStart(4, "0")}.png`), animations: "allow", caret: "hide" });
    n++;
    if (n % 25 === 0) console.log(`frame ${f} (${((Date.now() - t0) / n).toFixed(0)} ms/frame)`);
  }
  console.log(`captured ${n} frames → ${outDir} in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
} catch (e) {
  console.error(e);
  process.exitCode = 1;
} finally {
  await cleanup();
}
