import { expect, test } from "bun:test";
import type { TraitInstance } from "../../src/web/studio/contracts.ts";
import type { AnyEntity } from "../../src/web/studio/plugins/runtime.ts";
import { TREE_EXAMPLES } from "../../src/web/studio/plugins/tree/examples.ts";
import { CARD_HEIGHT, CARD_WIDTH, layoutTree, setTreeGraph, treeErrors, treeGraphOf, treeRoots, type TreeDirection, type TreeGraph } from "../../src/web/studio/plugins/tree/model.ts";

const graph: TreeGraph = {
  nodes: ["start-a", "start-b", "shared", "end"],
  edges: [{ from: "start-a", to: "shared" }, { from: "start-b", to: "shared" }, { from: "shared", to: "end" }],
  direction: "right",
};

test("tree topology survives encoding and view changes", () => {
  const trait: TraitInstance = { id: "graph", template_id: "tree_graph", values: {} };
  const entity = { id: "owner", name: "Owner", traits: [trait] } as AnyEntity;
  const patch = setTreeGraph(entity, trait, { ...graph, direction: "radial" });
  const restored = treeGraphOf(patch.traits![0]!);
  expect(treeRoots(restored)).toEqual(["start-a", "start-b"]);
  expect(restored).toEqual({ ...graph, direction: "radial", positions: {} });
});

test("saved positions survive round-trip independently by direction", () => {
  const trait: TraitInstance = { id: "graph", template_id: "tree_graph", values: {} };
  const entity = { id: "owner", name: "Owner", traits: [trait] } as AnyEntity;
  const saved = { ...graph, positions: { right: { shared: { x: 510, y: 220 } }, radial: { shared: { x: 760, y: 340 } } } };
  const restored = treeGraphOf(setTreeGraph(entity, trait, saved).traits![0]!);
  expect(layoutTree(restored).positions.shared).toEqual({ x: 510, y: 220 });
  expect(layoutTree({ ...restored, direction: "radial" }).positions.shared).toEqual({ x: 760, y: 340 });
  expect(() => treeGraphOf({ ...trait, values: { nodes: ["shared"], positions: JSON.stringify({ right: { missing: { x: 1, y: 2 } } }) } })).toThrow();
});

test("node shapes round-trip and reject unknown nodes or shapes", () => {
  const trait: TraitInstance = { id: "graph", template_id: "tree_graph", values: {} };
  const entity = { id: "owner", name: "Owner", traits: [trait] } as AnyEntity;
  const restored = treeGraphOf(setTreeGraph(entity, trait, { ...graph, shapes: { shared: "diamond", end: "ellipse" } }).traits![0]!);
  expect(restored.shapes).toEqual({ shared: "diamond", end: "ellipse" });
  expect(() => treeGraphOf({ ...trait, values: { nodes: ["shared"], shapes: '{"shared":"triangle"}' } })).toThrow();
  expect(() => treeGraphOf({ ...trait, values: { nodes: ["shared"], shapes: '{"missing":"card"}' } })).toThrow();
  const replaced = setTreeGraph({ ...entity, traits: [setTreeGraph(entity, trait, { ...graph, shapes: { shared: "diamond" } }).traits![0]!] }, trait, graph);
  expect(treeGraphOf(replaced.traits![0]!).shapes).toEqual({});
});

test("node and edge appearance round-trips and clears when omitted", () => {
  const trait: TraitInstance = { id: "graph", template_id: "tree_graph", values: {} };
  const entity = { id: "owner", name: "Owner", traits: [trait] } as AnyEntity;
  const edgeKey = JSON.stringify(["start-a", "shared"]);
  const appearanceGraph: TreeGraph = {
    ...graph,
    appearance: {
      nodes: {
        shared: {
          background: "#112233",
          outline: "#445566",
          outlineWidth: 3,
          width: 250,
          height: 120,
        },
      },
      edges: {
        [edgeKey]: {
          color: "#abcdef",
          width: 4,
        },
      },
    },
  };
  const patched = setTreeGraph(entity, trait, appearanceGraph);
  const restored = treeGraphOf(patched.traits![0]!);
  expect(restored.appearance).toEqual(appearanceGraph.appearance);

  // Clearing old appearance when passed graph with appearance: undefined
  const replaced = setTreeGraph({ ...entity, traits: [patched.traits![0]!] }, trait, graph);
  expect(treeGraphOf(replaced.traits![0]!).appearance).toEqual({ nodes: {}, edges: {} });
});

test("existing graphs without appearance remain readable", () => {
  const traitEmpty: TraitInstance = { id: "graph", template_id: "tree_graph", values: { nodes: ["a"], edges: [] } };
  expect(treeGraphOf(traitEmpty).appearance).toBeUndefined();

  const traitBlank: TraitInstance = { id: "graph", template_id: "tree_graph", values: { nodes: ["a"], edges: [], appearance: "" } };
  expect(treeGraphOf(traitBlank).appearance).toBeUndefined();
});

