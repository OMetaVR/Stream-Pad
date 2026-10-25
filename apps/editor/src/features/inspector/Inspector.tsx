// Right pane: everything about the selected key — how it looks (label, icon, cover, colour) and what it
// does (the macro feature's <MacroSection>).

import { ChevronsUpDown, Lock, MousePointerClick } from "lucide-react";
import { useRef } from "react";
import { MacroSection } from "@/features/macro";
import { emptyBinding, GLOBAL_ACTION_INFO } from "@/lib/model";
import { KEY_INFO } from "@/lib/types";
import { useBinding, useGlobal } from "@/state/selectors";
import { editor, useEditor, useRequest } from "@/state/store";
import { Button, ColorPicker, EmptyState, Field, Input, Kbd, RefIcon, ScrollArea } from "@/ui";
import { CoverPicker } from "./CoverPicker";
import { IconPicker } from "./IconPicker";

export function Inspector() {
  const { profileId, keyId } = useEditor((s) => s.selection);
  const binding = useBinding(profileId, keyId);
  const global = useGlobal(keyId);
  const labelRef = useRef<HTMLInputElement>(null);
  useRequest("focusLabel", () => labelRef.current?.focus());

  if (!profileId || !keyId) {
    return (
      <aside className="flex h-full items-center border-l border-line bg-panel">
        <EmptyState icon={MousePointerClick} title="Select a key" description="Click a key on the pad, or use the arrow keys, to choose what it does." />
      </aside>
    );
  }

  const info = KEY_INFO[keyId];
  const b = binding ?? emptyBinding();
  const update = (fn: Parameters<typeof editor.updateBinding>[2], coalesce?: string | false) => editor.updateBinding(profileId, keyId, fn, coalesce === undefined ? undefined : { coalesce });

  return (
    <aside className="flex h-full min-h-0 flex-col border-l border-line bg-panel">
      <div className="flex h-14 shrink-0 items-center gap-3 border-b border-line-subtle px-4">
        <span className="flex h-9 min-w-9 items-center justify-center rounded-[9px] bg-keycap px-2 text-base font-semibold shadow-[inset_0_1px_0_var(--sp-keycap-highlight),0_2px_0_var(--sp-keycap-skirt)]">
          {info.legend.main}
        </span>
        <div className="min-w-0">
          <h2 className="truncate text-md font-semibold">{b.label || info.name}</h2>
          <p className="text-xs text-muted">{b.label ? info.name : global ? "Global key" : "Not set up yet"}</p>
        </div>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="flex flex-col gap-6 px-4 pb-8 pt-4">
          {global ? (
            <EmptyState
              size="sm"
              icon={Lock}
              title={GLOBAL_ACTION_INFO[global.tap].description}
              description={`This key does the same thing in every profile${global.hold ? `, and opens “${GLOBAL_ACTION_INFO[global.hold].label}” when held` : ""}.`}
              action={
                <Button size="sm" onClick={() => editor.openSettings("globals")}>
                  Change in Settings
                </Button>
              }
            />
          ) : (
            <>
              <section className="flex flex-col gap-3" aria-label="Appearance">
                <h3 className="text-sm font-medium text-muted">Appearance</h3>
                <Field label="Label">
                  <Input ref={labelRef} value={b.label} placeholder="e.g. Mute mic" onChange={(e) => update((d) => void (d.label = e.currentTarget.value))} />
                </Field>
                <div className="grid grid-cols-[1fr_auto] items-end gap-3">
                  <Field label="Icon" as="div">
                    <IconPicker
                      value={b.icon}
                      onChange={(icon) => update((d) => void (d.icon = icon), false)}
                      trigger={
                        <button type="button" className="flex h-8 w-full items-center gap-2 rounded-sm border border-line bg-raised px-2 text-left text-base hover:border-line-strong">
                          {b.icon ? <RefIcon iconRef={b.icon} size={16} /> : null}
                          <span className={b.icon ? "truncate" : "text-faint"}>{b.icon ? (b.icon.startsWith("lucide:") ? b.icon.slice(7) : "App icon") : "Choose an icon"}</span>
                          <ChevronsUpDown size={14} className="ml-auto text-faint" />
                        </button>
                      }
                    />
                  </Field>
                  <Field label="Key colour" as="div">
                    <ColorPicker value={b.color} allowNone onChange={(c) => update((d) => void (d.color = c), false)} />
                  </Field>
                </div>
                <Field label="Cover" as="div">
                  <CoverPicker
                    keyId={keyId}
                    value={b.cover}
                    crop={b.coverCrop ?? null}
                    onChange={(cover, crop) => update((d) => void ((d.cover = cover), (d.coverCrop = crop)), false)}
                  />
                </Field>
              </section>
              <MacroSection profileId={profileId} keyId={keyId} binding={b} onChange={(next) => editor.setBinding(profileId, keyId, next)} />
              <p className="text-xs text-faint">
                <Kbd>Enter</Kbd> choose an action · <Kbd>Del</Kbd> clear key · <Kbd combo="mod+c" /> / <Kbd combo="mod+v" /> copy & paste
              </p>
            </>
          )}
        </div>
      </ScrollArea>
    </aside>
  );
}
