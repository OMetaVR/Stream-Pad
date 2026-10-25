import { createRoot } from "react-dom/client";
import { LucideProvider } from "lucide-react";
import { TooltipProvider } from "@/ui";
import { Header } from "@/shell/Header";
import { Sidebar } from "@/features/sidebar/Sidebar";
import { NumpadCanvas } from "@/features/numpad/NumpadCanvas";
import { Inspector } from "@/features/inspector/Inspector";
import { App } from "@/App";
import { flushSync } from "react-dom";

const parts = { Header, Sidebar, NumpadCanvas, Inspector, App } as const;
let root: ReturnType<typeof createRoot> | null = null;
export function render(name: keyof typeof parts) {
  const C = parts[name];
  root?.unmount();
  root = createRoot(document.getElementById("root")!);
  flushSync(() => root!.render(<LucideProvider size={16}><TooltipProvider><C /></TooltipProvider></LucideProvider>));
  return document.getElementById("root")!.innerHTML.length;
}
