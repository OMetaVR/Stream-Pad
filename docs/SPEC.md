# StreamPad for Linux — Product & Technical Spec (v1)

StreamPad turns the numpad of any keyboard into a dual‑mode device: a normal numpad, and (after pressing
**Scroll Lock**) a profile‑based **macropad** with an on‑screen display. It is the Linux successor of the
AutoHotkey script at github.com/OMetaVR/Stream-Pad, with a completely new interaction + display layer.

Primary target: **KDE Plasma 6 on Wayland** (KWin 6.7, Qt 6.11). Must degrade gracefully on other
Wayland compositors (wlroots/Hyprland: layer-shell works, KWin features unavailable) and X11.
The dev machine: EndeavourOS, KDE Plasma 6.7.4 Wayland, NVIDIA RTX 3060 Ti, two 2560×1440 monitors
(DP‑1 at 0,0 left; DP‑2 at 2560,0 right, primary). User is in the `input` group, `/dev/uinput` is `root:input 0660`.

---------------------------------------------------------------------------------------------------------

## 1. Components

| Component | Path | Tech | Binary |
|---|---|---|---|
| Shared model | `crates/streampad-core` | Rust lib | – |
| Input engine | `crates/streampad-input` | Rust lib (evdev + uinput) | – |
| Script runtime | `crates/streampad-script` | Rust lib (rquickjs / QuickJS) | – |
| Daemon | `crates/streampadd` | Rust bin (tokio, zbus) | `streampadd` |
| Overlay | `apps/overlay` | C++17 + Qt 6 QML + LayerShellQt (CMake) | `streampad-overlay` |
| Editor | `apps/editor` | Tauri 2 (Rust) + React 19 + TS + Vite | `streampad` |
| Packaging | `packaging/` | install.sh, systemd user unit, .desktop, udev rule, icons | – |

Root `Cargo.toml` is a workspace with members `crates/*`. `apps/editor/src-tauri` is its **own** workspace
(empty `[workspace]` table) with a path dependency on `crates/streampad-core`.

Process model:

```
 keyboard(s) ──evdev grab──▶ streampadd ──uinput mirror per device──▶ compositor (normal typing)
                               │  ├─ "StreamPad Virtual Keyboard" uinput (synthetic macro output)
                               │  ├─ QuickJS workers (macros)
                               │  ├─ KWin bridge (D-Bus + KWin scripts)
                               │  └─ Unix socket  $XDG_RUNTIME_DIR/streampad/daemon.sock  (JSON lines)
                               │          ▲                         ▲
                     spawns/supervises    │                         │
                               ▼          │                         │
                      streampad-overlay ──┘        streampad (editor, Tauri) ──┘
                                                   (reads/writes config.json; daemon hot-reloads it)
```

The daemon is the single source of truth for runtime state (macro mode, current profile, overlay
visibility + target monitor). The overlay is purely presentational. The editor owns config editing.

---------------------------------------------------------------------------------------------------------

## 2. Behaviour

### 2.1 Modes
* **Numpad mode** (default): every key passes through untouched.
* **Macro mode**: toggled by the toggle key (default Scroll Lock, consumed — not passed through).
  Scroll Lock LED on every grabbed keyboard reflects macro mode.
  In macro mode every *pad key* (§3.1) is captured: globals run their global action, bound keys run their
  macro, unbound keys are blocked (or passed through if `settings.unboundKeys = "passthrough"`).
  Non‑pad keys always pass through.
* A key whose **press** was passed through always has its repeat/release passed through, even if the mode
  changed in between; likewise a captured press keeps its release captured (no stuck keys, ever).

### 2.2 Globals (macro mode only; configurable in `settings.globals`)
| Key | Tap | Hold (≥ `settings.holdMs`, default 600 ms) |
|---|---|---|
| Numpad Enter | toggle overlay (pin/unpin) | open the editor |
| Numpad + | next profile (wraps) | – |
| Numpad − | previous profile (wraps) | – |

If a global has a `hold` action, its tap fires on release (when held < holdMs) and its hold fires as soon as
holdMs elapses while still held. If it has no hold action, tap fires on press. Auto‑repeat is ignored for globals.
Global actions: `toggleOverlay | showOverlay | hideOverlay | nextProfile | prevProfile | firstProfile |
toggleMacroMode | openEditor | none`.

### 2.3 Overlay visibility model (owned by daemon)
`visible = macroMode && (pinned || peeking)`
* Enter (tap) toggles `pinned`. Leaving macro mode hides the overlay but remembers `pinned`
  (re‑entering macro mode with pinned=true shows it again).
* **Peek** = show temporarily for `overlay.peekMs` (default 1600 ms) then auto‑hide (unless pinned):
  triggered on entering macro mode (`peekOnModeChange`) and on profile change (`peekOnProfileChange`) —
  this replaces the AHK "Current Profile: X" tooltip.
* Target monitor: `overlay.monitor = null` → the monitor under the mouse cursor at the moment the overlay is
  shown (daemon asks KWin; fallback: let compositor choose = active screen). A connector name (`"DP-1"`)
  pins it to that monitor (fallback to cursor monitor if disconnected).
* While visible, the overlay stays on its monitor (does not jump around while you move the mouse).

