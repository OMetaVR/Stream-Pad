// `virtual:lucide-catalog` — every lucide icon name with its search tags, generated at build time from the
// pinned lucide-react (names) and lucide-static (tags) packages, which match the icons the daemon ships.
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import type { Plugin } from "vite";

const ID = "virtual:lucide-catalog";
const RESOLVED = "\0" + ID;

export function lucideCatalog(): Plugin {
  const require = createRequire(import.meta.url);
  return {
    name: "streampad:lucide-catalog",
    resolveId: (id) => (id === ID ? RESOLVED : undefined),
    load(id) {
      if (id !== RESOLVED) return undefined;
      const imports = readFileSync(require.resolve("lucide-react/dist/esm/dynamicIconImports.mjs"), "utf8");
      const names = [...imports.matchAll(/^\s*"([a-z0-9-]+)":\s*\(\)\s*=>/gm)].map((m) => m[1]!);
      const tags = JSON.parse(readFileSync(require.resolve("lucide-static/tags.json"), "utf8")) as Record<string, string[]>;
      const catalog = names.map((name) => [name, (tags[name] ?? []).join(" ")]);
      return `export default ${JSON.stringify(catalog)};`;
    },
  };
}
