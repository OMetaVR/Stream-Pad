// Key cover: drop / choose / link an image or GIF, crop it to the key (non-destructively), or reuse a cover
// from another key.

import { Crop, FolderOpen, ImagePlus, Link2, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import { getBackend } from "@/lib/backend";
import { cn } from "@/lib/cn";
import type { CropRect, KeyId } from "@/lib/types";
import { useEditor } from "@/state/store";
import { Button, IconButton, toast, Tooltip } from "@/ui";
import { CoverImage, keyAspect } from "./coverImage";
import { CropDialog } from "./CropDialog";
import { UrlImport } from "./UrlImport";

const IMAGE = /^image\/(png|jpe?g|webp|gif|svg\+xml)$/;

export async function importDroppedImage(files: FileList | null | undefined, kind: "cover" | "icon" = "cover"): Promise<string | null> {
  const file = [...(files ?? [])].find((f) => IMAGE.test(f.type));
  if (!file) {
    if (files?.length) toast.error("That isn't an image", { description: "Use PNG, JPG, WebP, GIF or SVG." });
    return null;
  }
  try {
    return await getBackend().importCoverFile(file, kind);
  } catch (e) {
    toast.error("Couldn't add the image", { description: e instanceof Error ? e.message : String(e) });
    return null;
  }
}

/** Every cover used anywhere in the config, most recent profiles first, for quick reuse. */
function useUsedCovers(exclude: string | null): string[] {
  const profiles = useEditor((s) => s.config?.profiles);
  return useMemo(() => {
    const seen = new Set<string>();
    for (const p of profiles ?? []) for (const b of Object.values(p.keys)) if (b?.cover && b.cover !== exclude) seen.add(b.cover);
    return [...seen].slice(0, 12);
  }, [profiles, exclude]);
}

interface Props {
  keyId: KeyId;
  value: string | null;
  crop: CropRect | null;
  onChange(cover: string | null, crop: CropRect | null): void;
}

export function CoverPicker({ keyId, value, crop, onChange }: Props) {
  const [over, setOver] = useState(false);
  const [cropping, setCropping] = useState(false);
  const url = value ? getBackend().assetUrl(value) : null;
  const used = useUsedCovers(value);

  /** A new cover starts uncropped and opens the cropper straight away. */
  const setNew = (ref: string) => {
    onChange(ref, null);
    setCropping(true);
  };

  const choose = async () => {
    const path = await getBackend().pickImage();
    if (!path) return;
    try {
      setNew(path.startsWith("data:") ? path : await getBackend().importCover(path));
    } catch (e) {
      toast.error("Couldn't add the image", { description: e instanceof Error ? e.message : String(e) });
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <div
        onDragOver={(e) => {
          if (e.dataTransfer.types.includes("Files")) {
            e.preventDefault();
            setOver(true);
          }
        }}
        onDragLeave={() => setOver(false)}
        onDrop={async (e) => {
          e.preventDefault();
          setOver(false);
          const ref = await importDroppedImage(e.dataTransfer.files);
          if (ref) setNew(ref);
        }}
        className={cn("flex items-center gap-3 rounded-md border border-dashed p-2 transition-quick", over ? "border-accent bg-accent-softer" : "border-line-strong")}
      >
        <button
          type="button"
          disabled={!url}
          onClick={() => setCropping(true)}
          title={url ? "Crop" : undefined}
          className="relative flex size-16 shrink-0 items-center justify-center overflow-hidden rounded-[11px] bg-keycap"
        >
          {url ? <CoverImage src={url} crop={crop} /> : <ImagePlus size={20} className="text-faint" />}
        </button>
        <div className="min-w-0 flex-1">
          <p className="text-sm text-muted">{url ? (crop ? "Cropped · GIFs animate on the pad" : "Fills the key · GIFs animate on the pad") : "Drop an image or GIF here"}</p>
          <div className="mt-1.5 flex flex-wrap gap-1.5">
            <Button size="sm" icon={FolderOpen} onClick={() => void choose()}>
              {url ? "Replace" : "Choose"}
            </Button>
            <UrlImport
              kind="cover"
              onImported={setNew}
              trigger={
                <Button size="sm" icon={Link2}>
                  Link
                </Button>
              }
            />
            {url && (
              <>
                <Button size="sm" icon={Crop} onClick={() => setCropping(true)}>
                  Crop
                </Button>
                <IconButton size="sm" icon={Trash2} label="Remove cover" onClick={() => onChange(null, null)} />
              </>
            )}
          </div>
        </div>
      </div>
      {used.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-xs text-faint">Reuse:</span>
          {used.map((ref) => {
            const u = getBackend().assetUrl(ref);
            return (
              u && (
                <Tooltip key={ref} content="Use this cover">
                  <button type="button" onClick={() => setNew(ref)} className="relative size-8 overflow-hidden rounded-[6px] bg-keycap ring-1 ring-line transition-quick hover:ring-accent">
                    <img src={u} alt="" className="size-full object-cover" />
                  </button>
                </Tooltip>
              )
            );
          })}
        </div>
      )}
      {url && (
        <CropDialog open={cropping} onOpenChange={setCropping} src={url} aspect={keyAspect(keyId)} value={crop} onSave={(c) => onChange(value, c)} />
      )}
    </div>
  );
}