### 2.4 Key repeat & concurrency (per binding)
* `repeat: true` → kernel auto‑repeat events re‑trigger the macro (e.g. volume up while held).
* `concurrency`: what happens if the macro is still running when triggered again:
  `"ignore"` (default) | `"restart"` (cancel running, start new) | `"parallel"`.
* Release: if a macro defines an `onRelease` handler (graph `trigger.release` chain, or script function
  `onRelease`), it runs when the key is released (same JS context as the press run — see §6).

### 2.5 Learn mode
Editor can enable learn mode (`setLearn {on:true}`): captured pad presses are **not executed**; daemon emits
`learn` events instead so the editor can select the physically pressed key. Auto‑expires after 30 s or when
the editor disconnects.

---------------------------------------------------------------------------------------------------------

## 3. Data model  (`crates/streampad-core` is the source of truth; TS types mirror it exactly)

All JSON uses camelCase. Unknown fields are ignored; missing fields take defaults (serde `default`).

### 3.1 Pad keys (`KeyId`) — layout is a faithful 4×5 numpad

| KeyId | evdev | grid (col,row,colSpan,rowSpan) | legend main | legend sub |
|---|---|---|---|---|
| `NumLock` | 69 KEY_NUMLOCK | 0,0,1,1 | `Num` | `Lock` |
| `KPSlash` | 98 | 1,0,1,1 | `/` | – |
| `KPAsterisk` | 55 | 2,0,1,1 | `*` | – |
| `KPMinus` | 74 | 3,0,1,1 | `−` | – |
| `KP7` | 71 | 0,1,1,1 | `7` | `Home` |
| `KP8` | 72 | 1,1,1,1 | `8` | `↑` |
| `KP9` | 73 | 2,1,1,1 | `9` | `PgUp` |
| `KPPlus` | 78 | 3,1,1,2 | `+` | – |
| `KP4` | 75 | 0,2,1,1 | `4` | `←` |
| `KP5` | 76 | 1,2,1,1 | `5` | – |
| `KP6` | 77 | 2,2,1,1 | `6` | `→` |
| `KP1` | 79 | 0,3,1,1 | `1` | `End` |
| `KP2` | 80 | 1,3,1,1 | `2` | `↓` |
| `KP3` | 81 | 2,3,1,1 | `3` | `PgDn` |
| `KPEnter` | 96 | 3,3,1,2 | `Enter` | – |
| `KP0` | 82 | 0,4,2,1 | `0` | `Ins` |
| `KPDot` | 83 | 2,4,1,1 | `.` | `Del` |

Toggle key default: `ScrollLock` (evdev 70). `settings.toggleKey` accepts any key name from the combo
key table (§3.4), e.g. `"ScrollLock"`, `"Pause"`, `"F13"`.

### 3.2 Config file — `$XDG_CONFIG_HOME/streampad/config.json`

```ts
interface Config {
  version: 1;
  settings: Settings;
  profiles: Profile[];          // order = cycling order; at least one profile
  library: string;              // shared JS prepended to every macro (user helper functions); default ""
}

interface Settings {
  toggleKey: string;                  // "ScrollLock"
  startInMacroMode: boolean;          // false. (Daemon restores last mode/profile from state.json if rememberState)
  rememberState: boolean;             // true
  unboundKeys: "block" | "passthrough";   // "block"
  holdMs: number;                     // 600
  globals: Partial<Record<KeyId, GlobalBinding>>;  // default: see §2.2
  devices: DeviceSettings;
  overlay: OverlaySettings;
  toastOnError: boolean;              // true: macro errors show an error toast on the overlay
}

interface GlobalBinding { tap: GlobalAction; hold: GlobalAction | null }

interface DeviceSettings {
  mode: "auto" | "manual";  // auto: grab every eligible keyboard (has numpad keys or the toggle key, has EV_REP)
  include: string[];        // manual mode: device ids to grab
  exclude: string[];        // auto mode: device ids never to grab
}
// device id = "<vendor hex4>:<product hex4>:<name>"  e.g. "1532:023f:Razer Razer Cynosa Lite"
// (phys suffix is NOT part of the id, so ids survive replugging into another port)

interface OverlaySettings {
  edge: "bottom" | "top" | "left" | "right";   // "bottom"
  align: "start" | "center" | "end";           // "end"   (bottom-right corner, next to a real numpad)
  offset: number;          // px from the aligned end along the edge; 0 = flush to the corner. default 0
  monitor: string | null;  // null = follow mouse; else connector name e.g. "DP-1"
  scale: number;           // 0.75–1.5, default 1.0 (multiplies all overlay geometry)
  theme: "dark" | "light"; // "dark"
  accent: string;          // "#8b7cf6"  (profile.color overrides when set)
  opacity: number;         // 0.5–1.0, default 0.94 (panel background alpha)
  blur: boolean;           // true: KWin blur-behind (KDE only)
  showHeader: boolean;     // true: profile name + position dots
  showLegends: boolean;    // true: key legends ("4" / "←"); false hides them entirely
  legendStyle: "full" | "number";  // "full" shows main+sub legend, "number" only main
  showLabels: boolean;     // true: action label under icon on keys without cover
  animationMs: number;     // 320 (slide-in duration; slide-out = 0.7×)
  peekOnModeChange: boolean;    // true
  peekOnProfileChange: boolean; // true
  peekMs: number;               // 1600
}

interface Profile {
  id: string;            // stable random id (e.g. "p_3f9c2a"); never reused
  name: string;          // "Audio"
  color: string | null;  // accent override "#ff7a59"
  icon: string | null;   // IconRef (see below), shown in overlay header + editor sidebar
  keys: Partial<Record<KeyId, Binding>>;
}

interface Binding {
  label: string;                 // "" allowed. Shown on the key (overlay + editor)
  icon: string | null;           // IconRef
  cover: string | null;          // AssetRef to png/jpg/webp/gif/svg — fills the key face; animated GIF/WebP play
  color: string | null;          // optional key tint
  macro: Macro | null;           // null = bound to nothing (key shows as empty)
  repeat: boolean;               // false
  concurrency: "ignore" | "restart" | "parallel";   // "ignore"
}

// IconRef:  "lucide:<kebab-name>"  → monochrome, tinted with theme text colour
//           AssetRef               → drawn as-is (e.g. an app's colour icon)
// AssetRef: absolute path, or path relative to $XDG_DATA_HOME/streampad/ (e.g. "covers/9f2c…e1.gif")

type Macro =
  | { type: "graph"; nodes: Node[]; edges: Edge[] }
  | { type: "script"; source: string };

interface Node { id: string; kind: string; params: Record<string, unknown>; position: { x: number; y: number } }
interface Edge { id: string; source: string; sourcePort: string; target: string }  // target port is always "in"
```

