import { Play } from "lucide-react";
import { useState } from "react";
import { getBackend } from "@/lib/backend";
import { cn } from "@/lib/cn";
import { useDaemon } from "@/state/daemon";
import { Spinner, toast, Tooltip } from "@/ui";

/**
 * Daemon status at a glance: Running (+ macro mode), Connecting…, or Not running — Start.
 * PLACEHOLDER: the editor main engineer owns its final design and placement (spec: sidebar bottom).
 */
export function DaemonPill() {
  const status = useDaemon((s) => s.status);
  const macroMode = useDaemon((s) => s.state?.macroMode ?? false);
  const [starting, setStarting] = useState(false);

  const start = async () => {
    setStarting(true);
    try {
      await getBackend().startDaemon();
    } catch (e) {
      toast.error("Could not start StreamPad", { description: e instanceof Error ? e.message : String(e) });
    } finally {
      setStarting(false);
    }
  };

  const pill = "flex h-7 items-center gap-2 rounded-full border border-line px-2.5 text-sm";
  if (status.kind === "connected") {
    return (
      <div className={pill}>
        <span className="relative flex size-2">
          <span className="absolute inset-0 rounded-full bg-success" />
          {macroMode && <span className="absolute -inset-[3px] rounded-full border border-accent" />}
        </span>
        <span className="text-fg">Running</span>
        {macroMode && <span className="text-accent-text">Macro mode</span>}
      </div>
    );
  }
  if (status.kind === "connecting" || starting) {
    return (
      <div className={cn(pill, "text-muted")}>
        <Spinner size={12} /> Connecting…
      </div>
    );
  }
  return (
    <Tooltip content={status.reason ?? "The background service is not running"} side="bottom">
      <button type="button" onClick={start} className={cn(pill, "text-muted hover:border-line-strong hover:text-fg")}>
        <span className="size-2 rounded-full bg-faint" />
        Not running
        <span className="flex items-center gap-1 font-medium text-accent-text">
          <Play size={12} aria-hidden /> Start
        </span>
      </button>
    </Tooltip>
  );
}
