# StreamPad editor — architecture

Tauri 2 shell (Rust, `src-tauri/`) around a React 19 + TypeScript (strict) + Vite UI (`src/`), styled with
Tailwind v4 on top of design tokens. Product/UX spec: `docs/SPEC.md` §9; data shapes: §3–4 and
`crates/streampad-core` (mirrored in `src/lib/types.ts`).

## Running

| | |
|---|---|
| `bun install` | once |
| `bun run dev` | browser + **mock backend** at http://127.0.0.1:5173 (UI kit reference at `/#kit`) |
| `bun tauri dev` | the real app with hot reload (Vite on 127.0.0.1:1420) |
| `bun tauri build --no-bundle` | release binary `streampad` in `$CARGO_TARGET_DIR/release/` |
| `bunx tsc --noEmit -p .` | type-check the UI (`bun run typecheck` also checks vite.config.ts) |
| `bun run build` | type-check + production bundle into `dist/` (what Tauri embeds) |
| `cargo clippy --all-targets` / `cargo test` in `src-tauri/` | Rust lint / unit tests |

Environment: `STREAMPAD_CONFIG_DIR`, `STREAMPAD_DATA_DIR`, `STREAMPAD_STATE_DIR`, `STREAMPAD_RUNTIME_DIR`,
`STREAMPAD_SOCKET`, `STREAMPAD_SHARE_DIR` relocate everything (always set them in tests). When any of the
first five is set, "Start daemon" / "Start at login" never touch the user's systemd unit: the editor spawns
`streampadd` directly (`$STREAMPAD_DAEMON_BIN`, next to the editor, `$PATH`, `~/.local/bin`) and uses
`$XDG_CONFIG_HOME/autostart/streampad-daemon.desktop`. Rust logging: `RUST_LOG=info`.
On NVIDIA the shell sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` (unless already set) before WebKitGTK starts.

### Parallel work in this directory (feature engineers)

Both feature engineers work here at the same time:
* type-check with `bunx tsc --noEmit -p .`;
* build with `bun run build -- --outDir /tmp/<label>-dist` — **never** the shared `dist/`;
* dev servers on distinct ports: editor main `bun run dev --port 5174`, macro editor `bun run dev --port 5175`
  (`strictPort` is on, so a clash fails loudly instead of silently moving);
* `bun tauri dev` uses port 1420 — only one at a time; coordinate, or pass
  `--config '{"build":{"devUrl":"http://127.0.0.1:1430","beforeDevCommand":"bun run dev --port 1430"}}'`.
* Rust: `CARGO_TARGET_DIR=<repo>/target/<label>` and `CARGO_BUILD_JOBS=4`.
* Do not edit `package.json`: every dependency you need is installed (see below). If something is truly
  missing, ask the integrator.

## Layout and ownership

```
apps/editor/
├─ index.html, vite.config.ts, vite/           shared — Vite plugins: lucide catalog, dev share-dir server
├─ public/mock/                                 shared — mock covers + app icon tiles (scripts/gen-mock-assets.py)
├─ src-tauri/                                   shared — Rust commands (see "Backend")
└─ src/
   ├─ main.tsx                                  boot: theme → backend → stores → autosave → render
   ├─ App.tsx                                   shell: providers, global hotkeys, accent, view switch   [editor main may polish]
   ├─ shell/                                    Header, SaveIndicator, DaemonPill, LoadGate, Wordmark   [editor main may polish]
   ├─ styles/                                   shared — tokens.css, index.css (Tailwind theme), base.css
   ├─ ui/                                       shared — UI kit (README.md documents every component)
   ├─ state/                                    shared — store.ts, autosave.ts, daemon.ts, selectors.ts
   ├─ lib/                                      shared — backend contract + implementations, types, helpers
   ├─ dev/                                      dev tooling: SimulatePanel (mock only), KitGallery (#kit)
   └─ features/
      ├─ macro/**                               MACRO EDITOR engineer (action picker, step list, flow, script)
      ├─ sidebar/ numpad/ inspector/ settings/  EDITOR MAIN engineer
      └─ (add new feature folders freely)
```

Rules for shared code (`ui/`, `styles/`, `state/`, `lib/`, `src-tauri/`, `vite*`): **additive changes only**,
targeted edits (never rewrite a shared file), keep `lib/types.ts` in sync with `crates/streampad-core`, and
list the change in your report. `lib/types.ts` and `lib/backend.ts` are cross-component contracts.

Contract between the two features — `@/features/macro` exports (stubs today):
* `MacroSection` `{ profileId: string; keyId: KeyId; binding: Binding; onChange(next: Binding): void }` —
  rendered by the inspector for non-global keys (`binding` is `emptyBinding()` when the key is unbound;
  the inspector passes `onChange = (b) => editor.setBinding(profileId, keyId, b)`).
* `FlowEditorView` / `ScriptEditorView` `{ profileId: string; keyId: KeyId }` — rendered full-window by
  App.tsx for views `{ kind: "flow" | "script", … }` (keyed by profile+key, so they remount per key).
* The inspector asks the macro feature to open its picker with `editor.request("actionPicker", { profileId, keyId })`;
  the macro feature listens with `useRequest("actionPicker", …)`.

## State — `src/state/`

### Editor store (`store.ts`)

`useEditor(selector)` reads; `editor.*` writes. State:
`config` (null until loaded) · `load` (`loading | ready | error{message}`) · `save` (`{status: idle|saving|saved|error, error, savedAt}`)
· `past`/`future` (history) · `selection` (`{profileId, keyId}`) · `view` · `clipboard` · `requests`.

Views: `{kind:"main"}` · `{kind:"settings", section?: "general"|"globals"|"overlay"|"devices"|"scripting"|"about"}`
· `{kind:"flow", profileId, keyId}` · `{kind:"script", profileId, keyId}`.

Actions (all config mutations take an optional `HistoryOptions { coalesce?: string | false; label?: string }`
and return `false` when nothing changed or the edit was refused):

| action | notes |
|---|---|
| `updateConfig(fn)` | immer draft of the whole Config (e.g. `library`) — not coalesced by default |
| `updateSettings(fn)` | draft of `settings`; coalesces under "settings" (slider drags = one undo step) |
| `addProfile({name?, after?, select?})` → id | unique name ("New profile", "New profile 2"…), selects it |
| `duplicateProfile(id)` → id | deep copy, new id, "Audio copy", inserted after, selected |
| `deleteProfile(id, {toast?})` | refuses the last profile; shows "Deleted “X”" with **Undo** (re-inserts at its index) |
| `restoreProfile(profile, index)` | used by that Undo |
| `renameProfile(id, name)` | coalesced per profile; trims nothing (trim on commit in the UI) |
| `updateProfile(id, fn)` | colour/icon/…; coalesced per profile |
| `reorderProfiles(from, to)` | indices (dnd-kit `arrayMove` semantics) |
| `setBinding(profileId, keyId, binding \| null)` | replace / clear; **refuses global keys**; empty bindings are dropped |
| `updateBinding(profileId, keyId, fn)` | draft of the binding (created empty if missing, dropped if left empty); coalesced per key |
| `clearBinding`, `swapBindings(p, a, b)` (selects b), `copyBindingTo(p, from, to)` (Alt-drag) | |
| `copyBinding(p, k)` / `pasteBinding(p, k)` | binding clipboard (`useEditor(s => s.clipboard)`), also writes JSON to the system clipboard |
| `isGlobalKey(keyId)` | active global (tap ≠ "none") |
| `undo()` / `redo()` | restore config **and** the selection of that moment |
| `selectProfile(id)`, `selectKey(keyId \| null)` | selection is not undoable; last profile is remembered across launches |
| `setView`, `showMain`, `openSettings(section?)`, `openFlow(p, k)`, `openScript(p, k)` | routing |
| `request(name, payload?)` + `useRequest(name, handler)` | tiny cross-feature request channel |
| `load()` | (re)load config.json — LoadGate's "Try again" |

History: ≤ 200 steps. Edits with the same coalesce key within 900 ms (bursts capped at 6 s) merge into
one step. Defaults: `updateBinding` → `binding:<profile>:<key>`, `renameProfile` → `rename:<id>`,
`updateProfile` → `profile:<id>`, `updateSettings` → `settings`; structural actions never merge. Pass a
more specific key to separate fields (`{ coalesce: \`step:${nodeId}:text\` }`) or `false` for discrete clicks.

Hotkeys wired globally in App.tsx: **Ctrl+Z** undo, **Ctrl+Shift+Z / Ctrl+Y** redo (skipped inside inputs,
textareas and CodeMirror, which keep their native undo), **Ctrl+,** settings, **Esc** back to the pad
(only when not on the main view; registered first so feature handlers win).

Selectors (`selectors.ts`): `useConfig()` (under LoadGate only), `useSelectedProfile()`, `useBinding(p, k)`,
`useGlobal(k)`.

### Autosave (`autosave.ts`)

Started in main.tsx. Every config change is saved 400 ms after the last edit via `backend.saveConfig`
(atomic write; the daemon hot-reloads). Saves never overlap; edits during a save are saved right after.
`flushSave()` runs on window close (Tauri `onCloseRequested`, max 2.5 s), `retrySave()` backs the header's
"Not saved · Retry". A config that fails to load is never overwritten (LoadGate shows the parse error).

### Daemon store (`daemon.ts`)

`useDaemon(selector)`: `status` (`connected{state} | connecting | stopped{reason}`), `state` (DaemonState or
null), `held` (keys down → press seq), `pressSeq` (last press seq per key), `runs` (active runs), `logs`
(last 400 log/error lines). Hooks: `useKeyPress(keyId) → {pressed, seq}` (seq bumps on every press — key
animations off it), `useDaemonEvent(kind, cb)` / `onDaemonEvent(kind, cb)` (typed per event: state, key,
profile, toast, learn, run, trace, log, configError), `useRunLog(runId | null)`, `clearLogs()`.
App.tsx already turns `configError` into an error toast.

## Backend — `src/lib/backend.ts` (contract) + implementations

`await initBackend()` (main.tsx) picks `backend.tauri.ts` inside Tauri, else `backend.mock.ts`;
then `getBackend()` anywhere. Shared helper: `daemon-channel.ts` (status cache; `state` events are folded
into `DaemonStatus`, so status listeners see every state change).

| method | Tauri | mock |
|---|---|---|
| `loadConfig` / `saveConfig` / `configPath` | `load_config` (creates `streampad_core::defaults::default_config()` if missing; parse errors are reported, file untouched) / `save_config` (`Config::save_atomic`) / `config_path` | localStorage, seeded with SPEC §7's five profiles (`mock/sample-config.ts`) |
| `pickImage` | dialog plugin, png/jpg/jpeg/webp/gif/svg | `<input type=file>` |
| `importCover(path)` / `importCoverFile(file)` | sha256 → `<data>/covers/<hash>.<ext>` (type sniffed from bytes, ≤ 64 MB), returns `covers/…`; bytes travel as a raw IPC body | data URLs in localStorage |
| `assetUrl(ref)` (sync, cached) | `lucide:x` → `<share>/icons/lucide/x.svg`, `~/…`, absolute, or data-dir-relative → `asset://` URL | `/__share/…` (dev server), `/mock/…` |
| `listApps` | `.desktop` files from `$XDG_DATA_HOME`, `$XDG_DATA_DIRS`, flatpak exports; Hidden/NoDisplay/OnlyShowIn/NotShowIn/TryExec; localized names; icons via the current theme (kdeglobals → GTK → breeze) + Inherits + hicolor + pixmaps, SVG or ≥ 64 px PNG preferred; cached | 28 apps with icon tiles |
| `listKdeShortcuts` | kglobalaccel over D-Bus (zbus), keys formatted "Meta+Ctrl+Down"; `[]` outside Plasma | ~30 KWin/Plasma shortcuts |
| `listOutputs` / `listWindows` / `listDevices` | daemon requests | DP-1 (0,0) + DP-2 (2560,0, primary) 2560×1440; windows; devices |
| `compileMacro` | `streampad_core::compile` locally (works without the daemon) | TS fallback compiler (`mock/compile.ts`) |
| `testMacro` / `cancelRun` | `runMacro` / `cancelRun` | simulated runs: run started → trace per node → log → finished/failed/cancelled |
| `daemonStatus` / `onDaemonStatus` / `onDaemonEvent` / `daemonRequest` | persistent socket client (`src-tauri/src/daemon.rs`): hello `{client:"editor", subscribe, trace}`, 5 s request timeout, reconnect with backoff | simulated daemon (`mock/daemon.ts`) |
| `startDaemon` | `systemctl --user start streampad.service` if installed (and no path overrides), else spawn `streampadd` (own process group, log in `<state>/daemon.log`) | restarts the simulation |
| `getAutostart` / `setAutostart` | `systemctl --user is-enabled/enable/disable`, fallback XDG autostart entry | localStorage |
| `openPath` | `xdg-open` | console |

Rust modules (`src-tauri/src/`): `main.rs` (builder, plugins: single-instance first, dialog), `window.rs`
(main window 1280×820 / min 960×640, dark background, boot script `window.__STREAMPAD__ = {version,
dataDir, shareDir, home, configFile}`, runtime asset scope), `config_cmd.rs`, `covers.rs`, `apps/`
(`desktop_entry.rs`, `icon_theme.rs`), `kde/` (`qt_keys.rs`), `daemon.rs`, `service.rs`, `system.rs`
(`open_path`, `compile_macro`), `ini.rs`, `env.rs`, `error.rs`. Webview events: `streampad:daemon-status`,
`streampad:daemon-event`. CSP: scripts `'self'` only — no `eval`/`new Function` in shipped code.

## Theming

`<html data-theme="dark|light">` is set by `lib/theme.ts` before the first render: it follows
`prefers-color-scheme` unless the user picks one (`useTheme()` → `{preference, resolved, setPreference}`,
stored in localStorage). The accent is the selected profile's colour (else `settings.overlay.accent`),
applied by App.tsx via `applyAccent()`, which also picks a readable `--sp-accent-fg`. Tokens:
`styles/tokens.css`; Tailwind mapping and motion utilities: `styles/index.css`; conventions:
`src/ui/README.md`.

## Icons

* UI icons: `lucide-react` components (`<LucideProvider size={16} strokeWidth={1.75}>` in App).
* Config icons (`IconRef`): render with `<RefIcon iconRef={…} />` — `lucide:<name>` through lucide-react,
  files through `assetUrl`. lucide-react is pinned to **1.61.0**, the same version as the lucide-static SVGs
  the daemon/overlay ship, so names always resolve on both sides.
* Icon picker data: `await loadLucideCatalog()` (`lib/lucide.ts`) → `{ name, tags }[]` for all 2171 icons
  (tags from lucide-static), lazily loaded; search it with `fuzzysort`.

## Hotkeys — `src/lib/hotkeys.ts`

```ts
useHotkey("delete", () => editor.clearBinding(p, k), { when: () => !!k, description: "Clear key" });
const off = registerHotkey({ combo: "mod+shift+f", handler: openFlow });
```
Combos: `mod`(=Ctrl)/`ctrl`/`shift`/`alt`/`meta` + `KeyboardEvent.key` lower-case (`arrowleft`, `enter`,
`escape`, `f2`, `/`). Last registered wins; return `false` to pass. Skipped by default while typing
(inputs/textarea/CodeMirror) and inside open dialogs/menus (`allowInInputs`, `allowInOverlays` opt in).
`listHotkeys()` lists described hotkeys for a help sheet.

## Toasts and confirmations

`toast("Copied")`, `toast.success/error/warning/info(title, {description, action, duration, id})`,
`toast.dismiss(id)`; `<Toaster/>` is mounted in App. Deletes use an Undo action rather than a confirm.
`await confirm({title, description, confirmLabel, destructive})` for real yes/no questions
(`<ConfirmHost/>` is mounted in App).

## Installed frontend dependencies (do not add more without the integrator)

react 19, @tauri-apps/api 2 + plugin-dialog, @xyflow/react, @codemirror/{state,view,language,commands,
autocomplete,lint,search,lang-javascript}, @lezer/highlight, acorn, elkjs, zustand, immer, radix-ui
(unified), lucide-react (pinned) + lucide-static (tags), motion, @dnd-kit/{core,sortable,utilities}, clsx,
tailwind-merge, fuzzysort, tailwindcss 4 + @tailwindcss/vite, @fontsource-variable/{inter,jetbrains-mono}.
Dev: typescript 7, vite 8, @vitejs/plugin-react, @tauri-apps/cli 2.

## Mock mode tips

* The floating **Simulate** panel (bottom-right, mock only) presses virtual pad keys (pointer down/up =
  press/release), toggles Scroll Lock / macro mode, stops/starts the daemon, and resets sample data.
* Sample covers: `covers/synthwave.png` (Audio · 9), `covers/live.gif` (animated, Apps · 9),
  `covers/aurora.webp` (Web · 0). App icons: `/mock/apps/<desktop-id>.svg`.
* `localStorage.clear()` + reload starts over.