A "simple" binding (one action) is just a graph `trigger.press → <action>`. A linear chain
(`trigger.press → a → b → c`, no flow nodes, ≤1 out edge each) is shown by the editor as a step list.

### 3.3 Node catalog (graph macros)

Ports: every non-trigger node has exec input `in`. Default exec output is `out`. Flow nodes have named outputs.
Text params marked † support `{{varName}}` interpolation (macro variables, §6).

| kind | params | outputs | JS (see §6) |
|---|---|---|---|
| `trigger.press` | – | `out` | entry (top-level code) — exactly one per graph |
| `trigger.release` | – | `out` | entry of `function onRelease()` — at most one |
| `key.shortcut` | `combo: string` | out | `press(combo)` |
| `key.down` / `key.up` | `combo` | out | `keyDown(combo)` / `keyUp(combo)` |
| `key.type` | `text†`, `delayMs?: number` (default 8) | out | `type(text, {delayMs})` |
| `media` | `action: "playPause"\|"next"\|"prev"\|"stop"\|"volumeUp"\|"volumeDown"\|"mute"` | out | `media(action)` |
| `app.launch` | `app: string` (desktop id e.g. `"org.kde.dolphin"` or a command), `focusIfRunning: boolean` (true), `args?: string` | out | `launch(app, {focusIfRunning, args})` |
| `open.url` | `url†` | out | `openUrl(url)` |
| `open.path` | `path†` | out | `openPath(path)` |
| `run.command` | `command†`, `wait: boolean` (false) | out | `run(command, {wait})` |
| `window.focus` | `match: string`, `by: "app"\|"title"` | out | `window.focus(match, {by})` |
| `window.moveToOutput` | `target: "next"\|"prev"\|"left"\|"right"\|"up"\|"down"\|<connector>` | out | `window.moveToOutput(target)` |
| `window.action` | `action: "minimize"\|"maximize"\|"fullscreen"\|"close"\|"keepAbove"\|"center"` | out | `window.<action>()` |
| `desktop.switch` | `target: "next"\|"prev"\|"left"\|"right"\|"up"\|"down"\|number(1-based)` | out | `desktop.switchTo(target)` |
| `desktop.moveWindow` | `target` (as above), `follow: boolean` (false) | out | `desktop.moveWindow(target, {follow})` |
| `desktop.create` / `desktop.remove` | – | out | `desktop.create()` / `desktop.remove()` |
| `desktop.overview` | – | out | `desktop.overview()` |
| `kde.shortcut` | `component: string` (e.g. `"kwin"`), `shortcut: string` (e.g. `"Overview"`) | out | `kde.shortcut(c, s)` |
| `profile.switch` | `target: "next"\|"prev"\|<profileId>` | out | `profile.set(target)` |
| `macroMode.set` | `value: "on"\|"off"\|"toggle"` | out | `macroMode.set(...)` |
| `overlay.set` | `value: "show"\|"hide"\|"toggle"\|"peek"` | out | `overlay.*()` |
| `toast` | `text†`, `durationMs?` (1600) | out | `toast(text, {durationMs})` |
| `notify` | `title†`, `body†` | out | `notify(title, body)` |
| `var.set` | `name`, `value: string` (parsed as JSON if valid JSON, else string) | out | `vars.set(name, value)` |
| `var.toggle` | `name` | out | `vars.toggle(name)` |
| `clipboard.set` | `text†` | out | `clipboard.set(text)` |
| `sound.play` | `path` | out | `sound.play(path)` |
| `delay` | `ms: number` (100) | out | `sleep(ms)` |
| `script` | `code: string` (JS statements) | out | inlined in a block `{ … }` |
| `flow.if` | `condition: Condition` | `true`, `false` | `if (cond) {…} else {…}` |
| `flow.repeat` | `count: number` (2) | `body`, `done` | `for (let i = 0; i < count; i++) {…}` then done |
| `flow.stop` | – | (none) | `return` |

