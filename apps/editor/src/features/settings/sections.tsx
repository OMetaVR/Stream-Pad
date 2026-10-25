// Settings › General, Globals, Devices, Scripting, About.

import { FolderOpen, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { getBackend } from "@/lib/backend";
import { GLOBAL_ACTIONS, LIMITS } from "@/lib/defaults";
import { GLOBAL_ACTION_INFO } from "@/lib/model";
import { useTheme, type ThemePreference } from "@/lib/theme";
import { KEY_IDS, KEY_INFO, type DeviceInfo, type GlobalAction, type KeyId } from "@/lib/types";
import { useDaemon } from "@/state/daemon";
import { editor, useEditor } from "@/state/store";
import { Badge, Button, Card, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger, EmptyState, Field, IconButton, NumberInput, Section, Segmented, Select, Switch, toast } from "@/ui";
import { CodeEditor } from "../macro/CodeEditor";

const TOGGLE_KEYS = ["ScrollLock", "Pause", "Insert", "PrintScreen", "Menu", ...Array.from({ length: 12 }, (_, i) => `F${i + 13}`)];

export function GeneralSection() {
  const s = useEditor((st) => st.config!.settings);
  const theme = useTheme();
  const [autostart, setAutostart] = useState<boolean | null>(null);
  useEffect(() => void getBackend().getAutostart().then(setAutostart, () => setAutostart(false)), []);

  return (
    <>
      <Section title="Macro mode">
        <Card divided>
          <Field label="Toggle key" hint="Flips the numpad between numbers and macros. Its light shows macro mode." inline>
            <Select
              className="w-44"
              value={TOGGLE_KEYS.includes(s.toggleKey) ? s.toggleKey : "ScrollLock"}
              onValueChange={(v) => editor.updateSettings((d) => void (d.toggleKey = v))}
              options={TOGGLE_KEYS.map((k) => ({ value: k, label: k.replace(/([a-z])([A-Z])/g, "$1 $2") }))}
            />
          </Field>
          <Field label="Start in macro mode" inline>
            <Switch checked={s.startInMacroMode} onCheckedChange={(v) => editor.updateSettings((d) => void (d.startInMacroMode = v))} />
          </Field>
          <Field label="Remember mode and profile" hint="Restore them after a restart." inline>
            <Switch checked={s.rememberState} onCheckedChange={(v) => editor.updateSettings((d) => void (d.rememberState = v))} />
          </Field>
          <Field label="Keys without an action" inline as="div">
            <Segmented
              value={s.unboundKeys}
              onValueChange={(v) => editor.updateSettings((d) => void (d.unboundKeys = v))}
              options={[
                { value: "block", label: "Do nothing" },
                { value: "passthrough", label: "Type normally" },
              ]}
            />
          </Field>
          <Field label="Hold time" hint="How long to hold a key for its hold action (e.g. hold Enter → editor)." inline>
            <NumberInput value={s.holdMs} min={LIMITS.holdMs.min} max={LIMITS.holdMs.max} step={50} suffix="ms" onChange={(v) => editor.updateSettings((d) => void (d.holdMs = v))} />
          </Field>
          <Field label="Show macro errors on screen" inline>
            <Switch checked={s.toastOnError} onCheckedChange={(v) => editor.updateSettings((d) => void (d.toastOnError = v))} />
          </Field>
        </Card>
      </Section>
      <Section title="System">
        <Card divided>
          <Field label="Start StreamPad at login" hint="Runs the background service when you log in." inline>
            <Switch
              checked={!!autostart}
              disabled={autostart === null}
              onCheckedChange={async (v) => {
                setAutostart(v);
                try {
                  await getBackend().setAutostart(v);
                } catch (e) {
                  setAutostart(!v);
                  toast.error("Couldn't change start at login", { description: String(e) });
                }
              }}
            />
          </Field>
          <Field label="Editor theme" inline as="div">
            <Segmented<ThemePreference>
              value={theme.preference}
              onValueChange={theme.setPreference}
              options={[
                { value: "system", label: "System" },
                { value: "dark", label: "Dark" },
                { value: "light", label: "Light" },
              ]}
            />
          </Field>
        </Card>
      </Section>
    </>
  );
}

const actionOptions = (withNothing: boolean) => [
  ...(withNothing ? [{ value: "__none", label: "Nothing" }] : []),
  ...GLOBAL_ACTIONS.filter((a) => a !== "none").map((a) => ({ value: a, label: GLOBAL_ACTION_INFO[a].description })),
];

export function GlobalsSection() {
  const globals = useEditor((s) => s.config!.settings.globals);
  const profiles = useEditor((s) => s.config!.profiles);
  const keys = KEY_IDS.filter((k) => globals[k] && globals[k]!.tap !== "none");
  const free = KEY_IDS.filter((k) => !keys.includes(k));
  const shadowed = (k: KeyId) => profiles.filter((p) => p.keys[k]).length;

  return (
    <Section
      title="Assigned keys"
      description="These keys do the same thing in every profile while macro mode is on. A hold action fires when you hold the key."
      actions={
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="sm" icon={Plus} disabled={!free.length}>
              Add global key
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="max-h-80 overflow-y-auto">
            {free.map((k) => (
              <DropdownMenuItem key={k} onSelect={() => editor.updateSettings((d) => void (d.globals[k] = { tap: "toggleOverlay", hold: null }), { coalesce: false })}>
                {KEY_INFO[k].name}
              </DropdownMenuItem>
            ))}
          </DropdownMenuContent>
        </DropdownMenu>
      }
    >
      <Card divided>
        {keys.length === 0 && <p className="px-4 py-6 text-center text-sm text-faint">No global keys. Every key belongs to the profiles.</p>}
        {keys.map((k) => {
          const g = globals[k]!;
          const n = shadowed(k);
          return (
            <div key={k} className="flex items-center gap-3 px-4 py-3">
              <div className="w-28 shrink-0">
                <div className="text-base font-medium">{KEY_INFO[k].name}</div>
                {n > 0 && <div className="text-xs text-warning">Hides {n} profile key{n > 1 ? "s" : ""}</div>}
              </div>
              <Field label="Tap" className="flex-1">
                <Select value={g.tap} onValueChange={(v) => editor.updateSettings((d) => void (d.globals[k]!.tap = v as GlobalAction), { coalesce: false })} options={actionOptions(false)} />
              </Field>
              <Field label="Hold" className="flex-1">
                <Select
                  value={g.hold ?? "__none"}
                  onValueChange={(v) => editor.updateSettings((d) => void (d.globals[k]!.hold = v === "__none" ? null : (v as GlobalAction)), { coalesce: false })}
                  options={actionOptions(true)}
                />
              </Field>
              <IconButton icon={Trash2} label="Make this a normal key" className="mt-5" onClick={() => editor.updateSettings((d) => void delete d.globals[k], { coalesce: false })} />
            </div>
          );
        })}
      </Card>
    </Section>
  );
}

export function DevicesSection() {
  const devSettings = useEditor((s) => s.config!.settings.devices);
  const live = useDaemon((s) => s.state?.devices);
  const [listed, setListed] = useState<DeviceInfo[] | null>(null);
  const [loading, setLoading] = useState(false);
  const refresh = async () => {
    setLoading(true);
    try {
      setListed(await getBackend().listDevices());
    } catch {
      setListed([]);
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => void refresh(), []);
  const devices = live ?? listed ?? [];
  const auto = devSettings.mode === "auto";
  const enabled = (id: string) => (auto ? !devSettings.exclude.includes(id) : devSettings.include.includes(id));
  const toggle = (id: string, on: boolean) =>
    editor.updateSettings((d) => {
      const list = auto ? d.devices.exclude : d.devices.include;
      const want = auto ? !on : on;
      const i = list.indexOf(id);
      if (want && i < 0) list.push(id);
      if (!want && i >= 0) list.splice(i, 1);
    }, { coalesce: false });

  return (
    <Section
      title="Detected keyboards"
      description="StreamPad listens to these keyboards. Everything you type passes through unchanged unless macro mode is on."
      actions={
        <Button size="sm" variant="ghost" icon={RefreshCw} loading={loading} onClick={() => void refresh()}>
          Refresh
        </Button>
      }
    >
      <div className="flex flex-col gap-3">
        <Segmented
          value={devSettings.mode}
          onValueChange={(v) => editor.updateSettings((d) => void (d.devices.mode = v), { coalesce: false })}
          options={[
            { value: "auto", label: "Every keyboard with a numpad" },
            { value: "manual", label: "Only the ones I pick" },
          ]}
        />
        <Card divided>
          {devices.length === 0 && <EmptyState size="sm" title="No keyboards found" description="Start the background service to see your keyboards." />}
          {devices.map((d) => (
            <div key={`${d.id}:${d.path}`} className="flex items-center gap-3 px-4 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-base font-medium">{d.name}</span>
                  {d.hasNumpad && <Badge>Numpad</Badge>}
                  {d.grabbed ? <Badge variant="success" dot>Active</Badge> : d.error ? <Badge variant="danger">Error</Badge> : null}
                </div>
                <div className="truncate font-code text-xs text-faint">
                  {d.path} · {d.id.split(":").slice(0, 2).join(":")}
                </div>
                {d.error && <div className="text-sm text-danger">{d.error}</div>}
              </div>
              <Switch checked={d.eligible && enabled(d.id)} disabled={!d.eligible} onCheckedChange={(v) => toggle(d.id, v)} aria-label={`Use ${d.name}`} />
            </div>
          ))}
        </Card>
      </div>
    </Section>
  );
}

export function ScriptingSection() {
  const library = useEditor((s) => s.config!.library);
  return (
    <Section title="Shared library" description="Functions and constants defined here are available in every macro and script.">
      <div className="overflow-hidden rounded-lg border border-line bg-panel">
        <CodeEditor
          value={library}
          lineNumbers
          minHeight={360}
          maxHeight={560}
          placeholder={'// e.g.\nfunction browserBack() {\n  press("alt+left");\n}'}
          onChange={(v) => editor.updateConfig((c) => void (c.library = v), { coalesce: "library" })}
        />
      </div>
    </Section>
  );
}

export function AboutSection() {
  const [path, setPath] = useState("");
  const version = useDaemon((s) => s.state?.version);
  useEffect(() => void getBackend().configPath().then(setPath, () => {}), []);
  const dir = path.replace(/\/[^/]*$/, "");
  return (
    <Section title="StreamPad">
      <Card divided>
        <Field label="Editor version" inline>
          <span className="font-code text-sm text-muted">{getBackend().appVersion()}</span>
        </Field>
        <Field label="Background service" inline>
          <span className="font-code text-sm text-muted">{version ?? "not running"}</span>
        </Field>
        <Field label="Config file" hint={path} inline>
          <Button size="sm" icon={FolderOpen} disabled={!dir} onClick={() => void getBackend().openPath(dir)}>
            Open folder
          </Button>
        </Field>
      </Card>
    </Section>
  );
}
