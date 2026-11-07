import { expect, test } from "bun:test";
import { Context, registry, nodeTypeContribution, entityPanelsFor, propertyEditorFor, definePlugin } from "../../src/web/studio/plugins/runtime.ts";
import { lowerPluginGraph, raisePluginGraph, validatePluginNodes } from "../../src/web/studio/plugins/graphLowering.ts";
import { modifyCanvasNode } from "../../src/web/studio/plugins/canvas/modifyCanvasNode.ts";
import {
  CANVAS_PROPERTY_KEY, UndoStack, applyOps, emptyCanvas, iconKey, parseCanvas, presentKey, pushRevision, restoreRevision,
  serializeCanvas, syncCanvasProperties, validateOps, type CanvasDoc, type CanvasShape,
} from "../../src/web/studio/plugins/canvas/model.ts";
import { validateNarrativeGraph, type NarrativeGraph } from "../../src/web/studio/narrativeGraphTypes.ts";
import type { EntityRecord } from "../../src/web/studio/contracts.ts";

const Dummy = { render: () => null };

function shape(id: string, kind: CanvasShape["kind"], extra: Partial<CanvasShape> = {}): CanvasShape {
  return { id, layer: "ly_default", kind, x: 0, y: 0, w: 10, h: 10, stroke: "#000", fill: "none", strokeWidth: 1, present: true, ...extra };
}
function mapDoc(): CanvasDoc {
  return { ...emptyCanvas(), shapes: [shape("s_gate", "marker", { icon: "door", label: "Iron Gate" }), shape("s_key", "marker", { icon: "key" }), shape("s_wall", "line", { points: [0, 0, 10, 0] })] };
}
function location(doc = mapDoc()): EntityRecord {
  return { id: "loc_crypt", name: "Crypt", root_tag: "location", tags: [], traits: [], properties: syncCanvasProperties([], doc) };
}

test("plugin contributions are removed when the plugin is disposed", () => {
  const root = new Context("test-root");
  const before = registry.entityPanels.length;
  const child = root.plugin(definePlugin({
    name: "demo",
    apply(ctx) {
      ctx.entityPanel({ id: "demo-panel", label: "Demo", appliesTo: (e) => e.root_tag === "location", component: Dummy });
      ctx.propertyEditor({ id: "demo-prop", matches: (p) => p.key === "demo", component: Dummy });
      ctx.nodeType({ type: "demo.node", label: "Demo", defaults: () => ({}), lower: () => ({ nodes: [], edges: [], entry: "", exit: "" }) });
    },
  }));
  expect(registry.plugins).toContain("demo");
  expect(entityPanelsFor(location()).map((p) => p.id)).toContain("demo-panel");
  expect(entityPanelsFor({ ...location(), root_tag: "item" }).map((p) => p.id)).not.toContain("demo-panel");
  expect(propertyEditorFor({ id: "x", key: "demo", name: "Demo", type: "string" })?.id).toBe("demo-prop");
  expect(nodeTypeContribution("demo.node")?.label).toBe("Demo");
  child.dispose();
  expect(registry.entityPanels.length).toBe(before);
  expect(nodeTypeContribution("demo.node")).toBeUndefined();
  expect(registry.plugins).not.toContain("demo");
});

test("disposing a parent disposes nested plugins and event listeners", () => {
  const root = new Context("outer");
  const seen: string[] = [];
  const parent = root.plugin({ name: "parent", apply(ctx) { ctx.plugin({ name: "nested", apply(inner) { inner.on("entity/saved", (e) => seen.push(e.id)); } }); } });
  root.emit("entity/saved", location());
  parent.dispose();
  root.emit("entity/saved", location());
  expect(seen).toEqual(["loc_crypt"]);
  expect(registry.plugins).not.toContain("nested");
});

test("duplicate node types are rejected", () => {
  const ctx = new Context("dup");
  ctx.nodeType({ type: "dup.node", label: "A", defaults: () => ({}), lower: () => ({ nodes: [], edges: [], entry: "", exit: "" }) });
  expect(() => ctx.nodeType({ type: "dup.node", label: "B", defaults: () => ({}), lower: () => ({ nodes: [], edges: [], entry: "", exit: "" }) })).toThrow();
  ctx.dispose();
});

