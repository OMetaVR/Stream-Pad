// Node catalog (docs/SPEC.md §3.3): everything the editor knows about each node kind — how it is named,
// grouped, configured, summarised and validated. The Rust compiler is authoritative; validation here only
// mirrors it so problems show up while editing.

import { formatCombo } from "@/lib/combo";
import type { AppEntry } from "@/lib/backend";
import type { Condition, Profile } from "@/lib/types";

export type Category =
  | "Media"
  | "Keyboard"
  | "Apps"
  | "Web & Files"
  | "Windows"
  | "Desktops"
  | "StreamPad"
  | "System"
  | "Flow"
  | "Advanced";

export const CATEGORIES: { id: Category; color: string }[] = [
  { id: "Media", color: "#f472b6" },
  { id: "Keyboard", color: "#60a5fa" },
  { id: "Apps", color: "#34d399" },
  { id: "Web & Files", color: "#22d3ee" },
  { id: "Windows", color: "#fbbf24" },
  { id: "Desktops", color: "#fb923c" },
  { id: "StreamPad", color: "#a78bfa" },
  { id: "System", color: "#94a3b8" },
  { id: "Flow", color: "#e879f9" },
  { id: "Advanced", color: "#a3a3a3" },
];

export const categoryColor = (c: Category) => CATEGORIES.find((x) => x.id === c)!.color;

export type ParamType =
  | "combo"
  | "text"
  | "longtext"
  | "code"
  | "number"
  | "boolean"
  | "select"
  | "app"
  | "kdeShortcut"
  | "output"
  | "desktop"
  | "profile"
  | "var"
  | "condition"
  | "file";

export interface ParamDef {
  key: string;
  type: ParamType;
  label: string;
  default?: unknown;
  required?: boolean;
  options?: { value: string; label: string }[];
  placeholder?: string;
  help?: string;
  min?: number;
  max?: number;
  unit?: string;
  /** Supports `{{var}}` interpolation. */
  interpolate?: boolean;
  /** Hidden from forms (set by another field's widget, e.g. `component` beside `shortcut`). */
  hidden?: boolean;
}

export interface NodeContext {
  apps: AppEntry[];
  profiles: Profile[];
}

export interface NodeDef {
  kind: string;
  title: string;
  description: string;
  category: Category;
  icon: string;
  params: ParamDef[];
  outputs: { port: string; label: string }[];
  keywords?: string;
  summary(p: Params, ctx: NodeContext): string;
  /** Suggested key label / icon when this is the key's first action. */
  defaultLabel?(p: Params, ctx: NodeContext): string;
  defaultIcon?(p: Params, ctx: NodeContext): string | null;
  /** Hidden from the action picker (triggers). */
  internal?: boolean;
}

export type Params = Record<string, unknown>;

const OUT = [{ port: "out", label: "then" }];
const str = (v: unknown) => (typeof v === "string" ? v : v == null ? "" : String(v));
const trunc = (s: string, n = 38) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);

export const MEDIA_ACTIONS = [
  { value: "playPause", label: "Play / Pause", lucide: "play" },
  { value: "next", label: "Next track", lucide: "skip-forward" },
  { value: "prev", label: "Previous track", lucide: "skip-back" },
  { value: "stop", label: "Stop", lucide: "square" },
  { value: "volumeUp", label: "Volume up", lucide: "volume-2" },
  { value: "volumeDown", label: "Volume down", lucide: "volume-1" },
  { value: "mute", label: "Mute", lucide: "volume-x" },
];

export const WINDOW_ACTIONS = [
  { value: "maximize", label: "Maximize", lucide: "maximize-2" },
  { value: "minimize", label: "Minimize", lucide: "minimize-2" },
  { value: "fullscreen", label: "Fullscreen", lucide: "fullscreen" },
  { value: "close", label: "Close", lucide: "x" },
  { value: "keepAbove", label: "Keep above others", lucide: "pin" },
  { value: "center", label: "Center", lucide: "align-center" },
];

