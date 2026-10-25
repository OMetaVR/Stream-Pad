// App shell: providers, global hotkeys, accent tinting and the top-level view switch.
// Layout (main view): sidebar (248) | numpad canvas | inspector (360), under a 44 px header.

import { LucideProvider } from "lucide-react";
import { MotionConfig } from "motion/react";
import { lazy, Suspense, useEffect } from "react";
import { FlowEditorView, ScriptEditorView } from "@/features/macro";
import { Inspector } from "@/features/inspector/Inspector";
import { NumpadCanvas } from "@/features/numpad/NumpadCanvas";
import { SettingsView } from "@/features/settings/SettingsView";
import { Sidebar } from "@/features/sidebar/Sidebar";
import { getBackend } from "@/lib/backend";
import { useHotkey } from "@/lib/hotkeys";
import { findProfile } from "@/lib/model";
import { applyAccent } from "@/lib/theme";
import { Header } from "@/shell/Header";
import { LoadGate } from "@/shell/LoadGate";
import { useDaemonEvent } from "@/state/daemon";
import { editor, useEditor } from "@/state/store";
import { ConfirmHost, toast, Toaster, TooltipProvider } from "@/ui";

const SimulatePanel = lazy(() => import("@/dev/SimulatePanel"));

/** The selected profile's colour tints selection states, focus rings and primary buttons. */
function useProfileAccent() {
  const accent = useEditor((s) => {
    if (!s.config) return null;
    return findProfile(s.config, s.selection.profileId)?.color ?? s.config.settings.overlay.accent;
  });
  useEffect(() => applyAccent(accent), [accent]);
}

function useGlobalHotkeys() {
  // Registered first = lowest priority: features registering the same combos win.
  useHotkey("mod+z", () => void editor.undo(), { description: "Undo" });
  useHotkey(["mod+shift+z", "mod+y"], () => void editor.redo(), { description: "Redo" });
  useHotkey("mod+,", () => editor.openSettings(), { description: "Open settings" });
  useHotkey("escape", () => (useEditor.getState().view.kind === "main" ? false : editor.showMain()), {
    description: "Back to the pad",
  });
}

function useDaemonNotices() {
  useDaemonEvent("configError", ({ message }) =>
    toast.error("The daemon rejected config.json", { description: message, id: "config-error" }),
  );
}

function CurrentView() {
  const view = useEditor((s) => s.view);
  switch (view.kind) {
    case "main":
      return (
        <div className="grid min-h-0 flex-1 grid-cols-[var(--sp-sidebar-w)_minmax(0,1fr)_var(--sp-inspector-w)]">
          <Sidebar />
          <NumpadCanvas />
          <Inspector />
        </div>
      );
    case "settings":
      return <SettingsView section={view.section} />;
    case "flow":
      return <FlowEditorView key={`${view.profileId}:${view.keyId}`} profileId={view.profileId} keyId={view.keyId} />;
    case "script":
      return <ScriptEditorView key={`${view.profileId}:${view.keyId}`} profileId={view.profileId} keyId={view.keyId} />;
  }
}

export function App() {
  useProfileAccent();
  useGlobalHotkeys();
  useDaemonNotices();
  return (
    <MotionConfig reducedMotion="user">
      <LucideProvider size={16} strokeWidth={1.75}>
        <TooltipProvider>
          <div className="flex h-full flex-col bg-bg text-fg">
            <Header />
            <LoadGate>
              <CurrentView />
            </LoadGate>
          </div>
          <Toaster />
          <ConfirmHost />
          {getBackend().isMock && (
            <Suspense>
              <SimulatePanel />
            </Suspense>
          )}
        </TooltipProvider>
      </LucideProvider>
    </MotionConfig>
  );
}