```ts
type Condition =
  | { type: "activeApp"; match: string }   // comma-separated list, case-insensitive substring match against
                                           // app id / resourceClass / desktopFile of the focused window;
                                           // "/regex/" form allowed
  | { type: "activeTitle"; match: string }
  | { type: "appRunning"; match: string }
  | { type: "var"; name: string; op: "truthy" | "falsy" | "eq" | "neq"; value?: string }
  | { type: "profile"; profileId: string }
  | { type: "time"; from: string; to: string }   // "HH:MM" local; wraps midnight
  | { type: "expr"; code: string };              // raw JS expression
```

Graph rules: exactly one `trigger.press`; ≤1 `trigger.release`; ≤1 edge per output port; no exec cycles
(loops only via `flow.repeat`); unknown kinds = compile error naming the node id. Nodes not reachable
from a trigger are ignored (editor shows them dimmed). Branches may re-converge (both `true` and
`false` lead to the same node) — the compiler must handle this correctly (no duplicated side effects).

### 3.4 Combo strings
`"ctrl+shift+t"`, `"meta+left"`, `"alt+f4"`, `"f13"`, `"kp4"`, `"playpause"`, `"ctrl+alt+delete"`.
Case-insensitive, `+`-separated, modifiers first: `ctrl`/`control`, `shift`, `alt`, `altgr`, `meta`/`super`/`win`.
Right-side variants: `rctrl`, `rshift`, `ralt`, `rmeta`. Key names = evdev `KEY_*` names without the prefix,
lower-case (`a`, `1`, `f1`…`f24`, `left`, `pageup`, `home`, `esc`, `enter`, `space`, `tab`, `backspace`,
`delete`, `insert`, `print`, `scrolllock`, `pause`, `menu`, `minus`, `equal`, `leftbrace`, `semicolon`,
`apostrophe`, `grave`, `backslash`, `comma`, `dot`, `slash`, `kp0`…`kp9`, `kpenter`, `kpplus`,
`kpminus`, `kpasterisk`, `kpslash`, `kpdot`, `volumeup`, `volumedown`, `mute`, `playpause`, `nextsong`,
`previoussong`, `stopcd`…) plus friendly aliases (`escape`, `return`, `pgup`, `pgdn`, `del`, `ins`, `next`,
`prev`, `up`, `down`, `right`, `win`). Parsed by `streampad_core::combo::parse_combo`.
Display format (editor/overlay): `Ctrl + Shift + T`, `Meta + ←`.

### 3.5 Runtime state — `$XDG_STATE_HOME/streampad/state.json` (daemon-owned)
`{ macroMode: bool, profileId: string, pinned: bool, vars: Record<string, JSON> }` (debounced writes).

### 3.6 Paths (all overridable via env for tests — tests MUST set these and never touch the real user dirs)
| What | Default | Env override |
|---|---|---|
| config dir | `$XDG_CONFIG_HOME/streampad` | `STREAMPAD_CONFIG_DIR` |
| data dir (covers, user icons) | `$XDG_DATA_HOME/streampad` | `STREAMPAD_DATA_DIR` |
| state dir | `$XDG_STATE_HOME/streampad` | `STREAMPAD_STATE_DIR` |
| runtime dir (socket, kwin scripts) | `$XDG_RUNTIME_DIR/streampad` | `STREAMPAD_RUNTIME_DIR` |
| socket | `<runtime>/daemon.sock` | `STREAMPAD_SOCKET` |
| shared assets (lucide icons) | first existing of `$STREAMPAD_SHARE_DIR`, `<exe>/../share/streampad`, `$XDG_DATA_HOME/streampad/share`, `/usr/local/share/streampad`, `/usr/share/streampad` | `STREAMPAD_SHARE_DIR` |

Lucide icons live at `<share>/icons/lucide/<name>.svg` (copied from npm `lucide-static/icons`).
`streampad_core::paths` implements all of this.

---------------------------------------------------------------------------------------------------------

## 4. IPC — Unix socket, newline-delimited JSON (UTF‑8, one object per line)

Socket: `$STREAMPAD_SOCKET` or `<runtime>/daemon.sock`, mode 0600.

```jsonc
// client → daemon
{ "id": 7, "method": "setProfile", "params": { "delta": 1 } }
// daemon → client (response; exactly one per request id)
{ "id": 7, "result": { … } }      |      { "id": 7, "error": { "message": "…" } }
// daemon → subscribed clients (unsolicited)
{ "event": "state", "data": { … } }
```

