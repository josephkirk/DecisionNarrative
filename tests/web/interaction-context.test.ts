import { describe, expect, test } from "bun:test";
import { buildInteractionContext, type InteractionRoute } from "../../src/web/studio/interactionContext.ts";
import type { PlaytestSnapshot, ProjectMetadata } from "../../src/web/studio/contracts.ts";

const project: ProjectMetadata = { id: "project-1", name: "The Archive", revision: 4 };
const emptyRuntime = { activePlaytestSessionId: null, activePlaytestSnapshot: null };

function route(input: Partial<InteractionRoute>): InteractionRoute {
  return {
    name: "storylets",
    path: "/storylets",
    fullPath: "/storylets",
    params: {},
    query: {},
    ...input,
  };
}

describe("studio interaction context", () => {
  test("captures the active storylet and graph selection from route state", () => {
    const context = buildInteractionContext({
      route: route({ name: "edit-storylet", path: "/storylets/s-1/edit", fullPath: "/storylets/s-1/edit?node=n-1", params: { id: "s-1" }, query: { node: "n-1" } }),
      project,
      ...emptyRuntime,
    });

    expect(context).toMatchObject({
      route: { name: "Storylet Editor", path: "/storylets/s-1/edit" },
      project: { id: "project-1", revision: 4 },
      entity: { type: "storylet", id: "s-1" },
      selection: { type: "graph_node", id: "n-1" },
    });
  });

  test("preserves an explicitly focused unsaved entity and selection", () => {
    const draft = { id: "draft-1", title: "Unsaved title", revision: 3 };
    const selection = { type: "graph_node" as const, id: "node-2", label: "Choice" };
    const context = buildInteractionContext({
      route: route({ name: "edit-storylet", params: { id: "saved-1" } }),
      project,
      ...emptyRuntime,
      focus: { entity: { type: "storylet", id: draft.id, label: draft.title, data: draft }, selection },
    });

    expect(context.entity).toEqual({ type: "storylet", id: "draft-1", label: "Unsaved title", data: draft });
    expect(context.selection).toEqual(selection);
  });

  test("maps Codex categories and excludes empty query values", () => {
    const context = buildInteractionContext({
      route: route({ name: "codex", path: "/codex", fullPath: "/codex?category=npc_behaviors&id=npc-2", query: { category: "npc_behaviors", id: "npc-2", node: null } }),
      project,
      ...emptyRuntime,
    });
    expect(context.entity).toEqual({ type: "npc_behavior", id: "npc-2" });
    expect(context.selection).toBeUndefined();
  });

  test("maps entity and folder routes to their own context entities", () => {
    const entityContext = buildInteractionContext({
      route: route({ name: "entity", path: "/entities/char-2", fullPath: "/entities/char-2", params: { id: "char-2" } }),
      project,
      ...emptyRuntime,
    });
    expect(entityContext.route.name).toBe("Entity Editor");
    expect(entityContext.entity).toEqual({ type: "entity", id: "char-2" });

    const folderContext = buildInteractionContext({
      route: route({ name: "browse", path: "/browse/entities/f-1", fullPath: "/browse/entities/f-1", params: { root: "entities", folderId: "f-1" } }),
      project,
      ...emptyRuntime,
    });
    expect(folderContext.route.name).toBe("Project Browser");
    expect(folderContext.entity).toEqual({ type: "folder", id: "f-1" });
  });

  test("exposes the active playtest revision without leaking runtime state when inactive", () => {
    const snapshot = {
      world: { revision: 9, turn: 3, current_location: "station", events: [] },
    } as PlaytestSnapshot;
    const context = buildInteractionContext({
      route: route({ name: "playtest", path: "/playtest", fullPath: "/playtest" }),
      project,
      activePlaytestSessionId: "session-7",
      activePlaytestSnapshot: snapshot,
    });
    expect(context.playtest).toEqual({ sessionId: "session-7", revision: 9, turn: 3, locationId: "station" });

    const inactive = buildInteractionContext({
      route: route({ name: "storylets" }),
      project,
      activePlaytestSessionId: null,
      activePlaytestSnapshot: snapshot,
    });
    expect(inactive.playtest).toBeUndefined();
  });
});
