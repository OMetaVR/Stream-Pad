import { Check, ChevronRight, type LucideIcon } from "lucide-react";
import { DropdownMenu as M } from "radix-ui";
import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/cn";
import { Kbd } from "./Kbd";
import { menuContent, menuItem, menuItemDanger, menuLabel, menuSeparator, menuShortcut } from "./menu-styles";

export const DropdownMenu = M.Root;
export const DropdownMenuTrigger = M.Trigger;
export const DropdownMenuGroup = M.Group;
export const DropdownMenuSub = M.Sub;
export const DropdownMenuRadioGroup = M.RadioGroup;

export function DropdownMenuContent({ className, sideOffset = 6, align = "start", ...props }: ComponentProps<typeof M.Content>) {
  return (
    <M.Portal>
      <M.Content sideOffset={sideOffset} align={align} collisionPadding={8} className={cn(menuContent, className)} {...props} />
    </M.Portal>
  );
}

export interface MenuItemProps extends Omit<ComponentProps<typeof M.Item>, "children"> {
  icon?: LucideIcon;
  /** Shortcut hint ("mod+d"). Display only — register the hotkey separately. */
  shortcut?: string;
  destructive?: boolean;
  children: ReactNode;
}

export function DropdownMenuItem({ icon: Icon, shortcut, destructive, className, children, ...props }: MenuItemProps) {
  return (
    <M.Item className={cn(menuItem, destructive && menuItemDanger, className)} {...props}>
      {Icon && <Icon size={15} className={destructive ? "" : "text-muted"} aria-hidden />}
      <span className="flex-1 truncate">{children}</span>
      {shortcut && <Kbd combo={shortcut} className={menuShortcut} />}
    </M.Item>
  );
}

export function DropdownMenuCheckboxItem({ className, children, ...props }: ComponentProps<typeof M.CheckboxItem>) {
  return (
    <M.CheckboxItem className={cn(menuItem, "pl-7", className)} {...props}>
      <M.ItemIndicator className="absolute left-2 text-accent-text">
        <Check size={14} />
      </M.ItemIndicator>
      {children}
    </M.CheckboxItem>
  );
}

export function DropdownMenuRadioItem({ className, children, ...props }: ComponentProps<typeof M.RadioItem>) {
  return (
    <M.RadioItem className={cn(menuItem, "pl-7", className)} {...props}>
      <M.ItemIndicator className="absolute left-2 text-accent-text">
        <Check size={14} />
      </M.ItemIndicator>
      {children}
    </M.RadioItem>
  );
}

export function DropdownMenuLabel({ className, ...props }: ComponentProps<typeof M.Label>) {
  return <M.Label className={cn(menuLabel, className)} {...props} />;
}

export function DropdownMenuSeparator({ className, ...props }: ComponentProps<typeof M.Separator>) {
  return <M.Separator className={cn(menuSeparator, className)} {...props} />;
}

export function DropdownMenuSubTrigger({ icon: Icon, className, children, ...props }: ComponentProps<typeof M.SubTrigger> & { icon?: LucideIcon }) {
  return (
    <M.SubTrigger className={cn(menuItem, className)} {...props}>
      {Icon && <Icon size={15} className="text-muted" aria-hidden />}
      <span className="flex-1 truncate">{children}</span>
      <ChevronRight size={14} className="text-faint" aria-hidden />
    </M.SubTrigger>
  );
}

export function DropdownMenuSubContent({ className, ...props }: ComponentProps<typeof M.SubContent>) {
  return (
    <M.Portal>
      <M.SubContent sideOffset={6} collisionPadding={8} className={cn(menuContent, className)} {...props} />
    </M.Portal>
  );
}
