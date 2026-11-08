import type { TraitInstance, TraitTemplateRecord } from "../../contracts.ts";
import type { AnyEntity } from "../runtime.ts";

export type TreeDirection = "right" | "left" | "down" | "up" | "radial";

export interface TreeEdge {
  from: string;
  to: string;
}
export interface TreePoint { x: number; y: number }
export type TreePositions = Partial<Record<TreeDirection, Record<string, TreePoint>>>;


export interface TreeGraph {
  nodes: string[];
  edges: TreeEdge[];
  direction: TreeDirection;
  positions?: TreePositions;
}

export const TREE_GRAPH_KEY = "tree_graph";
export const TREE_ITEM_KEY = "tree_item";

export const CARD_WIDTH = 164;
export const CARD_HEIGHT = 68;
export const PADDING = 40;
export const LEVEL_GAP = 240;
export const ITEM_GAP = 110;

const VALID_DIRECTIONS: readonly TreeDirection[] = ["right", "left", "down", "up", "radial"];

export function treeGraphTemplate(): TraitTemplateRecord {
  return {
    id: TREE_GRAPH_KEY,
    key: TREE_GRAPH_KEY,
    name: "TreeGraph",
    allow_multiple: false,
    properties: [
      { id: "nodes", key: "nodes", name: "Nodes", type: "entity", is_array: true, default: [] },
      { id: "edges", key: "edges", name: "Edges", type: "string", is_array: true, default: [] },
      { id: "direction", key: "direction", name: "Direction", type: "string", default: "right" },
      { id: "positions", key: "positions", name: "Saved layouts", type: "string", default: "" },
    ],
  };
}

export function treeItemTemplate(): TraitTemplateRecord {
  return {
    id: TREE_ITEM_KEY,
    key: TREE_ITEM_KEY,
    name: "TreeItem",
    properties: [],
    allow_multiple: false,
  };
}

export function treeTrait(entity: AnyEntity, templates: TraitTemplateRecord[], key: string = TREE_GRAPH_KEY): TraitInstance | undefined {
  const template = templates.find((candidate) => candidate.key === key);
  return template ? entity.traits?.find((instance) => instance.template_id === template.id) : undefined;
}

export function treeGraphOf(instance: TraitInstance): TreeGraph {
  const values = instance?.values ?? {};

  let nodes: string[];
  if (values.nodes === undefined) {
    nodes = [];
  } else if (!Array.isArray(values.nodes)) {
    throw new Error(`Malformed nodes: expected an array of strings, received ${typeof values.nodes}.`);
  } else {
    for (let i = 0; i < values.nodes.length; i++) {
      const item = values.nodes[i];
      if (typeof item !== "string") {
        throw new Error(`Malformed node in nodes array at index ${i}: expected string, received ${typeof item}.`);
      }
    }
    nodes = [...(values.nodes as string[])];
  }

  let edges: TreeEdge[];
  if (values.edges === undefined) {
    edges = [];
  } else if (!Array.isArray(values.edges)) {
    throw new Error(`Malformed edges: expected an array of strings, received ${typeof values.edges}.`);
  } else {
    edges = (values.edges as unknown[]).map((raw, index) => {
      if (typeof raw !== "string") {
        throw new Error(`Malformed edge at index ${index}: expected a JSON-encoded string, received ${typeof raw}.`);
      }
      let parsed: unknown;
      try {
        parsed = JSON.parse(raw);
      } catch (err) {
        throw new Error(`Malformed edge JSON at index ${index}: ${raw} (${err instanceof Error ? err.message : String(err)})`);
      }
      if (!Array.isArray(parsed) || parsed.length !== 2 || typeof parsed[0] !== "string" || typeof parsed[1] !== "string") {
        throw new Error(`Malformed edge structure at index ${index}: expected [from, to] string pair, received ${raw}.`);
      }
      return { from: parsed[0], to: parsed[1] };
    });
  }

  let direction: TreeDirection;
  if (values.direction === undefined) {
    direction = "right";
  } else if (typeof values.direction !== "string" || !VALID_DIRECTIONS.includes(values.direction as TreeDirection)) {
    throw new Error(`Malformed direction: expected one of ${VALID_DIRECTIONS.join(", ")}, received ${String(values.direction)}.`);
  } else {
    direction = values.direction as TreeDirection;
  }

  let positions: TreePositions = {};
  if (values.positions !== undefined && (typeof values.positions !== "string" || values.positions.trim() !== "")) {
    if (typeof values.positions !== "string") throw new Error("Malformed saved layouts: expected a JSON string.");
    let data: unknown;
    try { data = JSON.parse(values.positions); } catch { throw new Error("Malformed saved layouts: invalid JSON."); }
    if (!data || typeof data !== "object" || Array.isArray(data)) throw new Error("Malformed saved layouts: expected an object.");
    const nodeIds = new Set(nodes);
    positions = Object.create(null) as TreePositions;
    for (const [view, raw] of Object.entries(data)) {
      if (!VALID_DIRECTIONS.includes(view as TreeDirection) || !raw || typeof raw !== "object" || Array.isArray(raw)) throw new Error("Malformed saved layout view.");
      const coordinates: Record<string, TreePoint> = Object.create(null);
      for (const [id, point] of Object.entries(raw)) {
        if (!nodeIds.has(id) || !point || typeof point !== "object" || Array.isArray(point)) throw new Error(`Invalid saved position for '${id}'.`);
        if (Object.keys(point).some((key) => key !== "x" && key !== "y")) throw new Error(`Invalid saved position for '${id}'.`);
        const { x, y } = point as Record<string, unknown>;
        if (typeof x !== "number" || typeof y !== "number" || !Number.isFinite(x) || !Number.isFinite(y)) throw new Error(`Invalid saved position for '${id}'.`);
        coordinates[id] = { x, y };
      }
      positions[view as TreeDirection] = coordinates;
    }
  }
  return { nodes, edges, direction, positions };
}

