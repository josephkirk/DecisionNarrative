import { expect, test } from "bun:test";
import type { StoryletRecord } from "../../src/web/studio/contracts.ts";
import { applyCommandOperationsForType, buildGraphFromStorylet, compileGraphToStorylet, graphConnectionIsCompatible, graphNodeValueType, graphTargetPinType, isSafeImageUrl, validateNarrativeGraph, validatePreconditionGraph, type NarrativeGraph, type NarrativeNode } from "../../src/web/studio/narrativeGraphTypes.ts";


test("empty placeholder graphs initialize with connected start and end nodes", () => {
  const storylet = {
    id: "new",
    graph: { nodes: [], edges: [] },
    content: { text_template: "Opening." },
    parameters: [],
    choices: [],
  } as StoryletRecord;
  const graph = buildGraphFromStorylet(storylet);
  expect(graph.nodes.map((node) => node.type)).toEqual(["start", "dialog", "end"]);
  expect(validateNarrativeGraph(graph)).toEqual([]);
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
  expect(validateNarrativeGraph({ nodes: [], edges: [null] } as unknown as NarrativeGraph)).toContain("Graph edges must have nonempty IDs, valid endpoint fields, and a valid kind.");
  const duplicateOutput = { ...graph, edges: [...graph.edges, { ...graph.edges[1]!, id: "dialog-end-duplicate" }] };
  expect(validateNarrativeGraph(duplicateOutput)).toContain("dialog node Dialog must have exactly one outgoing edge from its output port.");
  const wrongInputPort = { ...graph, edges: graph.edges.map((edge) => edge.id === "dialog-end" ? { ...edge, to_port: "output" } : edge) };
  expect(validateNarrativeGraph(wrongInputPort)).toContain("Flow edge dialog-end uses an invalid source or target port.");
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
        type: "condition",
        title: "Condition",
        x: 150,
        y: 0,
        width: 180,
        height: 120,
      },
      { id: "tag", type: "has_entity_tag", title: "Has ready tag", target_entity_id: "world", tag: "flag.ready", x: 150, y: 150, width: 180, height: 100 },
      graph.nodes[2]!,
    ],
    edges: [
      { id: "start-condition", from_node: "start", from_port: "output", to_node: "condition", to_port: "input", kind: "flow" },
      { id: "tag-condition", from_node: "tag", from_port: "output", to_node: "condition", to_port: "input", kind: "data" },
      { id: "condition-true", from_node: "condition", from_port: "true", to_node: "end", to_port: "input", kind: "flow" },
      { id: "condition-false", from_node: "condition", from_port: "false", to_node: "end", to_port: "input", kind: "flow" },
    ],
  };
  expect(validateNarrativeGraph(conditionalGraph)).toEqual([]);
  expect(validateNarrativeGraph({ ...conditionalGraph, edges: conditionalGraph.edges.slice(0, 2) }))
    .toContain("Condition Node Condition must have exactly one 'false' branch.");
});

test("precondition graphs switch predicate booleans into candidate End results", () => {
  const graph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "tag", type: "has_entity_tag", title: "Has visited tag", tag: "loc.chamber.visited", x: 150, y: 0, width: 180, height: 100 },
      { id: "condition", type: "condition", title: "Switch", x: 350, y: 0, width: 180, height: 120 },
      { id: "valid", type: "end", title: "Valid", precondition_result: true, x: 600, y: -50, width: 120, height: 80 },
      { id: "reject", type: "end", title: "Reject", precondition_result: false, x: 600, y: 80, width: 120, height: 80 },
    ],
    edges: [
      { id: "start-condition", from_node: "start", from_port: "output", to_node: "condition", to_port: "input", kind: "flow" },
      { id: "tag-condition", from_node: "tag", from_port: "output", to_node: "condition", to_port: "input", kind: "data" },
      { id: "condition-true-valid", from_node: "condition", from_port: "true", to_node: "valid", to_port: "input", kind: "flow" },
      { id: "condition-false-reject", from_node: "condition", from_port: "false", to_node: "reject", to_port: "input", kind: "flow" },
    ],
  };
  expect(validatePreconditionGraph(graph)).toEqual([]);
  expect(validatePreconditionGraph({
    ...graph,
    nodes: graph.nodes.map((node) => node.id === "valid" ? { ...node, precondition_result: false } : node),
  })).toContain("Precondition graph needs at least one valid-candidate End node.");
  const sideEffectGraph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "dialog", type: "dialog", title: "Side effect", x: 150, y: 0, width: 180, height: 100 },
      { id: "end", type: "end", title: "Valid", precondition_result: true, x: 400, y: 0, width: 120, height: 80 },
    ],
    edges: [
      { id: "start-dialog", from_node: "start", from_port: "output", to_node: "dialog", to_port: "input" },
      { id: "dialog-end", from_node: "dialog", from_port: "output", to_node: "end", to_port: "input" },
    ],
  };
  expect(validatePreconditionGraph(sideEffectGraph)).toContain("Precondition graph cannot contain dialog nodes.");
});

