import { X } from "lucide-react";
import { Dialog as D } from "radix-ui";
import { useState, type ReactNode } from "react";
import { create } from "zustand";
import { cn } from "@/lib/cn";
import { Button } from "./Button";
import { IconButton } from "./IconButton";

export const Dialog = D.Root;
export const DialogTrigger = D.Trigger;
export const DialogClose = D.Close;

export interface DialogContentProps {
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  /** Right-aligned action row. */
  footer?: ReactNode;
  size?: "sm" | "md" | "lg";
  /** Hide the × button (e.g. for confirmations that need an explicit choice). */
  hideClose?: boolean;
  className?: string;
  /** Visually hide the title row and render children edge to edge (command palettes). */
  bare?: boolean;
  /** Autofocus target override; by default Radix focuses the first focusable element. */
  onOpenAutoFocus?: (e: Event) => void;
}

const widths = { sm: "w-[400px]", md: "w-[520px]", lg: "w-[720px]" };

export function DialogContent({ title, description, children, footer, size = "md", hideClose, className, bare, onOpenAutoFocus }: DialogContentProps) {
  return (
    <D.Portal>
      <D.Overlay className="fixed inset-0 z-50 bg-(--sp-scrim) data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out" />
      <D.Content
        onOpenAutoFocus={onOpenAutoFocus}
        className={cn(
          "fixed left-1/2 top-1/2 z-50 flex max-h-[min(720px,calc(100vh-48px))] max-w-[calc(100vw-48px)] -translate-x-1/2 -translate-y-1/2 flex-col",
          "rounded-lg bg-panel shadow-dialog outline-none data-[state=open]:animate-dialog-in data-[state=closed]:animate-dialog-out",
          widths[size],
          className,
        )}
      >
        <div className={cn("flex min-w-0 flex-col gap-1 px-5 pb-3 pt-4", !hideClose && "pr-12", bare && "sr-only")}>
          <D.Title className="text-lg font-semibold tracking-[-0.01em] text-fg">{title}</D.Title>
          {description ? (
            <D.Description className="text-base text-muted">{description}</D.Description>
          ) : (
            <D.Description className="sr-only">{title}</D.Description>
          )}
        </div>
        {children && <div className={cn("min-h-0 flex-1", bare ? "flex flex-col overflow-hidden" : "overflow-y-auto px-5 pb-4")}>{children}</div>}
        {footer && <div className="flex items-center justify-end gap-2 border-t border-line px-5 py-3">{footer}</div>}
        {/* Last in DOM order so initial focus lands on the dialog's first control, not on ×. */}
        {!hideClose && (
          <D.Close asChild>
            <IconButton icon={X} label="Close" shortcut="escape" className="absolute right-3.5 top-3.5" />
          </D.Close>
        )}
      </D.Content>
    </D.Portal>
  );
}

// ---------------------------------------------------------------------------------------------------------
// Imperative confirmation: `if (await confirm({ title: "Convert to script?" })) …`. <ConfirmHost/> is
// mounted once in App.tsx.

export interface ConfirmOptions {
  title: string;
  description?: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  /** Red confirm button for irreversible actions. */
  destructive?: boolean;
}

interface PendingConfirm extends ConfirmOptions {
  resolve: (ok: boolean) => void;
}

const useConfirmStore = create<{ pending: PendingConfirm | null }>(() => ({ pending: null }));

export function confirm(options: ConfirmOptions): Promise<boolean> {
  return new Promise((resolve) => {
    useConfirmStore.getState().pending?.resolve(false);
    useConfirmStore.setState({ pending: { ...options, resolve } });
  });
}

export function ConfirmHost() {
  const pending = useConfirmStore((s) => s.pending);
  const [open, setOpen] = useState(false);
  const [shown, setShown] = useState<PendingConfirm | null>(null);
  if (pending && pending !== shown) {
    setShown(pending);
    setOpen(true);
  }
  const finish = (ok: boolean) => {
    shown?.resolve(ok);
    useConfirmStore.setState({ pending: null });
    setOpen(false);
  };
  return (
    <Dialog open={open} onOpenChange={(o) => !o && finish(false)}>
      {shown && (
        <DialogContent
          size="sm"
          hideClose
          title={shown.title}
          description={shown.description}
          footer={
            <>
              <Button variant="ghost" onClick={() => finish(false)}>
                {shown.cancelLabel ?? "Cancel"}
              </Button>
              <Button variant={shown.destructive ? "danger" : "primary"} onClick={() => finish(true)} autoFocus>
                {shown.confirmLabel ?? "Continue"}
              </Button>
            </>
          }
        />
      )}
    </Dialog>
  );
}
