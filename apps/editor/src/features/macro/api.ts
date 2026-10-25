// The macro JavaScript API (docs/SPEC.md §6, crates/streampad-script/API.md) as data for autocomplete,
// hover docs and the reference panel.

export interface ApiEntry {
  /** Full path as typed, e.g. "window.focus". */
  name: string;
  signature: string;
  doc: string;
  /** Text inserted from the reference panel. */
  insert: string;
  group: string;
}

const e = (group: string, name: string, signature: string, doc: string, insert = `${name}()`): ApiEntry => ({ group, name, signature, doc, insert });

export const API: ApiEntry[] = [
  e("Keyboard", "press", "press(...combos: string[])", "Tap each key combination in order.", 'press("ctrl+shift+t")'),
  e("Keyboard", "keyDown", "keyDown(combo: string)", "Press keys and keep them held (release with keyUp or onRelease).", 'keyDown("f13")'),
  e("Keyboard", "keyUp", "keyUp(combo: string)", "Release keys held with keyDown.", 'keyUp("f13")'),
  e("Keyboard", "type", "type(text: string, opts?: { delayMs?: number })", "Type text (US layout; other characters are pasted).", 'type("gg wp")'),
  e("Media", "media", 'media(action: "playPause" | "next" | "prev" | "stop" | "volumeUp" | "volumeDown" | "mute")', "Send a media key.", 'media("playPause")'),
  e("Media", "sound.play", "sound.play(path: string)", "Play an audio file.", 'sound.play("~/Sounds/airhorn.ogg")'),
  e("Flow", "sleep", "sleep(ms: number)", "Wait. Cancelling the macro interrupts it immediately.", "sleep(100)"),
  e("Apps", "launch", "launch(app: string, opts?: { focusIfRunning?: boolean; args?: string })", "Open an app by desktop id (e.g. org.kde.dolphin) or command; focuses it if already open.", 'launch("spotify")'),
  e("Apps", "closeApp", "closeApp(app: string, opts?: { force?: boolean }): boolean", "Close every window of an app; force: true kills it. Returns whether anything was running.", 'closeApp("steam")'),
  e("Apps", "openUrl", "openUrl(url: string)", "Open a link in the default browser.", 'openUrl("https://github.com")'),
  e("Apps", "openPath", "openPath(path: string)", "Open a file or folder with its default app.", 'openPath("~/Videos")'),
  e("System", "run", "run(command: string, opts?: { wait?: boolean }): { code, stdout, stderr } | undefined", "Run a shell command. With wait: true, returns its output (10 s limit).", 'run("notify-send hi")'),
  e("Windows", "window.active", "window.active(): WindowInfo | null", "The focused window: { title, appId, resourceClass, desktopFile, pid, output }.", "window.active()"),
  e("Windows", "window.list", "window.list(): WindowInfo[]", "All open windows.", "window.list()"),
  e("Windows", "window.isActive", "window.isActive(match: string, opts?: { by?: \"app\" | \"title\" }): boolean", "Whether the focused window matches (comma-separated, case-insensitive, or /regex/).", 'window.isActive("firefox, chromium")'),
  e("Windows", "window.isRunning", "window.isRunning(match: string): boolean", "Whether any open window matches.", 'window.isRunning("obs")'),
  e("Windows", "window.focus", "window.focus(match: string, opts?: { by?: \"app\" | \"title\" }): boolean", "Bring a matching window to the front. Returns false if none.", 'window.focus("discord")'),
  e("Windows", "window.moveToOutput", 'window.moveToOutput(target: "next" | "prev" | "left" | "right" | "up" | "down" | string)', "Move the focused window to another monitor (or a connector name like DP-1).", 'window.moveToOutput("right")'),
  e("Windows", "window.minimize", "window.minimize()", "Minimize the focused window."),
  e("Windows", "window.maximize", "window.maximize()", "Maximize or restore the focused window."),
  e("Windows", "window.fullscreen", "window.fullscreen()", "Toggle fullscreen on the focused window."),
  e("Windows", "window.close", "window.close()", "Close the focused window."),
  e("Windows", "window.keepAbove", "window.keepAbove()", "Toggle keep-above on the focused window."),
  e("Windows", "window.center", "window.center()", "Center the focused window on its monitor."),
  e("Desktops", "desktop.switchTo", "desktop.switchTo(target: string | number)", "Switch virtual desktop: next/prev/left/right/up/down or a 1-based number.", 'desktop.switchTo("next")'),
  e("Desktops", "desktop.moveWindow", "desktop.moveWindow(target: string | number, opts?: { follow?: boolean })", "Send the focused window to another desktop.", 'desktop.moveWindow("next", { follow: true })'),
  e("Desktops", "desktop.create", "desktop.create()", "Add a virtual desktop."),
  e("Desktops", "desktop.remove", "desktop.remove()", "Remove the current virtual desktop."),
  e("Desktops", "desktop.overview", "desktop.overview()", "Show the overview."),
  e("Desktops", "desktop.current", "desktop.current(): { index, name, count }", "The current desktop (1-based index)."),
  e("System", "kde.shortcut", "kde.shortcut(component: string, name: string)", "Trigger a Plasma global shortcut.", 'kde.shortcut("kwin", "Overview")'),
  e("StreamPad", "profile.next", "profile.next()", "Switch to the next profile."),
  e("StreamPad", "profile.prev", "profile.prev()", "Switch to the previous profile."),
  e("StreamPad", "profile.set", "profile.set(idOrNameOrIndex: string | number)", "Switch to a profile by id, name or 0-based index.", 'profile.set("Audio")'),
  e("StreamPad", "profile.current", "profile.current(): { id, name, index }", "The active profile."),
  e("StreamPad", "macroMode.get", "macroMode.get(): boolean", "Whether macro mode is on."),
  e("StreamPad", "macroMode.set", "macroMode.set(on: boolean)", "Turn macro mode on or off.", "macroMode.set(false)"),
  e("StreamPad", "macroMode.toggle", "macroMode.toggle(): boolean", "Flip macro mode; returns the new state."),
  e("StreamPad", "overlay.show", "overlay.show()", "Show the on-screen pad."),
  e("StreamPad", "overlay.hide", "overlay.hide()", "Hide the on-screen pad."),
  e("StreamPad", "overlay.toggle", "overlay.toggle()", "Show or hide the on-screen pad."),
  e("StreamPad", "overlay.peek", "overlay.peek(ms?: number)", "Show the pad briefly.", "overlay.peek(1500)"),
  e("StreamPad", "toast", 'toast(text: string, opts?: { durationMs?: number; kind?: "info" | "success" | "error" })', "Show a small message next to the on-screen pad.", 'toast("Saved!")'),
  e("System", "notify", "notify(title: string, body?: string)", "Send a desktop notification.", 'notify("StreamPad", "Hello")'),
  e("Variables", "vars.get", "vars.get(name: string, fallback?: any): any", "Read a variable (shared by every key, kept across restarts).", 'vars.get("count", 0)'),
  e("Variables", "vars.set", "vars.set(name: string, value: any)", "Store a variable.", 'vars.set("count", 1)'),
  e("Variables", "vars.toggle", "vars.toggle(name: string): boolean", "Flip a variable between true and false; returns the new value.", 'vars.toggle("gameMode")'),
  e("System", "clipboard.get", "clipboard.get(): string", "Read the clipboard."),
  e("System", "clipboard.set", "clipboard.set(text: string)", "Write the clipboard.", 'clipboard.set("hello")'),
  e("Flow", "isTimeBetween", "isTimeBetween(from: string, to: string): boolean", "Whether the local time is between two HH:MM times (wraps midnight).", 'isTimeBetween("22:00", "06:00")'),
  e("Flow", "log", "log(...values: any[])", "Print to the editor's console.", 'log("hello")'),
  e("Flow", "event", "event: { key, type, repeat, profile: { id, name } }", "What triggered this run. type is \"press\" or \"release\".", "event.key"),
  e("Flow", "onRelease", "function onRelease(event) { … }", "Define this to run code when the key is released (e.g. push-to-talk).", "function onRelease() {\n  \n}"),
];

export const API_BY_NAME = new Map(API.map((a) => [a.name, a]));

export const SNIPPETS: { label: string; detail: string; code: string }[] = [
  {
    label: "if active app",
    detail: "Different action per focused app",
    code: 'if (window.isActive("firefox, chromium")) {\n  press("alt+left");\n} else {\n  media("prev");\n}',
  },
  { label: "push to talk", detail: "Hold a key while the pad key is held", code: 'keyDown("f13");\n\nfunction onRelease() {\n  keyUp("f13");\n}' },
  {
    label: "toggle variable",
    detail: "Flip a mode and announce it",
    code: 'const on = vars.toggle("gameMode");\ntoast(`Game mode: ${on ? "ON" : "OFF"}`);',
  },
  { label: "focus or launch", detail: "Bring an app forward, or open it", code: 'launch("spotify", { focusIfRunning: true });' },
];
