import { describe, expect, test } from "bun:test";
import type { TreeGraph } from "../../src/web/studio/plugins/tree/model.ts";
import { treeListRows } from "../../src/web/studio/plugins/tree/listModel.ts";

describe("treeListRows", () => {
  test("mixed roots: preserves graph.nodes order for roots and traverses rootless components", () => {
    const graph: TreeGraph = {
      nodes: ["cycle-b", "root-1", "cycle-a", "root-2", "isolated"],
      edges: [
        { from: "root-1", to: "child-1" },
        // cycle-a and cycle-b form a rootless cycle (both have in-degree 1)
        { from: "cycle-a", to: "cycle-b" },
        { from: "cycle-b", to: "cycle-a" },
      ],
      direction: "right",
    };

    // Filtered by nodes in graph: "child-1" is not in graph.nodes so it is ignored.
    // True roots in graph.nodes order: root-1, root-2, isolated
    // Rootless component nodes: cycle-b appears first in graph.nodes, so it becomes canonical root for that component
    const rows = treeListRows(graph, new Set());

    expect(rows.map((r) => ({ nodeId: r.nodeId, depth: r.depth, reference: r.reference }))).toEqual([
      { nodeId: "root-1", depth: 0, reference: false },
      { nodeId: "root-2", depth: 0, reference: false },
      { nodeId: "isolated", depth: 0, reference: false },
      { nodeId: "cycle-b", depth: 0, reference: false },
      { nodeId: "cycle-a", depth: 1, reference: false },
      { nodeId: "cycle-b", depth: 2, reference: true },
    ]);

    // Root rows omit edge
    expect("edge" in rows[0]!).toBe(false);
    expect("edge" in rows[1]!).toBe(false);
    expect("edge" in rows[2]!).toBe(false);
    expect("edge" in rows[3]!).toBe(false);

    // All emitted rows have unique keys
    const keys = rows.map((r) => r.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  test("shared nodes: produces canonical row on first encounter and reference row on second without unfolding", () => {
    const graph: TreeGraph = {
      nodes: ["parent-a", "parent-b", "shared", "child-of-shared"],
      edges: [
        { from: "parent-a", to: "shared", relationship: "first_link" },
        { from: "parent-b", to: "shared", relationship: "second_link" },
        { from: "shared", to: "child-of-shared", relationship: "descendant" },
      ],
      direction: "right",
    };

    const rows = treeListRows(graph, new Set());

    expect(rows.map((r) => ({ id: r.nodeId, depth: r.depth, ref: r.reference, hasChildren: r.hasChildren }))).toEqual([
      { id: "parent-a", depth: 0, ref: false, hasChildren: true },
      { id: "shared", depth: 1, ref: false, hasChildren: true },
      { id: "child-of-shared", depth: 2, ref: false, hasChildren: false },
      { id: "parent-b", depth: 0, ref: false, hasChildren: true },
      { id: "shared", depth: 1, ref: true, hasChildren: false },
    ]);

    const canonicalShared = rows.find((r) => r.nodeId === "shared" && !r.reference)!;
    const referenceShared = rows.find((r) => r.nodeId === "shared" && r.reference)!;

    expect(canonicalShared.depth).toBe(1);
    expect(canonicalShared.reference).toBe(false);
    expect(canonicalShared.edge).toEqual({ from: "parent-a", to: "shared", relationship: "first_link" });
    expect(canonicalShared.hasChildren).toBe(true);

    expect(referenceShared.depth).toBe(1);
    expect(referenceShared.reference).toBe(true);
    expect(referenceShared.edge).toEqual({ from: "parent-b", to: "shared", relationship: "second_link" });
    expect(referenceShared.hasChildren).toBe(false);

    expect(canonicalShared.key).not.toBe(referenceShared.key);
  });

  test("self links: produces canonical root row and reference child row without infinite loop", () => {
    const graph: TreeGraph = {
      nodes: ["selfie"],
      edges: [{ from: "selfie", to: "selfie", relationship: "recursive_self" }],
      direction: "right",
    };

    const rows = treeListRows(graph, new Set());

    expect(rows).toHaveLength(2);

    expect(rows[0]!.nodeId).toBe("selfie");
    expect(rows[0]!.depth).toBe(0);
    expect(rows[0]!.reference).toBe(false);
    expect(rows[0]!.hasChildren).toBe(true);
    expect("edge" in rows[0]!).toBe(false);

    expect(rows[1]!.nodeId).toBe("selfie");
    expect(rows[1]!.depth).toBe(1);
    expect(rows[1]!.reference).toBe(true);
    expect(rows[1]!.hasChildren).toBe(false);
    expect(rows[1]!.edge).toEqual({ from: "selfie", to: "selfie", relationship: "recursive_self" });

    expect(rows[0]!.key).not.toBe(rows[1]!.key);
  });

  test("cycles: terminates directed cycle cleanly with reference row and edge preserved", () => {
    const graph: TreeGraph = {
      nodes: ["start", "step-1", "step-2"],
      edges: [
        { from: "start", to: "step-1", relationship: "next" },
        { from: "step-1", to: "step-2", relationship: "next" },
        { from: "step-2", to: "start", relationship: "loop_back" },
      ],
      direction: "right",
    };

    const rows = treeListRows(graph, new Set());

    expect(rows.map((r) => ({ id: r.nodeId, depth: r.depth, ref: r.reference }))).toEqual([
      { id: "start", depth: 0, ref: false },
      { id: "step-1", depth: 1, ref: false },
      { id: "step-2", depth: 2, ref: false },
      { id: "start", depth: 3, ref: true },
    ]);

    const loopBackRow = rows[3]!;
    expect(loopBackRow.nodeId).toBe("start");
    expect(loopBackRow.depth).toBe(3);
    expect(loopBackRow.reference).toBe(true);
    expect(loopBackRow.edge).toEqual({ from: "step-2", to: "start", relationship: "loop_back" });
    expect(loopBackRow.hasChildren).toBe(false);

    const keys = rows.map((r) => r.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  test("collapse: hides descendants without promoting hidden nodes to roots", () => {
    const graph: TreeGraph = {
      nodes: ["root", "mid-a", "leaf-a1", "leaf-a2", "mid-b", "leaf-b1"],
      edges: [
        { from: "root", to: "mid-a" },
        { from: "mid-a", to: "leaf-a1" },
        { from: "mid-a", to: "leaf-a2" },
        { from: "root", to: "mid-b" },
        { from: "mid-b", to: "leaf-b1" },
      ],
      direction: "right",
    };

    // When "mid-a" is collapsed, leaf-a1 and leaf-a2 must be hidden.
    // They must NOT appear as root rows later!
    const collapsedMidA = treeListRows(graph, new Set(["mid-a"]));
    expect(collapsedMidA.map((r) => r.nodeId)).toEqual(["root", "mid-a", "mid-b", "leaf-b1"]);

    // mid-a still has hasChildren: true so UI can render the expand chevron
    const midARow = collapsedMidA.find((r) => r.nodeId === "mid-a")!;
    expect(midARow.hasChildren).toBe(true);

    // When root is collapsed, all descendants are hidden
    const collapsedRoot = treeListRows(graph, new Set(["root"]));
    expect(collapsedRoot.map((r) => r.nodeId)).toEqual(["root"]);
    expect(collapsedRoot[0]!.hasChildren).toBe(true);
  });

  test("edge label propagation: preserves edge relationships faithfully across all rows", () => {
    const graph: TreeGraph = {
      nodes: ["character", "sword", "shield"],
      edges: [
        { from: "character", to: "sword", relationship: "wields" },
        { from: "character", to: "shield", relationship: "guards_with" },
        { from: "sword", to: "shield", relationship: "pairs_with" },
      ],
      direction: "right",
    };

    const rows = treeListRows(graph, new Set());

    expect(rows).toHaveLength(4);

    // Root row
    expect(rows[0]!.nodeId).toBe("character");
    expect(rows[0]!.depth).toBe(0);
    expect(rows[0]!.reference).toBe(false);
    expect(rows[0]!.hasChildren).toBe(true);
    expect("edge" in rows[0]!).toBe(false);

    // Canonical sword
    expect(rows[1]!.nodeId).toBe("sword");
    expect(rows[1]!.depth).toBe(1);
    expect(rows[1]!.reference).toBe(false);
    expect(rows[1]!.hasChildren).toBe(true);
    expect(rows[1]!.edge).toEqual({ from: "character", to: "sword", relationship: "wields" });

    // Canonical shield under sword
    expect(rows[2]!.nodeId).toBe("shield");
    expect(rows[2]!.depth).toBe(2);
    expect(rows[2]!.reference).toBe(false);
    expect(rows[2]!.hasChildren).toBe(false);
    expect(rows[2]!.edge).toEqual({ from: "sword", to: "shield", relationship: "pairs_with" });

    // Reference shield under character
    expect(rows[3]!.nodeId).toBe("shield");
    expect(rows[3]!.depth).toBe(1);
    expect(rows[3]!.reference).toBe(true);
    expect(rows[3]!.hasChildren).toBe(false);
    expect(rows[3]!.edge).toEqual({ from: "character", to: "shield", relationship: "guards_with" });

    const keys = rows.map((r) => r.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  test("deep graphs: traverses 1,000-node chain without call stack overflow", () => {
    const count = 1000;
    const nodes: string[] = [];
    const edges: TreeGraph["edges"] = [];

    for (let i = 0; i < count; i++) {
      nodes.push(`node-${i}`);
      if (i > 0) {
        edges.push({ from: `node-${i - 1}`, to: `node-${i}` });
      }
    }

    const graph: TreeGraph = {
      nodes,
      edges,
      direction: "right",
    };

    const rows = treeListRows(graph, new Set());
    expect(rows).toHaveLength(count);
    expect(rows[0]!.depth).toBe(0);
    expect("edge" in rows[0]!).toBe(false);
    expect(rows[count - 1]!.depth).toBe(count - 1);
    expect(rows[count - 1]!.nodeId).toBe(`node-${count - 1}`);
  });

  test("regression: distinct endpoint pairs produce unique keys even when IDs contain arrow delimiters", () => {
    const graph: TreeGraph = {
      nodes: ["root", "a->b", "c", "a", "b->c"],
      edges: [
        { from: "root", to: "a->b" },
        { from: "root", to: "c" },
        { from: "root", to: "a" },
        { from: "root", to: "b->c" },
        // Two edges with distinct endpoint pairs that would collide under naive `${from}->${to}`
        { from: "a->b", to: "c" },
        { from: "a", to: "b->c" },
      ],
      direction: "right",
    };

    const rows = treeListRows(graph, new Set());
    const keys = rows.map((r) => r.key);
    expect(new Set(keys).size).toBe(keys.length);
  });
});