test("canvas sync declares one boolean per gameplay state and drops stale ones", () => {
  const doc = mapDoc();
  const props = syncCanvasProperties([{ id: "hp", key: "hp", name: "HP", type: "int", value: 3 }, { id: "map_old_present", key: "map_old_present", name: "old", type: "bool" }], doc);
  const keys = props.map((p) => p.key);
  expect(keys).toContain("hp");
  expect(keys).toContain(CANVAS_PROPERTY_KEY);
  expect(keys).toContain(presentKey(doc.shapes[0]!));
  expect(keys).toContain(iconKey(doc.shapes[0]!));
  expect(props.find((p) => p.key === iconKey(doc.shapes[0]!))!.value).toBe("door");
  expect(keys).toContain(presentKey(doc.shapes[1]!));
  expect(keys).not.toContain("map_old_present");
  expect(keys).not.toContain(presentKey(doc.shapes[2]!));
  expect(parseCanvas(props.find((p) => p.key === CANVAS_PROPERTY_KEY)!.value).shapes.length).toBe(3);
});

test("canvas history records revisions, restores them and caps length", () => {
  let doc = pushRevision(mapDoc(), "Draw");
  doc = pushRevision({ ...doc, shapes: [] }, "Clear");
  const restored = restoreRevision(doc, 0);
  expect(restored.shapes.length).toBe(3);
  expect(restored.revisions.length).toBe(2); // restore adds no history
  expect(restored.revisions.at(-1)!.label).toBe("Clear");
  let many = emptyCanvas();
  for (let i = 0; i < 60; i++) many = pushRevision(many, `r${i}`);
  expect(many.revisions.length).toBe(40);
  const undo = new UndoStack();
  undo.record(mapDoc());
  const previous = undo.undo({ layers: [], shapes: [] });
  expect(previous?.shapes.length).toBe(3);
  expect(undo.redo(previous!)?.shapes.length).toBe(0);
  expect(parseCanvas(serializeCanvas(doc)).revisions.length).toBe(2);
  expect(parseCanvas("not json").shapes).toEqual([]);
});

test("modify canvas ops validate and preview", () => {
  const doc = mapDoc();
  expect(validateOps({ canvas_entity: "loc_crypt", ops: [{ shape_id: "s_gate", action: "set_icon", icon: "door-open" }] }, doc)).toEqual([]);
  expect(validateOps({ canvas_entity: "loc_crypt", ops: [{ shape_id: "s_key", action: "set_icon" }] }, doc).length).toBe(1);
  expect(validateOps({ canvas_entity: "loc_crypt", ops: [{ shape_id: "s_wall", action: "remove" }] }, doc).length).toBe(1);
  expect(validateOps({ canvas_entity: "", ops: [] }, doc).length).toBe(1);
  const after = applyOps(doc, [{ shape_id: "s_gate", action: "set_icon", icon: "door-open" }, { shape_id: "s_key", action: "remove" }]);
  expect(after.shapes.find((s) => s.id === "s_gate")!.icon).toBe("door-open");
  expect(after.shapes.find((s) => s.id === "s_key")!.present).toBe(false);
});

function storyGraph(): NarrativeGraph {
  return {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80, text: "Go" },
      { id: "mc", type: "plugin", title: "Unlock", x: 200, y: 0, width: 200, height: 100, plugin_type: "canvas.modify",
        plugin_config: { canvas_entity: "loc_crypt", ops: [{ shape_id: "s_key", action: "remove" }, { shape_id: "s_gate", action: "set_icon", icon: "door-open" }] } },
      { id: "end", type: "end", title: "End", x: 400, y: 0, width: 100, height: 80, is_terminal: true },
    ],
    edges: [
      { id: "e1", kind: "flow", from_node: "start", from_port: "output", to_node: "mc", to_port: "input" },
      { id: "e2", kind: "flow", from_node: "mc", from_port: "output", to_node: "end", to_port: "input" },
    ],
  };
}

