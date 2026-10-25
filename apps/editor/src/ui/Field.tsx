import { createContext, useContext, useId, type AriaAttributes, type ReactNode } from "react";
import { cn } from "@/lib/cn";

interface FieldContextValue {
  id: string;
  labelId: string;
  describedBy: string | undefined;
  invalid: boolean;
}

const FieldContext = createContext<FieldContextValue | null>(null);

interface ControlAria {
  id?: string;
  "aria-label"?: string;
  "aria-describedby"?: string;
  "aria-invalid"?: AriaAttributes["aria-invalid"];
}

/**
 * ARIA wiring for the control inside a <Field>: id, labelledby (also covers controls a <label> cannot
 * target, like slider thumbs), describedby (hint/error) and invalid. Kit controls call this themselves;
 * use it for custom controls. Explicit props win.
 */
export function useFieldControl(props: ControlAria) {
  const ctx = useContext(FieldContext);
  return {
    id: props.id ?? ctx?.id,
    "aria-labelledby": props["aria-label"] ? undefined : ctx?.labelId,
    "aria-describedby": props["aria-describedby"] ?? ctx?.describedBy,
    "aria-invalid": props["aria-invalid"] ?? (ctx?.invalid || undefined),
  };
}

export interface FieldProps {
  label: ReactNode;
  /** One line of help under the control. Hidden while an error shows. */
  hint?: ReactNode;
  error?: ReactNode;
  /** Right side of the label row (a counter, a reset link…). */
  aside?: ReactNode;
  /** Label left, control right — for switches and compact controls in settings lists. */
  inline?: boolean;
  /** "div" renders the label as plain text (for groups such as Segmented, which use aria-labelledby). */
  as?: "label" | "div";
  className?: string;
  children: ReactNode;
}

export function Field({ label, hint, error, aside, inline, as = "label", className, children }: FieldProps) {
  const id = useId();
  const labelId = `${id}-label`;
  const messageId = `${id}-msg`;
  const message = error || hint;
  const LabelTag = as;
  const labelEl = (
    <LabelTag id={labelId} htmlFor={as === "label" ? id : undefined} className="text-sm font-medium text-fg">
      {label}
    </LabelTag>
  );
  const messageEl = message && (
    <p id={messageId} className={cn("text-sm", error ? "text-danger" : "text-muted")} role={error ? "alert" : undefined}>
      {message}
    </p>
  );
  return (
    <FieldContext.Provider value={{ id, labelId, describedBy: message ? messageId : undefined, invalid: !!error }}>
      {inline ? (
        <div className={cn("flex items-center justify-between gap-6 py-1", className)}>
          <div className="flex min-w-0 flex-col gap-0.5">
            {labelEl}
            {messageEl}
          </div>
          <div className="flex shrink-0 items-center gap-2">{children}</div>
        </div>
      ) : (
        <div className={cn("flex flex-col gap-1.5", className)}>
          <div className="flex min-h-4 items-center justify-between gap-2">
            {labelEl}
            {aside && <div className="text-sm text-muted">{aside}</div>}
          </div>
          {children}
          {messageEl}
        </div>
      )}
    </FieldContext.Provider>
  );
}
