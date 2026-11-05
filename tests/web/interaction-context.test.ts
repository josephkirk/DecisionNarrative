import { describe, expect, test } from "bun:test";
import { buildInteractionContext, type InteractionRoute } from "../../src/web/studio/interactionContext.ts";
import type { ProjectMetadata } from "../../src/web/studio/contracts.ts";

const project: ProjectMetadata = { id: "project-1", name: "The Archive", revision: 4 };

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
      focus: { entity: { type: "storylet", id: draft.id, label: draft.title, data: draft }, selection },
    });

    expect(context.entity).toEqual({ type: "storylet", id: "draft-1", label: "Unsaved title", data: draft });
    expect(context.selection).toEqual(selection);
  });

  test("maps supported Codex categories and excludes empty query values", () => {
    const context = buildInteractionContext({
      route: route({ name: "codex", path: "/codex", fullPath: "/codex?category=events&id=event-2", query: { category: "events", id: "event-2", node: null } }),
      project,
    });
    expect(context.entity).toEqual({ type: "event", id: "event-2" });
    expect(context.selection).toBeUndefined();
  });

  test("maps entity-template and folder routes to their own context entities", () => {
    const entityContext = buildInteractionContext({
      route: route({ name: "entity-template", path: "/entity-templates/char-2", fullPath: "/entity-templates/char-2", params: { id: "char-2" } }),
      project,
    });
    expect(entityContext.route.name).toBe("Entity Template Editor");
    expect(entityContext.entity).toEqual({ type: "entity_template", id: "char-2" });

    const folderContext = buildInteractionContext({
      route: route({ name: "browse", path: "/browse/entity_templates/f-1", fullPath: "/browse/entity_templates/f-1", params: { root: "entity_templates", folderId: "f-1" } }),
      project,
    });
    expect(folderContext.route.name).toBe("Project Browser");
    expect(folderContext.entity).toEqual({ type: "folder", id: "f-1" });
  });

});
