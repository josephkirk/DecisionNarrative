import type { Plugin } from "./runtime.ts";
import { vectorCanvasPlugin } from "./canvas/index.ts";
import { pluginManager } from "./manager.ts";

/** Built-in plugins. Third-party plugins load the same way via `pluginManager.register()`. */
export const BUILTIN_PLUGINS: Plugin[] = [vectorCanvasPlugin];

let installed = false;
export function installBuiltinPlugins(): void {
  if (installed) return;
  installed = true;
  for (const plugin of BUILTIN_PLUGINS) pluginManager.register(plugin, { builtin: true });
}

export { pluginManager, type PluginInfo } from "./manager.ts";
export * from "./runtime.ts";
