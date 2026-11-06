import { root, type Plugin } from "./runtime.ts";
import { vectorCanvasPlugin } from "./canvas/index.ts";

/** Built-in plugins. Third-party plugins load the same way via `root.plugin()`. */
export const BUILTIN_PLUGINS: Plugin[] = [vectorCanvasPlugin];

let installed = false;
export function installBuiltinPlugins(): void {
  if (installed) return;
  installed = true;
  for (const plugin of BUILTIN_PLUGINS) root.plugin(plugin);
}

export * from "./runtime.ts";
