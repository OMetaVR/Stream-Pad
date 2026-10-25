import type { LucideIcon } from "lucide-react";
import { Tabs as T } from "radix-ui";
import { createContext, useContext, type ComponentProps } from "react";
import { cn } from "@/lib/cn";

export const Tabs = T.Root;

type Variant = "underline" | "pill";
const VariantContext = createContext<Variant>("underline");

export function TabsList({ variant = "underline", className, ...props }: ComponentProps<typeof T.List> & { variant?: Variant }) {
  return (
    <VariantContext.Provider value={variant}>
      <T.List
        className={cn(
          "flex items-center",
          variant === "underline" ? "h-10 gap-4 border-b border-line" : "h-8 gap-0.5 rounded-sm bg-bg/60 p-0.5",
          className,
        )}
        {...props}
      />
    </VariantContext.Provider>
  );
}

export function TabsTrigger({ icon: Icon, className, children, ...props }: ComponentProps<typeof T.Trigger> & { icon?: LucideIcon }) {
  const variant = useContext(VariantContext);
  return (
    <T.Trigger
      className={cn(
        "inline-flex items-center gap-1.5 font-medium text-muted transition-quick hover:text-fg disabled:opacity-40 data-[state=active]:text-fg",
        variant === "underline"
          ? "-mb-px h-full border-b-2 border-transparent text-base data-[state=active]:border-accent"
          : "h-full rounded-[5px] px-2.5 text-sm data-[state=active]:bg-control-active data-[state=active]:shadow-raised",
        className,
      )}
      {...props}
    >
      {Icon && <Icon size={15} aria-hidden />}
      {children}
    </T.Trigger>
  );
}

export function TabsContent({ className, ...props }: ComponentProps<typeof T.Content>) {
  return <T.Content className={cn("outline-none", className)} {...props} />;
}
