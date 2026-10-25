# StreamPad macro API

Reference for the JavaScript that StreamPad macros run. Hand-written scripts and flows from the flow
editor use the same API (flows compile to it, see [Flows](#flows)). The short form of this API is in
docs/SPEC.md §6; this file is the precise reference the editor's autocomplete and docs are built from.

- [How a macro runs](#how-a-macro-runs)
- [`event`](#event)
- Keyboard: [`press`](#press) · [`keyDown` / `keyUp`](#keydown--keyup) · [`type`](#type) · [`media`](#media)
- Timing: [`sleep`](#sleep) · [`isTimeBetween`](#istimebetween)
- Apps & files: [`launch`](#launch) · [`openUrl`](#openurl) · [`openPath`](#openpath) · [`run`](#run)
- Windows: [`window.*`](#window) · [matching rules](#matching-rules)
- Desktops & KDE: [`desktop.*`](#desktop) · [`kde.shortcut`](#kdeshortcut)
- StreamPad: [`profile.*`](#profile) · [`macroMode.*`](#macromode) · [`overlay.*`](#overlay)
- Feedback: [`toast`](#toast) · [`notify`](#notify) · [`sound.play`](#soundplay) · [`log` / `console`](#log--console)
- Data: [`vars.*`](#vars) · [`clipboard.*`](#clipboard)
- [Flows](#flows) · [Host boundary](#host-boundary-for-daemon-implementers)

---

## How a macro runs

- **One run per trigger.** Every key press starts a fresh JavaScript engine (QuickJS, modern ES2023) on
  its own thread. Nothing but [`vars`](#vars) survives from one run to the next.
- **Order.** The shared library (Settings → Scripting) runs first, then the macro. Functions and
  constants the library defines are available to every macro.
- **Top level = press handler.** The macro's top-level code runs when the key is pressed. A top-level
  `return` ends it early:
  ```js
  if (!window.isActive("obs")) return;
  press("ctrl+shift+r");
  ```
- **Synchronous.** Every API call blocks until it is done, and [`sleep`](#sleep) pauses the macro, so
  macros read top to bottom without `await`. (Promises still work: `.then` callbacks and `async`
  functions run right after the handler that started them.)
- **Release handler.** If the macro defines `onRelease`, the run stays alive until the key is released
  and then calls `onRelease(event)` with `event.type === "release"`. A release that arrives while the
  press handler is still running is remembered, so a quick tap still runs `onRelease` once the press
  handler is done. If the press handler fails or is cancelled, `onRelease` does not run.
  ```js
  keyDown("shift");
  function onRelease() {
    keyUp("shift");
  }
  ```
  `const onRelease = () => { … }` works too. "Test" in the editor releases the key right away.
- **Cancellation.** A run can be cancelled (the editor's stop button, `concurrency: "restart"`, daemon
  shutdown). Cancelling interrupts `sleep` immediately and stops any running code, including endless
  loops; no API call succeeds after that.
- **Limits.** 64 MB of memory and about 1 MB of JavaScript stack per run. Going over fails the run with
  `out of memory …` or `RangeError: Maximum call stack size exceeded`.
- **Errors.** An uncaught exception (or a rejected promise nobody handles) fails the run. The error reads
  `message (line L:C)`, followed by the call stack when more than one function is involved; it is shown
  in the editor console and, unless error toasts are turned off in Settings, as an error toast. API calls
  throw `Error`s with readable messages (`invalid shortcut "ctrl+nope": unknown key "nope"`, `window
  management is not supported on this desktop (needs KDE Plasma / KWin)`) that `try`/`catch` can handle;
  wrong argument types throw a `TypeError` naming the function, e.g.
  `TypeError: press: expected a shortcut as a string, got 5`.
- **Strict mode, read-only API.** Code runs in strict mode. The API globals are read-only and the
  namespace objects are frozen, so `press = …` or `window.focus = …` throws. Declaring a local with the
  same name inside the macro is allowed and only affects that macro.
- **Not a browser.** There is no DOM, `fetch`, timers (`setTimeout`) or Node.js modules; everything the
  macro does to the system goes through this API. `window` is StreamPad's window API, not a browser
  window.

## event

```ts
const event: {
  key: KeyId | null;                 // "KP4", "KPEnter", … ; null when run from the editor without a key
  type: "press" | "release";         // "release" inside onRelease
  repeat: boolean;                   // true when triggered by key auto-repeat (bindings with "repeat")
  profile: { id: string; name: string };  // the profile the binding belongs to
};
```

Describes what triggered the run. Read-only. `onRelease(event)` receives the same object with
`type: "release"`; the global `event` also changes to it while `onRelease` runs.

```js
if (event.repeat) return;           // ignore auto-repeat
log(`pressed ${event.key} in ${event.profile.name}`);
```

## Keyboard

Shortcut strings ("combos") are `+`-separated, case-insensitive, modifiers first: `"ctrl+shift+t"`,
`"meta+left"`, `"alt+f4"`, `"f13"`, `"kp4"`, `"playpause"`. Modifiers: `ctrl`/`control`, `shift`, `alt`,
`altgr`, `meta`/`super`/`win`, right-hand `rctrl`, `rshift`, `ralt`, `rmeta`. Key names are the Linux
`KEY_*` names in lower case (`a`, `1`, `f1`…`f24`, `left`, `pageup`, `home`, `esc`, `enter`, `space`, `tab`,
`backspace`, `delete`, `insert`, `print`, `minus`, `equal`, `comma`, `dot`, `slash`, `kp0`…`kp9`,
`kpenter`, `volumeup`, `mute`, …) plus aliases (`escape`, `return`, `pgup`, `pgdn`, `del`, `ins`, `next`,
`prev`, `up`, `down`). Full syntax: docs/SPEC.md §3.4. Keys are sent by StreamPad's virtual keyboard.

### press

```ts
function press(...combos: string[]): void
```

Taps each shortcut in order: presses its keys (modifiers first) and releases them in reverse order.
Throws if no shortcut is given or one is invalid.

```js
press("ctrl+c");
press("ctrl+a", "ctrl+c");          // select all, then copy
```

### keyDown / keyUp

```ts
function keyDown(combo: string): void
function keyUp(combo: string): void
```

Holds the keys of `combo` down / releases them. Pair every `keyDown` with a `keyUp`; `onRelease` is the
natural place for it ("hold Numpad 5 = hold Shift").

```js
keyDown("ctrl");
press("c");
keyUp("ctrl");
```

### type

```ts
function type(text: string, opts?: { delayMs?: number /* 8 */ }): void
```

Types `text` key by key with `delayMs` milliseconds between keys. Text the US keyboard layout cannot
produce (emoji, accented letters) is pasted through the clipboard instead. Non-string values are converted
to text the way [`log`](#log--console) prints them.

```js
type("Thanks for the follow! ");
type(`Run #${vars.get("runs", 0)}`, { delayMs: 20 });
```

### media

```ts
function media(action: "playPause" | "next" | "prev" | "stop" | "volumeUp" | "volumeDown" | "mute"): void
```

Sends a media key (works with any MPRIS player and the system volume).

```js
media("playPause");
```

## Timing

### sleep

```ts
function sleep(ms: number): void
```

Pauses the macro for `ms` milliseconds. `0` or less returns immediately; `Infinity` waits until the run is
cancelled. Cancelling interrupts the sleep at once. Throws a `TypeError` for anything but a number.

```js
press("ctrl+t");
sleep(150);                         // give the new tab a moment
type("github.com\n");
```

### isTimeBetween

```ts
function isTimeBetween(from: string, to: string): boolean
```

Whether the local time is in `[from, to)`, both `"HH:MM"` (24-hour). Ranges wrap past midnight
(`"22:00"` to `"06:00"`); `from === to` means all day.

```js
if (isTimeBetween("22:00", "07:00")) media("volumeDown");
```

## Apps & files

### launch

```ts
function launch(app: string, opts?: { focusIfRunning?: boolean /* true */; args?: string /* "" */ }): void
```

Starts an app by desktop file id (`"org.kde.dolphin"`, `"firefox"`) or as a command line. With
`focusIfRunning` (the default) an already open window of the app is focused instead of starting another
copy; pressing again cycles through its windows. `args` are extra command-line arguments. Apps are started
fully detached, so they keep running when StreamPad restarts.

```js
launch("discord");
launch("firefox", { focusIfRunning: false, args: "--private-window" });
```

### openUrl

```ts
function openUrl(url: string): void
```

Opens `url` in the default browser (or the app registered for its scheme).

```js
openUrl("https://youtube.com");
```

### openPath

```ts
function openPath(path: string): void
```

Opens a file with its default app, or a folder in the file manager. `~` expands to the home folder.
Throws if the path does not exist.

```js
openPath("~/Videos/Recordings");
```

### run

```ts
function run(command: string, opts?: { wait?: boolean /* false */ }):
  { code: number; stdout: string; stderr: string } | undefined
```

Runs `command` with `/bin/sh -c`. By default it is started detached and `run` returns `undefined` at once.
With `wait: true` the macro waits for it to exit and gets its exit code and output (cancelling the macro
stops the command).

```js
run("notify-send 'Stream starting'");
const { stdout } = run("playerctl metadata title", { wait: true });
toast(stdout.trim());
```

## Windows

```ts
interface WindowInfo {
  id: string; title: string;
  appId: string; resourceClass: string; desktopFile: string;   // app identity (see matching rules)
  pid: number; output: string | null;                         // monitor connector, e.g. "DP-1"
  active: boolean;
}
```

Window functions need KDE Plasma (KWin). Elsewhere `window.active()` returns `null`, `window.list()`
returns `[]` (so conditions are simply false) and the other window functions throw an error saying window
management is not supported on this desktop.

### window.active

```ts
window.active(): WindowInfo | null
```

The focused window, or `null`.

```js
const w = window.active();
if (w) log(w.appId, w.title);
```

### window.list

```ts
window.list(): WindowInfo[]
```

All normal (non-desktop, non-panel) windows.

```js
log(window.list().map((w) => w.title));
```

### window.isActive

```ts
window.isActive(match: string, opts?: { by?: "app" | "title" /* "app" */ }): boolean
```

Whether the focused window matches `match` (see [matching rules](#matching-rules)); with `by: "title"`
the window title is matched instead of the app. This is the flow editor's "Active app" / "Active window
title" condition.

```js
if (window.isActive("firefox, chromium, zen")) press("alt+left");
else media("prev");
```

### window.isRunning

```ts
window.isRunning(match: string, opts?: { by?: "app" | "title" }): boolean
```

Whether any window matches `match`. The "App is running" condition.

```js
if (!window.isRunning("obs")) launch("com.obsproject.Studio");
```

### window.focus

```ts
window.focus(match: string, opts?: { by?: "app" | "title" /* "app" */ }): boolean
```

Focuses (and un-minimizes) the most recently used window that matches; if a matching window is already
focused, the next one is focused, so repeated presses cycle. Returns whether a window was found.

```js
if (!window.focus("spotify")) launch("spotify");
window.focus("YouTube", { by: "title" });
```

### window.moveToOutput

```ts
window.moveToOutput(target: "next" | "prev" | "left" | "right" | "up" | "down" | string): void
```

Moves the focused window to another monitor: a direction, or a connector name such as `"DP-1"`.

```js
window.moveToOutput("left");
```

### window.minimize · maximize · fullscreen · close · keepAbove · center

```ts
window.minimize(): void     // minimize the focused window
window.maximize(): void     // toggle maximized
window.fullscreen(): void   // toggle fullscreen
window.close(): void        // close it (like clicking ×)
window.keepAbove(): void    // toggle "keep above others"
window.center(): void       // center it on its monitor
```

All act on the focused window and throw if there is none.

```js
window.keepAbove();
```

### Matching rules

Used by `isActive`, `isRunning`, `focus` and the flow conditions:

- **List of names**: `"firefox, chromium"` matches when any comma-separated item is a case-insensitive
  substring of the window's `appId`, `resourceClass` or `desktopFile` (or of the `title` with
  `by: "title"`). Spaces around items are ignored.
- **Regular expression**: a pattern written as `/…/flags`, e.g. `"/^org\\.kde\\./"`, is a JavaScript
  regular expression tested against the same fields. Without flags it is case-insensitive; give flags
  (for example `/…/u`) to choose them yourself.

## Desktop

Virtual desktop functions (KDE Plasma only). Targets: `"next"`, `"prev"`, `"left"`, `"right"`, `"up"`,
`"down"`, or a desktop number starting at 1.

### desktop.switchTo

```ts
desktop.switchTo(target: string | number): void
```

```js
desktop.switchTo("right");
desktop.switchTo(1);
```

### desktop.moveWindow

```ts
desktop.moveWindow(target: string | number, opts?: { follow?: boolean /* false */ }): void
```

Moves the focused window to another desktop; with `follow: true` you switch along with it.

```js
desktop.moveWindow("next", { follow: true });
```

### desktop.create · remove · overview

```ts
desktop.create(): void      // add a desktop
desktop.remove(): void      // remove the current desktop
desktop.overview(): void    // toggle the Overview effect
```

```js
desktop.overview();
```

### desktop.current

```ts
desktop.current(): { index: number; name: string; count: number }   // index is 1-based
```

```js
const d = desktop.current();
toast(`${d.name} (${d.index}/${d.count})`);
```

### kde.shortcut

```ts
kde.shortcut(component: string, name: string): void
```

Triggers a global shortcut registered with KDE by its component and action name (as listed by the editor's
shortcut picker), e.g. `("kwin", "Overview")`, `("kwin", "Window Close")`, `("plasmashell", "activate
application launcher")`.

```js
kde.shortcut("kwin", "Expose");
```

## StreamPad

### profile

```ts
profile.next(): void
profile.prev(): void
profile.set(target: string | number): void   // profile id, profile name or 0-based index
profile.current(): { id: string; name: string; index: number }   // index is 0-based
```

Switches the current macropad profile (wrapping around at the ends) or reads it.

```js
profile.set("Audio");
if (profile.current().index === 0) toast("first profile");
```

### macroMode

```ts
macroMode.get(): boolean
macroMode.set(on: boolean): void
macroMode.toggle(): boolean                 // returns the new state
```

Reads or changes macro mode (what Scroll Lock toggles). `set` requires a boolean.

```js
macroMode.set(false);                       // back to a normal numpad
```

### overlay

```ts
overlay.show(): void                        // show the on-screen numpad until hidden
overlay.hide(): void
overlay.toggle(): void
overlay.peek(ms?: number): void             // show briefly (default: the "peek" setting, 1600 ms)
```

```js
overlay.peek(3000);
```

## Feedback

### toast

```ts
function toast(text: string, opts?: { durationMs?: number /* 1600 */; kind?: "info" | "success" | "error" }): void
```

Shows a small message next to the on-screen numpad. Non-string values are printed like `log` prints them.

```js
toast(`Game mode: ${vars.toggle("gameMode") ? "ON" : "OFF"}`);
toast("Upload failed", { kind: "error", durationMs: 4000 });
```

### notify

```ts
function notify(title: string, body?: string): void
```

Sends a desktop notification.

```js
notify("Recording", "Saved to ~/Videos");
```

### sound.play

```ts
sound.play(path: string): void
```

Plays a sound file (ogg, oga, wav, …) without waiting for it to finish. Relative paths are relative to
StreamPad's data folder (`~/.local/share/streampad/`).

```js
sound.play("/usr/share/sounds/freedesktop/stereo/complete.oga");
```

### log / console

```ts
function log(...args: any[]): void
console.log / console.info / console.debug(...args: any[]): void   // same as log
console.warn(...args: any[]): void
console.error(...args: any[]): void
```

Writes a line to the editor's console (and the daemon log). Arguments are joined with spaces; strings are
printed as they are, everything else is pretty-printed: objects and arrays on one line when short, indented
otherwise (at most 4 levels deep and 100 entries per collection), errors with their stack, `Map`, `Set`,
`Date`, circular references marked `[Circular]`.

```js
log("active window:", window.active());
console.warn("no player running");
```

## Data

### vars

```ts
vars.get(name: string, fallback?: any): any        // fallback (default null) when unset or null
vars.set(name: string, value: any): void           // any JSON value; undefined is stored as null
vars.toggle(name: string): boolean                 // flips truthiness, returns the new value (atomic)
```

Named values shared by all macros and kept across runs and restarts. They are what `{{name}}` in flow
editor text fields reads. Values must be JSON data (numbers, strings, booleans, null, arrays, objects).

```js
const count = vars.get("deaths", 0) + 1;
vars.set("deaths", count);
toast(`Deaths: ${count}`);
```

### clipboard

```ts
clipboard.get(): string                    // "" when the clipboard holds no text
clipboard.set(text: string): void
```

```js
clipboard.set(clipboard.get().trim().toUpperCase());
press("ctrl+v");
```

---

## Flows

The flow editor compiles a graph to readable JavaScript using this API; the Code tab shows it and
"Convert to script" turns it into a script. Each node becomes one statement:

| Node | JavaScript |
|---|---|
| Press trigger | start of the top-level code |
| Release trigger | `function onRelease(event) { … }` |
| Shortcut / Key down / Key up | `press(combo)` / `keyDown(combo)` / `keyUp(combo)` |
| Type text | `type(text)` or `type(text, { delayMs })` |
| Media | `media(action)` |
| Launch app | `launch(app)` or `launch(app, { focusIfRunning: false, args })` |
| Open URL / Open file | `openUrl(url)` / `openPath(path)` |
| Run command | `run(command)` or `run(command, { wait: true })` |
| Focus window | `window.focus(match)` or `window.focus(match, { by: "title" })` |
| Move to monitor | `window.moveToOutput(target)` |
| Window action | `window.minimize()`, `window.maximize()`, … |
| Switch desktop / Move to desktop | `desktop.switchTo(t)` / `desktop.moveWindow(t, { follow: true })` |
| New / Remove desktop / Overview | `desktop.create()` / `desktop.remove()` / `desktop.overview()` |
| KDE shortcut | `kde.shortcut(component, shortcut)` |
| Switch profile | `profile.next()`, `profile.prev()` or `profile.set(id)` |
| Macro mode | `macroMode.set(true)`, `macroMode.set(false)` or `macroMode.toggle()` |
| Overlay | `overlay.show()`, `overlay.hide()`, `overlay.toggle()` or `overlay.peek()` |
| Toast / Notification | `toast(text)` / `notify(title, body)` |
| Set variable / Toggle variable | `vars.set(name, value)` / `vars.toggle(name)` |
| Set clipboard / Play sound | `clipboard.set(text)` / `sound.play(path)` |
| Delay | `sleep(ms)` |
| Script | the code, inside `{ … }` so its declarations stay local |
| If | `if (condition) { … } else { … }`; branches that meet again continue after the `if` |
| Repeat | `for (let i = 0; i < count; i++) { … }` (`j`, `k` when nested), then the "done" output |
| Stop | `return` (ends the whole macro, also inside a repeat) |

Conditions of the If node:

| Condition | JavaScript |
|---|---|
| Active app | `window.isActive(match)` |
| Active window title | `window.isActive(match, { by: "title" })` |
| App is running | `window.isRunning(match)` |
| Variable is truthy / falsy | `vars.get(name)` / `!vars.get(name)` |
| Variable equals / differs | `vars.get(name) === value` / `!==` (the value is JSON when it parses as JSON, else text) |
| Profile is | `profile.current().id === id` |
| Time between | `isTimeBetween(from, to)` |
| Expression | the expression as written |

**`{{name}}` in text fields.** Type text, URL, file path, command, toast text and notification title/body
may contain `{{name}}`, which is replaced by the variable's value (empty when unset):
`"Deaths: {{deaths}}"` compiles to ``toast(`Deaths: ${vars.get("deaths", "")}`)``.

Nodes that are not connected to a trigger are ignored. A flow whose branches share steps in a way that
`if`/`else` cannot express compiles to one function per step instead; it behaves the same.

## Host boundary (for daemon implementers)

The runtime installs three native functions and builds the API above on them in `src/prelude.js`:

- `__host(method: string, argsJson: string): string`: calls `Host::call(run, method, args)`; the result is
  returned as JSON, an `Err(message)` is thrown as `Error(message)`.
- `sleep(ms)`: interruptible by cancel.
- `__node(id)`: calls `Host::call(run, "trace", { nodeId })`; inserted by the compiler with `trace: true`.
  Errors from it are ignored.

Methods and arguments the prelude sends (every argument is always present, defaults filled in):

| JS | method | args | result |
|---|---|---|---|
| `press(c)` (once per combo) | `key.press` | `{ combo }` | – |
| `keyDown(c)` / `keyUp(c)` | `key.down` / `key.up` | `{ combo }` | – |
| `type(t, o)` | `key.type` | `{ text, delayMs }` | – |
| `media(a)` | `key.media` | `{ action }` | – |
| `launch(a, o)` | `app.launch` | `{ app, focusIfRunning, args }` (`args` is a string, `""` if none) | – |
| `openUrl(u)` / `openPath(p)` | `open.url` / `open.path` | `{ url }` / `{ path }` | – |
| `run(c, o)` | `run.command` | `{ command, wait }` | `{ code, stdout, stderr }` or `null` |
| `window.active()` / `isActive` | `window.active` | `{}` | `WindowInfo` or `null` |
| `window.list()` / `isRunning` | `window.list` | `{}` | `WindowInfo[]` |
| `window.focus(m, o)` | `window.focus` | `{ match, by }` | `bool` |
| `window.moveToOutput(t)` | `window.moveToOutput` | `{ target }` | – |
| `window.minimize()` … | `window.action` | `{ action }` | – |
| `desktop.switchTo(t)` | `desktop.switch` | `{ target }` (string or number ≥ 1) | – |
| `desktop.moveWindow(t, o)` | `desktop.moveWindow` | `{ target, follow }` | – |
| `desktop.create/remove/overview()` | `desktop.create` / `desktop.remove` / `desktop.overview` | `{}` | – |
| `desktop.current()` | `desktop.current` | `{}` | `{ index, name, count }` |
| `kde.shortcut(c, n)` | `kde.shortcut` | `{ component, name }` | – |
| `profile.next/prev/set(x)` | `profile.set` | `{ target }` (`"next"`, `"prev"`, id, name or 0-based index) | – |
| `profile.current()` | `profile.current` | `{}` | `{ id, name, index }` |
| `macroMode.get()` | `macroMode.get` | `{}` | `bool` |
| `macroMode.set(on)` | `macroMode.set` | `{ on }` | – |
| `macroMode.toggle()` | `macroMode.toggle` | `{}` | `bool` (new state) |
| `overlay.show/hide/toggle/peek(ms)` | `overlay.set` | `{ value }`, plus `durationMs` for `peek(ms)` | – |
| `toast(t, o)` | `toast` | `{ text, durationMs, kind }` | – |
| `notify(t, b)` | `notify` | `{ title, body }` | – |
| `vars.get(n)` | `vars.get` | `{ name }` | JSON value or `null` |
| `vars.set(n, v)` | `vars.set` | `{ name, value }` | – |
| `vars.toggle(n)` | `vars.toggle` | `{ name }` | `bool` |
| `clipboard.get()` / `clipboard.set(t)` | `clipboard.get` / `clipboard.set` | `{}` / `{ text }` | `string` / – |
| `sound.play(p)` | `sound.play` | `{ path }` | – |
| `log(…)`, `console.*` | `log` | `{ level: "info" \| "warn" \| "error", message }` | – |
| `__node(id)` | `trace` | `{ nodeId }` | – |

"–" means the result is ignored (return `null`). Host calls happen on the run's worker thread and may
block; once a run is cancelled no further host calls are made, and a call that returns after the cancel has
its result discarded.
