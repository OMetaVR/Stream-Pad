// Settings › Overlay: where the on-screen pad appears (a map of the real monitors with 8 position hotspots)
// and how it looks.

import { Eye, MousePointer2 } from "lucide-react";
import { useMemo } from "react";
import { getBackend } from "@/lib/backend";
import { cn } from "@/lib/cn";
import { LIMITS } from "@/lib/defaults";
import type { OutputInfo, OverlaySettings } from "@/lib/types";
import { editor, useEditor } from "@/state/store";
import { Button, Card, ColorPicker, Field, NumberInput, Section, Segmented, Slider, Switch, toast } from "@/ui";
import { useOutputs } from "../macro/data";
import { panelOrigin, panelPath, panelSize } from "./panelShape";

type Spot = { edge: OverlaySettings["edge"]; align: OverlaySettings["align"]; fx: number; fy: number; label: string };

const SPOTS: Spot[] = [
  { edge: "top", align: "start", fx: 0, fy: 0, label: "Top left" },
  { edge: "top", align: "center", fx: 0.5, fy: 0, label: "Top" },
  { edge: "top", align: "end", fx: 1, fy: 0, label: "Top right" },
  { edge: "left", align: "center", fx: 0, fy: 0.5, label: "Left" },
  { edge: "right", align: "center", fx: 1, fy: 0.5, label: "Right" },
  { edge: "bottom", align: "start", fx: 0, fy: 1, label: "Bottom left" },
  { edge: "bottom", align: "center", fx: 0.5, fy: 1, label: "Bottom" },
  { edge: "bottom", align: "end", fx: 1, fy: 1, label: "Bottom right" },
];

const FALLBACK: OutputInfo[] = [{ name: "Screen", x: 0, y: 0, width: 2560, height: 1440, scale: 1, primary: true, model: null }];

const set = (fn: (o: OverlaySettings) => void) => editor.updateSettings((s) => fn(s.overlay));

function isSpot(o: OverlaySettings, s: Spot) {
  if (o.edge !== s.edge) return false;
  if (s.align === "center") return o.align === "center";
  return o.align === s.align;
}

function MonitorMap({ o }: { o: OverlaySettings }) {
  const real = useOutputs([]);
  const outputs = real.length ? real : FALLBACK;
  const follow = o.monitor == null;
  const bounds = useMemo(() => {
    const minX = Math.min(...outputs.map((m) => m.x));
    const minY = Math.min(...outputs.map((m) => m.y));
    const maxX = Math.max(...outputs.map((m) => m.x + m.width / m.scale));
    const maxY = Math.max(...outputs.map((m) => m.y + m.height / m.scale));
    return { minX, minY, w: maxX - minX, h: maxY - minY };
  }, [outputs]);
  const W = 560;
  const k = Math.min(W / bounds.w, 240 / bounds.h);
  const H = bounds.h * k;
  const accent = useEditor((s) => s.config?.settings.overlay.accent);
  const { w: pw, h: ph } = panelSize(o);

  return (
    <div className="flex justify-center rounded-lg border border-line bg-bg p-6">
      <svg width={bounds.w * k} height={H} viewBox={`0 0 ${bounds.w * k} ${H}`} className="overflow-visible">
        {outputs.map((m) => {
          const mx = (m.x - bounds.minX) * k;
          const my = (m.y - bounds.minY) * k;
          const mw = (m.width / m.scale) * k;
          const mh = (m.height / m.scale) * k;
          const active = follow || o.monitor === m.name;
          const origin = panelOrigin(o, m.width / m.scale, m.height / m.scale);
          return (
            <g key={m.name}>
              <rect
                x={mx + 3}
                y={my + 3}
                width={mw - 6}
                height={mh - 6}
                rx={6}
                className={cn("cursor-pointer transition-colors", active ? "fill-(--sp-panel) stroke-(--sp-accent-line)" : "fill-(--sp-panel)/50 stroke-(--sp-line) hover:stroke-(--sp-line-strong)")}
                strokeWidth={1.5}
                onClick={() => set((x) => void (x.monitor = m.name))}
              >
                <title>{`Always show on ${m.name}`}</title>
              </rect>
              <text x={mx + mw / 2} y={my + mh / 2 - 4} textAnchor="middle" className="pointer-events-none fill-(--sp-muted) text-[12px] font-medium">
                {m.name}
              </text>
              <text x={mx + mw / 2} y={my + mh / 2 + 12} textAnchor="middle" className="pointer-events-none fill-(--sp-faint) text-[10px]">
                {m.width}×{m.height}
                {m.primary ? " · primary" : ""}
              </text>
              {active && (
                <g transform={`translate(${mx + 3 + origin.x * k * ((mw - 6) / mw)} ${my + 3 + origin.y * k * ((mh - 6) / mh)}) scale(${k * ((mw - 6) / mw)})`}>
                  <path d={panelPath(o)} fill={o.theme === "light" ? "#f6f6f8" : "#121318"} stroke={accent ?? "var(--sp-accent)"} strokeOpacity={0.9} strokeWidth={2 / k} />
                  <rect x={pw * 0.18} y={ph * 0.12} width={pw * 0.64} height={ph * 0.76} rx={10} fill={accent ?? "var(--sp-accent)"} opacity={0.18} />
                </g>
              )}
              {active &&
                SPOTS.map((s) => {
                  const cx = mx + 3 + s.fx * (mw - 6) + (s.fx === 0 ? 10 : s.fx === 1 ? -10 : 0);
                  const cy = my + 3 + s.fy * (mh - 6) + (s.fy === 0 ? 10 : s.fy === 1 ? -10 : 0);
                  const on = isSpot(o, s);
                  return (
                    <g key={s.label} className="cursor-pointer" onClick={() => set((x) => void ((x.edge = s.edge), (x.align = s.align)))}>
                      <circle cx={cx} cy={cy} r={9} fill="transparent" />
                      <circle cx={cx} cy={cy} r={on ? 5 : 3.5} className={on ? "fill-(--sp-accent)" : "fill-(--sp-line-strong) hover:fill-(--sp-muted)"} />
                      <title>{s.label}</title>
                    </g>
                  );
                })}
            </g>
          );
        })}
      </svg>
    </div>
  );
}