const DIRECTIONS = [
  { value: "next", label: "Next" },
  { value: "prev", label: "Previous" },
  { value: "left", label: "Left" },
  { value: "right", label: "Right" },
  { value: "up", label: "Up" },
  { value: "down", label: "Down" },
];

export function appName(id: string, ctx: NodeContext): string {
  const app = ctx.apps.find((a) => a.id === id || a.id.toLowerCase() === id.toLowerCase());
  if (app) return app.name;
  const last = id.split(/[./ ]/).filter(Boolean).pop() ?? id;
  return last.charAt(0).toUpperCase() + last.slice(1);
}

export function appIcon(id: string, ctx: NodeContext): string | null {
  return ctx.apps.find((a) => a.id === id)?.iconPath ?? null;
}

export function targetLabel(t: unknown): string {
  const s = str(t);
  if (/^\d+$/.test(s)) return `desktop ${s}`;
  return DIRECTIONS.find((d) => d.value === s)?.label.toLowerCase() ?? s;
}

export function describeCondition(c: Condition | undefined, ctx: NodeContext): string {
  if (!c) return "if …";
  switch (c.type) {
    case "activeApp":
      return c.match ? `if ${trunc(c.match.split(",").map((s) => s.trim()).filter(Boolean).slice(0, 3).join(", "), 30)} is focused` : "if app is focused";
    case "activeTitle":
      return `if title has “${trunc(c.match, 24)}”`;
    case "appRunning":
      return `if ${trunc(c.match, 28)} is running`;
    case "var": {
      const v = c.name || "variable";
      if (c.op === "falsy") return `if ${v} is off`;
      if (c.op === "eq") return `if ${v} = ${c.value ?? ""}`;
      if (c.op === "neq") return `if ${v} ≠ ${c.value ?? ""}`;
      return `if ${v} is on`;
    }
    case "profile":
      return `if profile is ${ctx.profiles.find((p) => p.id === c.profileId)?.name ?? "…"}`;
    case "time":
      return `if between ${c.from} and ${c.to}`;
    case "expr":
      return `if ${trunc(c.code, 30)}`;
  }
}

export const BROWSERS = "firefox,chromium,chrome,opera,brave,vivaldi,edge,zen,librewolf";

