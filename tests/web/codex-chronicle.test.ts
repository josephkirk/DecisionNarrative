import { expect, test } from "bun:test";
import { isChronicleEntry, sortChronicle } from "../../src/web/studio/chronicleModel.ts";
import { graphNodeTypes, validateNarrativeGraph, type NarrativeGraph } from "../../src/web/studio/narrativeGraphTypes.ts";
import type { EventRecord } from "../../src/web/studio/contracts.ts";

const event = (id: string, over: Partial<EventRecord> = {}): EventRecord => ({
  id, title: id, description: "", event_type: "canon", trigger: {}, effects: {}, ...over,
});

test("isChronicleEntry is true only for canon events with a story time", () => {
  expect(isChronicleEntry(event("a", { story_time: 10 }))).toBe(true);
  expect(isChronicleEntry(event("b", { story_time: 0 }))).toBe(true);
  expect(isChronicleEntry(event("c"))).toBe(false);
  expect(isChronicleEntry(event("d", { event_type: "world", story_time: 5 }))).toBe(false);
  expect(isChronicleEntry(event("e", { story_time: null as unknown as undefined }))).toBe(false);
});

test("sortChronicle orders by story_time, then id, and drops non-entries", () => {
  const sorted = sortChronicle([
    event("z", { story_time: 20 }),
    event("b", { story_time: 10 }),
    event("a", { story_time: 10 }),
    event("w", { event_type: "world", story_time: 1 }),
    event("u"),
  ]);
  expect(sorted.map((e) => e.id)).toEqual(["a", "b", "z"]);
});

test("reveal nodes are story-graph only", () => {
  expect(graphNodeTypes("story")).toContain("reveal");
  expect(graphNodeTypes("preconditions")).not.toContain("reveal");
});

test("a reveal node must name an event", () => {
  const graph: NarrativeGraph = {
    nodes: [
      { id: "s", type: "start", title: "Start", text: "Start", x: 0, y: 0, width: 100, height: 80 },
      { id: "r", type: "reveal", title: "Tell", event_id: "", kind: "recollection", visibility: "private", x: 0, y: 0, width: 100, height: 80 },
      { id: "e", type: "end", title: "End", x: 0, y: 0, width: 100, height: 80 },
    ],
    edges: [
      { id: "a", kind: "flow", from_node: "s", from_port: "output", to_node: "r", to_port: "input" },
      { id: "b", kind: "flow", from_node: "r", from_port: "output", to_node: "e", to_port: "input" },
    ],
  };
  expect(validateNarrativeGraph(graph).some((issue) => issue.includes("Reveal node Tell must choose a canon event"))).toBe(true);
  graph.nodes[1].event_id = "flood";
  expect(validateNarrativeGraph(graph).some((issue) => issue.includes("Reveal"))).toBe(false);
});
