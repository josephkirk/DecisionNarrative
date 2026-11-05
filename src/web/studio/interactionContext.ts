import type { ComputedRef, InjectionKey } from "vue";
import type { ProjectMetadata } from "./contracts.ts";

export interface InteractionRoute {
  name: unknown;
  path: string;
  fullPath: string;
  params: Record<string, unknown>;
  query: Record<string, unknown>;
}

export interface InteractionEntity {
  type: string;
  id: string;
  label?: string;
  data?: unknown;
}

export interface InteractionSelection {
  type: string;
  id: string;
  label?: string;
  data?: unknown;
}

export interface InteractionFocus {
  entity?: InteractionEntity | null;
  selection?: InteractionSelection | null;
}

export interface StudioInteractionContext {
  capturedAt: number;
  route: { name: string; path: string; fullPath: string };
  project: ProjectMetadata | null;
  entity?: InteractionEntity;
  selection?: InteractionSelection;
}

export interface InteractionContextSource {
  context: ComputedRef<StudioInteractionContext>;
  setFocus(focus: InteractionFocus | null): void;
}

export const interactionContextKey: InjectionKey<InteractionContextSource> = Symbol("studio-interaction-context");

const viewNames: Record<string, string> = {
  storylets: "Storylet Library",
  "new-storylet": "New Storylet",
  "edit-storylet": "Storylet Editor",
  browse: "Project Browser",
  "entity-template": "Entity Template Editor",
  "trait-template": "Trait Template Editor",
  asset: "Asset Editor",
  codex: "Rules & Events",
  tags: "Tag Manager",
  project: "Project",
};

function first(value: unknown): string | undefined {
  if (typeof value === "string") return value;
  if (Array.isArray(value)) return value.find((item): item is string => typeof item === "string");
  return undefined;
}

function routeEntity(route: InteractionRoute): InteractionEntity | undefined {
  const id = first(route.params.id);
  if (route.name === "edit-storylet" && id) return { type: "storylet", id };
  if (route.name === "entity-template" && id) return { type: "entity_template", id };
  if (route.name === "trait-template" && id) return { type: "trait_template", id };
  if (route.name === "asset" && id) return { type: "asset", id };
  if (route.name === "browse") {
    const root = first(route.params.root);
    const folderId = first(route.params.folderId);
    if (folderId) return { type: "folder", id: folderId };
    if (root) return { type: "folder_root", id: root };
    return undefined;
  }
  const queryId = first(route.query.id);
  if (route.name === "codex" && queryId && first(route.query.category) === "events") {
    return { type: "event", id: queryId };
  }
  const tag = first(route.query.tag);
  if (route.name === "tags" && tag) return { type: "tag", id: tag, label: tag };
  return undefined;
}

/** Resolves route, project, and runtime identity into a local, model-free view of current user context. */
export function buildInteractionContext(input: {
  route: InteractionRoute;
  project: ProjectMetadata | null;
  focus?: InteractionFocus | null;
}): StudioInteractionContext {
  const { route, project, focus } = input;
  const name = typeof route.name === "string" ? route.name : "unknown";
  const entity = focus?.entity === null ? undefined : focus?.entity ?? routeEntity(route);
  const nodeId = first(route.query.node);
  const itemId = first(route.query.item);
  const selection = focus?.selection === null ? undefined : focus?.selection
    ?? (nodeId ? { type: "graph_node" as const, id: nodeId }
      : itemId ? { type: "graph_item" as const, id: itemId } : undefined);

  return {
    capturedAt: Date.now(),
    route: { name: viewNames[name] ?? name, path: route.path, fullPath: route.fullPath },
    project,
    ...(entity ? { entity } : {}),
    ...(selection ? { selection } : {}),
  };
}