test("precondition predicates emit typed boolean outputs", () => {
  const graph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "entity", type: "get_entity", title: "World", entity_path: "world", x: 120, y: 0, width: 160, height: 100 },
      { id: "property", type: "get_entity_property", title: "Trust", property_key: "trust", value_type: "int", x: 300, y: 0, width: 180, height: 100 },
      { id: "compare", type: "compare", title: "Trust threshold", operator: "gte", value_type: "int", value: 33, x: 500, y: 0, width: 180, height: 100 },
      { id: "condition", type: "condition", title: "Switch", x: 700, y: 0, width: 180, height: 120 },
      { id: "valid", type: "end", title: "Valid", precondition_result: true, x: 920, y: -50, width: 120, height: 80 },
      { id: "reject", type: "end", title: "Reject", precondition_result: false, x: 920, y: 100, width: 120, height: 80 },
    ],
    edges: [
      { id: "start-condition", from_node: "start", from_port: "output", to_node: "condition", to_port: "input", kind: "flow" },
      { id: "entity-property", from_node: "entity", from_port: "output", to_node: "property", to_port: "entity", kind: "data" },
      { id: "property-compare", from_node: "property", from_port: "output", to_node: "compare", to_port: "input", kind: "data" },
      { id: "compare-condition", from_node: "compare", from_port: "output", to_node: "condition", to_port: "input", kind: "data" },
      { id: "switch-true", from_node: "condition", from_port: "true", to_node: "valid", to_port: "input", kind: "flow" },
      { id: "switch-false", from_node: "condition", from_port: "false", to_node: "reject", to_port: "input", kind: "flow" },
    ],
  };
  expect(validatePreconditionGraph(graph)).toEqual([]);
});

test("Compare input connections enforce the selected type, including Entity Ref", () => {
  const makeGraph = (propertyType: NonNullable<NarrativeNode["value_type"]>, compareType: NonNullable<NarrativeNode["value_type"]>): NarrativeGraph => ({
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "entity", type: "get_entity", title: "Entity", entity_path: "source", x: 100, y: 0, width: 160, height: 100 },
      { id: "property", type: "get_entity_property", title: "Property", property_key: "value", value_type: propertyType, x: 300, y: 0, width: 180, height: 100 },
      { id: "compare", type: "compare", title: "Compare", value_type: compareType, value: compareType === "int" ? 1 : "", x: 500, y: 0, width: 180, height: 100 },
      { id: "condition", type: "condition", title: "Condition", x: 700, y: 0, width: 180, height: 120 },
      { id: "valid", type: "end", title: "Valid", precondition_result: true, x: 920, y: -50, width: 120, height: 80 },
      { id: "reject", type: "end", title: "Reject", precondition_result: false, x: 920, y: 100, width: 120, height: 80 },
    ],
    edges: [
      { id: "start-condition", from_node: "start", from_port: "output", to_node: "condition", to_port: "input" },
      { id: "entity-property", from_node: "entity", from_port: "output", to_node: "property", to_port: "entity", kind: "data" },
      { id: "property-compare", from_node: "property", from_port: "output", to_node: "compare", to_port: "input", kind: "data" },
      { id: "compare-condition", from_node: "compare", from_port: "output", to_node: "condition", to_port: "input", kind: "data" },
      { id: "condition-valid", from_node: "condition", from_port: "true", to_node: "valid", to_port: "input" },
      { id: "condition-reject", from_node: "condition", from_port: "false", to_node: "reject", to_port: "input" },
    ],
  });
  const intCompare = makeGraph("int", "int");
  expect(validateNarrativeGraph(intCompare)).toEqual([]);
  expect(graphTargetPinType(intCompare.nodes[3]!, "input")).toBe("int");
  expect(graphConnectionIsCompatible("int", intCompare.nodes[3]!, "input")).toBe(true);
  expect(graphConnectionIsCompatible("string", intCompare.nodes[3]!, "input")).toBe(false);
  expect(validateNarrativeGraph(makeGraph("int", "string"))).toContain("Data edge property-compare uses an invalid port or incompatible type.");

  const entityCompare = makeGraph("entity", "entity");
  expect(graphNodeValueType(entityCompare.nodes[2]!)).toBe("entity_ref");
  expect(graphTargetPinType(entityCompare.nodes[3]!, "input")).toBe("entity_ref");
  expect(graphConnectionIsCompatible("entity_ref", entityCompare.nodes[3]!, "input")).toBe(true);
  expect(validateNarrativeGraph(entityCompare)).toEqual([]);
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
    content: { text_template: "", sceneOpening: "", dialogue_quips: [] },
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

