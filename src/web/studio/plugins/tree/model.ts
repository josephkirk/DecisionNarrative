import type { TraitInstance, TraitTemplateRecord } from "../../contracts.ts";
import type { AnyEntity } from "../runtime.ts";

export type TreeDirection = "right" | "left" | "down" | "up" | "radial";
export type TreeShape = "card" | "rounded" | "ellipse" | "diamond";
export const TREE_SHAPES: readonly TreeShape[] = ["card", "rounded", "ellipse", "diamond"];

export interface TreeEdge {
  from: string;
  to: string;
}
export interface TreePoint { x: number; y: number }
export type TreePositions = Partial<Record<TreeDirection, Record<string, TreePoint>>>;


export interface TreeNodeAppearance {
  background?: string;
  outline?: string;
  outlineWidth?: number;
  width?: number;
  height?: number;
}

export interface TreeEdgeAppearance {
  color?: string;
  width?: number;
}

export interface TreeAppearance {
  nodes: Record<string, TreeNodeAppearance>;
  edges: Record<string, TreeEdgeAppearance>;
}

export interface TreeGraph {
  nodes: string[];
  edges: TreeEdge[];
  direction: TreeDirection;
  positions?: TreePositions;
  shapes?: Record<string, TreeShape>;
  appearance?: TreeAppearance;
}

export const TREE_GRAPH_KEY = "tree_graph";
export const TREE_ITEM_KEY = "tree_item";

