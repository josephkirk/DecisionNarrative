import { markRaw, reactive, type Component } from "vue";
import type { EntityProperty, EntityRecord, PropertyDefinition } from "../contracts.ts";
import type { NarrativeEdge, NarrativeGraph, NarrativeNode } from "../narrativeGraphTypes.ts";

export type Disposer = () => void;

export interface EntityPanelContribution {
  id: string;
  owner?: string;
  label: string;
  appliesTo(entity: EntityRecord): boolean;
  component: Component;
  order?: number;
}

export interface PropertyEditorContribution {
  id: string;
  owner?: string;
  matches(property: PropertyDefinition | EntityProperty, entity?: EntityRecord): boolean;
  component: Component;
}

export interface TraitInspectorContribution {
  id?: string;
  traitKey: string;
  owner?: string;
  component: Component;
}

export interface LoweredFragment {
  nodes: NarrativeNode[];
  edges: NarrativeEdge[];
  entry: string;
  exit: string;
  inputs?: Record<string, Array<{ node: string; port: string }>>;
}

export interface LowerContext {
  id(suffix: string): string;
  entity(id: string): EntityRecord | undefined;
  graph: NarrativeGraph;
  node: NarrativeNode;
}

export interface NodeTypeContribution<Config = Record<string, unknown>> {
  type: string;
  label: string;
  owner?: string;
  category?: string;
  color?: string;
  entityInput?: boolean;
  itemInput?: boolean;
  booleanOutput?: boolean;
  defaults(): Config;
  summary?(config: Config): string;
  validate?(config: Config, ctx: LowerContext): string[];
  lower(config: Config, ctx: LowerContext): LoweredFragment;
  onDelete?(config: Config, node: NarrativeNode): void | Promise<void>;
  inspector?: Component;
}

export interface StudioEvents {
  "entity/saved": (entity: EntityRecord) => void;
  "graph/lowered": (graph: NarrativeGraph) => void;
  "workspace/loaded": () => void;
  "storylet/opened": (storyletId: string) => void;
  "storylet/saved": (storyletId: string, graph: NarrativeGraph) => void;
  "storylet/closed": (storyletId: string) => void;
}

export interface Plugin {
  name: string;
  description?: string;
  version?: string;
  apply(ctx: Context): void | Promise<void>;
}

export function definePlugin(plugin: Plugin): Plugin { return plugin; }

export interface Registry {
  entityPanels: EntityPanelContribution[];
  propertyEditors: PropertyEditorContribution[];
  traitInspectors: TraitInspectorContribution[];
  nodeTypes: NodeTypeContribution<any>[];
  plugins: string[];
}
export const registry: Registry = reactive<Registry>({ entityPanels: [], propertyEditors: [], traitInspectors: [], nodeTypes: [], plugins: [] }) as Registry;

type Listener = (...args: any[]) => void;
const listeners = new Map<string, Set<Listener>>();

function addTo<T>(list: T[], item: T): Disposer {
  list.push(item);
  return () => { const index = list.indexOf(item); if (index >= 0) list.splice(index, 1); };
}

export class Context {
  private readonly disposers: Disposer[] = [];
  private disposed = false;
  constructor(readonly name: string, readonly parent?: Context) {}

  effect(dispose: Disposer): Disposer {
    if (this.disposed) { dispose(); return () => {}; }
    this.disposers.push(dispose);
    return dispose;
  }

  entityPanel(panel: EntityPanelContribution): Disposer {
    return this.effect(addTo(registry.entityPanels, markRaw({ ...panel, owner: this.name, component: markRaw(panel.component) })));
  }

  propertyEditor(editor: PropertyEditorContribution): Disposer {
    return this.effect(addTo(registry.propertyEditors, markRaw({ ...editor, owner: this.name, component: markRaw(editor.component) })));
  }

  traitInspector(inspector: TraitInspectorContribution): Disposer {
    return this.effect(addTo(registry.traitInspectors, markRaw({ ...inspector, owner: this.name, component: markRaw(inspector.component) })));
  }

  nodeType<C>(node: NodeTypeContribution<C>): Disposer {
    if (registry.nodeTypes.some((existing) => existing.type === node.type)) {
      throw new Error(`Plugin node type '${node.type}' is already registered.`);
    }
    const entry = markRaw({ ...node, owner: this.name, inspector: node.inspector ? markRaw(node.inspector) : undefined });
    return this.effect(addTo(registry.nodeTypes, entry as NodeTypeContribution<any>));
  }

  on<K extends keyof StudioEvents>(event: K, listener: StudioEvents[K]): Disposer {
    const set = listeners.get(event) ?? new Set();
    listeners.set(event, set);
    set.add(listener as Listener);
    return this.effect(() => set.delete(listener as Listener));
  }

  emit<K extends keyof StudioEvents>(event: K, ...args: Parameters<StudioEvents[K]>): void {
    for (const listener of [...(listeners.get(event) ?? [])]) listener(...args);
  }

  plugin(plugin: Plugin): Context {
    const child = new Context(plugin.name, this);
    this.effect(() => child.dispose());
    void loadInto(child, plugin);
    return child;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    while (this.disposers.length) this.disposers.pop()!();
  }
}

async function loadInto(ctx: Context, plugin: Plugin) {
  registry.plugins.push(plugin.name);
  ctx.effect(() => { const index = registry.plugins.indexOf(plugin.name); if (index >= 0) registry.plugins.splice(index, 1); });
  try { await plugin.apply(ctx); } catch (error) { ctx.dispose(); throw error; }
}

export const root = new Context("studio");

export function emit<K extends keyof StudioEvents>(event: K, ...args: Parameters<StudioEvents[K]>): void {
  root.emit(event, ...args);
}

export function nodeTypeContribution(type: string | undefined): NodeTypeContribution<any> | undefined {
  return type ? registry.nodeTypes.find((node) => node.type === type) : undefined;
}

export async function pluginNodeDeleted(node: NarrativeNode | undefined): Promise<void> {
  if (node?.type !== "plugin") return;
  const contribution = nodeTypeContribution(node.plugin_type);
  try {
    await contribution?.onDelete?.(node.plugin_config ?? {}, node);
  } catch (error) {
    console.warn(`Plugin '${contribution?.owner}' failed to clean up node ${node.id}:`, error);
  }
}

export function entityPanelsFor(entity: EntityRecord): EntityPanelContribution[] {
  return registry.entityPanels.filter((panel) => panel.appliesTo(entity)).sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
}

export function propertyEditorFor(property: PropertyDefinition | EntityProperty, entity?: EntityRecord): PropertyEditorContribution | undefined {
  return registry.propertyEditors.find((editor) => editor.matches(property, entity));
}

export function traitInspectorFor(traitKey: string | undefined): TraitInspectorContribution | undefined {
  return traitKey ? registry.traitInspectors.find((inspector) => inspector.traitKey === traitKey) : undefined;
}
