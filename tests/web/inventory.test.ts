import { expect, test } from "bun:test";
import { studioState } from "../../src/web/studio/state.ts";
import type { EntityTemplateRecord } from "../../src/web/studio/contracts.ts";
import type { NarrativeGraph, NarrativeNode } from "../../src/web/studio/narrativeGraphTypes.ts";
import { validateNarrativeGraph } from "../../src/web/studio/narrativeGraphTypes.ts";
import { Context } from "../../src/web/studio/plugins/runtime.ts";
import { lowerPluginGraph, raisePluginGraph, validatePluginNodes } from "../../src/web/studio/plugins/graphLowering.ts";
import { inventoryErrors, inventoryOf, inventoryOwnerTemplate, itemTemplate, setInventory, traitInstance } from "../../src/web/studio/plugins/inventory/model.ts";
import { inventoryNodes } from "../../src/web/studio/plugins/inventory/nodes.ts";

function entity(id: string, trait: string): EntityTemplateRecord {
  return { id, object_id: id, game_id: id, name: id, tags: [], properties: [], traits: [{ id: `${id}-trait`, template_id: trait, values: {} }] };
}
const node = (id: string, type: NarrativeNode["type"], rest: Partial<NarrativeNode> = {}): NarrativeNode =>
  ({ id, type, title: id, x: 0, y: 0, width: 120, height: 100, ...rest });

function graph(operation: "add" | "remove"): NarrativeGraph {
  return {
    nodes: [node("start", "start", { text: "Start" }), node("owner", "get_entity", { entity_path: "owner" }), node("item", "get_entity", { entity_path: "key" }),
      node("modify", "plugin", { plugin_type: `inventory.${operation}`, plugin_config: {} }),
      node("has", "plugin", { plugin_type: "inventory.has", plugin_config: {} }), node("condition", "condition"),
      node("yes", "end", { is_terminal: true }), node("no", "end", { is_terminal: true })],
    edges: [
      { id: "f0", kind: "flow", from_node: "start", from_port: "output", to_node: "modify", to_port: "input" },
      { id: "f1", kind: "flow", from_node: "modify", from_port: "output", to_node: "condition", to_port: "input" },
      { id: "f2", kind: "flow", from_node: "condition", from_port: "true", to_node: "yes", to_port: "input" },
      { id: "f3", kind: "flow", from_node: "condition", from_port: "false", to_node: "no", to_port: "input" },
      ...["modify", "has"].flatMap((target) => [
        { id: `${target}-owner`, kind: "data" as const, from_node: "owner", from_port: "output", to_node: target, to_port: "entity" },
        { id: `${target}-item`, kind: "data" as const, from_node: "item", from_port: "output", to_node: target, to_port: "item" },
      ]),
      { id: "result", kind: "data", from_node: "has", from_port: "output", to_node: "condition", to_port: "input" },
    ],
  };
}

test("inventory dimensions enforce capacity and preserve item references", () => {
  const owner = entity("owner", "inventory_owner");
  const trait = owner.traits[0]!;
  const inventory = inventoryOf(trait);
  expect(inventory).toEqual({ rows: 4, columns: 4, items: [] });
  expect(inventoryErrors({ rows: 1, columns: 1, items: ["key", "coin"] })).toContain("The grid is too small for its items.");
  expect(inventoryErrors({ rows: 1, columns: 2, items: ["key", "key"] })).toContain("Inventory items must be unique, nonempty entity IDs.");
  expect(inventoryErrors({ rows: 0, columns: 2, items: [] }).length).toBe(1);
  const updated = setInventory(owner, trait, { rows: 2, columns: 2, items: ["key"] });
  expect(inventoryOf(updated.traits![0]!)).toEqual({ rows: 2, columns: 2, items: ["key"] });
  expect(traitInstance({ ...owner, ...updated }, [inventoryOwnerTemplate()], "inventory_owner")?.id).toBe(trait.id);
});

test("inventory nodes validate traits and round-trip entity inputs and boolean output", () => {
  const previous = studioState.traitTemplates;
  const ctx = new Context("inventory-test");
  try {
    studioState.traitTemplates = [inventoryOwnerTemplate(), itemTemplate()];
    for (const entry of inventoryNodes) ctx.nodeType(entry);
    const entities = [entity("owner", "inventory_owner"), entity("key", "item")];
    for (const operation of ["add", "remove"] as const) {
      const authored = graph(operation);
      expect(validatePluginNodes(authored, entities)).toEqual([]);
      const lowered = lowerPluginGraph(authored, entities);
      expect(validateNarrativeGraph(lowered)).toEqual([]);
      expect(lowered.nodes.some((n) => n.type === `inventory_${operation}`)).toBe(true);
      expect(lowered.nodes.some((n) => n.type === "inventory_has")).toBe(true);
      const raised = raisePluginGraph(JSON.parse(JSON.stringify(lowered)));
      expect(raised.edges.map((e) => [e.from_node, e.to_node, e.to_port]).sort()).toEqual(authored.edges.map((e) => [e.from_node, e.to_node, e.to_port]).sort());
      const missing = graph(operation); missing.edges = missing.edges.filter((edge) => edge.id !== "modify-item");
      expect(validatePluginNodes(missing, entities).some((issue) => issue.includes("connect Item"))).toBe(true);
      expect(validatePluginNodes(authored, [entities[0]!, entity("key", "other")]).some((issue) => issue.includes("must have the Item trait"))).toBe(true);
      expect(validatePluginNodes(authored, [entity("owner", "other"), entities[1]!]).some((issue) => issue.includes("must have the InventoryOwner trait"))).toBe(true);
    }
  } finally { ctx.dispose(); studioState.traitTemplates = previous; }
});

test("inventory plugin creates only missing traits and never rewrites an existing trait", async () => {
  const { StudioApi } = await import("../../src/web/studio/api.ts");
  const { inventoryPlugin } = await import("../../src/web/studio/plugins/inventory/index.ts");
  const saved: any[] = [];
  const original = { saveContent: StudioApi.saveContent, getProject: StudioApi.getProject, listContent: StudioApi.listContent };
  (StudioApi as any).saveContent = async (_kind: string, payload: any) => { saved.push(payload); return payload; };
  (StudioApi as any).getProject = async () => studioState.project;
  (StudioApi as any).listContent = async () => [];
  const priorProject = studioState.project; const priorTraits = studioState.traitTemplates;
  try {
    studioState.project = { id: "p", name: "P" } as any;
    studioState.traitTemplates = [{ id: "user-item", key: "item", name: "My Item", unique: false, properties: [] } as any];
    const ctx = new Context("inventory-test");
    inventoryPlugin.apply(ctx);
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(saved.map((template) => template.key)).toEqual(["inventory_owner"]);
    expect(saved.some((template) => template.key === "item")).toBe(false);
    ctx.dispose();
  } finally {
    Object.assign(StudioApi, original);
    studioState.project = priorProject; studioState.traitTemplates = priorTraits;
  }
});
