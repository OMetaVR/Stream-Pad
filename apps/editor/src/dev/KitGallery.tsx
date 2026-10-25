// Dev-only living reference of the UI kit: `bun run dev`, then open http://127.0.0.1:5173/#kit.

import {
  Bell,
  Copy,
  Download,
  Inbox,
  Keyboard,
  Layers,
  Pencil,
  Play,
  Plus,
  Search,
  Settings,
  Trash2,
  Workflow,
} from "lucide-react";
import { LucideProvider } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useTheme, type ThemePreference } from "@/lib/theme";
import {
  Badge,
  Button,
  Card,
  Checkbox,
  ColorPicker,
  confirm,
  ConfirmHost,
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
  Dialog,
  DialogClose,
  DialogContent,
  DialogTrigger,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  EmptyState,
  Field,
  IconButton,
  Input,
  Kbd,
  NumberInput,
  Popover,
  PopoverContent,
  PopoverTrigger,
  RefIcon,
  ScrollArea,
  Section,
  Segmented,
  Select,
  Slider,
  Spinner,
  Switch,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Textarea,
  toast,
  Toaster,
  Tooltip,
  TooltipProvider,
} from "@/ui";

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[140px_1fr] items-start gap-4">
      <span className="pt-1.5 text-sm text-faint">{label}</span>
      <div className="flex flex-wrap items-center gap-3">{children}</div>
    </div>
  );
}

