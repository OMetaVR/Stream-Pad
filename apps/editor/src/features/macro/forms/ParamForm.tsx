// Form for one node's params, driven by the catalog's ParamDefs. Shared by the step list and the flow editor.

import { AppWindow, Globe, MonitorSmartphone } from "lucide-react";
import { useMemo } from "react";
import type { Condition } from "@/lib/types";
import { useEditor } from "@/state/store";
import { Button, Field, Input, NumberInput, Select, Switch, Textarea } from "@/ui";
import { BROWSERS, nodeDef, type ParamDef, type Params } from "../catalog";
import { CodeEditor } from "../CodeEditor";
import { useApps, useKdeShortcuts, useOutputs, useWindows, variableNames } from "../data";
import { PickerButton, SearchPicker, type PickerItem } from "./SearchPicker";
import { ShortcutRecorder } from "./ShortcutRecorder";

interface Props {
  kind: string;
  params: Params;
  onChange(patch: Params): void;
  /** Focus the first field (used right after adding a step). */
  autoFocus?: boolean;
}

export function ParamForm({ kind, params, onChange, autoFocus }: Props) {
  const fields = nodeDef(kind).params.filter((d) => !d.hidden);
  if (fields.length === 0) return <p className="text-sm text-faint">Nothing to configure.</p>;
  return (
    <div className="flex flex-col gap-3">
      {fields.map((d, i) => (
        <ParamField key={d.key} def={d} params={params} onChange={onChange} autoFocus={autoFocus && i === 0} />
      ))}
    </div>
  );
}

const str = (v: unknown) => (typeof v === "string" ? v : v == null ? "" : String(v));
const interpHint = "Use {{name}} to insert a variable.";

function ParamField({ def, params, onChange, autoFocus }: { def: ParamDef; params: Params; onChange(p: Params): void; autoFocus?: boolean }) {
  const v = params[def.key] ?? def.default;
  const set = (value: unknown) => onChange({ [def.key]: value });
  const hint = def.help ?? (def.interpolate ? interpHint : undefined);

  switch (def.type) {
    case "boolean":
      return (
        <Field label={def.label} hint={def.help} inline>
          <Switch checked={Boolean(v)} onCheckedChange={set} />
        </Field>
      );
    case "number":
      return (
        <Field label={def.label}>
          <NumberInput value={Number(v ?? 0)} min={def.min} max={def.max} step={1} suffix={def.unit} onChange={set} />
        </Field>
      );
    case "select":
      return (
        <Field label={def.label}>
          <Select value={str(v) || def.options![0]!.value} onValueChange={set} options={def.options!} />
        </Field>
      );
    case "combo":
      return (
        <Field label={def.label} as="div">
          <ShortcutRecorder value={str(v)} onChange={set} autoFocus={autoFocus} />
        </Field>
      );
    case "longtext":
      return (
        <Field label={def.label} hint={hint}>
          <Textarea autosize maxRows={8} mono={def.key === "command"} autoFocus={autoFocus} value={str(v)} placeholder={def.placeholder} onChange={(e) => set(e.currentTarget.value)} />
        </Field>
      );
    case "code":
      return (
        <Field label={def.label} as="div" hint="Full API available — see the script editor's reference.">
          <div className="overflow-hidden rounded-sm border border-line bg-raised">
            <CodeEditor value={str(v)} onChange={set} minHeight={96} maxHeight={280} />
          </div>
        </Field>
      );
    case "app":
      return (
        <Field label={def.label} as="div">
          <AppField value={str(v)} onChange={set} />
        </Field>
      );
    case "kdeShortcut":
      return (
        <Field label={def.label} as="div">
          <KdeShortcutField component={str(params.component) || "kwin"} value={str(v)} onChange={(component, shortcut) => onChange({ component, shortcut })} />
        </Field>
      );
    case "output":
      return (
        <Field label={def.label}>
          <OutputField value={str(v) || "next"} onChange={set} />
        </Field>
      );
    case "desktop":
      return (
        <Field label={def.label}>
          <Select
            value={str(v) || "next"}
            onValueChange={(x) => set(/^\d+$/.test(x) ? Number(x) : x)}
            options={[
              ...["next", "prev", "left", "right", "up", "down"].map((d) => ({ value: d, label: d === "prev" ? "Previous" : d[0]!.toUpperCase() + d.slice(1) })),
              ...Array.from({ length: 9 }, (_, i) => ({ value: String(i + 1), label: `Desktop ${i + 1}` })),
            ]}
          />
        </Field>
      );
    case "profile":
      return (
        <Field label={def.label}>
          <ProfileField value={str(v) || "next"} onChange={set} />
        </Field>
      );
    case "var":
      return (
        <Field label={def.label} hint="Any name. The same name is shared by every key.">
          <VarInput value={str(v)} onChange={set} autoFocus={autoFocus} />
        </Field>
      );
    case "condition":
      return <ConditionEditor value={(v as Condition) ?? { type: "activeApp", match: "" }} onChange={set} />;
    case "file":
    case "text":
    default:
      return (
        <Field label={def.label} hint={hint}>
          <Input mono={def.type === "file" || def.key === "url"} autoFocus={autoFocus} value={str(v)} placeholder={def.placeholder} onChange={(e) => set(e.currentTarget.value)} />
        </Field>
      );
  }
}

