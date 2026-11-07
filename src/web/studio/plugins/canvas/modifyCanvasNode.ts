import type { NarrativeEdge, NarrativeGraph, NarrativeNode } from "../../narrativeGraphTypes.ts";
import type { NodeTypeContribution } from "../runtime.ts";
import {
  CANVAS_PROPERTY_KEY, CANVAS_VERSION_KEY, nodeVersion, parseCanvas, validateModifyCanvas, versionChanges, versionSnapshot, type ModifyCanvasConfig,
} from "./model.ts";

export const MODIFY_CANVAS = "canvas.modify";

export function canvasOf(entity: { properties: Array<{ key: string; value?: unknown }> } | undefined) {
  const raw = entity?.properties.find((property) => property.key === CANVAS_PROPERTY_KEY)?.value;
  return raw ? parseCanvas(raw) : null;
}

/** The editable canvas must be identifiable from a directly connected Get Entity node. */
export function canvasInputEntityId(graph: NarrativeGraph, nodeId: string): string {
  const edges = graph.edges.filter((edge) => edge.kind === "data" && edge.to_node === nodeId && edge.to_port === "entity");
  if (edges.length !== 1 || edges[0]!.from_port !== "output") return "";
  const source = graph.nodes.find((node) => node.id === edges[0]!.from_node);
  return source?.type === "get_entity" ? source.entity_path?.trim() ?? "" : "";
}

/**
 * Modify Canvas: the node owns one version of an entity's canvas. Lowered to
 * Apply Command nodes: one switching the entity's active canvas version, then
 * one per gameplay state the version changes (presence, marker icon).
 */
export const modifyCanvasNode: Omit<NodeTypeContribution<ModifyCanvasConfig>, "inspector"> = {
  type: MODIFY_CANVAS,
  label: "Modify Canvas",
  category: "World",
  color: "#ff9f0a",
  entityInput: true,
  defaults: () => ({ canvas_entity: "", version_id: "" }),
  summary(config) {
    if (!config.canvas_entity) return "Connect an entity";
    return config.version_id ? "Canvas version saved" : "Open to edit the canvas";
  },
  validate(config, lower) {
    const entityId = canvasInputEntityId(lower.graph, lower.node.id);
    if (!entityId) return ["Modify Canvas requires an Entity input connected to a configured Get Entity node."];
    if (config.canvas_entity && config.canvas_entity !== entityId) return ["Modify Canvas entity input changed; open the canvas and save a version for the connected entity."];
    return validateModifyCanvas(config, canvasOf(lower.entity(entityId)));
  },
  lower(config, lower) {
    const entityId = canvasInputEntityId(lower.graph, lower.node.id);
    if (!entityId || config.canvas_entity !== entityId) throw new Error("Modify Canvas requires a version for its connected entity.");
    const doc = canvasOf(lower.entity(entityId));
    const issues = validateModifyCanvas(config, doc);
    if (issues.length) throw new Error(issues.join(" "));
    const version = nodeVersion(doc!, config.version_id)!;
    const nodes: NarrativeNode[] = [];
    const edges: NarrativeEdge[] = [];
    const commands = [
      { title: `Canvas version: ${version.label}`, key: CANVAS_VERSION_KEY, value: config.version_id as boolean | string, type: "string" as const },
      ...versionChanges(doc!, versionSnapshot(version)).map((change) => ({ title: change.text, key: change.key, value: change.value, type: change.type })),
    ];
    commands.forEach((command, index) => {
      const id = lower.id(`op${index}`);
      nodes.push({ id, type: "apply_command", title: command.title, x: 0, y: 0, width: 220, height: 120, property_key: command.key, operation: "set", value_type: command.type, value: command.value });
      if (index > 0) {
        const previous = lower.id(`op${index - 1}`);
        edges.push({ id: `${previous}__next`, kind: "flow", from_node: previous, from_port: "output", to_node: id, to_port: "input" });
      }
    });
    return { nodes, edges, entry: lower.id("op0"), exit: lower.id(`op${commands.length - 1}`), inputs: { entity: commands.map((_, index) => ({ node: lower.id(`op${index}`), port: "entity" })) } };
  },
};
