import { Check, CircleAlert, RotateCw } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useState } from "react";
import { retrySave } from "@/state/autosave";
import { useEditor } from "@/state/store";
import { Spinner, Tooltip } from "@/ui";

const SAVED_VISIBLE_MS = 2200;

/** Quiet autosave status: "Saving…", a brief "Saved", or a persistent error with Retry. */
export function SaveIndicator() {
  const save = useEditor((s) => s.save);
  const [showSaved, setShowSaved] = useState(false);

  useEffect(() => {
    if (save.status !== "saved") return;
    setShowSaved(true);
    const t = setTimeout(() => setShowSaved(false), SAVED_VISIBLE_MS);
    return () => clearTimeout(t);
  }, [save.status, save.savedAt]);

  const visible = save.status === "saving" || save.status === "error" || (save.status === "saved" && showSaved);
  return (
    <div className="flex h-7 min-w-20 items-center justify-end" aria-live="polite">
      <AnimatePresence mode="wait" initial={false}>
        {visible && (
          <motion.div
            key={save.status}
            initial={{ opacity: 0, y: 2 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, transition: { duration: 0.25 } }}
            transition={{ duration: 0.16 }}
            className="flex items-center gap-1.5 text-sm"
          >
            {save.status === "saving" && (
              <span className="flex items-center gap-1.5 text-faint">
                <Spinner size={12} /> Saving…
              </span>
            )}
            {save.status === "saved" && (
              <span className="flex items-center gap-1.5 text-muted">
                <Check size={13} className="text-success" aria-hidden /> Saved
              </span>
            )}
            {save.status === "error" && (
              <Tooltip content={save.error ?? "Could not save"} side="bottom">
                <button
                  type="button"
                  onClick={() => void retrySave()}
                  className="flex items-center gap-1.5 rounded-sm px-1.5 py-0.5 text-danger hover:bg-danger-soft"
                >
                  <CircleAlert size={13} aria-hidden /> Not saved
                  <RotateCw size={12} aria-hidden className="opacity-70" />
                  <span className="sr-only">Retry saving</span>
                </button>
              </Tooltip>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
