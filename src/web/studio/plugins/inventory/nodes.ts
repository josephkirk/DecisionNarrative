import { studioState } from "../../state.ts";
import type { NodeType, NarrativeNode } from "../../narrativeGraphTypes.ts";
import type { LowerContext, NodeTypeContribution } from "../runtime.ts";
import { INVENTORY_OWNER_KEY, ITEM_KEY, traitInstance } from "./model.ts";

function validateInputs(ctx: LowerContext): string[] {
  const issues: string[] = [];
  for (const [port, key, label] of [["entity", INVENTORY_OWNER_KEY, "InventoryOwner"], ["item", ITEM_KEY, "Item"]] as const) {
    const edges = ctx.graph.edges.filter((edge) => edge.kind === "data" && edge.to_node === ctx.node.id && edge.to_port === port);
    const source = edges.length === 1 && edges[0]!.from_port === "output" ? ctx.graph.nodes.find((node) => node.id === edges[0]!.from_node) : undefined;
    const entity = source?.type === "get_entity" && source.entity_path ? ctx.entity(source.entity_path) : undefined;
    if (!entity) issues.push(`${ctx.node.title}: connect ${label} to a configured Get Entity node.`);
    else if (!traitInstance(entity, studioState.traitTemplates, key)) issues.push(`${ctx.node.title}: ${entity.name} must have the ${label} trait.`);
  }
  return issues;
}

function inventoryNode(type: NodeType, label: string, booleanOutput = false): NodeTypeContribution {
  return {
    type: `inventory.${type.slice("inventory_".length)}`,
    label, category: "World", color: "#f59e0b", entityInput: true, itemInput: true, booleanOutput,
    defaults: () => ({}),
    summary: () => "Upper input: owner · Lower input: item",
    validate: (_config, ctx) => validateInputs(ctx),
    lower(_config, ctx) {
      const issues = validateInputs(ctx);
      if (issues.length) throw new Error(issues.join(" "));
      const id = ctx.id("inventory");
      const node: NarrativeNode = { id, type, title: label, x: 0, y: 0, width: 220, height: 120 };
      return { nodes: [node], edges: [], entry: id, exit: id,
        inputs: { entity: [{ node: id, port: "entity" }], item: [{ node: id, port: "item" }] } };
    },
  };
}

export const inventoryNodes = [
  inventoryNode("inventory_add", "Add Item"),
  inventoryNode("inventory_remove", "Remove Item"),
  inventoryNode("inventory_has", "Has Item", true),
];
