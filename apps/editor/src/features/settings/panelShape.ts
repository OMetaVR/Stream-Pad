// SVG outline of the overlay panel (docs/SPEC.md §8 "Shape"): flush with a screen edge, with concave
// fillets flaring into that edge (and into the adjacent edge when it sits in a corner with no offset).

import type { OverlaySettings } from "@/lib/types";

type Pt = [number, number];
type Cmd = { M: Pt } | { L: Pt } | { A: { r: number; sweep: 0 | 1; to: Pt } };

/** Unscaled panel size (key 58, gap 8, padding 14, header 40) — the overlay's geometry. */
export function panelSize(o: Pick<OverlaySettings, "scale" | "showHeader">): { w: number; h: number } {
  const k = 58, g = 8, p = 14;
  const w = 4 * k + 3 * g + 2 * p;
  const h = 5 * k + 4 * g + 2 * p + (o.showHeader ? 40 : 0);
  return { w: w * o.scale, h: h * o.scale };
}

/**
 * Path in panel-local coordinates (0,0 = panel's top-left); fillets extend outside the box by `r`.
 * Built in an "edge frame" where the flush side is at the bottom, then mapped onto the real edge.
 */
export function panelPath(o: Pick<OverlaySettings, "edge" | "align" | "offset" | "scale" | "showHeader">, r = 20, R = 22): string {
  const { w, h } = panelSize(o);
  r *= o.scale;
  R *= o.scale;
  const vertical = o.edge === "left" || o.edge === "right";
  const L = vertical ? h : w; // length along the edge
  const D = vertical ? w : h; // depth away from the edge
  const startFlush = o.align === "start" && o.offset === 0;
  const endFlush = o.align === "end" && o.offset === 0;

  const c: Cmd[] = [];
  if (startFlush) c.push({ M: [0, D] }, { L: [0, -r] }, { A: { r, sweep: 0, to: [r, 0] } });
  else c.push({ M: [-r, D] }, { A: { r, sweep: 0, to: [0, D - r] } }, { L: [0, R] }, { A: { r: R, sweep: 1, to: [R, 0] } });
  if (endFlush) c.push({ L: [L - r, 0] }, { A: { r, sweep: 0, to: [L, -r] } }, { L: [L, D] });
  else c.push({ L: [L - R, 0] }, { A: { r: R, sweep: 1, to: [L, R] } }, { L: [L, D - r] }, { A: { r, sweep: 0, to: [L + r, D] } });

  // edge frame → panel coords; mirrored mappings flip the arc direction
  const map: Record<OverlaySettings["edge"], { f: (p: Pt) => Pt; flip: boolean }> = {
    bottom: { f: ([x, y]) => [x, y], flip: false },
    top: { f: ([x, y]) => [x, D - y], flip: true },
    left: { f: ([x, y]) => [D - y, x], flip: false },
    right: { f: ([x, y]) => [y, x], flip: true },
  };
  const { f, flip } = map[o.edge];
  const fmt = ([x, y]: Pt) => `${+x.toFixed(2)} ${+y.toFixed(2)}`;
  return (
    c
      .map((cmd) => {
        if ("M" in cmd) return `M ${fmt(f(cmd.M))}`;
        if ("L" in cmd) return `L ${fmt(f(cmd.L))}`;
        const sweep = flip ? 1 - cmd.A.sweep : cmd.A.sweep;
        return `A ${cmd.A.r} ${cmd.A.r} 0 0 ${sweep} ${fmt(f(cmd.A.to))}`;
      })
      .join(" ") + " Z"
  );
}

/** Top-left of the panel box on a `sw`×`sh` screen. */
export function panelOrigin(o: OverlaySettings, sw: number, sh: number): { x: number; y: number } {
  const { w, h } = panelSize(o);
  const along = (len: number, size: number) => (o.align === "start" ? o.offset * o.scale : o.align === "end" ? len - size - o.offset * o.scale : (len - size) / 2);
  switch (o.edge) {
    case "bottom":
      return { x: along(sw, w), y: sh - h };
    case "top":
      return { x: along(sw, w), y: 0 };
    case "left":
      return { x: 0, y: along(sh, h) };
    case "right":
      return { x: sw - w, y: along(sh, h) };
  }
}
