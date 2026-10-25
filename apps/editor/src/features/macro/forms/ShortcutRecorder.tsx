// Records a key combination by pressing it ("ctrl+shift+t", docs/SPEC.md §3.4), with a text fallback for
// combos the browser can't see (e.g. Meta shortcuts the desktop grabs first).

import { Keyboard, Pencil } from "lucide-react";
import { useRef, useState } from "react";
import { comboKeys } from "@/lib/combo";
import { cn } from "@/lib/cn";
import { IconButton, Input, Kbd } from "@/ui";

const CODE_NAMES: Record<string, string> = {
  Escape: "esc", Enter: "enter", Space: "space", Tab: "tab", Backspace: "backspace", Delete: "delete",
  Insert: "insert", Home: "home", End: "end", PageUp: "pageup", PageDown: "pagedown",
  ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down",
  Minus: "minus", Equal: "equal", BracketLeft: "leftbrace", BracketRight: "rightbrace", Semicolon: "semicolon",
  Quote: "apostrophe", Backquote: "grave", Backslash: "backslash", Comma: "comma", Period: "dot", Slash: "slash",
  NumpadEnter: "kpenter", NumpadAdd: "kpplus", NumpadSubtract: "kpminus", NumpadMultiply: "kpasterisk",
  NumpadDivide: "kpslash", NumpadDecimal: "kpdot", PrintScreen: "sysrq", ScrollLock: "scrolllock", Pause: "pause",
  CapsLock: "capslock", NumLock: "numlock", ContextMenu: "menu", MediaPlayPause: "playpause",
  MediaTrackNext: "nextsong", MediaTrackPrevious: "previoussong", MediaStop: "stopcd",
  AudioVolumeUp: "volumeup", AudioVolumeDown: "volumedown", AudioVolumeMute: "mute",
};

const MODIFIER_CODES = new Set(["ControlLeft", "ControlRight", "ShiftLeft", "ShiftRight", "AltLeft", "AltRight", "MetaLeft", "MetaRight", "OSLeft", "OSRight"]);

/** KeyboardEvent.code → evdev-style key name, or null for keys we can't name. */
export function codeToKey(code: string): string | null {
  if (CODE_NAMES[code]) return CODE_NAMES[code];
  let m = /^Key([A-Z])$/.exec(code);
  if (m) return m[1]!.toLowerCase();
  m = /^Digit(\d)$/.exec(code);
  if (m) return m[1]!;
  m = /^Numpad(\d)$/.exec(code);
  if (m) return `kp${m[1]}`;
  m = /^F(\d{1,2})$/.exec(code);
  if (m) return `f${m[1]}`;
  return null;
}

function modifiers(e: React.KeyboardEvent): string[] {
  const out: string[] = [];
  if (e.ctrlKey) out.push("ctrl");
  if (e.shiftKey) out.push("shift");
  if (e.altKey) out.push("alt");
  if (e.metaKey) out.push("meta");
  return out;
}

export function ShortcutRecorder({ value, onChange, autoFocus }: { value: string; onChange(v: string): void; autoFocus?: boolean }) {
  const [recording, setRecording] = useState(false);
  const [held, setHeld] = useState<string[]>([]);
  const [asText, setAsText] = useState(false);
  const peak = useRef<string[]>([]);

  if (asText) {
    return (
      <div className="flex gap-1.5">
        <Input
          mono
          autoFocus
          wrapperClassName="flex-1"
          value={value}
          placeholder="ctrl+shift+t"
          onChange={(e) => onChange(e.currentTarget.value.toLowerCase().replace(/\s+/g, ""))}
        />
        <IconButton icon={Keyboard} label="Record by pressing keys" onClick={() => setAsText(false)} />
      </div>
    );
  }

  const shown = recording ? held : comboKeys(value);
  return (
    <div className="flex gap-1.5">
      <button
        type="button"
        autoFocus={autoFocus}
        onFocus={() => {
          setRecording(true);
          setHeld([]);
          peak.current = [];
        }}
        onBlur={() => setRecording(false)}
        onKeyDown={(e) => {
          if (e.key === "Tab" && !e.ctrlKey && !e.altKey && !e.metaKey) return;
          e.preventDefault();
          e.stopPropagation();
          const mods = modifiers(e);
          if (MODIFIER_CODES.has(e.code)) {
            setHeld(comboKeys(mods.join("+")));
            if (mods.length >= peak.current.length) peak.current = mods;
            return;
          }
          const key = codeToKey(e.code);
          if (!key) return;
          onChange([...mods, key].join("+"));
          (e.currentTarget as HTMLButtonElement).blur();
        }}
        onKeyUp={(e) => {
          e.preventDefault();
          if (modifiers(e).length === 0 && peak.current.length > 0) {
            // Only modifiers were pressed: record them (useful for "Hold keys down: shift").
            onChange(peak.current.join("+"));
            (e.currentTarget as HTMLButtonElement).blur();
          }
        }}
        className={cn(
          "flex h-8 min-w-0 flex-1 items-center gap-1 rounded-sm border bg-raised px-2 text-left transition-quick",
          recording ? "border-accent-line ring-2 ring-accent/25" : "border-line hover:border-line-strong",
        )}
      >
        {shown.length > 0 ? (
          shown.map((k, i) => <Kbd key={`${k}${i}`}>{k}</Kbd>)
        ) : (
          <span className="text-faint">{recording ? "Press keys…" : "Click, then press a shortcut"}</span>
        )}
        {recording && shown.length > 0 && <span className="ml-1 text-xs text-faint">…</span>}
      </button>
      <IconButton icon={Pencil} label="Type it instead" onClick={() => setAsText(true)} />
    </div>
  );
}
