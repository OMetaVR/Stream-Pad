import { FileWarning, FolderOpen, RotateCw } from "lucide-react";
import type { ReactNode } from "react";
import { getBackend } from "@/lib/backend";
import { editor, useEditor } from "@/state/store";
import { Button, EmptyState, Spinner } from "@/ui";

/** Renders children only once config.json is loaded; otherwise a spinner or a recoverable error. */
export function LoadGate({ children }: { children: ReactNode }) {
  const load = useEditor((s) => s.load);
  if (load.kind === "ready") return <>{children}</>;
  if (load.kind === "loading") {
    return (
      <div className="flex flex-1 items-center justify-center text-muted">
        <Spinner size={20} label="Loading your profiles" />
      </div>
    );
  }
  const openFolder = async () => {
    const path = await getBackend().configPath();
    await getBackend().openPath(path.slice(0, path.lastIndexOf("/")) || path);
  };
  return (
    <div className="flex flex-1 items-center justify-center">
      <EmptyState
        icon={FileWarning}
        title="config.json could not be read"
        description={
          <>
            <span className="font-code text-fg selectable">{load.message}</span>
            <br />
            Fix the file (StreamPad keeps running with the last good version) and try again. Nothing was overwritten.
          </>
        }
        action={
          <>
            <Button icon={FolderOpen} onClick={() => void openFolder()}>
              Open config folder
            </Button>
            <Button variant="primary" icon={RotateCw} onClick={() => void editor.load()}>
              Try again
            </Button>
          </>
        }
      />
    </div>
  );
}
