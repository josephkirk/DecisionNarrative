/**
 * Plugin manager: owns which known plugins are loaded. Disabling a plugin
 * disposes its context, which removes everything it registered. The disabled
 * set is a per-browser preference kept in localStorage.
 */
import { markRaw, reactive } from "vue";
import { registry, root, type Context, type Plugin } from "./runtime.ts";

const STORAGE_KEY = "dn.studio.disabledPlugins";

export interface PluginInfo {
  name: string;
  description: string;
  version: string;
  builtin: boolean;
  enabled: boolean;
  error?: string;
  contributions: { panels: number; propertyEditors: number; nodeTypes: number };
}

interface Entry { plugin: Plugin; builtin: boolean; ctx?: Context; error?: string }

function readDisabled(): string[] {
  try { return JSON.parse(globalThis.localStorage?.getItem(STORAGE_KEY) ?? "[]") as string[]; } catch { return []; }
}
function writeDisabled(names: string[]) {
  try { globalThis.localStorage?.setItem(STORAGE_KEY, JSON.stringify(names)); } catch { /* best effort */ }
}

const state: { entries: Entry[]; disabled: string[] } = reactive({ entries: [], disabled: readDisabled() }) as { entries: Entry[]; disabled: string[] };

function find(name: string) { return state.entries.find((entry) => entry.plugin.name === name); }

function load(entry: Entry) {
  entry.error = undefined;
  try { entry.ctx = markRaw(root.plugin(entry.plugin)); } catch (error) {
    entry.ctx = undefined;
    entry.error = error instanceof Error ? error.message : String(error);
  }
}

function unload(entry: Entry) { entry.ctx?.dispose(); entry.ctx = undefined; }

export const pluginManager = {
  register(plugin: Plugin, options: { builtin?: boolean } = {}) {
    if (find(plugin.name)) return;
    state.entries.push({ plugin, builtin: options.builtin ?? true });
    if (!state.disabled.includes(plugin.name)) load(find(plugin.name)!);
  },
  unregister(name: string) {
    const entry = find(name);
    if (!entry) return;
    unload(entry);
    state.entries.splice(state.entries.indexOf(entry), 1);
  },
  setEnabled(name: string, enabled: boolean) {
    const entry = find(name);
    if (!entry) return;
    state.disabled = state.disabled.filter((n) => n !== name);
    if (enabled) { if (!entry.ctx) load(entry); } else { state.disabled.push(name); unload(entry); }
    writeDisabled(state.disabled);
  },
  reload(name: string) {
    const entry = find(name);
    if (!entry?.ctx) return;
    unload(entry);
    load(entry);
  },
  list(): PluginInfo[] {
    return state.entries.map(({ plugin, builtin, ctx, error }) => ({
      name: plugin.name,
      description: plugin.description ?? "",
      version: plugin.version ?? "",
      builtin,
      enabled: !!ctx,
      error,
      contributions: {
        panels: registry.entityPanels.filter((p) => p.owner === plugin.name).length,
        propertyEditors: registry.propertyEditors.filter((p) => p.owner === plugin.name).length,
        nodeTypes: registry.nodeTypes.filter((n) => n.owner === plugin.name).length,
      },
    }));
  },
};
