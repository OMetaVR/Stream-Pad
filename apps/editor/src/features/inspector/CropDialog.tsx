// Crop a key cover. Always works on the original image with a frame locked to the key's shape, so a crop can
// be tightened or widened again at any time. Drag the frame to move it, a corner to resize, scroll to zoom.

import { RotateCcw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { CropRect } from "@/lib/types";
import { Button, Dialog, DialogContent, Slider } from "@/ui";
import { CoverImage, fullCrop } from "./coverImage";

const STAGE_W = 560;
const STAGE_H = 380;
const MIN_FRACTION = 0.125;

interface Props {
  open: boolean;
  onOpenChange(open: boolean): void;
  src: string;
  /** Key face aspect ratio (w / h). */
  aspect: number;
  value: CropRect | null;
  onSave(crop: CropRect | null): void;
  title?: string;
}

type Drag = { mode: "move" | "nw" | "ne" | "sw" | "se"; startX: number; startY: number; start: CropRect };

export function CropDialog({ open, onOpenChange, src, aspect, value, onSave, title = "Crop cover" }: Props) {
  const [natural, setNatural] = useState<{ w: number; h: number } | null>(null);
  const [crop, setCrop] = useState<CropRect | null>(value);
  const drag = useRef<Drag | null>(null);

  useEffect(() => {
    if (!open) return;
    setNatural(null);
    setCrop(value);
    const img = new Image();
    img.onload = () => setNatural({ w: img.naturalWidth, h: img.naturalHeight });
    img.src = src;
  }, [open, src, value]);

  if (!natural) {
    return (
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent title={title} size="lg">
          <div className="flex h-[380px] items-center justify-center text-sm text-faint">Loading image…</div>
        </DialogContent>
      </Dialog>
    );
  }

  const full = fullCrop(natural.w, natural.h, aspect);
  const rect = crop ?? full;
  // Stage: the whole image, fitted.
  const fit = Math.min(STAGE_W / natural.w, STAGE_H / natural.h);
  const dw = natural.w * fit;
  const dh = natural.h * fit;
  const zoom = full.w / rect.w; // 1 = uncropped

  const clamp = (r: CropRect): CropRect => {
    const w = Math.min(full.w, Math.max(full.w * MIN_FRACTION, r.w));
    const h = (w / full.w) * full.h;
    return { w, h, x: Math.min(1 - w, Math.max(0, r.x)), y: Math.min(1 - h, Math.max(0, r.y)) };
  };
  const zoomTo = (z: number, around = { x: rect.x + rect.w / 2, y: rect.y + rect.h / 2 }) => {
    const w = full.w / Math.max(1, z);
    const h = full.h / Math.max(1, z);
    setCrop(clamp({ w, h, x: around.x - w / 2, y: around.y - h / 2 }));
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const dx = (e.clientX - d.startX) / dw;
    const dy = (e.clientY - d.startY) / dh;
    const s = d.start;
    if (d.mode === "move") {
      setCrop(clamp({ ...s, x: s.x + dx, y: s.y + dy }));
      return;
    }
    // Resize from a corner keeping the aspect; the opposite corner stays put.
    const east = d.mode.includes("e");
    const south = d.mode.includes("s");
    const byX = (east ? dx : -dx) / s.w;
    const byY = (south ? dy : -dy) / s.h;
    const k = Math.max(byX, byY);
    let w = Math.min(full.w, Math.max(full.w * MIN_FRACTION, s.w * (1 + k)));
    let h = (w / full.w) * full.h;
    let x = east ? s.x : s.x + s.w - w;
    let y = south ? s.y : s.y + s.h - h;
    if (x < 0 || y < 0 || x + w > 1 || y + h > 1) {
      // Shrink to fit inside the image from the fixed corner.
      const maxW = east ? 1 - s.x : s.x + s.w;
      const maxH = south ? 1 - s.y : s.y + s.h;
      w = Math.min(w, maxW, (maxH / full.h) * full.w);
      h = (w / full.w) * full.h;
      x = east ? s.x : s.x + s.w - w;
      y = south ? s.y : s.y + s.h - h;
    }
    setCrop({ x, y, w, h });
  };

  const handle = (mode: Drag["mode"], cls: string) => (
    <span
      onPointerDown={(e) => {
        e.stopPropagation();
        e.currentTarget.setPointerCapture(e.pointerId);
        drag.current = { mode, startX: e.clientX, startY: e.clientY, start: rect };
      }}
      className={`absolute size-3.5 rounded-full border-2 border-white bg-accent shadow ${cls}`}
      style={{ cursor: mode === "nw" || mode === "se" ? "nwse-resize" : "nesw-resize" }}
    />
  );

  const pw = 120;
  const ph = pw / aspect;
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        title={title}
        description="Drag the frame or its corners. Scroll to zoom."
        size="lg"
        footer={
          <>
            <Button variant="ghost" icon={RotateCcw} onClick={() => setCrop(null)} disabled={!crop} className="mr-auto">
              Show the whole image
            </Button>
            <Button onClick={() => onOpenChange(false)}>Cancel</Button>
            <Button
              variant="primary"
              onClick={() => {
                const near = (a: number, b: number) => Math.abs(a - b) < 0.002;
                const whole = near(rect.w, full.w) && near(rect.x, full.x) && near(rect.y, full.y);
                onSave(whole ? null : rect);
                onOpenChange(false);
              }}
            >
              Save
            </Button>
          </>
        }
      >
        <div className="flex gap-5">
          <div
            className="relative flex shrink-0 items-center justify-center overflow-hidden rounded-md bg-[repeating-conic-gradient(var(--sp-control)_0_25%,var(--sp-raised)_0_50%)] bg-[length:16px_16px]"
            style={{ width: STAGE_W, height: STAGE_H }}
            onWheel={(e) => zoomTo(zoom * (e.deltaY < 0 ? 1.1 : 1 / 1.1))}
          >
            <div className="relative touch-none" style={{ width: dw, height: dh }} onPointerMove={onPointerMove} onPointerUp={() => (drag.current = null)}>
              <img src={src} alt="" draggable={false} className="absolute inset-0 size-full select-none" />
              <div
                onPointerDown={(e) => {
                  e.currentTarget.setPointerCapture(e.pointerId);
                  drag.current = { mode: "move", startX: e.clientX, startY: e.clientY, start: rect };
                }}
                className="absolute cursor-move rounded-[6px] outline-2 outline-white/90"
                style={{
                  left: rect.x * dw,
                  top: rect.y * dh,
                  width: rect.w * dw,
                  height: rect.h * dh,
                  boxShadow: "0 0 0 9999px rgba(0,0,0,0.55)",
                }}
              >
                {handle("nw", "-left-1.5 -top-1.5")}
                {handle("ne", "-right-1.5 -top-1.5")}
                {handle("sw", "-bottom-1.5 -left-1.5")}
                {handle("se", "-bottom-1.5 -right-1.5")}
              </div>
            </div>
          </div>
          <div className="flex min-w-0 flex-1 flex-col gap-4">
            <div className="flex flex-col gap-2">
              <span className="text-sm font-medium text-muted">On the key</span>
              <div className="relative overflow-hidden rounded-[11px] bg-keycap shadow-[inset_0_1px_0_var(--sp-keycap-highlight),0_3px_0_var(--sp-keycap-skirt)]" style={{ width: pw, height: ph }}>
                <CoverImage src={src} crop={rect} />
              </div>
            </div>
            <div className="flex flex-col gap-2">
              <span className="text-sm font-medium text-muted">Zoom</span>
              <Slider value={zoom} min={1} max={1 / MIN_FRACTION} step={0.05} format={(v) => `${v.toFixed(1)}×`} onValueChange={(v) => zoomTo(v)} />
            </div>
            <p className="text-xs text-faint">
              The original image is kept, so you can crop more — or less — any time. {natural.w}×{natural.h}px
            </p>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
