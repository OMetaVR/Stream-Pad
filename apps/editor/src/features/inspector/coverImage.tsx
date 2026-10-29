// Cover rendering shared by the numpad, the inspector and the crop dialog. Matches the overlay
// (apps/overlay/qml/KeyCover.qml): no crop = centred cover fit; a crop's region fills the face.

import { ImageOff } from "lucide-react";
import { useLayoutEffect, useRef, useState } from "react";
import { cn } from "@/lib/cn";
import { KEY_INFO, type CropRect, type KeyId } from "@/lib/types";

/** Face aspect ratio (w / h) of a key on the overlay (key unit 58, gap 8). */
export function keyAspect(key: KeyId): number {
  const g = KEY_INFO[key].grid;
  const span = (n: number) => n * 58 + (n - 1) * 8;
  return span(g.colSpan) / span(g.rowSpan);
}

/** Largest centred crop with the key's aspect ratio — what an uncropped cover shows. */
export function fullCrop(iw: number, ih: number, aspect: number): CropRect {
  const imageAspect = iw / ih;
  if (imageAspect > aspect) {
    const w = aspect / imageAspect;
    return { x: (1 - w) / 2, y: 0, w, h: 1 };
  }
  const h = imageAspect / aspect;
  return { x: 0, y: (1 - h) / 2, w: 1, h };
}

/** Image box (in face pixels) that makes `crop` cover a `fw`×`fh` face. */
export function placement(crop: CropRect, iw: number, ih: number, fw: number, fh: number) {
  const scale = Math.max(fw / (crop.w * iw), fh / (crop.h * ih));
  return {
    left: fw / 2 - (crop.x + crop.w / 2) * iw * scale,
    top: fh / 2 - (crop.y + crop.h / 2) * ih * scale,
    width: iw * scale,
    height: ih * scale,
  };
}

/** Fills its (positioned, sized) parent. */
export function CoverImage({ src, crop, className }: { src: string; crop: CropRect | null; className?: string }) {
  const [natural, setNatural] = useState<{ w: number; h: number } | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [box, setBox] = useState<{ w: number; h: number } | null>(null);
  const host = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const el = host.current;
    if (!el || !crop) return;
    const ro = new ResizeObserver(([e]) => {
      const { width: w, height: h } = e!.contentRect;
      setBox((b) => (b && b.w === w && b.h === h ? b : { w, h }));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [crop]);
  const cropped = crop && natural && box ? placement(crop, natural.w, natural.h, box.w, box.h) : null;
  if (failed === src) {
    return (
      <div className={cn("absolute inset-0 flex flex-col items-center justify-center gap-1 bg-keycap text-faint", className)} title="This image file is missing — choose it again">
        <ImageOff size={18} />
      </div>
    );
  }
  return (
    <div
      className={cn("absolute inset-0 overflow-hidden", className)}
      ref={host}
    >
      <img
        src={src}
        alt=""
        draggable={false}
        onLoad={(e) => setNatural({ w: e.currentTarget.naturalWidth, h: e.currentTarget.naturalHeight })}
        onError={() => setFailed(src)}
        className={cn("absolute max-w-none select-none", !cropped && "inset-0 size-full object-cover")}
        style={cropped ?? undefined}
      />
    </div>
  );
}
