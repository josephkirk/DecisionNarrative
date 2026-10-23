import { expect, test } from "bun:test";
import type { StoryletRecord } from "../../src/web/studio/contracts.ts";
import { applyJumpPreconditions, compileGraphToStorylet, isSafeImageUrl, validateNarrativeGraph, type NarrativeGraph } from "../../src/web/studio/narrativeGraphTypes.ts";

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


test("storylet graph requires one Start and at least one reachable End", () => {
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
    .toContain("Graph must have at least one End node.");
  const multipleEnds: NarrativeGraph = {
    nodes: [
      graph.nodes[0]!,
      { id: "choice", type: "choice", title: "Choice", x: 150, y: 0, width: 180, height: 100, choices: [{ id: "a", text: "A" }, { id: "b", text: "B" }] },
      { id: "end-a", type: "end", title: "End A", x: 400, y: 0, width: 100, height: 80, is_terminal: true },
      { id: "end-b", type: "end", title: "End B", x: 400, y: 120, width: 100, height: 80, jump_to_storylet: "next" },
    ],
    edges: [
      { id: "start-choice", from_node: "start", from_port: "output", to_node: "choice", to_port: "input" },
      { id: "choice-a", from_node: "choice", from_port: "choice_0", to_node: "end-a", to_port: "input" },
      { id: "choice-b", from_node: "choice", from_port: "choice_1", to_node: "end-b", to_port: "input" },
    ],
  };
  expect(validateNarrativeGraph(multipleEnds)).toEqual([]);
  const disconnected = { ...graph, nodes: [...graph.nodes, { id: "orphan", type: "dialog" as const, title: "Orphan", x: 0, y: 150, width: 180, height: 100 }] };
  expect(validateNarrativeGraph(disconnected)).toContain("Every node must be reachable from Start.");
  expect(validateNarrativeGraph(disconnected)).toContain("Every node must have a path to an End node.");
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

test("only terminal End nodes mark a storylet as an ending", () => {
  const makeStorylet = () => ({
    id: "storylet",
    revision: 1,
    approved_revision: null,
    title: "Storylet",
    ending: false,
    location_tags: [],
    cluster_type: "player_passage",
    urgency_weight: 50,
    repeatability: "always",
    cooldown_turns: 0,
    parameters: [],
    preconditions: { all_of: [], any_of: [], visited_locations_any_of: [], met_characters_any_of: [] },
    content: { text_template: "", sceneOpening: "", dialogue_quips: [] },
    choices: [],
    effects: {},
    created_at: 0,
  } as StoryletRecord);
  const graph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "end", type: "end", title: "End", x: 400, y: 0, width: 100, height: 80, is_terminal: false, jump_to_storylet: "next" },
    ],
    edges: [{ id: "start-end", from_node: "start", from_port: "output", to_node: "end", to_port: "input" }],
  };
  const storylet = makeStorylet();
  compileGraphToStorylet(graph, storylet);
  expect(storylet.ending).toBe(false);

  graph.nodes[1] = { ...graph.nodes[1]!, is_terminal: true, jump_to_storylet: undefined };
  compileGraphToStorylet(graph, storylet);
  expect(storylet.ending).toBe(true);

  const mixed: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "choice", type: "choice", title: "Choice", x: 150, y: 0, width: 180, height: 100, choices: [{ id: "stay", text: "Stay" }, { id: "go", text: "Go" }] },
      { id: "end-stay", type: "end", title: "Stay End", x: 400, y: 0, width: 100, height: 80, is_terminal: true },
      { id: "end-go", type: "end", title: "Go End", x: 400, y: 120, width: 100, height: 80, jump_to_storylet: "next" },
    ],
    edges: [
      { id: "start-choice", from_node: "start", from_port: "output", to_node: "choice", to_port: "input" },
      { id: "choice-stay", from_node: "choice", from_port: "choice_0", to_node: "end-stay", to_port: "input" },
      { id: "choice-go", from_node: "choice", from_port: "choice_1", to_node: "end-go", to_port: "input" },
    ],
  };
  const mixedStorylet = makeStorylet();
  compileGraphToStorylet(mixed, mixedStorylet);
  expect(mixedStorylet.ending).toBe(false);
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
