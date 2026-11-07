import type { NarrativeEdge, NarrativeNode } from "../../narrativeGraphTypes.ts";
import type { NodeTypeContribution } from "../runtime.ts";
import {
  CANVAS_PROPERTY_KEY, CANVAS_VERSION_KEY, nodeVersion, parseCanvas, validateModifyCanvas, versionChanges, type ModifyCanvasConfig,
} from "./model.ts";

export const MODIFY_CANVAS = "canvas.modify";

export function canvasOf(entity: { properties: Array<{ key: string; value?: unknown }> } | undefined) {
  const raw = entity?.properties.find((property) => property.key === CANVAS_PROPERTY_KEY)?.value;
  return raw ? parseCanvas(raw) : null;
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
  defaults: () => ({ canvas_entity: "", version_id: "" }),
  summary(config) {
    if (!config.canvas_entity) return "Choose a canvas";
    return config.version_id ? "Canvas version saved" : "Open to edit the canvas";
  },
  validate(config, lower) {
    return validateModifyCanvas(config, canvasOf(lower.entity(config.canvas_entity)));
  },
  lower(config, lower) {
    const doc = canvasOf(lower.entity(config.canvas_entity));
    const issues = validateModifyCanvas(config, doc);
    if (issues.length) throw new Error(issues.join(" "));
    const version = nodeVersion(doc!, config.version_id)!;
    const source = lower.id("entity");
    const nodes: NarrativeNode[] = [{
      id: source, type: "get_entity", title: "Canvas Entity", x: 0, y: 0, width: 200, height: 100, entity_path: config.canvas_entity,
    }];
    const edges: NarrativeEdge[] = [];
    const commands = [
      { title: `Canvas version: ${version.label}`, key: CANVAS_VERSION_KEY, value: config.version_id as boolean | string, type: "string" as const },
      ...versionChanges(doc!, version.snapshot).map((change) => ({ title: change.text, key: change.key, value: change.value, type: change.type })),
    ];
    commands.forEach((command, index) => {
      const id = lower.id(`op${index}`);
      nodes.push({ id, type: "apply_command", title: command.title, x: 0, y: 0, width: 220, height: 120, property_key: command.key, operation: "set", value_type: command.type, value: command.value });
      edges.push({ id: `${id}__entity`, kind: "data", from_node: source, from_port: "output", to_node: id, to_port: "entity" });
      if (index > 0) {
        const previous = lower.id(`op${index - 1}`);
        edges.push({ id: `${previous}__next`, kind: "flow", from_node: previous, from_port: "output", to_node: id, to_port: "input" });
      }
    });
    return { nodes, edges, entry: lower.id("op0"), exit: lower.id(`op${commands.length - 1}`) };
  },
};