export function OverlaySection() {
  const o = useEditor((s) => s.config!.settings.overlay);
  const outputs = useOutputs([]);
  const preview = async () => {
    try {
      await getBackend().daemonRequest("overlay", { action: "peek", preview: true, durationMs: 3000 });
    } catch (e) {
      toast.error("Couldn't show the overlay", { description: e instanceof Error ? e.message : String(e) });
    }
  };
  return (
    <>
      <Section
        title="Position"
        description="Click a monitor to pin the pad to it, and a dot to choose the spot. The pad slides in from that edge."
        actions={
          <Button size="sm" icon={Eye} onClick={() => void preview()}>
            Preview on screen
          </Button>
        }
      >
        <div className="flex flex-col gap-3">
          <Segmented
            fill
            value={o.monitor == null ? "follow" : "fixed"}
            onValueChange={(v) => set((x) => void (x.monitor = v === "follow" ? null : (outputs.find((m) => m.primary) ?? outputs[0])?.name ?? null))}
            options={[
              { value: "follow", label: "Monitor under the mouse", icon: MousePointer2 },
              { value: "fixed", label: o.monitor ? `Always on ${o.monitor}` : "Always on one monitor" },
            ]}
          />
          <MonitorMap o={o} />
          <Card divided>
            <Field label="Distance from the corner" hint={o.align === "center" ? "Centred along the edge." : "0 = tucked into the corner, flowing into both edges."} inline>
              <NumberInput value={o.offset} min={0} max={2000} step={4} suffix="px" disabled={o.align === "center"} onChange={(v) => set((x) => void (x.offset = v))} />
            </Field>
          </Card>
        </div>
      </Section>

      <Section title="Look">
        <Card divided>
          <Field label="Size" inline>
            <Slider className="w-56" value={o.scale} min={LIMITS.overlayScale.min} max={LIMITS.overlayScale.max} step={0.05} format={(v) => `${Math.round(v * 100)}%`} onValueChange={(v) => set((x) => void (x.scale = v))} />
          </Field>
          <Field label="Theme" inline as="div">
            <Segmented value={o.theme} onValueChange={(v) => set((x) => void (x.theme = v))} options={[{ value: "dark", label: "Dark" }, { value: "light", label: "Light" }]} />
          </Field>
          <Field label="Accent" hint="Profiles with their own colour use that instead." as="div">
            <ColorPicker value={o.accent} onChange={(c) => c && set((x) => void (x.accent = c))} />
          </Field>
          <Field label="Background opacity" inline>
            <Slider className="w-56" value={o.opacity} min={LIMITS.overlayOpacity.min} max={LIMITS.overlayOpacity.max} step={0.01} format={(v) => `${Math.round(v * 100)}%`} onValueChange={(v) => set((x) => void (x.opacity = v))} />
          </Field>
          <Field label="Blur behind" hint="KDE Plasma only." inline>
            <Switch checked={o.blur} onCheckedChange={(v) => set((x) => void (x.blur = v))} />
          </Field>
        </Card>
      </Section>

      <Section title="Keys">
        <Card divided>
          <Field label="Profile header" hint="Profile name and position above the keys." inline>
            <Switch checked={o.showHeader} onCheckedChange={(v) => set((x) => void (x.showHeader = v))} />
          </Field>
          <Field label="Key legends" hint="The printed key (“4 ←”). Shown on keys without a cover too." inline>
            <Switch checked={o.showLegends} onCheckedChange={(v) => set((x) => void (x.showLegends = v))} />
          </Field>
          <Field label="Legend style" inline as="div">
            <Segmented
              value={o.legendStyle}
              onValueChange={(v) => set((x) => void (x.legendStyle = v))}
              options={[{ value: "full", label: "4 ←" }, { value: "number", label: "4" }]}
            />
          </Field>
          <Field label="Action labels" hint="The label under each key's icon." inline>
            <Switch checked={o.showLabels} onCheckedChange={(v) => set((x) => void (x.showLabels = v))} />
          </Field>
        </Card>
      </Section>

      <Section title="Motion">
        <Card divided>
          <Field label="Slide-in speed" inline>
            <Slider className="w-56" value={o.animationMs} min={0} max={800} step={10} format={(v) => (v === 0 ? "Instant" : `${v} ms`)} onValueChange={(v) => set((x) => void (x.animationMs = v))} />
          </Field>
          <Field label="Peek when switching profiles" hint="Shows the pad briefly when you press + or −." inline>
            <Switch checked={o.peekOnProfileChange} onCheckedChange={(v) => set((x) => void (x.peekOnProfileChange = v))} />
          </Field>
          <Field label="Peek when macro mode turns on" inline>
            <Switch checked={o.peekOnModeChange} onCheckedChange={(v) => set((x) => void (x.peekOnModeChange = v))} />
          </Field>
          <Field label="Peek duration" inline>
            <NumberInput value={o.peekMs} min={LIMITS.peekMs.min} max={LIMITS.peekMs.max} step={100} suffix="ms" onChange={(v) => set((x) => void (x.peekMs = v))} />
          </Field>
        </Card>
      </Section>
    </>
  );
}
