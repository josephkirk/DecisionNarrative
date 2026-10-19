import type { ComputedRef, InjectionKey } from "vue";
import type { ChronoPlaytestSnapshot, PlaytestSnapshot, ProjectMetadata } from "./contracts.ts";

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
  playtest?: {
    sessionId: string;
    revision: number;
    turn: number;
    locationId: string | null;
  };
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
  codex: "Story Codex",
  tags: "Tag Manager",
  scenarios: "Starting Scenarios",
  playtest: "Playtest",
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
  const queryId = first(route.query.id);
  if (route.name === "codex" && queryId) {
    const category = first(route.query.category);
    const type = category === "characters" ? "character"
      : category === "locations" ? "location"
        : category === "qualities" ? "quality"
          : category === "npc_behaviors" ? "npc_behavior"
            : category === "events" ? "event" : undefined;
    if (type) return { type, id: queryId };
  }
  if (route.name === "scenarios" && queryId) return { type: "scenario", id: queryId };
  const tag = first(route.query.tag);
  if (route.name === "tags" && tag) return { type: "tag", id: tag, label: tag };
  return undefined;
}

/** Resolves route, project, and runtime identity into a local, model-free view of current user context. */
export function buildInteractionContext(input: {
  route: InteractionRoute;
  project: ProjectMetadata | null;
  activePlaytestSessionId: string | null;
  activePlaytestSnapshot: PlaytestSnapshot | null;
  activeChronoPlaytestSessionId?: string | null;
  activeChronoPlaytestSnapshot?: ChronoPlaytestSnapshot | null;
  focus?: InteractionFocus | null;
}): StudioInteractionContext {
  const { route, project, activePlaytestSessionId, activePlaytestSnapshot, activeChronoPlaytestSessionId, activeChronoPlaytestSnapshot, focus } = input;
  const name = typeof route.name === "string" ? route.name : "unknown";
  const entity = focus?.entity === null ? undefined : focus?.entity ?? routeEntity(route);
  const nodeId = first(route.query.node);
  const itemId = first(route.query.item);
  const selection = focus?.selection === null ? undefined : focus?.selection
    ?? (nodeId ? { type: "graph_node" as const, id: nodeId }
      : itemId ? { type: "graph_item" as const, id: itemId } : undefined);
  const chronoSnapshot = activeChronoPlaytestSessionId ? activeChronoPlaytestSnapshot : null;
  const legacySnapshot = activePlaytestSessionId ? activePlaytestSnapshot : null;
  const playtest = chronoSnapshot && activeChronoPlaytestSessionId
    ? {
      sessionId: activeChronoPlaytestSessionId,
      revision: chronoSnapshot.session.pinnedProjectRevision,
      turn: chronoSnapshot.context.turn,
      locationId: chronoSnapshot.context.world.currentLocation,
    }
    : legacySnapshot && activePlaytestSessionId
      ? {
        sessionId: activePlaytestSessionId,
        revision: legacySnapshot.world.revision,
        turn: legacySnapshot.world.turn,
        locationId: legacySnapshot.world.current_location ?? null,
      }
      : null;

  return {
    capturedAt: Date.now(),
    route: { name: viewNames[name] ?? name, path: route.path, fullPath: route.fullPath },
    project,
    ...(entity ? { entity } : {}),
    ...(selection ? { selection } : {}),
    ...(playtest ? { playtest } : {}),
  };
}