function AppIconImg({ url, size = 20 }: { url: string | null; size?: number }) {
  return url ? <img src={url} alt="" width={size} height={size} className="shrink-0 object-contain" /> : <AppWindow size={size - 4} className="shrink-0 text-faint" />;
}

function AppField({ value, onChange }: { value: string; onChange(v: string): void }) {
  const apps = useApps([]);
  const items = useMemo<PickerItem[]>(
    () =>
      apps
        .slice()
        .sort((a, b) => a.name.localeCompare(b.name))
        .map((a) => ({ id: a.id, title: a.name, subtitle: a.description || a.id, keywords: `${a.id} ${a.categories.join(" ")}`, icon: <AppIconImg url={a.iconUrl} /> })),
    [apps],
  );
  const current = apps.find((a) => a.id === value);
  return (
    <SearchPicker
      items={items}
      value={value}
      placeholder="Search apps…"
      onPick={(i) => onChange(i.id)}
      freeform={(q) => ({ id: q, title: `Run “${q}”`, subtitle: "Use as a command", icon: <AppWindow size={16} className="text-faint" /> })}
      trigger={
        <PickerButton placeholder="Choose an app">
          {value ? (
            <>
              <AppIconImg url={current?.iconUrl ?? null} size={18} />
              <span className="truncate">{current?.name ?? value}</span>
              {!current && <span className="truncate text-xs text-faint">command</span>}
            </>
          ) : null}
        </PickerButton>
      }
    />
  );
}

function KdeShortcutField({ component, value, onChange }: { component: string; value: string; onChange(c: string, s: string): void }) {
  const shortcuts = useKdeShortcuts([]);
  const items = useMemo<PickerItem[]>(
    () =>
      shortcuts.map((s) => ({
        id: `${s.component}\u0000${s.name}`,
        title: s.friendlyName || s.name,
        subtitle: s.keys.length ? s.keys.join(", ") : s.name,
        group: s.componentName,
        keywords: `${s.name} ${s.componentName}`,
      })),
    [shortcuts],
  );
  const current = shortcuts.find((s) => s.component === component && s.name === value);
  return (
    <SearchPicker
      items={items}
      value={`${component}\u0000${value}`}
      placeholder="Search Plasma shortcuts…"
      empty={shortcuts.length ? "Nothing found" : "No KDE shortcuts found (is this Plasma?)"}
      onPick={(i) => {
        const [c, s] = i.id.split("\u0000");
        onChange(c!, s!);
      }}
      trigger={
        <PickerButton placeholder="Choose a shortcut">
          {value ? (
            <span className="truncate">
              {current?.friendlyName ?? value} <span className="text-faint">· {current?.componentName ?? component}</span>
            </span>
          ) : null}
        </PickerButton>
      }
    />
  );
}

function OutputField({ value, onChange }: { value: string; onChange(v: string): void }) {
  const outputs = useOutputs([]);
  return (
    <Select
      value={value}
      onValueChange={onChange}
      options={[
        { value: "next", label: "Next monitor" },
        { value: "prev", label: "Previous monitor" },
        { value: "left", label: "Monitor to the left" },
        { value: "right", label: "Monitor to the right" },
        { value: "up", label: "Monitor above" },
        { value: "down", label: "Monitor below" },
        ...outputs.map((o) => ({ value: o.name, label: o.name, description: `${o.width}×${o.height}${o.model ? ` · ${o.model}` : ""}`, icon: MonitorSmartphone })),
      ]}
    />
  );
}

function ProfileField({ value, onChange }: { value: string; onChange(v: string): void }) {
  const profiles = useEditor((s) => s.config?.profiles ?? []);
  return (
    <Select
      value={value}
      onValueChange={onChange}
      options={[{ value: "next", label: "Next profile" }, { value: "prev", label: "Previous profile" }, ...profiles.map((p) => ({ value: p.id, label: p.name || "Untitled" }))]}
    />
  );
}

function VarInput({ value, onChange, autoFocus }: { value: string; onChange(v: string): void; autoFocus?: boolean }) {
  const config = useEditor((s) => s.config);
  const names = useMemo(() => variableNames(config), [config]);
  const listId = useMemo(() => `vars-${Math.random().toString(36).slice(2)}`, []);
  return (
    <>
      <Input mono autoFocus={autoFocus} list={listId} value={value} placeholder="gameMode" onChange={(e) => onChange(e.currentTarget.value.replace(/[^\w.-]/g, ""))} />
      <datalist id={listId}>
        {names.map((n) => (
          <option key={n} value={n} />
        ))}
      </datalist>
    </>
  );
}

