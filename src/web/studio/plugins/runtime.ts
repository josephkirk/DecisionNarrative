/**
 * Studio plugin runtime.
 *
 * Inspired by DeepSeek Harness / Cordis "everything is a plugin": a plugin is
 * a function applied to a Context. Everything a plugin contributes (UI panels,
 * property editors, graph node types, event listeners, child plugins) is
 * registered through that Context and is removed automatically when the
 * plugin is disposed. The studio core only knows the extension points
 * declared here; it never imports a concrete plugin.
 */
import { markRaw, reactive, type Component } from "vue";
import type { EntityProperty, EntityRecord, EntityTemplateRecord, PropertyDefinition } from "../contracts.ts";
import type { NarrativeEdge, NarrativeGraph, NarrativeNode } from "../narrativeGraphTypes.ts";

export type Disposer = () => void;
export type AnyEntity = EntityRecord | EntityTemplateRecord;

/** UI attached to a whole entity; shown as an extra tab in the entity editor. */
export interface EntityPanelContribution {
  id: string;
  /** Set by the runtime: name of the plugin that registered it. */
  owner?: string;
  label: string;
  /** Return false to hide the panel for this entity. */
  appliesTo(entity: AnyEntity): boolean;
  /** Receives props `{ entity, readonly }`; emits `patch(Partial<entity>)` or `save()` for the entity draft. */
  component: Component;
  order?: number;
}

/** UI attached to one property; replaces the default value editor. */
export interface PropertyEditorContribution {
  id: string;
  /** Set by the runtime: name of the plugin that registered it. */
  owner?: string;
  matches(property: PropertyDefinition | EntityProperty, entity?: AnyEntity): boolean;
  /** Receives props `{ property, entity, modelValue, disabled }`, emits `update:modelValue`. */
  component: Component;
}

/** UI attached to a trait; replaces the default property list in the entity trait inspector. */
export interface TraitInspectorContribution {
  id?: string;
  traitKey: string;
  /** Set by the runtime: name of the plugin that registered it. */
  owner?: string;
  /** Receives props `{ entity, readonly }`; emits `patch(Partial<AnyEntity>)`. */
  component: Component;
}

/** Core nodes a plugin node expands to. The engine only executes core nodes. */
export interface LoweredFragment {
  nodes: NarrativeNode[];
  edges: NarrativeEdge[];
  /** Node receiving the plugin node's incoming flow edge. */
  entry: string;
  /** Node whose `output` port carries the plugin node's outgoing flow. */
  exit: string;
  /** Destinations for an external data input, keyed by authoring port. */
  inputs?: Record<string, Array<{ node: string; port: string }>>;
}

export interface LowerContext {
  /** Unique, deterministic ID for a lowered node. */
  id(suffix: string): string;
  entity(id: string): AnyEntity | undefined;
  graph: NarrativeGraph;
  node: NarrativeNode;
}

/**
 * A graph node contributed by a plugin. It is a pure transform: it reads its
 * config (the input) and returns the core nodes that perform it (the output).
 */
export interface NodeTypeContribution<Config = Record<string, unknown>> {
  type: string;
  label: string;
  /** Set by the runtime: name of the plugin that registered it. */
  owner?: string;
  category?: string;
  color?: string;
  /** Exposes an EntityRef input in the story graph. */
  entityInput?: boolean;
  /** Exposes an additional Item EntityRef input. */
  itemInput?: boolean;
  /** A pure boolean data node has no flow input. */
  booleanOutput?: boolean;
  defaults(): Config;
  /** Short summary rendered on the node body. */
  summary?(config: Config): string;
  validate?(config: Config, ctx: LowerContext): string[];
  lower(config: Config, ctx: LowerContext): LoweredFragment;
  /** Called when the author deletes the node from a graph: clean up data it owns. */
  onDelete?(config: Config, node: NarrativeNode): void | Promise<void>;
  /** Receives props `{ node, config, graph }`, emits `update:config`. */
  inspector?: Component;
}

export interface StudioEvents {
  "entity/saved": (entity: AnyEntity) => void;
  "graph/lowered": (graph: NarrativeGraph) => void;
  /** Workspace rows were (re)loaded from the service. */
  "workspace/loaded": () => void;
  /** A storylet editor opened the stored storylet ("" for a new one). */
  "storylet/opened": (storyletId: string) => void;
  /** The storylet was saved; `graph` is the authoring graph (plugin nodes not lowered). */
  "storylet/saved": (storyletId: string, graph: NarrativeGraph) => void;
  /** The storylet editor closed; unsaved edits are gone. */
  "storylet/closed": (storyletId: string) => void;
}

export interface Plugin {
  name: string;
  /** Shown in the Plugins manager. */
  description?: string;
  version?: string;
  apply(ctx: Context): void | Promise<void>;
}

export function definePlugin(plugin: Plugin): Plugin { return plugin; }

/** Reactive registries every view reads. */
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

  /** Register cleanup that runs when this plugin is disposed. */
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

  /** Load a child plugin; it is disposed together with this context. */
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

/** The root context owned by the studio. */
export const root = new Context("studio");

export function emit<K extends keyof StudioEvents>(event: K, ...args: Parameters<StudioEvents[K]>): void {
  root.emit(event, ...args);
}

export function nodeTypeContribution(type: string | undefined): NodeTypeContribution<any> | undefined {
  return type ? registry.nodeTypes.find((node) => node.type === type) : undefined;
}

/** Tell a plugin that the author deleted one of its nodes. Errors are reported, not thrown. */
export async function pluginNodeDeleted(node: NarrativeNode | undefined): Promise<void> {
  if (node?.type !== "plugin") return;
  const contribution = nodeTypeContribution(node.plugin_type);
  try {
    await contribution?.onDelete?.(node.plugin_config ?? {}, node);
  } catch (error) {
    console.warn(`Plugin '${contribution?.owner}' failed to clean up node ${node.id}:`, error);
  }
}

export function entityPanelsFor(entity: AnyEntity): EntityPanelContribution[] {
  return registry.entityPanels.filter((panel) => panel.appliesTo(entity)).sort((a, b) => (a.order ?? 0) - (b.order ?? 0));
}

export function propertyEditorFor(property: PropertyDefinition | EntityProperty, entity?: AnyEntity): PropertyEditorContribution | undefined {
  return registry.propertyEditors.find((editor) => editor.matches(property, entity));
}

export function traitInspectorFor(traitKey: string | undefined): TraitInspectorContribution | undefined {
  return traitKey ? registry.traitInspectors.find((inspector) => inspector.traitKey === traitKey) : undefined;
}
