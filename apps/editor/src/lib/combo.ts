// Display helpers for combo strings (docs/SPEC.md §3.4: "ctrl+shift+t" → "Ctrl + Shift + T") and the
// editor's own hotkey strings ("mod+shift+z"). Display only — the daemon parses combos authoritatively.

const NAMES: Record<string, string> = {
  mod: "Ctrl", ctrl: "Ctrl", control: "Ctrl", lctrl: "Ctrl", rctrl: "Ctrl",
  shift: "Shift", lshift: "Shift", rshift: "Shift",
  alt: "Alt", lalt: "Alt", option: "Alt", ralt: "AltGr", altgr: "AltGr",
  meta: "Meta", super: "Meta", win: "Meta", windows: "Meta", cmd: "Meta", lmeta: "Meta", rmeta: "Meta", lwin: "Meta", rwin: "Meta",
  left: "←", right: "→", up: "↑", down: "↓",
  arrowleft: "←", arrowright: "→", arrowup: "↑", arrowdown: "↓",
  esc: "Esc", escape: "Esc", enter: "Enter", return: "Enter", ret: "Enter",
  space: "Space", " ": "Space", tab: "Tab", backspace: "Backspace",
  delete: "Delete", del: "Delete", insert: "Insert", ins: "Insert",
  home: "Home", end: "End", pageup: "PgUp", pgup: "PgUp", pagedown: "PgDn", pgdn: "PgDn", pgdown: "PgDn",
  print: "Print", printscreen: "Print", prtsc: "Print", sysrq: "SysRq",
  scrolllock: "Scroll Lock", pause: "Pause", capslock: "Caps Lock", caps: "Caps Lock", numlock: "Num Lock", menu: "Menu",
  playpause: "Play/Pause", play: "Play/Pause", nextsong: "Next", next: "Next", nexttrack: "Next",
  previoussong: "Previous", prev: "Previous", previous: "Previous", prevtrack: "Previous",
  stopcd: "Stop", stop: "Stop", volumeup: "Vol+", volup: "Vol+", "vol+": "Vol+",
  volumedown: "Vol−", voldown: "Vol−", "vol-": "Vol−", mute: "Mute",
  minus: "-", equal: "=", leftbrace: "[", rightbrace: "]", semicolon: ";", apostrophe: "'",
  grave: "`", backslash: "\\", comma: ",", dot: ".", slash: "/", plus: "+",
  kpenter: "Num Enter", kpplus: "Num +", kpminus: "Num −", kpasterisk: "Num *", kpslash: "Num /", kpdot: "Num .",
};

/** One key name → its display label ("pgup" → "PgUp", "kp4" → "Num 4", "f13" → "F13", "t" → "T"). */
export function keyLabel(name: string): string {
  const n = name.trim().toLowerCase().replace(/^key_/, "");
  if (NAMES[n]) return NAMES[n];
  const kp = /^(?:kp|numpad)(\d)$/.exec(n);
  if (kp) return `Num ${kp[1]}`;
  if (/^f\d{1,2}$/.test(n)) return n.toUpperCase();
  if (n.length === 1) return n.toUpperCase();
  return n.charAt(0).toUpperCase() + n.slice(1);
}

/** Split a combo into its keys, honouring a trailing "++" (= the plus key). */
export function comboParts(combo: string): string[] {
  const s = combo.trim();
  if (!s) return [];
  if (s === "+") return ["kpplus"];
  const plus = s.endsWith("++");
  const parts = (plus ? s.slice(0, -2) : s).split("+").map((p) => p.trim()).filter(Boolean);
  if (plus) parts.push("kpplus");
  return parts;
}

/** Display labels for each key of a combo, e.g. `["Ctrl", "Shift", "T"]`. */
export function comboKeys(combo: string): string[] {
  return comboParts(combo).map(keyLabel);
}

/** `"ctrl+shift+t"` → `"Ctrl + Shift + T"`. */
export function formatCombo(combo: string): string {
  return comboKeys(combo).join(" + ");
}
