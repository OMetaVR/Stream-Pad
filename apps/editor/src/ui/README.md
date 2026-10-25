# StreamPad UI kit

Import everything from `@/ui`. A living reference of every component, in both themes, runs at
`http://127.0.0.1:5173/#kit` (`bun run dev`; dev builds only).

## Conventions

**Look.** Quiet, precise, instrument-like (docs/SPEC.md §9). One accent; everything else is graphite and
hairlines. Spend boldness on the numpad, keep chrome calm. No gradients, no drop shadows except the
token shadows below, no ALL-CAPS labels.

**Colour** — only token utilities exist (Tailwind's default palette is removed in `styles/index.css`):

| utility | use |
|---|---|
| `bg-bg` | app background, the numpad stage |
| `bg-panel` | sidebar, inspector, cards, dialogs |
| `bg-raised` | inputs, menus, popovers, toasts |
| `bg-control` / `-hover` / `-active` | secondary buttons, selected rows, pressed states |
| `border-line-subtle` / `line` / `line-strong` | hairlines (default `line`; `strong` on hover) |
| `text-fg` / `text-muted` / `text-faint` | primary text / secondary / placeholders, legends |
| `bg-accent` `text-accent-fg` | primary actions, checked controls |
| `bg-accent-soft` `text-accent-text` `border-accent-line` | selection, active toggles, focus |
| `danger` `success` `warning` (+ `-soft`) | status only |
| `bg-keybed` `bg-keycap` `keycap-skirt` `keycap-highlight` | numpad rendering (same values as the overlay) |

The accent follows the selected profile's colour (`applyAccent` in `lib/theme.ts`, wired in App.tsx) —
never hard-code `#8b7cf6`. Opacity modifiers work on every token (`bg-control/60`).

**Type** (Inter Variable, JetBrains Mono for code): `text-2xs` 10/14 · `text-xs` 11/16 · `text-sm` 12/16
(labels, hints, menus' secondary text) · `text-base` 13/20 (body, controls) · `text-md` 14/20 (pane titles)
· `text-lg` 16/24 (dialog titles) · `text-xl` 20/28 (page titles) · `text-2xl` 24/32. Weights: body is
the token weight (430 dark / 400 light), `font-medium` for labels and buttons, `font-semibold` for titles.
Titles get slight negative tracking (`tracking-[-0.01em]`). `font-code` = mono at 0.92 em for combos,
ids, paths. Use `tabular-nums` for numbers that change.

**Spacing** — 4 px grid (`--spacing: 4px`, so `p-3` = 12 px). Panes pad 16 (`px-4`), dialogs 20 (`px-5`),
settings pages 32. Gaps: 6–8 inside controls, 12–20 between fields, 24+ between sections.

**Radii** — `rounded-sm` 6 (controls, items) · `rounded-md` 8 (menus, popovers, toasts) · `rounded-lg` 12
(cards, dialogs) · `rounded-xl` 16 (hero frames such as the numpad). Keycaps use 11 px (overlay spec).

**Sizes** — controls are 28 (`sm`) / 32 (`md`, default) / 40 (`lg`, hero CTAs) px tall. Icons: lucide,
16 px, stroke 1.75 by default (set by `<LucideProvider>` in App.tsx); pass `size` for others.

**Motion** — `--sp-dur-fast` 120 ms (hover, press), `--sp-dur` 160 ms (popovers, toggles),
`--sp-dur-slow` 220 ms (dialogs); easing `ease-out` (`--sp-ease-out`) for entrances, `ease-in` for exits.
Utilities: `transition-quick`, `transition-base`, `animate-pop-in/out`, `animate-fade-in/out`,
`animate-dialog-in/out`. Motion answers user actions; nothing animates on its own except live state
(key presses, running macros). `prefers-reduced-motion` is honoured globally (CSS and `<MotionConfig>`).

**Accessibility** — every control is keyboard reachable with a visible focus ring (global
`:focus-visible` outline in the accent). Icon-only buttons must be `IconButton` (required `label`).
Inside a `<Field>`, kit controls get their id / `aria-labelledby` / `aria-describedby` / `aria-invalid`
automatically; custom controls call `useFieldControl(props)`.

**Overriding styles** — every component takes `className`, merged with `cn()` (`lib/cn.ts`,
tailwind-merge configured for our tokens), so `className="w-64 px-2"` reliably wins.

## Components

```tsx
import { Play, Plus, Search, Trash2, Workflow } from "lucide-react";
import * as UI from "@/ui";
```

**Button** — `variant` primary | secondary (default) | ghost | danger, `size` sm | md | lg, `icon`,
`iconRight`, `loading`, `asChild`.
```tsx
<Button variant="primary" icon={Play} loading={running} onClick={test}>Test</Button>
```

**IconButton** — square, tooltip = label (+ shortcut). `active` renders a pressed toggle.
```tsx
<IconButton icon={Plus} label="New profile" shortcut="mod+n" onClick={add} />
```

**Input** — `icon`, `trailing`, `mono`, `size`; `wrapperClassName` sizes the box.
```tsx
<Input icon={Search} placeholder="Search actions" value={q} onChange={(e) => setQ(e.currentTarget.value)} />
```

**Textarea** — `autosize` (up to `maxRows`), `mono`.
```tsx
<Textarea autosize mono value={code} onChange={(e) => setCode(e.currentTarget.value)} />
```

**NumberInput** — commits clamped, step-rounded numbers (step 1 ⇒ integers, which u32 config fields need).
↑/↓ step, Shift ×10, Enter commits, Esc reverts.
```tsx
<NumberInput value={s.holdMs} min={150} max={5000} step={50} suffix="ms" onChange={(v) => editor.updateSettings((d) => { d.holdMs = v; })} />
```

**Switch** / **Checkbox**
```tsx
<Switch checked={s.blur} onCheckedChange={(v) => editor.updateSettings((d) => { d.overlay.blur = v; })} />
<Checkbox checked={b.repeat} onCheckedChange={setRepeat} label="Repeat while held" />
```

**Segmented** — single choice with a sliding pill; `fill` stretches; icon-only options need `title`.
```tsx
<Segmented value={edge} onValueChange={setEdge} options={[{ value: "bottom", label: "Bottom" }, { value: "top", label: "Top" }]} />
```

**Select** — `options` with optional `icon` / `description`; values must not be "".
```tsx
<Select value={b.concurrency} onValueChange={setConcurrency} options={[{ value: "ignore", label: "Ignore the press" }, { value: "restart", label: "Restart" }]} />
```

**Slider** — `onValueChange` fires while dragging (store edits coalesce into one undo step), `format`
controls the readout.
```tsx
<Slider value={o.scale} min={0.5} max={2} step={0.05} format={(v) => `${Math.round(v * 100)}%`} onValueChange={setScale} />
```

**ColorPicker** — curated swatches, hex field, native picker; `allowNone` adds a "Default" (null) choice.
```tsx
<ColorPicker value={profile.color} allowNone onChange={(c) => editor.updateProfile(id, (p) => { p.color = c; })} />
```

**Field** — label + control + hint/error; `inline` puts the control on the right (settings rows);
`aside` fills the label row's right side; `as="div"` for groups (Segmented).
```tsx
<Field label="Label" hint="Shown on the key" error={err}><Input value={label} onChange={…} /></Field>
<Field label="Start at login" inline><Switch checked={on} onCheckedChange={setOn} /></Field>
```

**Tooltip** — wraps any ref-accepting trigger; `shortcut` renders keycaps. `TooltipProvider` is in App.
```tsx
<Tooltip content="Global: next profile — change in Settings"><span>…</span></Tooltip>
```

**Kbd** — keycap hints: `combo` ("mod+shift+z" → Ctrl Shift Z) or literal children.
```tsx
<Kbd combo="ctrl+alt+t" /> <Kbd>Esc</Kbd>
```

**Popover** — `Popover`, `PopoverTrigger`, `PopoverContent` (`padding` none | sm | md), `PopoverAnchor`, `PopoverClose`.
```tsx
<Popover><PopoverTrigger asChild><Button>Icon</Button></PopoverTrigger><PopoverContent className="w-80">…</PopoverContent></Popover>
```

**Dialog** — `Dialog`, `DialogTrigger`, `DialogContent` (`title`, `description`, `footer`, `size` sm | md | lg,
`hideClose`), `DialogClose`. For yes/no questions use the imperative **confirm()** (host mounted in App).
StreamPad prefers undo toasts over confirmations for deletes.
```tsx
if (await confirm({ title: "Convert to script?", description: "This can't be undone into a flow.", confirmLabel: "Convert" })) convert();
```

**DropdownMenu** / **ContextMenu** — same item API: `icon`, `shortcut` (display only), `destructive`;
plus Label, Separator, CheckboxItem, (Dropdown) RadioGroup/RadioItem, Sub/SubTrigger/SubContent.
```tsx
<ContextMenu>
  <ContextMenuTrigger asChild><li>…</li></ContextMenuTrigger>
  <ContextMenuContent>
    <ContextMenuItem icon={Pencil} shortcut="f2" onSelect={rename}>Rename</ContextMenuItem>
    <ContextMenuSeparator />
    <ContextMenuItem icon={Trash2} destructive onSelect={remove}>Delete</ContextMenuItem>
  </ContextMenuContent>
</ContextMenu>
```

**Tabs** — `TabsList variant="underline" | "pill"`, `TabsTrigger icon`, `TabsContent`.
```tsx
<Tabs defaultValue="flow"><TabsList><TabsTrigger value="flow" icon={Workflow}>Flow</TabsTrigger><TabsTrigger value="code">Code</TabsTrigger></TabsList>…</Tabs>
```

**Section** / **Card** — Section = titled group (`description`, `actions`); Card = bordered panel
(`padding`, `divided` for settings lists).
```tsx
<Section title="Overlay" description="Where the on-screen pad appears"><Card divided>{rows}</Card></Section>
```

**Badge** — `variant` neutral | accent | success | warning | danger, `dot`, `icon`.
```tsx
<Badge variant="success" dot>Grabbed</Badge>
```

**ScrollArea** — overlay scrollbars; give the root a bounded size.
```tsx
<ScrollArea className="min-h-0 flex-1">{list}</ScrollArea>
```

**Toaster / toast()** — bottom-centre stack (max 4), pauses on hover; `action` for Undo; reuse an `id`
to replace a toast.
```tsx
toast({ title: `Deleted “${name}”`, action: { label: "Undo", onClick: restore } });
toast.error("Could not start StreamPad", { description: e.message });
```

**EmptyState** — icon, title, description, and always the next action.
```tsx
<EmptyState icon={Zap} title="No action yet" description="Pick what this key does." action={<Button variant="primary">Choose an action</Button>} />
```

**Spinner** — `size`, optional `label` for screen readers.
```tsx
<Spinner size={14} />
```

**RefIcon** — renders any `IconRef` from the config: `lucide:<name>` via lucide-react (tinted by
`currentColor`), files via `backend.assetUrl`, with a fallback glyph when missing.
```tsx
<RefIcon iconRef={binding.icon} size={22} className="text-fg" />
```

**Separator** — `orientation` horizontal | vertical.