export function KitGallery() {
  const theme = useTheme();
  const [text, setText] = useState("Mute mic");
  const [notes, setNotes] = useState("Hold to talk.\nRelease to mute again.");
  const [ms, setMs] = useState(600);
  const [on, setOn] = useState(true);
  const [check, setCheck] = useState(false);
  const [edge, setEdge] = useState<"bottom" | "top" | "left" | "right">("bottom");
  const [mode, setMode] = useState<"ignore" | "restart" | "parallel">("ignore");
  const [scale, setScale] = useState(1);
  const [color, setColor] = useState<string | null>("#8b7cf6");

  return (
    <LucideProvider size={16} strokeWidth={1.75}>
      <TooltipProvider>
        <ScrollArea className="h-full">
          <div className="mx-auto flex max-w-[920px] flex-col gap-10 px-10 py-10">
            <header className="flex items-center justify-between">
              <div>
                <h1 className="text-2xl font-semibold tracking-[-0.02em]">UI kit</h1>
                <p className="text-muted">Every component in src/ui, in both themes.</p>
              </div>
              <Segmented<ThemePreference>
                value={theme.preference}
                onValueChange={theme.setPreference}
                options={[
                  { value: "system", label: "System" },
                  { value: "dark", label: "Dark" },
                  { value: "light", label: "Light" },
                ]}
              />
            </header>

            <Section title="Buttons" description="primary / secondary / ghost / danger × sm / md / lg">
              <Card className="flex flex-col gap-4">
                <Row label="Variants">
                  <Button variant="primary" icon={Play}>
                    Test
                  </Button>
                  <Button icon={Workflow}>Open flow editor</Button>
                  <Button variant="ghost" icon={Copy}>
                    Duplicate
                  </Button>
                  <Button variant="danger" icon={Trash2}>
                    Delete
                  </Button>
                </Row>
                <Row label="Sizes">
                  <Button size="sm">Small</Button>
                  <Button size="md">Medium</Button>
                  <Button size="lg" variant="primary" icon={Plus}>
                    Choose an action
                  </Button>
                </Row>
                <Row label="States">
                  <Button loading>Saving</Button>
                  <Button disabled>Disabled</Button>
                </Row>
                <Row label="IconButton">
                  <IconButton icon={Pencil} label="Rename" shortcut="f2" />
                  <IconButton icon={Settings} label="Settings" shortcut="mod+," />
                  <IconButton icon={Keyboard} label="Press to select" active />
                  <IconButton icon={Plus} label="New profile" variant="secondary" size="md" />
                </Row>
              </Card>
            </Section>

            <Section title="Inputs">
              <Card className="grid grid-cols-2 gap-5">
                <Field label="Label" hint="Shown on the key">
                  <Input value={text} onChange={(e) => setText(e.currentTarget.value)} />
                </Field>
                <Field label="Search" aside={<Kbd combo="mod+k" />}>
                  <Input icon={Search} placeholder="Search actions" />
                </Field>
                <Field label="Hold time" hint="How long to hold Enter to open the editor">
                  <NumberInput value={ms} onChange={setMs} min={150} max={5000} step={50} suffix="ms" />
                </Field>
                <Field label="Combo" error="Unknown key “ctrl+nope”">
                  <Input mono value="ctrl+nope" readOnly />
                </Field>
                <Field label="Notes" className="col-span-2">
                  <Textarea autosize value={notes} onChange={(e) => setNotes(e.currentTarget.value)} />
                </Field>
              </Card>
            </Section>

            <Section title="Choices">
              <Card className="flex flex-col gap-4">
                <Field label="Start in macro mode" hint="Otherwise StreamPad starts as a normal numpad" inline>
                  <Switch checked={on} onCheckedChange={setOn} />
                </Field>
                <Row label="Checkbox">
                  <Checkbox checked={check} onCheckedChange={setCheck} label="Repeat while held" />
                  <Checkbox checked="indeterminate" onCheckedChange={() => undefined} label="Some keys" />
                </Row>
                <Field label="Screen edge" as="div">
                  <Segmented
                    value={edge}
                    onValueChange={setEdge}
                    options={[
                      { value: "bottom", label: "Bottom" },
                      { value: "top", label: "Top" },
                      { value: "left", label: "Left" },
                      { value: "right", label: "Right" },
                    ]}
                  />
                </Field>
                <Field label="When already running">
                  <Select
                    value={mode}
                    onValueChange={setMode}
                    className="w-64"
                    options={[
                      { value: "ignore", label: "Ignore the press", description: "Let the running macro finish" },
                      { value: "restart", label: "Restart", description: "Cancel it and start again" },
                      { value: "parallel", label: "Run another copy", description: "Both run at the same time" },
                    ]}
                  />
                </Field>
                <Field label="Scale">
                  <Slider value={scale} onValueChange={setScale} min={0.5} max={2} step={0.05} format={(v) => `${Math.round(v * 100)}%`} />
                </Field>
                <Field label="Accent" inline>
                  <ColorPicker value={color} onChange={setColor} allowNone showValue />
                </Field>
              </Card>
            </Section>

            <Section title="Overlays">
              <Card className="flex flex-col gap-4">
                <Row label="Tooltip + Kbd">
                  <Tooltip content="Undo" shortcut="mod+z">
                    <Button size="sm">Hover me</Button>
                  </Tooltip>
                  <Kbd combo="ctrl+shift+t" size="md" />
                  <Kbd combo="meta+left" />
                  <Kbd>Esc</Kbd>
                </Row>
                <Row label="Popover">
                  <Popover>
                    <PopoverTrigger asChild>
                      <Button size="sm">Open popover</Button>
                    </PopoverTrigger>
                    <PopoverContent className="w-64">
                      <p className="text-sm text-muted">Popovers hold small forms and pickers.</p>
                    </PopoverContent>
                  </Popover>
                </Row>
                <Row label="Dialog">
                  <Dialog>
                    <DialogTrigger asChild>
                      <Button size="sm">Open dialog</Button>
                    </DialogTrigger>
                    <DialogContent
                      title="Export profile"
                      description="Saves “Audio” as a .json file you can share."
                      footer={
                        <>
                          <DialogClose asChild>
                            <Button variant="ghost">Cancel</Button>
                          </DialogClose>
                          <Button variant="primary" icon={Download}>
                            Export
                          </Button>
                        </>
                      }
                    >
                      <Field label="File name">
                        <Input defaultValue="audio.streampad.json" />
                      </Field>
                    </DialogContent>
                  </Dialog>
                  <Button
                    size="sm"
                    onClick={async () => {
                      const ok = await confirm({
                        title: "Convert to script?",
                        description: "The flow becomes JavaScript. You can't go back to the node view.",
                        confirmLabel: "Convert",
                      });
                      toast(ok ? "Converted" : "Kept the flow");
                    }}
                  >
                    confirm()
                  </Button>
                </Row>
                <Row label="Menus">
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button size="sm">Dropdown</Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent>
                      <DropdownMenuLabel>Profile</DropdownMenuLabel>
                      <DropdownMenuItem icon={Pencil} shortcut="f2">
                        Rename
                      </DropdownMenuItem>
                      <DropdownMenuItem icon={Copy} shortcut="mod+d">
                        Duplicate
                      </DropdownMenuItem>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem icon={Trash2} destructive>
                        Delete
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                  <ContextMenu>
                    <ContextMenuTrigger className="flex h-16 w-48 items-center justify-center rounded-md border border-dashed border-line-strong text-sm text-muted">
                      Right-click here
                    </ContextMenuTrigger>
                    <ContextMenuContent>
                      <ContextMenuItem icon={Copy} shortcut="mod+c">
                        Copy key
                      </ContextMenuItem>
                      <ContextMenuSeparator />
                      <ContextMenuItem icon={Trash2} shortcut="delete" destructive>
                        Clear key
                      </ContextMenuItem>
                    </ContextMenuContent>
                  </ContextMenu>
                </Row>
                <Row label="Toasts">
                  <Button size="sm" onClick={() => toast("Copied “Mute mic”")}>
                    Info
                  </Button>
                  <Button size="sm" onClick={() => toast.success("Macro finished")}>
                    Success
                  </Button>
                  <Button size="sm" onClick={() => toast.error("Could not save", { description: "Disk is full" })}>
                    Error
                  </Button>
                  <Button
                    size="sm"
                    onClick={() => toast({ title: "Deleted “Audio”", action: { label: "Undo", onClick: () => toast("Restored") } })}
                  >
                    With Undo
                  </Button>
                </Row>
              </Card>
            </Section>

            <Section title="Structure">
              <Card className="flex flex-col gap-5">
                <Tabs defaultValue="flow">
                  <TabsList>
                    <TabsTrigger value="flow" icon={Workflow}>
                      Flow
                    </TabsTrigger>
                    <TabsTrigger value="code">Code</TabsTrigger>
                  </TabsList>
                  <TabsContent value="flow" className="pt-3 text-sm text-muted">
                    Underline tabs.
                  </TabsContent>
                  <TabsContent value="code" className="pt-3 text-sm text-muted">
                    Generated JavaScript.
                  </TabsContent>
                </Tabs>
                <Tabs defaultValue="a">
                  <TabsList variant="pill" className="w-fit">
                    <TabsTrigger value="a">Recent</TabsTrigger>
                    <TabsTrigger value="b">All</TabsTrigger>
                  </TabsList>
                </Tabs>
                <Row label="Badges">
                  <Badge>Neutral</Badge>
                  <Badge variant="accent" dot>
                    Active
                  </Badge>
                  <Badge variant="success" dot>
                    Grabbed
                  </Badge>
                  <Badge variant="warning">No numpad</Badge>
                  <Badge variant="danger" icon={Bell}>
                    Error
                  </Badge>
                </Row>
                <Row label="Spinner + RefIcon">
                  <Spinner />
                  <Spinner size={20} />
                  <RefIcon iconRef="lucide:audio-lines" size={20} />
                  <RefIcon iconRef="lucide:gamepad-2" size={20} className="text-accent-text" />
                  <RefIcon iconRef="/mock/apps/firefox.svg" size={24} />
                  <RefIcon iconRef="lucide:does-not-exist" size={20} />
                </Row>
                <Row label="ScrollArea">
                  <ScrollArea className="h-28 w-64 rounded-md border border-line">
                    <ul className="p-2 text-sm">
                      {Array.from({ length: 20 }, (_, i) => (
                        <li key={i} className="px-2 py-1 text-muted">
                          Action {i + 1}
                        </li>
                      ))}
                    </ul>
                  </ScrollArea>
                </Row>
              </Card>
              <Card padding="none">
                <EmptyState
                  icon={Inbox}
                  title="No action yet"
                  description="Pick what this key does — launch an app, press a shortcut, control media…"
                  action={
                    <Button variant="primary" icon={Layers}>
                      Choose an action
                    </Button>
                  }
                />
              </Card>
            </Section>
          </div>
        </ScrollArea>
        <Toaster />
        <ConfirmHost />
      </TooltipProvider>
    </LucideProvider>
  );
}