const defs: NodeDef[] = [
  {
    kind: "trigger.press",
    title: "When pressed",
    description: "Runs when the key goes down.",
    category: "Flow",
    icon: "arrow-down-to-line",
    params: [],
    outputs: OUT,
    internal: true,
    summary: () => "Key goes down",
  },
  {
    kind: "trigger.release",
    title: "When released",
    description: "Runs when the key comes back up — for hold-to-talk style macros.",
    category: "Flow",
    icon: "arrow-up-from-line",
    params: [],
    outputs: OUT,
    internal: true,
    summary: () => "Key comes up",
  },
  // ---- Media
  {
    kind: "media",
    title: "Media control",
    description: "Play/pause, skip tracks or change the volume.",
    category: "Media",
    icon: "music",
    keywords: "play pause next previous volume mute sound audio spotify",
    params: [{ key: "action", type: "select", label: "Action", default: "playPause", options: MEDIA_ACTIONS.map(({ value, label }) => ({ value, label })) }],
    outputs: OUT,
    summary: (p) => MEDIA_ACTIONS.find((m) => m.value === p.action)?.label ?? "Media",
    defaultLabel: (p) => MEDIA_ACTIONS.find((m) => m.value === p.action)?.label ?? "",
    defaultIcon: (p) => `lucide:${MEDIA_ACTIONS.find((m) => m.value === p.action)?.lucide ?? "music"}`,
  },
  {
    kind: "sound.play",
    title: "Play a sound",
    description: "Play an audio file (great for soundboards).",
    category: "Media",
    icon: "audio-lines",
    keywords: "soundboard sfx wav mp3 ogg",
    params: [{ key: "path", type: "file", label: "Sound file", required: true, placeholder: "/path/to/sound.ogg" }],
    outputs: OUT,
    summary: (p) => trunc(str(p.path).split("/").pop() || "No file"),
    defaultLabel: (p) => str(p.path).split("/").pop()?.replace(/\.[^.]+$/, "") ?? "",
    defaultIcon: () => "lucide:audio-lines",
  },
  // ---- Keyboard
  {
    kind: "key.shortcut",
    title: "Press a shortcut",
    description: "Send a key combination, like Ctrl + Shift + T.",
    category: "Keyboard",
    icon: "keyboard",
    keywords: "hotkey combo keys send tap",
    params: [{ key: "combo", type: "combo", label: "Shortcut", required: true }],
    outputs: OUT,
    summary: (p) => (p.combo ? formatCombo(str(p.combo)) : "No shortcut"),
    defaultLabel: (p) => (p.combo ? formatCombo(str(p.combo)) : ""),
    defaultIcon: () => "lucide:keyboard",
  },
  {
    kind: "key.type",
    title: "Type text",
    description: "Type out text as if you typed it.",
    category: "Keyboard",
    icon: "text-cursor-input",
    keywords: "write snippet paste string",
    params: [
      { key: "text", type: "longtext", label: "Text", required: true, interpolate: true, placeholder: "gg wp!" },
      { key: "delayMs", type: "number", label: "Delay between keys", default: 8, min: 0, max: 500, unit: "ms" },
    ],
    outputs: OUT,
    summary: (p) => (p.text ? `“${trunc(str(p.text))}”` : "No text"),
    defaultLabel: (p) => trunc(str(p.text), 14),
    defaultIcon: () => "lucide:text-cursor-input",
  },
  {
    kind: "key.down",
    title: "Hold keys down",
    description: "Press keys without releasing them. Pair with “Release keys”.",
    category: "Keyboard",
    icon: "arrow-down-square",
    keywords: "hold push to talk ptt",
    params: [{ key: "combo", type: "combo", label: "Keys", required: true }],
    outputs: OUT,
    summary: (p) => (p.combo ? `Hold ${formatCombo(str(p.combo))}` : "No keys"),
  },
  {
    kind: "key.up",
    title: "Release keys",
    description: "Release keys held by “Hold keys down”.",
    category: "Keyboard",
    icon: "arrow-up-square",
    keywords: "release push to talk ptt",
    params: [{ key: "combo", type: "combo", label: "Keys", required: true }],
    outputs: OUT,
    summary: (p) => (p.combo ? `Release ${formatCombo(str(p.combo))}` : "No keys"),
  },
  // ---- Apps
  {
    kind: "app.launch",
    title: "Open an app",
    description: "Launch an application, or bring it to the front if it's already open.",
    category: "Apps",
    icon: "app-window",
    keywords: "launch start run program focus activate",
    params: [
      { key: "app", type: "app", label: "App", required: true },
      { key: "focusIfRunning", type: "boolean", label: "Focus it if already open", default: true },
      { key: "args", type: "text", label: "Arguments", placeholder: "optional", hidden: false },
    ],
    outputs: OUT,
    summary: (p, ctx) => (p.app ? appName(str(p.app), ctx) : "No app"),
    defaultLabel: (p, ctx) => (p.app ? appName(str(p.app), ctx) : ""),
    defaultIcon: (p, ctx) => (p.app ? appIcon(str(p.app), ctx) ?? "lucide:app-window" : "lucide:app-window"),
  },
  {
    kind: "app.close",
    title: "Close an app",
    description: "Close all of an app's windows — or force-quit it when it hangs.",
    category: "Apps",
    icon: "circle-x",
    keywords: "quit exit kill stop end terminate force",
    params: [
      { key: "app", type: "app", label: "App", required: true },
      { key: "force", type: "boolean", label: "Force quit", default: false, help: "Kill it instead of asking it to close (unsaved work is lost)." },
    ],
    outputs: OUT,
    summary: (p, ctx) => (p.app ? `${p.force ? "Force quit" : "Close"} ${appName(str(p.app), ctx)}` : "No app"),
    defaultLabel: (p, ctx) => (p.app ? `Close ${appName(str(p.app), ctx)}` : ""),
    defaultIcon: () => "lucide:circle-x",
  },
  {
    kind: "window.focus",
    title: "Focus a window",
    description: "Bring an open window to the front.",
    category: "Apps",
    icon: "scan-eye",
    keywords: "activate switch to raise",
    params: [
      { key: "match", type: "text", label: "Window", required: true, placeholder: "firefox", help: "App id or part of the title" },
      {
        key: "by",
        type: "select",
        label: "Match by",
        default: "app",
        options: [
          { value: "app", label: "App" },
          { value: "title", label: "Title" },
        ],
      },
    ],
    outputs: OUT,
    summary: (p) => (p.match ? `Focus ${trunc(str(p.match))}` : "No window"),
  },
  // ---- Web & files
  {
    kind: "open.url",
    title: "Open a website",
    description: "Open a link in your default browser.",
    category: "Web & Files",
    icon: "globe",
    keywords: "url link browser web site",
    params: [{ key: "url", type: "text", label: "URL", required: true, interpolate: true, placeholder: "https://youtube.com" }],
    outputs: OUT,
    summary: (p) => trunc(str(p.url).replace(/^https?:\/\/(www\.)?/, "") || "No URL"),
    defaultLabel: (p) => {
      const host = str(p.url).replace(/^https?:\/\/(www\.)?/, "").split(/[/?#]/)[0] ?? "";
      const name = host.split(".").slice(-2, -1)[0] ?? host;
      return name ? name.charAt(0).toUpperCase() + name.slice(1) : "";
    },
    defaultIcon: () => "lucide:globe",
  },
  {
    kind: "open.path",
    title: "Open a file or folder",
    description: "Open a file or folder with its default app.",
    category: "Web & Files",
    icon: "folder-open",
    keywords: "file folder directory document",
    params: [{ key: "path", type: "file", label: "Path", required: true, interpolate: true, placeholder: "~/Videos" }],
    outputs: OUT,
    summary: (p) => trunc(str(p.path) || "No path"),
    defaultLabel: (p) => str(p.path).split("/").filter(Boolean).pop() ?? "",
    defaultIcon: () => "lucide:folder-open",
  },
  // ---- Windows
  {
    kind: "window.moveToOutput",
    title: "Move window to monitor",
    description: "Move the focused window to another monitor.",
    category: "Windows",
    icon: "monitor-up",
    keywords: "screen display move hotswap",
    params: [{ key: "target", type: "output", label: "Monitor", default: "next" }],
    outputs: OUT,
    summary: (p) => `To ${targetLabel(p.target)} monitor`,
    defaultLabel: (p) => `To ${targetLabel(p.target)}`,
    defaultIcon: () => "lucide:monitor-up",
  },
  {
    kind: "window.action",
    title: "Window action",
    description: "Maximize, minimize, close or pin the focused window.",
    category: "Windows",
    icon: "app-window",
    keywords: "maximize minimize close fullscreen pin above center",
    params: [{ key: "action", type: "select", label: "Action", default: "maximize", options: WINDOW_ACTIONS.map(({ value, label }) => ({ value, label })) }],
    outputs: OUT,
    summary: (p) => WINDOW_ACTIONS.find((w) => w.value === p.action)?.label ?? "Window",
    defaultLabel: (p) => WINDOW_ACTIONS.find((w) => w.value === p.action)?.label ?? "",
    defaultIcon: (p) => `lucide:${WINDOW_ACTIONS.find((w) => w.value === p.action)?.lucide ?? "app-window"}`,
  },
  // ---- Desktops
  {
    kind: "desktop.switch",
    title: "Switch desktop",
    description: "Go to another virtual desktop.",
    category: "Desktops",
    icon: "layers",
    keywords: "virtual workspace",
    params: [{ key: "target", type: "desktop", label: "Desktop", default: "next" }],
    outputs: OUT,
    summary: (p) => `Go to ${targetLabel(p.target)}`,
    defaultLabel: (p) => `Desktop ${targetLabel(p.target)}`,
    defaultIcon: () => "lucide:layers",
  },
  {
    kind: "desktop.moveWindow",
    title: "Move window to desktop",
    description: "Send the focused window to another virtual desktop.",
    category: "Desktops",
    icon: "square-arrow-out-up-right",
    keywords: "virtual workspace send",
    params: [
      { key: "target", type: "desktop", label: "Desktop", default: "next" },
      { key: "follow", type: "boolean", label: "Follow the window", default: false },
    ],
    outputs: OUT,
    summary: (p) => `Window to ${targetLabel(p.target)}${p.follow ? " (follow)" : ""}`,
    defaultLabel: (p) => `Send ${targetLabel(p.target)}`,
    defaultIcon: () => "lucide:square-arrow-out-up-right",
  },
  {
    kind: "desktop.create",
    title: "New desktop",
    description: "Add a virtual desktop.",
    category: "Desktops",
    icon: "square-plus",
    params: [],
    outputs: OUT,
    summary: () => "Add a desktop",
    defaultLabel: () => "New desktop",
    defaultIcon: () => "lucide:square-plus",
  },
  {
    kind: "desktop.remove",
    title: "Remove desktop",
    description: "Remove the current virtual desktop.",
    category: "Desktops",
    icon: "square-minus",
    params: [],
    outputs: OUT,
    summary: () => "Remove current desktop",
    defaultLabel: () => "Remove desktop",
    defaultIcon: () => "lucide:square-minus",
  },
  {
    kind: "desktop.overview",
    title: "Overview",
    description: "Show all windows and desktops.",
    category: "Desktops",
    icon: "layout-grid",
    keywords: "expose mission control task view",
    params: [],
    outputs: OUT,
    summary: () => "Show overview",
    defaultLabel: () => "Overview",
    defaultIcon: () => "lucide:layout-grid",
  },
  // ---- StreamPad
  {
    kind: "profile.switch",
    title: "Switch profile",
    description: "Change the active StreamPad profile.",
    category: "StreamPad",
    icon: "layers-2",
    params: [{ key: "target", type: "profile", label: "Profile", default: "next" }],
    outputs: OUT,
    summary: (p, ctx) =>
      p.target === "next" ? "Next profile" : p.target === "prev" ? "Previous profile" : `To ${ctx.profiles.find((x) => x.id === p.target)?.name ?? "…"}`,
    defaultLabel: (p, ctx) => (p.target === "next" || p.target === "prev" ? "" : ctx.profiles.find((x) => x.id === p.target)?.name ?? ""),
    defaultIcon: () => "lucide:layers-2",
  },
  {
    kind: "overlay.set",
    title: "Show or hide the pad",
    description: "Control the on-screen numpad.",
    category: "StreamPad",
    icon: "eye",
    params: [
      {
        key: "value",
        type: "select",
        label: "Action",
        default: "toggle",
        options: [
          { value: "toggle", label: "Toggle" },
          { value: "show", label: "Show" },
          { value: "hide", label: "Hide" },
          { value: "peek", label: "Peek briefly" },
        ],
      },
    ],
    outputs: OUT,
    summary: (p) => `${str(p.value || "toggle").replace(/^./, (c) => c.toUpperCase())} the pad`,
  },
  {
    kind: "macroMode.set",
    title: "Macro mode",
    description: "Turn macro mode on or off (like Scroll Lock).",
    category: "StreamPad",
    icon: "toggle-right",
    params: [
      {
        key: "value",
        type: "select",
        label: "Mode",
        default: "toggle",
        options: [
          { value: "toggle", label: "Toggle" },
          { value: "on", label: "On" },
          { value: "off", label: "Off" },
        ],
      },
    ],
    outputs: OUT,
    summary: (p) => `Macro mode ${str(p.value || "toggle")}`,
  },
  {
    kind: "toast",
    title: "Show a message",
    description: "Pop a small message next to the on-screen pad.",
    category: "StreamPad",
    icon: "message-square",
    keywords: "toast tooltip notice popup",
    params: [
      { key: "text", type: "text", label: "Message", required: true, interpolate: true, placeholder: "Game mode: {{gameMode}}" },
      { key: "durationMs", type: "number", label: "Duration", default: 1600, min: 300, max: 10000, unit: "ms" },
    ],
    outputs: OUT,
    summary: (p) => (p.text ? `“${trunc(str(p.text))}”` : "No message"),
  },
  {
    kind: "var.set",
    title: "Set a variable",
    description: "Remember a value. Variables survive restarts.",
    category: "StreamPad",
    icon: "variable",
    keywords: "state store remember",
    params: [
      { key: "name", type: "var", label: "Variable", required: true },
      { key: "value", type: "text", label: "Value", placeholder: "true, 3, or text" },
    ],
    outputs: OUT,
    summary: (p) => `${str(p.name) || "variable"} = ${str(p.value) || "…"}`,
  },
  {
    kind: "var.toggle",
    title: "Toggle a variable",
    description: "Flip a variable between on and off (like the old game mode).",
    category: "StreamPad",
    icon: "toggle-left",
    keywords: "switch flag mode",
    params: [{ key: "name", type: "var", label: "Variable", required: true }],
    outputs: OUT,
    summary: (p) => `Toggle ${str(p.name) || "variable"}`,
    defaultLabel: (p) => str(p.name),
    defaultIcon: () => "lucide:toggle-left",
  },
  // ---- System
  {
    kind: "run.command",
    title: "Run a command",
    description: "Run a shell command.",
    category: "System",
    icon: "square-terminal",
    keywords: "shell bash terminal script exec",
    params: [
      { key: "command", type: "longtext", label: "Command", required: true, interpolate: true, placeholder: "notify-send hello" },
      { key: "wait", type: "boolean", label: "Wait for it to finish", default: false },
    ],
    outputs: OUT,
    summary: (p) => (p.command ? trunc(str(p.command)) : "No command"),
    defaultIcon: () => "lucide:square-terminal",
  },
  {
    kind: "kde.shortcut",
    title: "KDE shortcut",
    description: "Trigger any Plasma global shortcut, e.g. Overview or Window to Next Screen.",
    category: "System",
    icon: "command",
    keywords: "plasma kwin global shortcut kglobalaccel",
    params: [
      { key: "shortcut", type: "kdeShortcut", label: "Shortcut", required: true },
      { key: "component", type: "text", label: "Component", default: "kwin", hidden: true },
    ],
    outputs: OUT,
    summary: (p) => (p.shortcut ? trunc(str(p.shortcut)) : "No shortcut"),
    defaultLabel: (p) => trunc(str(p.shortcut), 16),
    defaultIcon: () => "lucide:command",
  },
  {
    kind: "notify",
    title: "Desktop notification",
    description: "Send a system notification.",
    category: "System",
    icon: "bell",
    params: [
      { key: "title", type: "text", label: "Title", required: true, interpolate: true },
      { key: "body", type: "text", label: "Body", interpolate: true },
    ],
    outputs: OUT,
    summary: (p) => trunc(str(p.title) || "No title"),
  },
  {
    kind: "clipboard.set",
    title: "Copy to clipboard",
    description: "Put text on the clipboard.",
    category: "System",
    icon: "clipboard-copy",
    params: [{ key: "text", type: "longtext", label: "Text", required: true, interpolate: true }],
    outputs: OUT,
    summary: (p) => (p.text ? `“${trunc(str(p.text))}”` : "No text"),
  },
  // ---- Flow
  {
    kind: "delay",
    title: "Wait",
    description: "Pause before the next step.",
    category: "Flow",
    icon: "timer",
    keywords: "delay sleep pause",
    params: [{ key: "ms", type: "number", label: "Duration", default: 100, min: 0, max: 600000, unit: "ms" }],
    outputs: OUT,
    summary: (p) => `${Number(p.ms ?? 100)} ms`,
  },
  {
    kind: "flow.if",
    title: "If / else",
    description: "Do different things depending on a condition (e.g. which app is focused).",
    category: "Flow",
    icon: "split",
    keywords: "condition branch when check",
    params: [{ key: "condition", type: "condition", label: "Condition", default: { type: "activeApp", match: "" } }],
    outputs: [
      { port: "true", label: "yes" },
      { port: "false", label: "no" },
    ],
    summary: (p, ctx) => describeCondition(p.condition as Condition | undefined, ctx),
  },
  {
    kind: "flow.repeat",
    title: "Repeat",
    description: "Run a few steps several times.",
    category: "Flow",
    icon: "repeat",
    keywords: "loop times",
    params: [{ key: "count", type: "number", label: "Times", default: 2, min: 0, max: 1000 }],
    outputs: [
      { port: "body", label: "each time" },
      { port: "done", label: "after" },
    ],
    summary: (p) => `${Number(p.count ?? 2)} times`,
  },
  {
    kind: "flow.stop",
    title: "Stop",
    description: "End the macro here.",
    category: "Flow",
    icon: "octagon-x",
    params: [],
    outputs: [],
    summary: () => "End the macro",
  },
  // ---- Advanced
  {
    kind: "script",
    title: "JavaScript",
    description: "Run a few lines of code with the full StreamPad API.",
    category: "Advanced",
    icon: "code",
    keywords: "js code script custom",
    params: [{ key: "code", type: "code", label: "Code", placeholder: 'press("ctrl+c")' }],
    outputs: OUT,
    summary: (p) => trunc(str(p.code).split("\n").find((l) => l.trim()) ?? "Empty"),
  },
];

export const NODE_DEFS: Record<string, NodeDef> = Object.fromEntries(defs.map((d) => [d.kind, d]));

/** Picker-visible kinds in catalog order. */
export const ACTION_DEFS = defs.filter((d) => !d.internal);

const UNKNOWN: NodeDef = {
  kind: "?",
  title: "Unknown step",
  description: "This step isn't known to this version of StreamPad.",
  category: "Advanced",
  icon: "circle-help",
  params: [],
  outputs: OUT,
  summary: () => "",
};

export function nodeDef(kind: string): NodeDef {
  return NODE_DEFS[kind] ?? { ...UNKNOWN, kind, title: kind };
}

export function defaultParams(kind: string): Params {
  const p: Params = {};
  for (const d of nodeDef(kind).params) if (d.default !== undefined) p[d.key] = structuredClone(d.default);
  return p;
}

/** First problem with a node's params, mirroring the Rust compiler's messages. */
export function validateNode(kind: string, p: Params): string | null {
  const def = NODE_DEFS[kind];
  if (!def) return `Unknown step “${kind}”`;
  for (const d of def.params) {
    const v = p[d.key];
    if (d.required && (v == null || str(v).trim() === "")) {
      if (d.type === "combo") return "Record a shortcut";
      if (d.type === "app") return "Pick an app";
      if (d.key === "url") return "Enter a URL";
      return `${d.label} is required`;
    }
    if (d.type === "number" && v != null && v !== "" && !Number.isFinite(Number(v))) return `${d.label} must be a number`;
  }
  if (kind === "flow.if") {
    const c = p.condition as Condition | undefined;
    if (!c) return "Choose a condition";
    if ((c.type === "activeApp" || c.type === "activeTitle" || c.type === "appRunning") && !c.match.trim()) return "Enter what to match";
    if (c.type === "var" && !c.name.trim()) return "Choose a variable";
    if (c.type === "expr" && !c.code.trim()) return "Enter an expression";
    if (c.type === "profile" && !c.profileId) return "Choose a profile";
  }
  return null;
}
