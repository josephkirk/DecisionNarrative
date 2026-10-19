import { expect, test } from "bun:test";
import type { StoryletRecord } from "../../src/web/studio/contracts.ts";
import { applyJumpPreconditions, isSafeImageUrl, validateNarrativeGraph, type NarrativeGraph } from "../../src/web/studio/narrativeGraphTypes.ts";

test("jump preconditions produce effect values that satisfy supported operators", () => {
  const target = {
    preconditions: {
      all_of: [
        { variable: "equal", op: "eq", value: true },
        { variable: "at_least", op: "gte", value: 3 },
        { variable: "at_most", op: "lte", value: 8 },
        { variable: "greater", op: "gt", value: 2.5 },
        { variable: "less", op: "lt", value: 2.5 },
        { variable: "different", op: "neq", value: "locked" },
        { variable: "not_false", op: "neq", value: false },
        { variable: "not_one", op: "neq", value: 1 },
        { variable: "unsupported", op: "contains", value: "x" },
      ],
    },
  } as unknown as StoryletRecord;
  const source = { effects: {} } as unknown as StoryletRecord;

  const updated = applyJumpPreconditions(target, source);

  expect(source.effects).toEqual({
    equal: true,
    at_least: 3,
    at_most: 8,
    greater: 3.5,
    less: 1.5,
    different: "locked_other",
    not_false: true,
    not_one: 2,
  });
  expect(updated).toHaveLength(8);
  expect(source.effects).not.toHaveProperty("unsupported");
});


test("storylet graph must contain one reachable Start-to-End flow", () => {
  const graph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "dialog", type: "dialog", title: "Dialog", x: 150, y: 0, width: 180, height: 100 },
      { id: "end", type: "end", title: "End", x: 400, y: 0, width: 100, height: 80 },
    ],
    edges: [
      { id: "start-dialog", from_node: "start", from_port: "output", to_node: "dialog", to_port: "input" },
      { id: "dialog-end", from_node: "dialog", from_port: "output", to_node: "end", to_port: "input" },
    ],
  };
  expect(validateNarrativeGraph(graph)).toEqual([]);
  const unsupportedMovement: NarrativeGraph = {
    nodes: [
      ...graph.nodes.slice(0, 2),
      { id: "move", type: "go_to_location", title: "Move", x: 350, y: 0, width: 180, height: 90, location_id: "market" },
      graph.nodes[2]!,
    ],
    edges: [
      graph.edges[0]!,
      { id: "dialog-move", from_node: "dialog", from_port: "output", to_node: "move", to_port: "input" },
      { id: "move-end", from_node: "move", from_port: "output", to_node: "end", to_port: "input" },
    ],
  };
  expect(validateNarrativeGraph(unsupportedMovement)).toContain("Go To Location node Move may only be targeted directly by a Choice branch.");
  expect(validateNarrativeGraph({ nodes: [null], edges: [] } as unknown as NarrativeGraph)).toContain("Graph nodes must have nonempty IDs and recognized types.");
  expect(validateNarrativeGraph({ nodes: [], edges: [null] } as unknown as NarrativeGraph)).toContain("Graph edges must have nonempty IDs and valid endpoint fields.");
  const duplicateOutput = { ...graph, edges: [...graph.edges, { ...graph.edges[1]!, id: "dialog-end-duplicate" }] };
  expect(validateNarrativeGraph(duplicateOutput)).toContain("dialog node Dialog must have exactly one outgoing edge from its output port.");
  const wrongInputPort = { ...graph, edges: graph.edges.map((edge) => edge.id === "dialog-end" ? { ...edge, to_port: "output" } : edge) };
  expect(validateNarrativeGraph(wrongInputPort)).toContain("Edge dialog-end must target an input port.");
  expect(validateNarrativeGraph({ ...graph, nodes: graph.nodes.filter((node) => node.type !== "end"), edges: graph.edges.filter((edge) => edge.to_node !== "end") }))
    .toContain("Graph must have exactly one End node.");
  expect(validateNarrativeGraph({ ...graph, nodes: [...graph.nodes, { ...graph.nodes[2]!, id: "end-2" }] }))
    .toContain("Graph must have exactly one End node.");
  const disconnected = { ...graph, nodes: [...graph.nodes, { id: "orphan", type: "dialog" as const, title: "Orphan", x: 0, y: 150, width: 180, height: 100 }] };
  expect(validateNarrativeGraph(disconnected)).toContain("Every node must be reachable from Start.");
  expect(validateNarrativeGraph(disconnected)).toContain("Every node must have a path to End.");
  const conditionalGraph: NarrativeGraph = {
    nodes: [
      graph.nodes[0]!,
      {
        id: "condition",
        type: "conditional",
        title: "Condition",
        x: 150,
        y: 0,
        width: 180,
        height: 120,
        conditions: [{ scope: "global", variable: "trust", op: "gte", value: 1 }],
      },
      graph.nodes[2]!,
    ],
    edges: [
      { id: "start-condition", from_node: "start", from_port: "output", to_node: "condition", to_port: "input" },
      { id: "condition-true", from_node: "condition", from_port: "true", to_node: "end", to_port: "input" },
      { id: "condition-false", from_node: "condition", from_port: "false", to_node: "end", to_port: "input" },
    ],
  };
  expect(validateNarrativeGraph(conditionalGraph)).toEqual([]);
  expect(validateNarrativeGraph({ ...conditionalGraph, edges: conditionalGraph.edges.slice(0, 2) }))
    .toContain("Conditional node Condition must have exactly one 'false' branch.");
  expect(validateNarrativeGraph({
    ...conditionalGraph,
    nodes: conditionalGraph.nodes.map((node) => node.type === "conditional" ? { ...node, conditions: [] } : node),
  })).toContain("Conditional node Condition must have at least one condition.");
});
test("image URL validation matches Rust and graph validation reports unsafe nodes", () => {
  const cases: Array<[string, boolean]> = [
    ["assets/a:b.png", true],
    ["HTTPS://x/y", true],
    ["https:x", false],
    ["\u007fx", false],
    ["javascript:alert(1)", false],
    ["//example.com/platform.png", false],
    ["", false],
  ];
  for (const [url, safe] of cases) expect(isSafeImageUrl(url)).toBe(safe);

  const graph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "image", type: "image", title: "Hero", x: 150, y: 0, width: 180, height: 100, image_url: "javascript:alert(1)" },
      { id: "end", type: "end", title: "End", x: 400, y: 0, width: 100, height: 80 },
    ],
    edges: [
      { id: "start-image", from_node: "start", from_port: "output", to_node: "image", to_port: "input" },
      { id: "image-end", from_node: "image", from_port: "output", to_node: "end", to_port: "input" },
    ],
  };
  expect(validateNarrativeGraph(graph)).toContain(
    "Image node Hero must have a safe HTTP(S) URL or project-relative path.",
  );
});
