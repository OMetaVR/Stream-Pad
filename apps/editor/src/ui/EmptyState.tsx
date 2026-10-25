import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export interface EmptyStateProps {
  icon?: LucideIcon;
  title: ReactNode;
  description?: ReactNode;
  /** Primary next step (usually a Button). */
  action?: ReactNode;
  size?: "sm" | "md";
  className?: string;
}

/** What to show when there is nothing yet — always pointing at the next action. */
export function EmptyState({ icon: Icon, title, description, action, size = "md", className }: EmptyStateProps) {
  return (
    <div className={cn("flex flex-col items-center justify-center text-center", size === "md" ? "gap-3 p-8" : "gap-2 p-4", className)}>
      {Icon && (
        <div
          className={cn(
            "flex items-center justify-center rounded-md border border-line bg-raised text-muted shadow-raised",
            size === "md" ? "size-10" : "size-8",
          )}
        >
          <Icon size={size === "md" ? 18 : 16} aria-hidden />
        </div>
      )}
      <div className="flex max-w-80 flex-col gap-1">
        <p className={cn("font-medium text-fg", size === "md" ? "text-md" : "text-base")}>{title}</p>
        {description && <p className="text-sm text-muted">{description}</p>}
      </div>
      {action && <div className="mt-1 flex items-center gap-2">{action}</div>}
    </div>
  );
}