### 4.1 Methods
| method | params | result |
|---|---|---|
| `hello` | `{ client: "overlay"\|"editor"\|"cli", subscribe: bool, trace?: bool, layerShell?: bool }` — the overlay sets `layerShell` (whether it could use layer-shell); it fills `integrations.layerShell` | `{ version, state: DaemonState }` |
| `getState` | – | `DaemonState` |
| `setMacroMode` | `{ on: bool }` or `{ toggle: true }` | `{ macroMode }` |
| `setProfile` | `{ index }` \| `{ id }` \| `{ delta: ±1 }` | `{ profileIndex }` |
| `overlay` | `{ action: "show"\|"hide"\|"toggle"\|"peek", durationMs? }` — `show/toggle/peek` also force macro mode visibility rules off for preview purposes: **`preview: true`** param shows even outside macro mode (used by editor "Preview overlay") | `{ visible }` |
| `runBinding` | `{ profileId, key }` | `{ runId }` |
| `runMacro` | `{ macro: Macro, key?: KeyId }` (test an unsaved macro) | `{ runId }` |
| `cancelRun` | `{ runId }` | `{}` |
| `compileMacro` | `{ macro }` | `{ code }` or error `{ message, nodeId? }` |
| `setLearn` | `{ on: bool }` | `{}` |
| `listDevices` | – | `DeviceInfo[]` |
| `listOutputs` | – | `OutputInfo[]` (monitors, as reported by the overlay or KWin) |
| `reportOutputs` | `OutputInfo[]` (sent by the overlay on connect + on screen changes) | `{}` |
| `listWindows` | – | `WindowInfo[]` (KWin) |
| `reload` | – | `{}` (re-read config.json) |
| `simulateKey` | `{ key: KeyId, pressed: bool }` | `{}` — debug/testing: behaves exactly as if a captured pad key event arrived (only honoured when macro mode is on, like real keys); also `{ key: "Toggle" }` toggles mode |
| `getVars` / `setVar` | – / `{ name, value }` | `Record` / `{}` |
| `openEditor` | – | `{}` |
| `quit` | – | `{}` |

### 4.2 Events
| event | data |
|---|---|
| `state` | `DaemonState` (full snapshot; sent on every change, coalesced to ≤ 1 per 16 ms) |
| `key` | `{ key: KeyId, pressed: bool, kind: "macro"\|"global"\|"empty"\|"learn" }` (macro mode only; drives press animation) |
| `profile` | `{ index, direction: 1 \| -1 }` (sent right before the corresponding `state`; drives slide direction) |
| `toast` | `{ id, text, kind: "info"\|"success"\|"error", durationMs, icon?: IconRef-resolved-path }` |
| `learn` | `{ key: KeyId }` |
| `run` | `{ runId, key?: KeyId, status: "started"\|"finished"\|"failed"\|"cancelled", error?: string }` |
| `trace` | `{ runId, nodeId }` (only to clients that said `trace: true`) |
| `log` | `{ runId?, level: "info"\|"warn"\|"error", message }` (script `log()` output; editor console) |
| `configError` | `{ message }` (config.json invalid — daemon keeps the last good config) |

```ts
interface DaemonState {
  version: string;
  macroMode: boolean;
  profileIndex: number;
  profiles: { id: string; name: string; color: string | null; icon: string | null }[];  // icon resolved to absolute path
  profile: RenderProfile;              // current profile, render-ready
  overlay: {
    visible: boolean; pinned: boolean; preview: boolean;
    output: string | null;             // connector name to show on; null = compositor decides
    settings: OverlaySettings;
    accent: string;                    // effective accent (profile.color ?? settings.overlay.accent)
  };
  learn: boolean;
  devices: DeviceInfo[];
  integrations: { kwin: boolean; layerShell: boolean | null };
  configError: string | null;
}
interface RenderProfile { id: string; name: string; color: string | null; icon: string | null; keys: Record<KeyId, RenderKey> } // all 17 keys present
interface RenderKey {
  label: string;
  icon: string | null;      // absolute file path (lucide resolved) or null
  iconTint: boolean;        // true for lucide icons (tint with text colour)
  cover: string | null;     // absolute file path
  color: string | null;
  kind: "macro" | "global" | "empty";
  global: GlobalAction | null;   // when kind == "global" (tap action)
  legend: { main: string; sub: string | null };
}
interface DeviceInfo { id: string; name: string; path: string; phys: string | null; hasNumpad: boolean; eligible: boolean; grabbed: boolean; error: string | null }
interface OutputInfo { name: string; x: number; y: number; width: number; height: number; scale: number; primary: boolean; model: string | null }
interface WindowInfo { id: string; title: string; appId: string; resourceClass: string; desktopFile: string; pid: number; output: string | null; active: boolean }
```

Global keys in `RenderKey`: the daemon fills `label` with a default ("Next", "Prev", "Show / Hide") and `icon`
with a lucide icon (`chevron-down`, `chevron-up`, `eye`). Globals are global: a profile binding on a key that
has a global is ignored by the daemon, and the editor doesn't allow creating one.

---------------------------------------------------------------------------------------------------------

## 5. Daemon (`streampadd`)

* CLI: `streampadd [--no-overlay] [--no-input] [--only-device <id|path|name>] [--grab-timeout <secs>]
  [--foreground]`, `streampadd ctl <method> [json-params]` (prints JSON result; `ctl watch` prints events),
  `streampadd list-devices`.
  `--grab-timeout` is a development safety valve: release all grabs and exit after N seconds.
* Single instance (socket in use → exit with message).
* Spawns `streampad-overlay` (looked up next to its own exe, then `$PATH`) unless `--no-overlay`; restarts it
  with backoff if it crashes; child gets `PR_SET_PDEATHSIG(SIGTERM)`.
* Launched apps/commands are fully detached (`setsid`, and wrapped in `systemd-run --user --scope -q --collect`
  when available) so restarting the daemon never kills user apps.
* Hot-reloads `config.json` on change (inotify via `notify`, debounced 150 ms). Invalid config →
  `configError` event + keep previous config.
