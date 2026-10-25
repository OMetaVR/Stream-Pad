// Settings: section nav on the left, the section on the right. Esc returns to the pad (App.tsx).

import { Info, Keyboard, Layers, Monitor, Settings2, SquareCode, type LucideIcon } from "lucide-react";
import { cn } from "@/lib/cn";
import { editor, type SettingsSection } from "@/state/store";
import { ScrollArea } from "@/ui";
import { OverlaySection } from "./OverlaySection";
import { UpdatesSection } from "./UpdatesSection";
import { AboutSection, DevicesSection, GeneralSection, GlobalsSection, ScriptingSection } from "./sections";

const SECTIONS: { id: SettingsSection; label: string; icon: LucideIcon; render: () => React.ReactNode }[] = [
  { id: "general", label: "General", icon: Settings2, render: () => <GeneralSection /> },
  { id: "globals", label: "Global keys", icon: Keyboard, render: () => <GlobalsSection /> },
  { id: "overlay", label: "On-screen pad", icon: Layers, render: () => <OverlaySection /> },
  { id: "devices", label: "Keyboards", icon: Monitor, render: () => <DevicesSection /> },
  { id: "scripting", label: "Scripting", icon: SquareCode, render: () => <ScriptingSection /> },
  { id: "about", label: "Updates & about", icon: Info, render: () => (<><UpdatesSection /><AboutSection /></>) },
];

export function SettingsView({ section = "general" }: { section?: SettingsSection }) {
  const current = SECTIONS.find((s) => s.id === section) ?? SECTIONS[0]!;
  return (
    <div className="flex min-h-0 flex-1">
      <nav className="flex w-(--sp-sidebar-w) shrink-0 flex-col gap-px border-r border-line bg-panel p-2" aria-label="Settings sections">
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            type="button"
            onClick={() => editor.openSettings(s.id)}
            aria-current={s.id === current.id ? "page" : undefined}
            className={cn(
              "flex h-8 items-center gap-2.5 rounded-sm px-2 text-base transition-quick",
              s.id === current.id ? "bg-control text-fg" : "text-muted hover:bg-control/60 hover:text-fg",
            )}
          >
            <s.icon size={15} aria-hidden />
            {s.label}
          </button>
        ))}
        <div className="flex-1" />
        <button type="button" onClick={editor.showMain} className="flex h-8 items-center rounded-sm px-2 text-sm text-faint hover:text-fg">
          ← Back to the pad <span className="ml-auto">Esc</span>
        </button>
      </nav>
      <ScrollArea className="min-h-0 flex-1">
        <div className="mx-auto flex max-w-[720px] flex-col gap-8 px-8 py-8">
          <h1 className="text-xl font-semibold tracking-[-0.015em]">{current.label}</h1>
          {current.render()}
        </div>
      </ScrollArea>
    </div>
  );
}
