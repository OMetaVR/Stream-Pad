import { Check, ChevronRight, type LucideIcon } from "lucide-react";
import { ContextMenu as M } from "radix-ui";
import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Kbd } from "./Kbd";
import { menuContent, menuItem, menuItemDanger, menuLabel, menuSeparator, menuShortcut } from "./menu-styles";

export const ContextMenu = M.Root;
export const ContextMenuTrigger = M.Trigger;
export const ContextMenuGroup = M.Group;
export const ContextMenuSub = M.Sub;

export function ContextMenuContent({ className, ...props }: ComponentProps<typeof M.Content>) {
  return (
    <M.Portal>
      <M.Content collisionPadding={8} className={cn(menuContent, className)} {...props} />
    </M.Portal>
  );
}

export interface ContextMenuItemProps extends Omit<ComponentProps<typeof M.Item>, "children"> {
  icon?: LucideIcon;
  shortcut?: string;
  destructive?: boolean;
  children: ReactNode;
}

export function ContextMenuItem({ icon: Icon, shortcut, destructive, className, children, ...props }: ContextMenuItemProps) {
  return (
    <M.Item className={cn(menuItem, destructive && menuItemDanger, className)} {...props}>
      {Icon && <Icon size={15} className={destructive ? "" : "text-muted"} aria-hidden />}
      <span className="flex-1 truncate">{children}</span>
      {shortcut && <Kbd combo={shortcut} className={menuShortcut} />}
    </M.Item>
  );
}

export function ContextMenuCheckboxItem({ className, children, ...props }: ComponentProps<typeof M.CheckboxItem>) {
  return (
    <M.CheckboxItem className={cn(menuItem, "pl-7", className)} {...props}>
      <M.ItemIndicator className="absolute left-2 text-accent-text">
        <Check size={14} />
      </M.ItemIndicator>
      {children}
    </M.CheckboxItem>
  );
}

export function ContextMenuLabel({ className, ...props }: ComponentProps<typeof M.Label>) {
  return <M.Label className={cn(menuLabel, className)} {...props} />;
}

export function ContextMenuSeparator({ className, ...props }: ComponentProps<typeof M.Separator>) {
  return <M.Separator className={cn(menuSeparator, className)} {...props} />;
}

export function ContextMenuSubTrigger({ icon: Icon, className, children, ...props }: ComponentProps<typeof M.SubTrigger> & { icon?: LucideIcon }) {
  return (
    <M.SubTrigger className={cn(menuItem, className)} {...props}>
      {Icon && <Icon size={15} className="text-muted" aria-hidden />}
      <span className="flex-1 truncate">{children}</span>
      <ChevronRight size={14} className="text-faint" aria-hidden />
    </M.SubTrigger>
  );
}

export function ContextMenuSubContent({ className, ...props }: ComponentProps<typeof M.SubContent>) {
  return (
    <M.Portal>
      <M.SubContent collisionPadding={8} className={cn(menuContent, className)} {...props} />
    </M.Portal>
  );
}