const CONDITION_TYPES: { value: Condition["type"]; label: string }[] = [
  { value: "activeApp", label: "Focused app is…" },
  { value: "activeTitle", label: "Focused window title contains…" },
  { value: "appRunning", label: "App is running" },
  { value: "var", label: "Variable" },
  { value: "profile", label: "Current profile is…" },
  { value: "time", label: "Time of day" },
  { value: "expr", label: "JavaScript expression" },
];

function blankCondition(type: Condition["type"]): Condition {
  switch (type) {
    case "activeApp":
    case "activeTitle":
    case "appRunning":
      return { type, match: "" };
    case "var":
      return { type, name: "", op: "truthy" };
    case "profile":
      return { type, profileId: "" };
    case "time":
      return { type, from: "09:00", to: "17:00" };
    case "expr":
      return { type, code: "" };
  }
}

export function ConditionEditor({ value, onChange }: { value: Condition; onChange(c: Condition): void }) {
  const windows = useWindows([]);
  return (
    <div className="flex flex-col gap-3">
      <Field label="Condition">
        <Select value={value.type} onValueChange={(t) => onChange(blankCondition(t as Condition["type"]))} options={CONDITION_TYPES} />
      </Field>
      {(value.type === "activeApp" || value.type === "appRunning" || value.type === "activeTitle") && (
        <Field
          label={value.type === "activeTitle" ? "Title contains" : "Apps"}
          hint={value.type === "activeTitle" ? "Case-insensitive. /regex/ works too." : "Comma-separated app ids or names. /regex/ works too."}
        >
          <Input mono value={value.match} placeholder={value.type === "activeTitle" ? "YouTube" : "firefox, chromium"} onChange={(e) => onChange({ ...value, match: e.currentTarget.value })} />
        </Field>
      )}
      {(value.type === "activeApp" || value.type === "appRunning") && (
        <div className="flex flex-wrap gap-1.5">
          <Button size="sm" variant="secondary" icon={Globe} onClick={() => onChange({ ...value, match: BROWSERS })}>
            Browsers
          </Button>
          <SearchPicker
            items={windows.map((w) => ({ id: w.id, title: w.title || w.appId, subtitle: w.appId || w.resourceClass }))}
            value={null}
            placeholder="Search open windows…"
            empty="No windows (is the daemon running?)"
            onPick={(i) => {
              const w = windows.find((x) => x.id === i.id);
              const id = (w?.desktopFile || w?.appId || w?.resourceClass || "").replace(/\.desktop$/, "");
              if (!id) return;
              const list = value.match.split(",").map((s) => s.trim()).filter(Boolean);
              if (!list.includes(id)) list.push(id);
              onChange({ ...value, match: list.join(", ") });
            }}
            trigger={
              <Button size="sm" variant="secondary" icon={AppWindow}>
                Pick an open window
              </Button>
            }
          />
        </div>
      )}
      {value.type === "var" && (
        <div className="grid grid-cols-[1fr_auto] gap-2">
          <Field label="Variable">
            <VarInput value={value.name} onChange={(name) => onChange({ ...value, name })} />
          </Field>
          <Field label="Is">
            <Select
              value={value.op}
              onValueChange={(op) => onChange({ ...value, op: op as "truthy" })}
              options={[
                { value: "truthy", label: "On" },
                { value: "falsy", label: "Off" },
                { value: "eq", label: "Equal to" },
                { value: "neq", label: "Not equal to" },
              ]}
            />
          </Field>
          {(value.op === "eq" || value.op === "neq") && (
            <Field label="Value" className="col-span-2">
              <Input mono value={value.value ?? ""} onChange={(e) => onChange({ ...value, value: e.currentTarget.value })} />
            </Field>
          )}
        </div>
      )}
      {value.type === "profile" && <ProfileOnly value={value.profileId} onChange={(profileId) => onChange({ ...value, profileId })} />}
      {value.type === "time" && (
        <div className="grid grid-cols-2 gap-2">
          <Field label="From">
            <Input type="time" value={value.from} onChange={(e) => onChange({ ...value, from: e.currentTarget.value })} />
          </Field>
          <Field label="To">
            <Input type="time" value={value.to} onChange={(e) => onChange({ ...value, to: e.currentTarget.value })} />
          </Field>
        </div>
      )}
      {value.type === "expr" && (
        <Field label="Expression" hint="Any JavaScript expression, e.g. vars.get(&quot;count&quot;) > 3">
          <Input mono value={value.code} placeholder='window.isActive("obs")' onChange={(e) => onChange({ ...value, code: e.currentTarget.value })} />
        </Field>
      )}
    </div>
  );
}

function ProfileOnly({ value, onChange }: { value: string; onChange(v: string): void }) {
  const profiles = useEditor((s) => s.config?.profiles ?? []);
  return (
    <Field label="Profile">
      <Select value={value || profiles[0]?.id || "none"} onValueChange={onChange} options={profiles.map((p) => ({ value: p.id, label: p.name || "Untitled" }))} />
    </Field>
  );
}