export const CARD_WIDTH = 164;
export const CARD_HEIGHT = 68;
export const PADDING = 40;
export const LEVEL_GAP = 240;
export const ITEM_GAP = 110;
export const MIN_OUTLINE_WIDTH = 1;
export const MAX_OUTLINE_WIDTH = 12;
export const MIN_NODE_WIDTH = 100;
export const MAX_NODE_WIDTH = 600;
export const MIN_NODE_HEIGHT = 60;
export const MAX_NODE_HEIGHT = 400;
export const MIN_WIRE_WIDTH = 1;
export const MAX_WIRE_WIDTH = 12;
const HEX_COLOR_REGEX = /^#[0-9a-fA-F]{6}$/;

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
      { id: "shapes", key: "shapes", name: "Node shapes", type: "string", default: "" },
      { id: "appearance", key: "appearance", name: "Appearance", type: "string", default: "" },
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
  let shapes: Record<string, TreeShape> | undefined;
  if (values.shapes !== undefined && (typeof values.shapes !== "string" || values.shapes.trim() !== "")) {
    if (typeof values.shapes !== "string") throw new Error("Malformed node shapes: expected a JSON string.");
    let data: unknown;
    try { data = JSON.parse(values.shapes); } catch { throw new Error("Malformed node shapes: invalid JSON."); }
    if (!data || typeof data !== "object" || Array.isArray(data)) throw new Error("Malformed node shapes: expected an object.");
    shapes = Object.create(null) as Record<string, TreeShape>;
    const nodeIds = new Set(nodes);
    for (const [id, shape] of Object.entries(data)) {
      if (!nodeIds.has(id) || typeof shape !== "string" || !TREE_SHAPES.includes(shape as TreeShape)) throw new Error(`Invalid node shape for '${id}'.`);
      shapes[id] = shape as TreeShape;
    }
  }
  let appearance: TreeAppearance | undefined;
  if (values.appearance !== undefined && (typeof values.appearance !== "string" || values.appearance.trim() !== "")) {
    if (typeof values.appearance !== "string") throw new Error("Malformed appearance: expected a JSON string.");
    let data: unknown;
    try { data = JSON.parse(values.appearance); } catch { throw new Error("Malformed appearance: invalid JSON."); }
    if (!data || typeof data !== "object" || Array.isArray(data)) throw new Error("Malformed appearance: expected an object.");
    for (const key of Object.keys(data)) {
      if (key !== "nodes" && key !== "edges") throw new Error(`Malformed appearance: unexpected field '${key}'.`);
    }
    const appearanceMap = data as Record<string, unknown>;
    const rawNodes = appearanceMap.nodes;
    const parsedNodes: Record<string, TreeNodeAppearance> = Object.create(null);
    if (rawNodes !== undefined) {
      if (!rawNodes || typeof rawNodes !== "object" || Array.isArray(rawNodes)) throw new Error("Malformed appearance: nodes must be an object.");
      const nodeIds = new Set(nodes);
      for (const [id, style] of Object.entries(rawNodes)) {
        if (!nodeIds.has(id)) throw new Error(`Invalid appearance: unknown node '${id}'.`);
        if (!style || typeof style !== "object" || Array.isArray(style)) throw new Error(`Invalid appearance for node '${id}': expected an object.`);
        for (const key of Object.keys(style)) {
          if (!["background", "outline", "outlineWidth", "width", "height"].includes(key)) {
            throw new Error(`Invalid appearance for node '${id}': unexpected field '${key}'.`);
          }
        }
        const s = style as Record<string, unknown>;
        const nodeAppearance: TreeNodeAppearance = {};
        if (s.background !== undefined) {
          if (typeof s.background !== "string" || !HEX_COLOR_REGEX.test(s.background)) {
            throw new Error(`Invalid appearance for node '${id}': background must be a hex color in #RRGGBB format.`);
          }
          nodeAppearance.background = s.background;
        }
        if (s.outline !== undefined) {
          if (typeof s.outline !== "string" || !HEX_COLOR_REGEX.test(s.outline)) {
            throw new Error(`Invalid appearance for node '${id}': outline must be a hex color in #RRGGBB format.`);
          }
          nodeAppearance.outline = s.outline;
        }
        if (s.outlineWidth !== undefined) {
          if (typeof s.outlineWidth !== "number" || !Number.isFinite(s.outlineWidth) || s.outlineWidth < MIN_OUTLINE_WIDTH || s.outlineWidth > MAX_OUTLINE_WIDTH) {
            throw new Error(`Invalid appearance for node '${id}': outlineWidth must be a finite number between ${MIN_OUTLINE_WIDTH} and ${MAX_OUTLINE_WIDTH}.`);
          }
          nodeAppearance.outlineWidth = s.outlineWidth;
        }
        if (s.width !== undefined) {
          if (typeof s.width !== "number" || !Number.isFinite(s.width) || s.width < MIN_NODE_WIDTH || s.width > MAX_NODE_WIDTH) {
            throw new Error(`Invalid appearance for node '${id}': width must be a finite number between ${MIN_NODE_WIDTH} and ${MAX_NODE_WIDTH}.`);
          }
          nodeAppearance.width = s.width;
        }
        if (s.height !== undefined) {
          if (typeof s.height !== "number" || !Number.isFinite(s.height) || s.height < MIN_NODE_HEIGHT || s.height > MAX_NODE_HEIGHT) {
            throw new Error(`Invalid appearance for node '${id}': height must be a finite number between ${MIN_NODE_HEIGHT} and ${MAX_NODE_HEIGHT}.`);
          }
          nodeAppearance.height = s.height;
        }
        parsedNodes[id] = nodeAppearance;
      }
    }
    const rawEdges = appearanceMap.edges;
    const parsedEdges: Record<string, TreeEdgeAppearance> = Object.create(null);
    if (rawEdges !== undefined) {
      if (!rawEdges || typeof rawEdges !== "object" || Array.isArray(rawEdges)) throw new Error("Malformed appearance: edges must be an object.");
      const validEdgeKeys = new Set(edges.map((e) => JSON.stringify([e.from, e.to])));
      for (const [edgeKey, style] of Object.entries(rawEdges)) {
        let parsedEdgeKey: unknown;
        try { parsedEdgeKey = JSON.parse(edgeKey); } catch { throw new Error(`Invalid appearance: malformed edge key '${edgeKey}'.`); }
        if (!Array.isArray(parsedEdgeKey) || parsedEdgeKey.length !== 2 || typeof parsedEdgeKey[0] !== "string" || typeof parsedEdgeKey[1] !== "string") {
          throw new Error(`Invalid appearance: malformed edge key '${edgeKey}'.`);
        }
        if (!validEdgeKeys.has(edgeKey)) throw new Error(`Invalid appearance: unknown edge '${edgeKey}'.`);
        if (!style || typeof style !== "object" || Array.isArray(style)) throw new Error(`Invalid appearance for edge '${edgeKey}': expected an object.`);
        for (const key of Object.keys(style)) {
          if (!["color", "width"].includes(key)) {
            throw new Error(`Invalid appearance for edge '${edgeKey}': unexpected field '${key}'.`);
          }
        }
        const s = style as Record<string, unknown>;
        const edgeAppearance: TreeEdgeAppearance = {};
        if (s.color !== undefined) {
          if (typeof s.color !== "string" || !HEX_COLOR_REGEX.test(s.color)) {
            throw new Error(`Invalid appearance for edge '${edgeKey}': color must be a hex color in #RRGGBB format.`);
          }
          edgeAppearance.color = s.color;
        }
        if (s.width !== undefined) {
          if (typeof s.width !== "number" || !Number.isFinite(s.width) || s.width < MIN_WIRE_WIDTH || s.width > MAX_WIRE_WIDTH) {
            throw new Error(`Invalid appearance for edge '${edgeKey}': width must be a finite number between ${MIN_WIRE_WIDTH} and ${MAX_WIRE_WIDTH}.`);
          }
          edgeAppearance.width = s.width;
        }
        parsedEdges[edgeKey] = edgeAppearance;
      }
    }
    appearance = { nodes: parsedNodes, edges: parsedEdges };
  }
  return { nodes, edges, direction, positions, ...(shapes ? { shapes } : {}), ...(appearance ? { appearance } : {}) };
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
              ...(graph.shapes !== undefined || "shapes" in trait.values || "shapes" in instance.values
                ? { shapes: JSON.stringify(graph.shapes ?? {}) }
                : {}),
              ...(graph.appearance !== undefined || "appearance" in trait.values || "appearance" in instance.values
                ? {
                    appearance: JSON.stringify({
                      nodes: graph.appearance?.nodes ?? {},
                      edges: graph.appearance?.edges ?? {},
                    }),
                  }
                : {}),
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

  if (graph.appearance) {
    if (graph.appearance.nodes) {
      for (const [id, style] of Object.entries(graph.appearance.nodes)) {
        if (!nodeSet.has(id)) {
          errors.push(`Appearance references unknown node '${id}'.`);
        }
        if (style.background !== undefined && !HEX_COLOR_REGEX.test(style.background)) {
          errors.push(`Invalid appearance background for node '${id}'.`);
        }
        if (style.outline !== undefined && !HEX_COLOR_REGEX.test(style.outline)) {
          errors.push(`Invalid appearance outline for node '${id}'.`);
        }
        if (style.outlineWidth !== undefined && (!Number.isFinite(style.outlineWidth) || style.outlineWidth < MIN_OUTLINE_WIDTH || style.outlineWidth > MAX_OUTLINE_WIDTH)) {
          errors.push(`Invalid appearance outlineWidth for node '${id}'.`);
        }
        if (style.width !== undefined && (!Number.isFinite(style.width) || style.width < MIN_NODE_WIDTH || style.width > MAX_NODE_WIDTH)) {
          errors.push(`Invalid appearance width for node '${id}'.`);
        }
        if (style.height !== undefined && (!Number.isFinite(style.height) || style.height < MIN_NODE_HEIGHT || style.height > MAX_NODE_HEIGHT)) {
          errors.push(`Invalid appearance height for node '${id}'.`);
        }
      }
    }
    if (graph.appearance.edges) {
      const validEdgeKeys = new Set(graph.edges.map((e) => JSON.stringify([e.from, e.to])));
      for (const [edgeKey, style] of Object.entries(graph.appearance.edges)) {
        if (!validEdgeKeys.has(edgeKey)) {
          errors.push(`Appearance references unknown edge '${edgeKey}'.`);
        }
        if (style.color !== undefined && !HEX_COLOR_REGEX.test(style.color)) {
          errors.push(`Invalid appearance color for edge '${edgeKey}'.`);
        }
        if (style.width !== undefined && (!Number.isFinite(style.width) || style.width < MIN_WIRE_WIDTH || style.width > MAX_WIRE_WIDTH)) {
          errors.push(`Invalid appearance width for edge '${edgeKey}'.`);
        }
      }
    }
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

  let maxNodeWidth = CARD_WIDTH;
  let maxNodeHeight = CARD_HEIGHT;
  if (graph.appearance?.nodes) {
    for (const node of graph.nodes) {
      const style = graph.appearance.nodes[node];
      if (style?.width && style.width > maxNodeWidth) maxNodeWidth = style.width;
      if (style?.height && style.height > maxNodeHeight) maxNodeHeight = style.height;
    }
  }

  const positions: Record<string, { x: number; y: number }> = Object.create(null);

  if (graph.direction === "radial") {
    // Keep rings separated even when a crowded inner level needs a large radius.
    const ringStep = 180 + Math.max(0, maxNodeHeight - CARD_HEIGHT, maxNodeWidth - CARD_WIDTH);
    const itemArc = 200 + Math.max(0, maxNodeWidth - CARD_WIDTH, maxNodeHeight - CARD_HEIGHT);
    const ringRadii: number[] = [];
    for (let level = 0; level < levels.length; level++) {
      const count = levels[level]!.length;
      ringRadii.push(
        level === 0 && count === 1
          ? 0
          : Math.max(
              140 + level * ringStep,
              (count * itemArc) / (2 * Math.PI),
              (ringRadii[level - 1] ?? -40) + ringStep,
            ),
      );
    }
    const maxRadius = ringRadii.at(-1) ?? 0;
    const centre = maxRadius + 120 + Math.max(0, (maxNodeWidth - CARD_WIDTH) / 2, (maxNodeHeight - CARD_HEIGHT) / 2);

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
    const rankGapH = LEVEL_GAP + Math.max(0, maxNodeWidth - CARD_WIDTH);
    const itemGapH = ITEM_GAP + Math.max(0, maxNodeHeight - CARD_HEIGHT);
    const itemGapV = LEVEL_GAP + Math.max(0, maxNodeWidth - CARD_WIDTH);
    const rankGapV = ITEM_GAP + Math.max(0, maxNodeHeight - CARD_HEIGHT);

    for (let level = 0; level <= maxLevel; level++) {
      const group = levels[level] ?? [];
      for (let index = 0; index < group.length; index++) {
        const node = group[index]!;
        let x = 0;
        let y = 0;
        switch (graph.direction) {
          case "right":
            x = PADDING + level * rankGapH;
            y = PADDING + index * itemGapH;
            break;
          case "left":
            x = PADDING + (maxLevel - level) * rankGapH;
            y = PADDING + index * itemGapH;
            break;
          case "down":
            x = PADDING + index * itemGapV;
            y = PADDING + level * rankGapV;
            break;
          case "up":
            x = PADDING + index * itemGapV;
            y = PADDING + (maxLevel - level) * rankGapV;
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
  for (const [id, pos] of Object.entries(positions)) {
    const nodeWidth = graph.appearance?.nodes?.[id]?.width ?? CARD_WIDTH;
    const nodeHeight = graph.appearance?.nodes?.[id]?.height ?? CARD_HEIGHT;
    if (pos.x + nodeWidth > maxX) maxX = pos.x + nodeWidth;
    if (pos.y + nodeHeight > maxY) maxY = pos.y + nodeHeight;
  }

  const width = Math.ceil(maxX + PADDING);
  const height = Math.ceil(maxY + PADDING);

  return { positions, width, height };
}