test("Modify Canvas lowers to valid core Apply Command nodes and raises back", () => {
  const ctx = new Context("canvas-test");
  if (!nodeTypeContribution("canvas.modify")) ctx.nodeType(modifyCanvasNode);
  const entities = [location()];
  const graph = storyGraph();
  expect(validatePluginNodes(graph, entities)).toEqual([]);
  const lowered = lowerPluginGraph(graph, entities);
  expect(lowered.nodes.some((n) => n.type === "plugin")).toBe(false);
  const commands = lowered.nodes.filter((n) => n.type === "apply_command");
  expect(commands.map((n) => [n.property_key, n.value_type, n.value])).toEqual([["map_s_key_present", "bool", false], ["map_s_gate_icon", "string", "door-open"]]);
  expect(lowered.nodes.find((n) => n.type === "get_entity")!.entity_path).toBe("loc_crypt");
  expect(validateNarrativeGraph(lowered)).toEqual([]);
  // Survives a JSON round trip through storage, as the service returns it.
  const raised = raisePluginGraph(JSON.parse(JSON.stringify(lowered)));
  expect(raised.nodes.map((n) => n.id).sort()).toEqual(["end", "mc", "start"]);
  expect(raised.nodes.find((n) => n.id === "mc")!.plugin_config).toEqual(graph.nodes[1]!.plugin_config);
  expect(raised.edges.map((e) => [e.from_node, e.to_node]).sort()).toEqual([["mc", "end"], ["start", "mc"]]);
  ctx.dispose();
});

test("plugin nodes with missing plugins or bad config are reported", () => {
  const graph = storyGraph();
  graph.nodes[1]!.plugin_type = "nope.missing";
  expect(validatePluginNodes(graph, [location()])[0]).toContain("not loaded");
  const ctx = new Context("canvas-test-2");
  if (!nodeTypeContribution("canvas.modify")) ctx.nodeType(modifyCanvasNode);
  const bad = storyGraph();
  (bad.nodes[1]!.plugin_config as { ops: unknown[] }).ops = [];
  expect(validatePluginNodes(bad, [location()])).toContain("Modify Canvas must apply at least one change.");
  ctx.dispose();
});

test("plugin manager disables and re-enables a plugin", async () => {
  const { pluginManager } = await import("../../src/web/studio/plugins/manager.ts");
  const demo = definePlugin({ name: "managed-demo", description: "d", apply(ctx) {
    ctx.entityPanel({ id: "managed-panel", label: "M", appliesTo: () => true, component: Dummy });
  } });
  pluginManager.register(demo, { builtin: false });
  expect(entityPanelsFor(location()).map((p) => p.id)).toContain("managed-panel");
  expect(pluginManager.list().find((p) => p.name === "managed-demo")!.contributions.panels).toBe(1);
  pluginManager.setEnabled("managed-demo", false);
  expect(entityPanelsFor(location()).map((p) => p.id)).not.toContain("managed-panel");
  expect(pluginManager.list().find((p) => p.name === "managed-demo")!.enabled).toBe(false);
  pluginManager.setEnabled("managed-demo", true);
  expect(entityPanelsFor(location()).map((p) => p.id)).toContain("managed-panel");
  pluginManager.unregister("managed-demo");
  expect(entityPanelsFor(location()).map((p) => p.id)).not.toContain("managed-panel");
});

test("new canvases start with one Default layer; notes are not runtime state", () => {
  const doc = emptyCanvas();
  expect(doc.layers.map((l) => l.name)).toEqual(["Default"]);
  const withNote: CanvasDoc = { ...doc, shapes: [shape("s_note", "note", { text: "todo", fill: "#ffe066" }), shape("s_ref", "storylet", { storylet_id: "unlock" })] };
  const keys = syncCanvasProperties([], withNote).map((p) => p.key);
  expect(keys).not.toContain(presentKey({ id: "s_note" }));
  expect(keys).toContain(presentKey({ id: "s_ref" }));
});