* Creates a default config (§7) when none exists.
* KWin bridge (KDE): owns D-Bus name `io.github.ometavr.StreamPad`, object `/io/github/ometavr/StreamPad`,
  interface `io.github.ometavr.StreamPad.KWin` with methods `Reply(token: s, json: s)` and `Event(json: s)`.
  One-shot queries/commands = temp KWin JS script (`org.kde.KWin /Scripting loadScript(path, name)` →
  `/Scripting/Script<id>` `org.kde.kwin.Script.run`) that answers via `callDBus(... "Reply", token, json)`;
  1 s timeout; always `unloadScript(name)` afterwards. A persistent watcher script reports active-window
  changes via `Event` so `window.active()` is instant. Never act on user windows in tests — create your own
  throwaway window to test window actions.
* Non-KDE: window/desktop features return a clear "unsupported on this desktop" error (toast), everything else works.

Systemd user unit `streampad.service`: `ExecStart=%h/.local/bin/streampadd`, `Restart=on-failure`,
`KillMode=process`, `After=graphical-session.target`, `PartOf=graphical-session.target`,
`WantedBy=graphical-session.target`.

---------------------------------------------------------------------------------------------------------

## 6. Macro runtime (JavaScript, QuickJS) — the API is the same for hand-written scripts and compiled graphs

Each run gets a fresh QuickJS context on a worker thread. The API is **synchronous** (`sleep` blocks the
worker, is interruptible by cancel). Execution order: `settings`-level `config.library` → macro code
(top level = press handler). If the code defines a global `function onRelease(e)`, the context stays alive
until the key is released, then `onRelease(event)` runs, then the context is disposed. Unhandled exceptions →
`run` failed event (+ error toast if `toastOnError`).

```ts
declare const event: { key: KeyId | null; type: "press"; repeat: boolean; profile: { id: string; name: string } };
function press(...combos: string[]): void;          // tap each combo in order
function keyDown(combo: string): void;  function keyUp(combo: string): void;
function type(text: string, opts?: { delayMs?: number }): void;
function media(action: "playPause"|"next"|"prev"|"stop"|"volumeUp"|"volumeDown"|"mute"): void;
function sleep(ms: number): void;
function launch(app: string, opts?: { focusIfRunning?: boolean; args?: string }): void;
function openUrl(url: string): void;  function openPath(path: string): void;
function run(command: string, opts?: { wait?: boolean }): { code: number; stdout: string; stderr: string } | undefined;
const window: {
  active(): WindowInfo | null; list(): WindowInfo[];
  isActive(match: string): boolean; isRunning(match: string): boolean;   // Condition "activeApp" semantics
  focus(match: string, opts?: { by?: "app" | "title" }): boolean;
  moveToOutput(target: string): void;
  minimize(): void; maximize(): void; fullscreen(): void; close(): void; keepAbove(): void; center(): void;
};
const desktop: { switchTo(t: string|number): void; moveWindow(t: string|number, opts?: {follow?: boolean}): void;
                 create(): void; remove(): void; overview(): void; current(): { index: number; name: string; count: number } };
const kde: { shortcut(component: string, name: string): void };
const profile: { next(): void; prev(): void; set(idOrNameOrIndex: string | number): void; current(): { id: string; name: string; index: number } };
const macroMode: { get(): boolean; set(on: boolean): void; toggle(): boolean };
const overlay: { show(): void; hide(): void; toggle(): void; peek(ms?: number): void };
function toast(text: string, opts?: { durationMs?: number; kind?: "info"|"success"|"error" }): void;
function notify(title: string, body?: string): void;
const vars: { get(name: string, fallback?: any): any; set(name: string, value: any): void; toggle(name: string): boolean };
const clipboard: { get(): string; set(text: string): void };
const sound: { play(path: string): void };
function log(...args: any[]): void;
```

**Host boundary.** The runtime installs one native function `__host(method: string, argsJson: string) -> string`
(+ native `sleep`, and `__node(id)` for tracing). A JS prelude builds the friendly API above on top of it.
The daemon implements `streampad_script::Host::call(method, args) -> Result<Value, String>` with these
methods (args/results are JSON):

`key.press {combo}` · `key.down {combo}` · `key.up {combo}` · `key.type {text, delayMs}` · `key.media {action}` ·
`app.launch {app, focusIfRunning, args}` · `open.url {url}` · `open.path {path}` ·
`run.command {command, wait} → {code, stdout, stderr}|null` ·
`window.active → WindowInfo|null` · `window.list → WindowInfo[]` · `window.focus {match, by} → bool` ·
`window.moveToOutput {target}` · `window.action {action}` ·
`desktop.switch {target}` · `desktop.moveWindow {target, follow}` · `desktop.create` · `desktop.remove` · `desktop.overview` ·
`desktop.current → {index, name, count}` · `kde.shortcut {component, name}` ·
`profile.set {target}` (target: "next"|"prev"|id|name|index) · `profile.current → {id, name, index}` ·
`macroMode.get → bool` · `macroMode.set {on} ` · `macroMode.toggle → bool` ·
`overlay.set {value: "show"|"hide"|"toggle"|"peek", durationMs?}` ·
`toast {text, durationMs, kind}` · `notify {title, body}` ·
`vars.get {name} → JSON|null` · `vars.set {name, value}` · `vars.toggle {name} → bool` (atomic in daemon) ·
`clipboard.get → string` · `clipboard.set {text}` · `sound.play {path}` ·
`log {level, message}` · `trace {nodeId}`.

