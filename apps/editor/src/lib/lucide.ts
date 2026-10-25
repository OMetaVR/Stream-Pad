// IconRef helpers. Lucide refs ("lucide:<kebab-name>") render through lucide-react — pinned to the same
// version (1.61.0) as the lucide-static SVGs the daemon/overlay use, so names always match.

import * as lucide from "lucide-react";
import { icons, type LucideIcon } from "lucide-react";
import type { IconRef } from "./types";

export const LUCIDE_PREFIX = "lucide:";

export type ParsedIconRef = { kind: "lucide"; name: string } | { kind: "asset"; ref: string };

export function parseIconRef(ref: IconRef): ParsedIconRef {
  return ref.startsWith(LUCIDE_PREFIX) ? { kind: "lucide", name: ref.slice(LUCIDE_PREFIX.length) } : { kind: "asset", ref };
}

export function lucideRef(name: string): IconRef {
  return LUCIDE_PREFIX + name;
}

/** "arrow-down-0-1" → "ArrowDown01" (lucide's own export naming). */
function toPascalCase(kebab: string): string {
  return kebab
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("");
}

const iconMap = icons as Record<string, LucideIcon | undefined>;
// `icons` only has canonical names; lucide-static (what the overlay ships) also has the aliases
// ("music", "arrow-down-square"…), which lucide-react exports as named components.
const allExports = lucide as unknown as Record<string, LucideIcon | undefined>;

/** The lucide-react component for a kebab-case name, or null if lucide has no such icon. */
export function lucideIcon(name: string): LucideIcon | null {
  const key = toPascalCase(name);
  return iconMap[key] ?? allExports[key] ?? null;
}

export interface LucideCatalogEntry {
  name: string;
  /** Space-separated keywords for search ("music sound audio …"). */
  tags: string;
}

let catalog: Promise<LucideCatalogEntry[]> | null = null;

/** Every icon name + tags, for icon pickers. Loaded on first use (≈ 150 kB), then cached. */
export function loadLucideCatalog(): Promise<LucideCatalogEntry[]> {
  catalog ??= import("virtual:lucide-catalog").then((m) => m.default.map(([name, tags]) => ({ name, tags })));
  return catalog;
}
