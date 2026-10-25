// Browser-mock only (lazy-loaded when backend.isMock): press virtual pad keys, toggle macro mode and
// start/stop the simulated daemon, so key events, overlay state and runs can be exercised without hardware.

import { FlaskConical, Power, RotateCcw, X } from "lucide-react";
import { useState, type PointerEvent } from "react";
import { cn } from "@/lib/cn";
import { mockControls } from "@/lib/backend.mock";
import { KEY_IDS, KEY_INFO, type KeyId } from "@/lib/types";
import { useDaemon, useKeyPress } from "@/state/daemon";
import { Button, IconButton, Switch, Tooltip } from "@/ui";

const UNIT = 34;

function SimKey({ keyId }: { keyId: KeyId }) {
  const { pressed } = useKeyPress(keyId);
  const info = KEY_INFO[keyId];
  const down = (e: PointerEvent<HTMLButtonElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    mockControls()?.pressKey(keyId, true);
  };
  const up = () => mockControls()?.pressKey(keyId, false);
  return (
    <button
      type="button"
      onPointerDown={down}
      onPointerUp={up}
      onPointerCancel={up}
      aria-label={`Press ${info.name}`}
      style={{
        gridColumn: `${info.grid.col + 1} / span ${info.grid.colSpan}`,
        gridRow: `${info.grid.row + 1} / span ${info.grid.rowSpan}`,
      }}
      className={cn(
        "rounded-[6px] bg-keycap text-xs font-semibold text-muted shadow-[inset_0_1px_0_var(--sp-keycap-highlight),0_2px_0_var(--sp-keycap-skirt)]",
        "active:translate-y-px",
        pressed && "bg-accent-soft text-accent-text",
      )}
    >
      {info.legend.main}
    </button>
  );
}

export default function SimulatePanel() {
  const [open, setOpen] = useState(false);
  const running = useDaemon((s) => s.status.kind === "connected");
  const macroMode = useDaemon((s) => s.state?.macroMode ?? false);
  const controls = mockControls();
  if (!controls) return null;

  if (!open) {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="fixed bottom-4 right-4 z-40 flex h-8 items-center gap-2 rounded-full border border-line bg-raised px-3 text-sm text-muted shadow-popover hover:text-fg"
      >
        <FlaskConical size={14} aria-hidden /> Simulate
      </button>
    );
  }

  return (
    <div
      role="region"
      aria-label="Simulate hardware (browser mock)"
      className="fixed bottom-4 right-4 z-40 flex w-[212px] flex-col gap-3 rounded-lg border border-line bg-raised p-3 shadow-popover"
    >
      <div className="flex items-center justify-between">
        <span className="flex items-center gap-1.5 text-sm font-medium text-fg">
          <FlaskConical size={14} aria-hidden /> Simulate
        </span>
        <IconButton icon={X} size="xs" label="Close" onClick={() => setOpen(false)} />
      </div>
      <div className="flex items-center justify-between text-sm text-muted">
        <span className="flex items-center gap-1.5">
          <Power size={13} aria-hidden /> Daemon
        </span>
        <Switch
          size="sm"
          aria-label="Daemon running"
          checked={running}
          onCheckedChange={(on) => (on ? controls.startDaemon() : controls.stopDaemon())}
        />
      </div>
      <Button
        size="sm"
        variant={macroMode ? "primary" : "secondary"}
        disabled={!running}
        onClick={() => controls.pressKey("Toggle", true)}
      >
        Scroll Lock · {macroMode ? "macro mode" : "numpad mode"}
      </Button>
      <div
        className={cn("grid gap-1.5", !running && "pointer-events-none opacity-40")}
        style={{ gridTemplateColumns: `repeat(4, ${UNIT}px)`, gridTemplateRows: `repeat(5, ${UNIT}px)` }}
      >
        {KEY_IDS.map((k) => (
          <SimKey key={k} keyId={k} />
        ))}
      </div>
      <Tooltip content="Replace the stored config with the sample profiles">
        <Button size="sm" variant="ghost" icon={RotateCcw} onClick={() => controls.resetConfig()}>
          Reset sample data
        </Button>
      </Tooltip>
    </div>
  );
}
