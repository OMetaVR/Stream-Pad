import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export interface SectionProps {
  title: ReactNode;
  description?: ReactNode;
  /** Buttons on the right of the heading. */
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}

/** A titled group of related controls (settings pages, inspector blocks). */
export function Section({ title, description, actions, children, className }: SectionProps) {
  return (
    <section className={cn("flex flex-col gap-3", className)}>
      <header className="flex items-start justify-between gap-4">
        <div className="flex min-w-0 flex-col gap-0.5">
          <h3 className="text-md font-semibold tracking-[-0.005em] text-fg">{title}</h3>
          {description && <p className="text-sm text-muted">{description}</p>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
      </header>
      {children}
    </section>
  );
}

export interface CardProps {
  children: ReactNode;
  padding?: "none" | "sm" | "md";
  /** Hairlines between direct children (settings lists). Children bring their own padding. */
  divided?: boolean;
  className?: string;
}

/** Bordered surface one step above the page. */
export function Card({ children, padding = "md", divided, className }: CardProps) {
  return (
    <div
      className={cn(
        "rounded-lg border border-line bg-panel",
        { none: "", sm: "p-3", md: "p-4" }[padding],
        divided && "divide-y divide-line [&>*]:px-4 [&>*]:py-3",
        className,
      )}
    >
      {children}
    </div>
  );
}