test("boolean precondition predicates and combinators use value sockets", () => {
  const graph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "entity", type: "get_entity", title: "World", entity_path: "world", x: 120, y: 160, width: 160, height: 100 },
      { id: "property", type: "get_entity_property", title: "Trust", property_key: "trust", value_type: "int", x: 300, y: 160, width: 180, height: 100 },
      { id: "compare", type: "compare", title: "Trust threshold", operator: "gte", value_type: "int", value: 33, x: 500, y: 160, width: 180, height: 100 },
      { id: "tag", type: "has_entity_tag", title: "Ready", target_entity_id: "world", tag: "flag.ready", x: 300, y: 0, width: 180, height: 100 },
      { id: "and", type: "and", title: "All", x: 520, y: 0, width: 160, height: 90 },
      { id: "condition", type: "condition", title: "Branch", x: 720, y: 0, width: 160, height: 100 },
      { id: "valid", type: "end", title: "Valid", precondition_result: true, x: 940, y: -50, width: 100, height: 80 },
      { id: "reject", type: "end", title: "Reject", precondition_result: false, x: 940, y: 80, width: 100, height: 80 },
    ],
    edges: [
      { id: "start-condition", from_node: "start", from_port: "output", to_node: "condition", to_port: "input", kind: "flow" },
      { id: "entity-property", from_node: "entity", from_port: "output", to_node: "property", to_port: "entity", kind: "data" },
      { id: "property-compare", from_node: "property", from_port: "output", to_node: "compare", to_port: "input", kind: "data" },
      { id: "tag-and", from_node: "tag", from_port: "output", to_node: "and", to_port: "input", kind: "data" },
      { id: "compare-and", from_node: "compare", from_port: "output", to_node: "and", to_port: "input", kind: "data" },
      { id: "and-condition", from_node: "and", from_port: "output", to_node: "condition", to_port: "input", kind: "data" },
      { id: "condition-valid", from_node: "condition", from_port: "true", to_node: "valid", to_port: "input", kind: "flow" },
      { id: "condition-reject", from_node: "condition", from_port: "false", to_node: "reject", to_port: "input", kind: "flow" },
    ],
  };
  expect(validatePreconditionGraph(graph)).toEqual([]);
  const storyGraph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "tag", type: "has_entity_tag", title: "Ready", target_entity_id: "world", tag: "flag.ready", x: 120, y: 0, width: 180, height: 100 },
      { id: "tag2", type: "has_entity_tag", title: "Second", target_entity_id: "world", tag: "flag.second", x: 120, y: 120, width: 180, height: 100 },
      { id: "or", type: "or", title: "Either", x: 320, y: 50, width: 160, height: 90 },
      { id: "condition", type: "condition", title: "Switch", x: 520, y: 0, width: 160, height: 100 },
      { id: "yes", type: "end", title: "Yes", x: 740, y: -40, width: 100, height: 80 },
      { id: "no", type: "end", title: "No", x: 740, y: 60, width: 100, height: 80 },
    ],
    edges: [
      { id: "start-condition", from_node: "start", from_port: "output", to_node: "condition", to_port: "input", kind: "flow" },
      { id: "tag-or", from_node: "tag", from_port: "output", to_node: "or", to_port: "input", kind: "data" },
      { id: "tag2-or", from_node: "tag2", from_port: "output", to_node: "or", to_port: "input", kind: "data" },
      { id: "or-condition", from_node: "or", from_port: "output", to_node: "condition", to_port: "input", kind: "data" },
      { id: "condition-yes", from_node: "condition", from_port: "true", to_node: "yes", to_port: "input", kind: "flow" },
      { id: "condition-no", from_node: "condition", from_port: "false", to_node: "no", to_port: "input", kind: "flow" },
    ],
  };
  expect(validateNarrativeGraph(storyGraph)).toEqual([]);
  expect(validatePreconditionGraph(storyGraph)).toEqual([]);
});
test("Apply Command authors a typed literal and receives an Entity input", () => {
  const graph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "entity", type: "get_entity", title: "World", entity_path: "world", x: 120, y: 100, width: 160, height: 100 },
      { id: "owner", type: "get_entity_property", title: "Owner", property_key: "owner", value_type: "entity", x: 300, y: 100, width: 180, height: 100 },
      { id: "command", type: "apply_command", title: "Set Courage", property_key: "courage", operation: "set", value_type: "int", value: 4, x: 520, y: 0, width: 180, height: 120 },
      { id: "end", type: "end", title: "End", x: 760, y: 0, width: 100, height: 80 },
    ],
    edges: [
      { id: "start-command", from_node: "start", from_port: "output", to_node: "command", to_port: "input", kind: "flow" },
      { id: "command-end", from_node: "command", from_port: "output", to_node: "end", to_port: "input", kind: "flow" },
      { id: "entity-owner", from_node: "entity", from_port: "output", to_node: "owner", to_port: "entity", kind: "data" },
      { id: "owner-command", from_node: "owner", from_port: "output", to_node: "command", to_port: "entity", kind: "data" },
    ],
  };
  expect(validateNarrativeGraph(graph)).toEqual([]);
  expect(validateNarrativeGraph({
    ...graph,
    nodes: graph.nodes.map((node) => node.id === "command" ? { ...node, operation: "toggle" as const } : node),
  })).toContain("Apply Command node Set Courage can only toggle boolean properties.");
  expect(validateNarrativeGraph({
    ...graph,
    edges: graph.edges.filter((edge) => edge.id !== "owner-command"),
  })).toContain("Apply Command node Set Courage must have exactly one Entity data input.");
});
test("Apply Command operations follow the selected value type", () => {
  expect(applyCommandOperationsForType("bool").map(({ value }) => value)).toEqual(["set", "toggle"]);
  expect(applyCommandOperationsForType("int").map(({ value }) => value)).toEqual(["set", "add", "subtract"]);
  expect(applyCommandOperationsForType("float").map(({ value }) => value)).toEqual(["set", "add", "subtract"]);
  expect(applyCommandOperationsForType("string").map(({ value }) => value)).toEqual(["set"]);
});

