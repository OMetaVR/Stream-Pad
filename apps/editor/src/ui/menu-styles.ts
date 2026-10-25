// Shared class strings for floating surfaces (menus, select lists, popovers) so they look identical.

export const floatingSurface =
  "z-50 rounded-md bg-raised text-fg shadow-popover outline-none " +
  "data-[state=open]:animate-pop-in data-[state=closed]:animate-pop-out";

export const menuContent = `${floatingSurface} min-w-44 overflow-hidden p-1 origin-(--radix-popper-transform-origin)`;

export const menuItem =
  "relative flex min-h-8 cursor-default select-none items-center gap-2 rounded-sm px-2 py-1 text-base text-fg outline-none " +
  "data-[highlighted]:bg-control-hover data-[disabled]:pointer-events-none data-[disabled]:opacity-40 " +
  "data-[state=open]:bg-control-hover";

export const menuItemDanger = "text-danger data-[highlighted]:bg-danger-soft";

export const menuLabel = "px-2 pb-1 pt-1.5 text-xs font-medium text-faint";

export const menuSeparator = "-mx-1 my-1 h-px bg-line";

export const menuShortcut = "ml-auto pl-4 text-sm text-faint";
