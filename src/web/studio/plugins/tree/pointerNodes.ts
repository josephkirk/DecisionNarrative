import { studioState } from "../../state.ts";
import type { NarrativeGraph, NarrativeNode } from "../../narrativeGraphTypes.ts";
import type { LowerContext, NodeTypeContribution } from "../runtime.ts";
import { TREE_GRAPH_KEY, treeGraphOf, treeTrait } from "./model.ts";

export const TREE_POSITION_KEY = "__tree_position";
export interface TreePointerConfig { entity_id: string; target_id: string }

export function treeInputEntityId(graph: NarrativeGraph, nodeId: string): string {
  const edges = graph.edges.filter((edge) => edge.kind === "data" && edge.to_node === nodeId && edge.to_port === "entity");
  if (edges.length !== 1 || edges[0]!.from_port !== "output") return "";
  const source = graph.nodes.find((node) => node.id === edges[0]!.from_node);
  return source?.type === "get_entity" ? source.entity_path?.trim() ?? "" : "";
}

function validate(config: TreePointerConfig, ctx: LowerContext): string[] {
  const id = treeInputEntityId(ctx.graph, ctx.node.id);
  const entity = id && ctx.entity(id);
  if (!entity) return ["Connect a configured Get Entity node to the tree input."];
  const trait = treeTrait(entity, studioState.traitTemplates, TREE_GRAPH_KEY);
  if (!trait) return [`${entity.name} must have the TreeGraph trait.`];
  if (config.entity_id !== id) return ["Choose a position for the connected tree."];
  try {
    if (!treeGraphOf(trait).nodes.includes(config.target_id)) return ["Choose a position in the connected tree."];
  } catch (error) { return [`Invalid tree: ${error instanceof Error ? error.message : String(error)}`]; }
  return [];
}

function pointerNode(check: boolean): Omit<NodeTypeContribution<TreePointerConfig>, "inspector"> {
  const label = check ? "Is Tree Position" : "Set Tree Position";
  return {
    type: check ? "tree.is_position" : "tree.set_position", label, category: "World", color: "#06b6d4",
    entityInput: true, booleanOutput: check,
    defaults: () => ({ entity_id: "", target_id: "" }),
    summary: (config) => config.target_id ? "Tree position selected" : "Choose a position",
    validate,
    lower(config, ctx) {
      const problems = validate(config, ctx);
      if (problems.length) throw new Error(problems.join(" "));
      const id = ctx.id("position");
      if (!check) {
        const node: NarrativeNode = { id, type: "apply_command", title: label, x: 0, y: 0, width: 220, height: 120,
          property_key: TREE_POSITION_KEY, operation: "set", value_type: "string", value: config.target_id };
        return { nodes: [node], edges: [], entry: id, exit: id, inputs: { entity: [{ node: id, port: "entity" }] } };
      }
      const compare = ctx.id("compare");
      const nodes: NarrativeNode[] = [
        { id, type: "get_entity_property", title: "Tree position", x: 0, y: 0, width: 220, height: 120, property_key: TREE_POSITION_KEY, value_type: "string" },
        { id: compare, type: "compare", title: label, x: 0, y: 0, width: 220, height: 120, operator: "eq", value_type: "string", value: config.target_id },
      ];
      return { nodes, edges: [{ id: ctx.id("value"), kind: "data", from_node: id, from_port: "output", to_node: compare, to_port: "input" }],
        entry: id, exit: compare, inputs: { entity: [{ node: id, port: "entity" }] } };
    },
  };
}
export const setTreePositionNode = pointerNode(false);
export const isTreePositionNode = pointerNode(true);