export function setTreeGraph(entity: AnyEntity, instance: TraitInstance, graph: TreeGraph): Partial<AnyEntity> {
  const encodedEdges = graph.edges.map((edge) => JSON.stringify([edge.from, edge.to]));
  return {
    traits: (entity.traits ?? []).map((trait) =>
      trait.id === instance.id
        ? {
            ...trait,
            values: {
              ...trait.values,
              nodes: [...graph.nodes],
              edges: encodedEdges,
              direction: graph.direction,
              positions: JSON.stringify(graph.positions ?? {}),
            },
          }
        : trait,
    ),
  };
}

export function treeErrors(graph: TreeGraph, eligibleIds: ReadonlySet<string>): string[] {
  const errors: string[] = [];

  if (!VALID_DIRECTIONS.includes(graph.direction)) {
    errors.push(`Invalid tree direction: ${graph.direction}.`);
  }

  if (graph.nodes.some((id) => typeof id !== "string" || !id)) {
    errors.push("Node IDs must be nonempty strings.");
  }

  if (new Set(graph.nodes).size !== graph.nodes.length) {
    errors.push("Node IDs must be unique.");
  }

  if (graph.nodes.some((id) => !eligibleIds.has(id))) {
    errors.push("Every node must reference an eligible entity.");
  }

  const nodeSet = new Set(graph.nodes);
  if (graph.edges.some((edge) => !nodeSet.has(edge.from) || !nodeSet.has(edge.to))) {
    errors.push("Edge endpoints must exist in the graph nodes.");
  }

  const seenEdges = new Set<string>();
  let hasDuplicate = false;
  for (const edge of graph.edges) {
    const key = `${edge.from}\0${edge.to}`;
    if (seenEdges.has(key)) {
      hasDuplicate = true;
      break;
    }
    seenEdges.add(key);
  }
  if (hasDuplicate) {
    errors.push("Duplicate edges are not allowed.");
  }

  return errors;
}

export function treeRoots(graph: TreeGraph): string[] {
  const targets = new Set(graph.edges.map((e) => e.to));
  return graph.nodes.filter((node) => !targets.has(node));
}

export interface TreeLayout {
  positions: Record<string, { x: number; y: number }>;
  width: number;
  height: number;
}