`{{name}}` interpolation in † params compiles to a template literal reading `vars.get("name", "")`.

---------------------------------------------------------------------------------------------------------

## 7. Default profiles (first run) — Linux/KDE ports of the AHK profiles

1. **Audio** (lucide `audio-lines`): 5 play/pause · 4 *if active app is a browser* (`firefox,chromium,chrome,opera,brave,
   vivaldi,edge,zen,librewolf`) → `alt+left` else media prev · 6 same with `alt+right` / media next ·
   8 volume up (repeat) · 2 volume down (repeat) · 0 mute · 3 toggle var `gameMode` + toast "Game mode: ON/OFF".
2. **Apps** (`layout-grid`): `app.launch` with `focusIfRunning: true` (the AHK ActivateOrLaunch) on keys 1–8:
   `discord`, `firefox`, `steam`, `spotify`, `org.kde.plasma-systemmonitor` (was Task Manager), `org.kde.dolphin`
   (was Explorer), `org.kde.konsole`, `org.pulseaudio.pavucontrol` (was VoiceMeeter). Pick the desktop ids that
   actually exist on the machine when generating defaults; skip keys whose app isn't installed.
3. **Web** (`globe`): 1 YouTube · 2 Google · 4 GitHub · 5 Discord web · 7 Twitch · 8 Kick · 0 ChatGPT (`open.url`).
4. **Windows** (`app-window`): 4/6/8/2 move active window to the monitor left/right/up/down · 5 maximize ·
   7 minimize · 9 keep above · 1 center · 3 fullscreen.
5. **Desktops** (`layers`): 4/6 switch desktop left/right · 1/3 move window to prev/next desktop (follow) ·
   8 new desktop · 2 remove current desktop · 5 overview.

Every default binding has a label and a lucide icon so the pad looks finished out of the box.

---------------------------------------------------------------------------------------------------------

## 8. Overlay (`streampad-overlay`) — visual spec

Qt 6 QML, compiled into the binary (`qt_add_qml_module`). One `Window` (transparent, frameless) using
LayerShellQt (`import org.kde.layershell` attached props, or the C++ API): layer `Top` (or `Overlay`),
keyboard interactivity `None`, exclusive zone `-1`/0, anchored to `settings.edge` (+ the aligned side for
start/end), scope `"streampad"`. Input region = the panel shape only (rest click-through).
Screen: `state.overlay.output` → matching `QScreen::name()`; null → `wantsToBeOnActiveScreen`.
The window is mapped only while visible or animating. On X11 / no layer-shell: frameless
`Qt::Tool | WindowStaysOnTopHint | WindowDoesNotAcceptFocus` window positioned manually.

**Geometry** (logical px × `scale`): key unit 58, gap 8, padding 14, header 40 (if `showHeader`),
corner radius 22 (outer convex corners), key radius 11, **edge fillet radius 20**.
Panel = 4×5 grid + padding (+ header on the side away from the edge).

**Shape — the signature detail.** The side of the panel that touches the screen edge is flush with it and its
two corners are *inverted* (concave quarter-circle fillets of radius 20 flaring outward along the screen edge),
so the panel looks like it grows out of the screen edge. The far corners are normal convex radius 22.
When aligned to a corner with `offset == 0` the side touching the adjacent screen edge also flares into that
edge (two fillets: one on each screen edge; the corner itself is square, hidden in the screen corner).
Draw it with a single path (QML `Shape`/`ShapePath` or a QQuickPaintedItem) — background + 1 px hairline
border along the *non-flush* sides only + soft shadow. Optional KWin blur-behind matching the shape.

**Animation.** Show: content translates from fully outside the screen edge (behind the edge) to rest,
`animationMs` with `Easing.OutCubic`-like curve (or a critically damped spring), opacity 0→1 over the first 40 %.
Hide: reverse, `0.7 × animationMs`, `Easing.InCubic`, then unmap. Interruptible (hide during show reverses smoothly
from current position). Profile change: keys + header slide 18 px & crossfade in the direction of change
(`profile` event direction; next = content moves up/left), 220 ms. Key press: keycap depresses 2 px, accent glow ring
fades in 60 ms / out 260 ms. Must run at the monitor refresh rate (no timers driving animation).

**Key face** (keycap look: base fill, 1 px top highlight, 3 px darker bottom skirt for depth):
* cover set → image/GIF fills the face (rounded clip, `PreserveAspectCrop`); GIF/WebP animate while visible;
  legend (if shown) sits top-left on a small dark scrim chip.
* else → icon (24 px, centred slightly above middle) + label (10.5 px, ≤ 2 lines, elided) under it (if `showLabels`);
  legend top-left: main 14 px semibold + sub 10 px muted (sub only when `legendStyle = "full"`).
* empty key → legend only, 45 % opacity. If `showLegends = false`, empty keys are blank keycaps.
* global keys → accent-tinted icon + label ("Next" / "Prev" / "Show / Hide").
* key `color` tints the keycap.

**Header**: profile icon + name (13 px semibold) on the left, profile position dots on the right (current = accent pill).

