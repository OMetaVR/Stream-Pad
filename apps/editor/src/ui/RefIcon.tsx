import { ImageOff, type LucideIcon } from "lucide-react";
import { useState } from "react";
import { getBackend } from "@/lib/backend";
import { cn } from "@/lib/cn";
import { lucideIcon, parseIconRef } from "@/lib/lucide";
import type { IconRef } from "@/lib/types";

export interface RefIconProps {
  /** "lucide:<name>" (tinted with the text colour) or an AssetRef / absolute path (drawn as-is). */
  iconRef: IconRef | null | undefined;
  size?: number;
  className?: string;
  /** Shown when the icon cannot be found or fails to load. */
  fallback?: LucideIcon;
  /** Accessible name; omit for decorative icons. */
  title?: string;
}

/** Renders any IconRef from the config: lucide names via lucide-react, files via the backend's assetUrl. */
export function RefIcon({ iconRef, size = 16, className, fallback: Fallback = ImageOff, title }: RefIconProps) {
  const [failed, setFailed] = useState<string | null>(null);
  if (!iconRef) return null;
  const parsed = parseIconRef(iconRef);
  const a11y = title ? { role: "img", "aria-label": title } : { "aria-hidden": true as const };
  if (parsed.kind === "lucide") {
    const Icon = lucideIcon(parsed.name) ?? Fallback;
    return <Icon size={size} className={className} {...a11y} />;
  }
  const url = getBackend().assetUrl(iconRef);
  if (!url || failed === iconRef) return <Fallback size={size} className={cn("text-faint", className)} {...a11y} />;
  return (
    <img
      src={url}
      width={size}
      height={size}
      alt={title ?? ""}
      draggable={false}
      onError={() => setFailed(iconRef)}
      className={cn("shrink-0 object-contain", className)}
      style={{ width: size, height: size }}
    />
  );
}