export function layoutTree(graph: TreeGraph): TreeLayout {
  if (!graph.nodes.length) {
    return { positions: {}, width: 320, height: 180 };
  }

  const nodeSet = new Set(graph.nodes);
  const depth = new Map<string, number>();

  const outgoing = new Map<string, string[]>();
  for (const node of graph.nodes) {
    outgoing.set(node, []);
  }
  for (const edge of graph.edges) {
    if (nodeSet.has(edge.from) && nodeSet.has(edge.to)) {
      outgoing.get(edge.from)?.push(edge.to);
    }
  }

  const roots = treeRoots(graph);
  const queue: string[] = [];
  let cursor = 0;
  const visit = (node: string, level: number) => {
    if (!depth.has(node)) {
      depth.set(node, level);
      queue.push(node);
    }
  };
  const drain = () => {
    while (cursor < queue.length) {
      const current = queue[cursor++]!;
      const nextDepth = depth.get(current)! + 1;
      for (const child of outgoing.get(current) ?? []) visit(child, nextDepth);
    }
  };
  for (const root of roots) visit(root, 0);
  drain();
  // A component containing only cycles has no indegree-zero root.
  for (const node of graph.nodes) {
    if (!depth.has(node)) {
      visit(node, 0);
      drain();
    }
  }

  // Find max level
  let maxLevel = 0;
  for (const d of depth.values()) {
    if (d > maxLevel) maxLevel = d;
  }

  const levels: string[][] = Array.from({ length: maxLevel + 1 }, () => []);
  for (const node of graph.nodes) levels[depth.get(node)!]!.push(node);

  const positions: Record<string, { x: number; y: number }> = Object.create(null);

  if (graph.direction === "radial") {
    // Keep rings separated even when a crowded inner level needs a large radius.
    const ringRadii: number[] = [];
    for (let level = 0; level < levels.length; level++) {
      const count = levels[level]!.length;
      ringRadii.push(level === 0 && count === 1 ? 0 : Math.max(140 + level * 180, (count * 200) / (2 * Math.PI), (ringRadii[level - 1] ?? -40) + 180));
    }
    const maxRadius = ringRadii.at(-1) ?? 0;
    const centre = maxRadius + 120;

    for (let level = 0; level <= maxLevel; level++) {
      const group = levels[level] ?? [];
      const count = group.length;
      const radius = ringRadii[level] ?? 140;
      for (let index = 0; index < count; index++) {
        const node = group[index]!;
        const angle = (2 * Math.PI * index) / count - Math.PI / 2;
        const x = centre + Math.cos(angle) * radius;
        const y = centre + Math.sin(angle) * radius;
        positions[node] = { x, y };
      }
    }
  } else {
    for (let level = 0; level <= maxLevel; level++) {
      const group = levels[level] ?? [];
      for (let index = 0; index < group.length; index++) {
        const node = group[index]!;
        let x = 0;
        let y = 0;
        switch (graph.direction) {
          case "right":
            x = PADDING + level * LEVEL_GAP;
            y = PADDING + index * ITEM_GAP;
            break;
          case "left":
            x = PADDING + (maxLevel - level) * LEVEL_GAP;
            y = PADDING + index * ITEM_GAP;
            break;
          case "down":
            x = PADDING + index * LEVEL_GAP;
            y = PADDING + level * ITEM_GAP;
            break;
          case "up":
            x = PADDING + index * LEVEL_GAP;
            y = PADDING + (maxLevel - level) * ITEM_GAP;
            break;
        }
        positions[node] = { x, y };
      }
    }
  }
  for (const [id, point] of Object.entries(graph.positions?.[graph.direction] ?? {})) {
    if (nodeSet.has(id)) positions[id] = point;
  }

  let maxX = 0;
  let maxY = 0;
  for (const pos of Object.values(positions)) {
    if (pos.x + CARD_WIDTH > maxX) maxX = pos.x + CARD_WIDTH;
    if (pos.y + CARD_HEIGHT > maxY) maxY = pos.y + CARD_HEIGHT;
  }

  const width = Math.ceil(maxX + PADDING);
  const height = Math.ceil(maxY + PADDING);

  return { positions, width, height };
}