**Toasts**: small pill (icon + text) that slides out from the same edge just beside/above the panel (or alone at
the overlay position when the panel is hidden). Error kind = red accent. Stack ≤ 3.

**Theme (dark)**: panel `#121318` @ opacity, border `rgba(255,255,255,0.08)`, keycap `#1f2128`, skirt `#14151a`,
highlight `rgba(255,255,255,0.06)`, text `#e9e9ef`, muted `#8b8d98`, accent from state.
**Theme (light)**: panel `#f6f6f8`, border `rgba(0,0,0,0.08)`, keycap `#ffffff`, skirt `#d9dae0`, text `#17181c`, muted `#6b6e7a`.
Font: Inter (bundled) → fallback system sans.

Dev/test: `streampad-overlay --mock <state.json> [--screenshot out.png [--after-ms N]]` renders without a
daemon (offscreen platform OK) for visual verification. `--socket <path>` overrides the socket.

---------------------------------------------------------------------------------------------------------

## 9. Editor (`streampad`) — UX spec

Aesthetic: quiet, precise, instrument-like (Linear / Raycast / Arc). Near-black graphite surfaces
(`#0c0d10` app bg, `#121318` panels, `#181a20` raised, `#20222a` controls), hairline borders
`rgba(255,255,255,0.06–0.10)`, text `#e9e9ef` / muted `#8b8d98` / faint `#5d606b`, one vivid accent
(default `#8b7cf6`, the current profile's colour tints selection states), Inter Variable 13 px base,
JetBrains Mono for code, radii 6/8/12, 150–220 ms ease-out micro-transitions, lucide icons 16 px stroke 1.75.
Light theme via tokens (follows system by default). The numpad is the hero of the main screen.

Layout:
* **Sidebar** (left, 248 px): logo/wordmark, profiles list (colour dot, icon, name, live "active" indicator for the
  daemon's current profile, drag to reorder, double-click rename, context menu: rename/duplicate/export/delete with
  undo-toast instead of confirm), "New profile" (blank or duplicate), bottom: Settings, daemon status pill
  (Running · Macro mode on · or "Not running — Start").
* **Canvas** (centre): large faithful numpad rendering (same visual language as the overlay; keycaps with legends,
  icons, labels, covers incl. animated GIFs). Click selects; arrow keys move selection; Delete clears; Enter opens the
  action picker; Ctrl+C/V copy/paste a binding; drag an image file onto a key to set its cover; drag a key onto
  another to swap (Alt = copy). Physical presses (daemon `key` events) animate the matching key.
  Toolbar above: profile name (inline editable), colour, icon, "Press to select" (learn mode), "Preview overlay".
  Global keys are shown with a lock badge + tooltip ("Global: next profile — change in Settings").
* **Inspector** (right, 360 px) for the selected key: label, icon (lucide picker w/ search + app icons), cover
  (drop zone/preview/remove), then the **Macro editor**:
  - Empty → big "Choose an action" button → command-palette style picker (search, categories: Media, Keyboard,
    Apps, Web & Files, Windows, Desktops, StreamPad, System, Advanced), recent actions on top.
  - One action / linear chain → step list (drag reorder, add step, delete, per-step param form inline).
    Choosing an action sets a smart default label + icon if the user hasn't set them (e.g. app name + app icon).
  - "Open flow editor" (branching/loops) and "Edit as script".
  - Options: repeat while held, when already running (ignore/restart/parallel). "Test" runs it now via the daemon.
* **Flow editor** (full-window view over the canvas, breadcrumb `Profile › Numpad 4 › Flow`, Esc to return):
  React Flow canvas with dotted grid; left node palette (search, drag or click to add); compact node cards
  (category colour stripe, icon, title, one-line param summary); right node inspector (same param forms as step list);
  drag a wire into empty space → quick-add menu at that spot creating a connected node; tidy-up auto-layout;
  validation errors on nodes; live execution highlight from daemon `trace` events; tabs **Flow | Code** (generated
  JS, read-only) with "Convert to script" (one-way, confirmed).
* **Script editor**: CodeMirror 6, JS, theme matching tokens, autocomplete + hover docs for the whole API (§6),
  syntax errors inline, API reference side panel with insert, "Test" with a console showing `log` + errors
  (daemon `log`/`run` events for that runId).
* **Settings** (own view, section nav): General (toggle key, start in macro mode, unbound keys, hold time, start at login),
  Globals (which key does what), Overlay (visual position picker: mini layout of the user's actual monitors from
  `listOutputs` with "Follow mouse" or a specific monitor; 8 position hotspots → edge+align; offset; scale; theme;
  accent; opacity; blur; header; legends + style; labels; animation speed; peek toggles/duration; "Preview" button),
  Devices (detected keyboards with grab toggles + status), Scripting (shared library editor), About.
* Autosave (debounced 400 ms, atomic write) with an unobtrusive "Saved" indicator; full undo/redo (Ctrl+Z / Ctrl+Shift+Z).
* Single instance. Works without the daemon (editing only; status pill offers Start).
* Every frontend call goes through `src/lib/backend.ts` (`Backend` interface) with a Tauri implementation and a
  **mock implementation** (used automatically when not running inside Tauri) so the UI runs in a normal browser
  via `bun run dev` for development + screenshots.