test("Entity tag nodes accept only EntityRef target data inputs", () => {
  const graph: NarrativeGraph = {
    nodes: [
      { id: "start", type: "start", title: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "entity", type: "get_entity", title: "Actor", entity_path: "actor", x: 120, y: 160, width: 160, height: 100 },
      { id: "add", type: "add_entity_tag", title: "Add", target_entity_id: "world", tag: "flag.ready", x: 300, y: 0, width: 160, height: 100 },
      { id: "remove", type: "remove_entity_tag", title: "Remove", target_entity_id: "world", tag: "flag.ready", x: 480, y: 0, width: 160, height: 100 },
      { id: "has", type: "has_entity_tag", title: "Has", target_entity_id: "world", tag: "flag.ready", x: 660, y: 160, width: 160, height: 100 },
      { id: "condition", type: "condition", title: "Branch", x: 840, y: 0, width: 160, height: 100 },
      { id: "yes", type: "end", title: "Yes", x: 1060, y: -40, width: 100, height: 80 },
      { id: "no", type: "end", title: "No", x: 1060, y: 80, width: 100, height: 80 },
    ],
    edges: [
      { id: "start-add", from_node: "start", from_port: "output", to_node: "add", to_port: "input", kind: "flow" },
      { id: "add-remove", from_node: "add", from_port: "output", to_node: "remove", to_port: "input", kind: "flow" },
      { id: "remove-condition", from_node: "remove", from_port: "output", to_node: "condition", to_port: "input", kind: "flow" },
      { id: "condition-yes", from_node: "condition", from_port: "true", to_node: "yes", to_port: "input", kind: "flow" },
      { id: "condition-no", from_node: "condition", from_port: "false", to_node: "no", to_port: "input", kind: "flow" },
      { id: "entity-add-target", from_node: "entity", from_port: "output", to_node: "add", to_port: "target", kind: "data" },
      { id: "entity-remove-target", from_node: "entity", from_port: "output", to_node: "remove", to_port: "target", kind: "data" },
      { id: "entity-has-target", from_node: "entity", from_port: "output", to_node: "has", to_port: "target", kind: "data" },
      { id: "has-condition", from_node: "has", from_port: "output", to_node: "condition", to_port: "input", kind: "data" },
    ],
  };
  expect(validateNarrativeGraph(graph)).toEqual([]);
  expect(validateNarrativeGraph({
    ...graph,
    edges: graph.edges.map((edge) => edge.id === "entity-add-target" ? { ...edge, from_node: "has" } : edge),
  }).some((issue) => issue.includes("invalid port or incompatible type"))).toBe(true);
});
