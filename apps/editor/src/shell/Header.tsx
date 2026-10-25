import { ChevronRight, Redo2, Undo2 } from "lucide-react";
import { Fragment } from "react";
import { findProfile } from "@/lib/model";
import { KEY_INFO } from "@/lib/types";
import { editor, useEditor, type View } from "@/state/store";
import { IconButton } from "@/ui";
import { DaemonPill } from "./DaemonPill";
import { SaveIndicator } from "./SaveIndicator";
import { Wordmark } from "./Wordmark";

interface Crumb {
  label: string;
  onClick?: () => void;
}

function useCrumbs(view: View): Crumb[] {
  const profileName = useEditor((s) =>
    (view.kind === "flow" || view.kind === "script") && s.config ? (findProfile(s.config, view.profileId)?.name ?? "") : "",
  );
  switch (view.kind) {
    case "main":
      return [];
    case "settings":
      return [{ label: "Settings" }];
    case "flow":
    case "script":
      return [
        { label: profileName || "Untitled", onClick: editor.showMain },
        { label: KEY_INFO[view.keyId].name, onClick: editor.showMain },
        { label: view.kind === "flow" ? "Flow" : "Script" },
      ];
  }
}

/** Top bar: wordmark, where you are, undo/redo, autosave status, daemon status. */
export function Header() {
  const view = useEditor((s) => s.view);
  const canUndo = useEditor((s) => s.past.length > 0);
  const canRedo = useEditor((s) => s.future.length > 0);
  const crumbs = useCrumbs(view);

  return (
    <header className="flex h-(--sp-header-h) shrink-0 items-center border-b border-line bg-bg pr-3">
      <div className="flex h-full w-(--sp-sidebar-w) shrink-0 items-center px-4">
        <Wordmark />
      </div>
      <nav aria-label="Breadcrumb" className="flex min-w-0 flex-1 items-center gap-1 text-base">
        {crumbs.map((c, i) => (
          <Fragment key={i}>
            {i > 0 && <ChevronRight size={14} className="text-faint" aria-hidden />}
            {c.onClick ? (
              <button type="button" onClick={c.onClick} className="truncate rounded-sm px-1.5 py-0.5 text-muted hover:bg-control hover:text-fg">
                {c.label}
              </button>
            ) : (
              <span className="truncate px-1.5 font-medium text-fg" aria-current="page">
                {c.label}
              </span>
            )}
          </Fragment>
        ))}
      </nav>
      <div className="flex items-center gap-1">
        <SaveIndicator />
        <IconButton icon={Undo2} label="Undo" shortcut="mod+z" tooltipSide="bottom" disabled={!canUndo} onClick={editor.undo} />
        <IconButton icon={Redo2} label="Redo" shortcut="mod+shift+z" tooltipSide="bottom" disabled={!canRedo} onClick={editor.redo} />
        <div className="ml-2">
          <DaemonPill />
        </div>
      </div>
    </header>
  );
}
