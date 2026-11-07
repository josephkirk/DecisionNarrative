import { expect, test } from "bun:test";
import { Context, registry, nodeTypeContribution, entityPanelsFor, propertyEditorFor, definePlugin } from "../../src/web/studio/plugins/runtime.ts";
import { lowerPluginGraph, raisePluginGraph, validatePluginNodes } from "../../src/web/studio/plugins/graphLowering.ts";
import { modifyCanvasNode } from "../../src/web/studio/plugins/canvas/modifyCanvasNode.ts";
import {
  CANVAS_PROPERTY_KEY, CANVAS_VERSION_KEY, UndoStack, emptyCanvas, iconKey, nodeVersion, parseCanvas, presentKey, pushRevision, refKey,
  restoreRevision, serializeCanvas, snapshotOf, syncCanvasProperties, upsertNodeVersion, validateModifyCanvas, versionChanges,
  type CanvasDoc, type CanvasShape,
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

/** The version a Modify Canvas node saved: key deleted, gate opened, a new ghost marker, wall moved. */
function unlockVersion(doc: CanvasDoc) {
  const shapes = doc.shapes.filter((shape) => shape.id !== "s_key").map((shape) =>
    shape.id === "s_gate" ? { ...shape, icon: "door-open" } : shape.id === "s_wall" ? { ...shape, x: 50 } : shape);
  shapes.push(shape("s_ghost", "marker", { icon: "skull", label: "Ghost" }));
  return { layers: doc.layers, shapes };
}
const SOURCE = { storylet_id: "unlock", storylet_title: "Unlock", node_id: "mc" };
function versionedDoc(): CanvasDoc {
  const doc = mapDoc();
  return upsertNodeVersion(doc, { id: "cv_unlock", label: "Modify Canvas · Unlock", source: SOURCE, snapshot: unlockVersion(doc) });
}

test("a Modify Canvas node owns exactly one version in the canvas history", () => {
  let doc = versionedDoc();
  expect(doc.revisions.length).toBe(1);
  doc = pushRevision(doc, "Edit");
  doc = upsertNodeVersion(doc, { id: "cv_unlock", label: "Modify Canvas · Unlock", source: SOURCE, snapshot: snapshotOf(doc) });
  expect(doc.revisions.filter((r) => r.id === "cv_unlock").length).toBe(1);
  expect(doc.revisions.at(-1)!.id).toBe("cv_unlock");
  // Edit history is capped but node versions are never trimmed.
  for (let i = 0; i < 60; i++) doc = pushRevision(doc, `r${i}`);
  expect(nodeVersion(doc, "cv_unlock")).toBeDefined();
  expect(doc.revisions.filter((r) => !r.source).length).toBe(40);
});

test("version changes are gameplay diffs against the entity canvas", () => {
  const doc = versionedDoc();
  const changes = versionChanges(doc, nodeVersion(doc, "cv_unlock")!.snapshot);
  expect(changes.map((c) => [c.key, c.value])).toEqual([
    ["map_s_gate_icon", "door-open"], ["map_s_key_present", false], ["map_s_ghost_present", true],
  ]);
  // Shapes that exist only in a version are declared on the entity, absent at start.
  const props = syncCanvasProperties([], doc);
  expect(props.find((p) => p.key === "map_s_ghost_present")!.value).toBe(false);
  expect(props.find((p) => p.key === CANVAS_VERSION_KEY)!.value).toBe("");
  expect(validateModifyCanvas({ canvas_entity: "loc_crypt", version_id: "cv_unlock" }, doc)).toEqual([]);
  expect(validateModifyCanvas({ canvas_entity: "loc_crypt", version_id: "" }, doc)[0]).toContain("no canvas version");
  expect(validateModifyCanvas({ canvas_entity: "loc_crypt", version_id: "cv_gone" }, doc)[0]).toContain("missing");
  expect(validateModifyCanvas({ canvas_entity: "", version_id: "" }, doc).length).toBe(1);
});

function storyGraph(): NarrativeGraph {
  return {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80, text: "Go" },
      { id: "mc", type: "plugin", title: "Unlock", x: 200, y: 0, width: 200, height: 100, plugin_type: "canvas.modify",
        plugin_config: { canvas_entity: "loc_crypt", version_id: "cv_unlock" } },
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
  const entities = [location(versionedDoc())];
  const graph = storyGraph();
  expect(validatePluginNodes(graph, entities)).toEqual([]);
  const lowered = lowerPluginGraph(graph, entities);
  expect(lowered.nodes.some((n) => n.type === "plugin")).toBe(false);
  const commands = lowered.nodes.filter((n) => n.type === "apply_command");
  expect(commands.map((n) => [n.property_key, n.value_type, n.value])).toEqual([
    [CANVAS_VERSION_KEY, "string", "cv_unlock"],
    ["map_s_gate_icon", "string", "door-open"], ["map_s_key_present", "bool", false], ["map_s_ghost_present", "bool", true],
  ]);
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
  expect(validatePluginNodes(storyGraph(), [location()])[0]).toContain("missing from the entity's canvas history");
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

test("Canvas panel applies only to entities with the Canvas trait", async () => {
  const { canvasTraitTemplate, hasCanvasTrait } = await import("../../src/web/studio/plugins/canvas/trait.ts");
  const templates = [{ ...canvasTraitTemplate(), id: "tt_canvas" }];
  const plain = location();
  const withTrait = { ...location(), traits: [{ id: "ti1", template_id: "tt_canvas", values: {} }] };
  expect(hasCanvasTrait(plain, templates)).toBe(false);
  expect(hasCanvasTrait(withTrait, templates)).toBe(true);
  expect(hasCanvasTrait(withTrait, [])).toBe(false);
});

test("canvas entity and storylet references become typed reference properties on the holder", () => {
  const doc: CanvasDoc = { ...emptyCanvas(), shapes: [
    shape("s_gem", "entity", { entity_id: "gem", label: "Gem" }),
    shape("s_tale", "storylet", { storylet_id: "crypt_tale" }),
    shape("s_blank", "entity"),
  ] };
  const props = syncCanvasProperties([], doc);
  const gem = props.find((p) => p.key === refKey(doc.shapes[0]!))!;
  expect(gem).toMatchObject({ type: "entity", value: "gem", name: "Canvas reference (Gem)" });
  expect(props.find((p) => p.key === refKey(doc.shapes[1]!))!.name).toBe("Canvas reference");
  expect(props.find((p) => p.key === refKey(doc.shapes[1]!))).toMatchObject({ type: "storylet", value: "crypt_tale" });
  expect(props.find((p) => p.key === refKey(doc.shapes[2]!))).toBeUndefined();
  // Removing the shape drops its reference property.
  const after = syncCanvasProperties(props, { ...doc, shapes: [doc.shapes[1]!] });
  expect(after.find((p) => p.key === refKey(doc.shapes[0]!))).toBeUndefined();
});
