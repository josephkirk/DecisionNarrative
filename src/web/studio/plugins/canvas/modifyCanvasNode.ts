import type { NarrativeEdge, NarrativeNode } from "../../narrativeGraphTypes.ts";
import type { NodeTypeContribution } from "../runtime.ts";
import { CANVAS_PROPERTY_KEY, opLabelText, opProperty, parseCanvas, validateOps, type ModifyCanvasConfig } from "./model.ts";

export const MODIFY_CANVAS = "canvas.modify";

function canvasOf(entity: { properties: Array<{ key: string; value?: unknown }> } | undefined) {
  const raw = entity?.properties.find((property) => property.key === CANVAS_PROPERTY_KEY)?.value;
  return raw ? parseCanvas(raw) : null;
}

/** Modify Canvas: config in, one Apply Command per canvas change out. */
export const modifyCanvasNode: Omit<NodeTypeContribution<ModifyCanvasConfig>, "inspector"> = {
  type: MODIFY_CANVAS,
  label: "Modify Canvas",
  category: "World",
  color: "#ff9f0a",
  defaults: () => ({ canvas_entity: "", ops: [] }),
  summary(config) {
    return config.ops.length ? `${config.ops.length} canvas change${config.ops.length === 1 ? "" : "s"}` : "No changes";
  },
  validate(config, lower) {
    return validateOps(config, canvasOf(lower.entity(config.canvas_entity)));
  },
  lower(config, lower) {
    const doc = canvasOf(lower.entity(config.canvas_entity));
    const issues = validateOps(config, doc);
    if (issues.length) throw new Error(issues.join(" "));
    const source = lower.id("entity");
    const nodes: NarrativeNode[] = [{
      id: source, type: "get_entity", title: "Canvas Entity", x: 0, y: 0, width: 200, height: 100, entity_path: config.canvas_entity,
    }];
    const edges: NarrativeEdge[] = [];
    config.ops.forEach((op, index) => {
      const shape = doc!.shapes.find((entry) => entry.id === op.shape_id)!;
      const { key, value, type } = opProperty(op, shape);
      const id = lower.id(`op${index}`);
      nodes.push({ id, type: "apply_command", title: opLabelText(op, shape), x: 0, y: 0, width: 220, height: 120, property_key: key, operation: "set", value_type: type, value });
      edges.push({ id: `${id}__entity`, kind: "data", from_node: source, from_port: "output", to_node: id, to_port: "entity" });
      if (index > 0) {
        const previous = lower.id(`op${index - 1}`);
        edges.push({ id: `${previous}__next`, kind: "flow", from_node: previous, from_port: "output", to_node: id, to_port: "input" });
      }
    });
    return { nodes, edges, entry: lower.id("op0"), exit: lower.id(`op${config.ops.length - 1}`) };
  },
};
