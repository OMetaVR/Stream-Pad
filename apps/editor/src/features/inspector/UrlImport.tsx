// "From a link…": paste an image URL; the backend downloads it into the data dir.

import { Link2 } from "lucide-react";
import { useState, type ReactNode } from "react";
import { getBackend, type AssetKind } from "@/lib/backend";
import { Button, Input, Popover, PopoverContent, PopoverTrigger } from "@/ui";

export function UrlImport({ kind, onImported, trigger }: { kind: AssetKind; onImported(ref: string): void; trigger: ReactNode }) {
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      onImported(await getBackend().importImageUrl(url.trim(), kind));
      setOpen(false);
      setUrl("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        setOpen(o);
        setError(null);
      }}
    >
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent className="w-80" align="start">
        <form
          className="flex flex-col gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (url.trim()) void submit();
          }}
        >
          <span className="text-sm font-medium">{kind === "icon" ? "Icon from a link" : "Cover from a link"}</span>
          <Input autoFocus icon={Link2} mono placeholder="https://…/image.gif" value={url} onChange={(e) => setUrl(e.currentTarget.value)} />
          {error && <p className="text-sm text-danger">{error}</p>}
          <p className="text-xs text-faint">PNG, JPEG, WebP, GIF or SVG. It's downloaded once and kept with your profiles.</p>
          <Button type="submit" variant="primary" size="sm" loading={busy} disabled={!url.trim()} className="self-end">
            Add
          </Button>
        </form>
      </PopoverContent>
    </Popover>
  );
}