test("appearance rejects invalid payload, unexpected fields, and out-of-range values", () => {
  const base = { id: "graph", template_id: "tree_graph", values: { nodes: ["a", "b"], edges: [JSON.stringify(["a", "b"])] } };

  // Malformed JSON and wrong type
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: "not-json" } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: 123 as unknown as string } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: "[]" } })).toThrow();

  // Unexpected top-level fields
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ extra: 123 }) } })).toThrow();

  // Unknown / stale node reference
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { missing: { background: "#112233" } } }) } })).toThrow();

  // Unknown / stale edge reference
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ edges: { [JSON.stringify(["a", "missing"])]: { color: "#112233" } } }) } })).toThrow();

  // Malformed edge key
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ edges: { "not-json": { color: "#112233" } } }) } })).toThrow();

  // Unexpected fields in node/edge appearance
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { unexpected: true } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ edges: { [JSON.stringify(["a", "b"])]: { unexpected: true } } }) } })).toThrow();

  // Invalid colors
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { background: "blue" } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { background: "#fff" } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { outline: "#gggggg" } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ edges: { [JSON.stringify(["a", "b"])]: { color: "112233" } } }) } })).toThrow();

  // Out-of-bounds numbers
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { outlineWidth: 0 } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { outlineWidth: 13 } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { width: 99 } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { width: 601 } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { height: 59 } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ nodes: { a: { height: 401 } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ edges: { [JSON.stringify(["a", "b"])]: { width: 0 } } }) } })).toThrow();
  expect(() => treeGraphOf({ ...base, values: { ...base.values, appearance: JSON.stringify({ edges: { [JSON.stringify(["a", "b"])]: { width: 13 } } }) } })).toThrow();
});

test("treeErrors flags unknown nodes and edges in appearance", () => {
  const eligible = new Set(graph.nodes);
  expect(treeErrors({
    ...graph,
    appearance: {
      nodes: { "unknown-node": { background: "#112233" } },
      edges: {},
    },
  }, eligible)).not.toEqual([]);

  expect(treeErrors({
    ...graph,
    appearance: {
      nodes: {},
      edges: { [JSON.stringify(["start-a", "end"])]: { color: "#112233" } },
    },
  }, eligible)).not.toEqual([]);
});


test("examples describe valid, distinct topologies", () => {
  expect(TREE_EXAMPLES.map((entry) => entry.id)).toEqual(["skill-tree", "roguelike-map", "relationship-tree"]);
  for (const example of TREE_EXAMPLES) {
    const data: TreeGraph = { nodes: example.items.map((item) => item.key), edges: example.edges, direction: example.direction };
    expect(treeErrors(data, new Set(data.nodes))).toEqual([]);
  }
  expect(treeRoots({ nodes: TREE_EXAMPLES[0]!.items.map((item) => item.key), edges: TREE_EXAMPLES[0]!.edges, direction: "radial" }).length).toBe(1);
  expect(treeRoots({ nodes: TREE_EXAMPLES[1]!.items.map((item) => item.key), edges: TREE_EXAMPLES[1]!.edges, direction: "down" }).length).toBeGreaterThan(1);
});

test("invalid and ineligible node references and malformed edges are rejected", () => {
  const eligible = new Set(["start-a", "start-b", "shared", "end"]);
  expect(treeErrors({ ...graph, nodes: ["start-a", "start-a"] }, eligible)).not.toEqual([]);
  expect(treeErrors({ ...graph, nodes: ["not-an-item"] }, eligible)).not.toEqual([]);
  expect(treeErrors({ ...graph, edges: [{ from: "start-a", to: "missing" }] }, eligible)).not.toEqual([]);
  expect(treeErrors({ ...graph, edges: [graph.edges[0]!, graph.edges[0]!] }, eligible)).not.toEqual([]);
  expect(() => treeGraphOf({ id: "g", template_id: "tree_graph", values: { edges: ["not-json"] } })).toThrow();
  expect(() => treeGraphOf({ id: "g", template_id: "tree_graph", values: { nodes: null } })).toThrow();
});

test("shared nodes and cyclic disconnected components are laid out exactly once", () => {
  const cyclic: TreeGraph = { ...graph, nodes: [...graph.nodes, "cycle-a", "cycle-b"], edges: [...graph.edges, { from: "cycle-a", to: "cycle-b" }, { from: "cycle-b", to: "cycle-a" }] };
  expect(treeErrors(cyclic, new Set(cyclic.nodes))).toEqual([]);
  expect(treeRoots(cyclic)).toEqual(["start-a", "start-b"]);
  for (const direction of ["right", "left", "down", "up", "radial"] as TreeDirection[]) {
    const layout = layoutTree({ ...cyclic, direction });
    for (const id of cyclic.nodes) {
      const point = layout.positions[id]!;
      expect(Number.isFinite(point.x) && Number.isFinite(point.y)).toBe(true);
      expect(point.x >= 0 && point.y >= 0 && point.x + CARD_WIDTH <= layout.width && point.y + CARD_HEIGHT <= layout.height).toBe(true);
    }
    expect(new Set(cyclic.nodes.map((id) => JSON.stringify(layout.positions[id]))).size).toBe(cyclic.nodes.length);
  }
});

test("opposite layout directions reverse the parent-child axis", () => {
  const positions = (direction: TreeDirection) => layoutTree({ ...graph, direction }).positions;
  expect(positions("right")["start-a"]!.x).toBeLessThan(positions("right").shared!.x);
  expect(positions("left")["start-a"]!.x).toBeGreaterThan(positions("left").shared!.x);
  expect(positions("down")["start-a"]!.y).toBeLessThan(positions("down").shared!.y);
  expect(positions("up")["start-a"]!.y).toBeGreaterThan(positions("up").shared!.y);
});

test("crowded radial levels keep adjacent node cards apart", () => {
  const roots = Array.from({ length: 40 }, (_, index) => `root-${index}`);
  const crowded: TreeGraph = { nodes: [...roots, "child"], edges: [{ from: roots[0]!, to: "child" }], direction: "radial" };
  const points = layoutTree(crowded).positions;
  for (let i = 0; i < crowded.nodes.length; i++) for (let j = i + 1; j < crowded.nodes.length; j++) {
    const a = points[crowded.nodes[i]!]!, b = points[crowded.nodes[j]!]!;
    expect(Math.abs(a.x - b.x) >= CARD_WIDTH || Math.abs(a.y - b.y) >= CARD_HEIGHT).toBe(true);
  }
});
