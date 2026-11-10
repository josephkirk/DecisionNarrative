import type { TraitInstance, TraitTemplateRecord } from "../../contracts.ts";
import type { AnyEntity } from "../runtime.ts";

export type TreeDirection = "right" | "left" | "down" | "up" | "radial";
export type TreeShape = "card" | "rounded" | "ellipse" | "diamond";
export const TREE_SHAPES: readonly TreeShape[] = ["card", "rounded", "ellipse", "diamond"];

export interface TreeEdge {
  from: string;
  to: string;
  relationship?: string;
}
export interface TreePoint { x: number; y: number }
export type TreePositions = Partial<Record<TreeDirection, Record<string, TreePoint>>>;


export interface TreeNodeAppearance {
  background?: string;
  outline?: string;
  outlineWidth?: number;
  width?: number;
  height?: number;
  locked?: boolean;
}

export type TreeWirePattern = "solid" | "dash" | "dot" | "brush";
export const TREE_WIRE_PATTERNS: readonly TreeWirePattern[] = ["solid", "dash", "dot", "brush"];

export interface TreeEdgeAppearance {
  color?: string;
  width?: number;
  pattern?: TreeWirePattern;
}

export interface TreeAppearanceDefaults {
  node?: Omit<TreeNodeAppearance, "locked"> & { shape?: TreeShape };
  edge?: TreeEdgeAppearance;
  customCss?: string;
}

export interface TreeCanvasImage {
  id: string;
  src: string;
  x: number;
  y: number;
  width: number;
  height: number;
  locked: boolean;
}

export interface TreeAppearance {
  canvasBackground?: string;
  defaults?: TreeAppearanceDefaults;
  nodes: Record<string, TreeNodeAppearance>;
  edges: Record<string, TreeEdgeAppearance>;
  images?: TreeCanvasImage[];
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

export function isValidImageSrc(reference: string): boolean {
  const trimmed = reference.trim();
  if (!trimmed) return false;
  if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
    return true;
  }
  if (trimmed.startsWith("/") || trimmed.includes("\\")) {
    return false;
  }
  const segments = trimmed.split("/");
  for (const segment of segments) {
    if (!segment || segment === "." || segment === "..") {
      return false;
    }
  }
  const lower = trimmed.toLowerCase();
  return lower.endsWith(".png") || lower.endsWith(".jpg") || lower.endsWith(".jpeg");
}

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
    properties: [
      { id: "level", key: "level", name: "Level", type: "int", default: 0, min: 0 },
      { id: "cost", key: "cost", name: "Cost", type: "int", default: 0, min: 0 },
      { id: "cost_entity", key: "cost_entity", name: "Cost entity", type: "entity", default: null },
      { id: "gameplay_tags_add", key: "gameplay_tags_add", name: "Gameplay tags added", type: "tag", is_array: true, default: [] },
      { id: "gameplay_tags_remove", key: "gameplay_tags_remove", name: "Gameplay tags removed", type: "tag", is_array: true, default: [] },
      { id: "description", key: "description", name: "Description", type: "string", multiline: true, default: "" },
      { id: "asset", key: "asset", name: "Asset", type: "asset", default: null },
      { id: "entity", key: "entity", name: "Gameplay entity", type: "entity", default: null },
    ],
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
      if (!Array.isArray(parsed) || (parsed.length !== 2 && parsed.length !== 3) || typeof parsed[0] !== "string" || typeof parsed[1] !== "string") {
        throw new Error(`Malformed edge structure at index ${index}: expected [from, to] or [from, to, relationship] string array, received ${raw}.`);
      }
      if (parsed.length === 3) {
        if (typeof parsed[2] !== "string" || parsed[2].trim() === "") {
          throw new Error(`Malformed edge relationship at index ${index}: expected non-empty string, received ${JSON.stringify(parsed[2])}.`);
        }
        return { from: parsed[0], to: parsed[1], relationship: parsed[2] };
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
      if (key !== "nodes" && key !== "edges" && key !== "canvasBackground" && key !== "defaults" && key !== "images") throw new Error(`Malformed appearance: unexpected field '${key}'.`);
    }
    const appearanceMap = data as Record<string, unknown>;
    const canvasBackground = appearanceMap.canvasBackground;
    if (canvasBackground !== undefined && (typeof canvasBackground !== "string" || !HEX_COLOR_REGEX.test(canvasBackground))) {
      throw new Error("Invalid canvas background: expected a hex color in #RRGGBB format.");
    }
    const rawDefaults = appearanceMap.defaults;
    let parsedDefaults: TreeAppearanceDefaults | undefined;
    if (rawDefaults !== undefined) {
      if (!rawDefaults || typeof rawDefaults !== "object" || Array.isArray(rawDefaults)) throw new Error("Malformed appearance: defaults must be an object.");
      for (const key of Object.keys(rawDefaults)) {
        if (key !== "node" && key !== "edge" && key !== "customCss") {
          throw new Error(`Invalid appearance defaults: unexpected field '${key}'.`);
        }
      }
      const defObj = rawDefaults as Record<string, unknown>;
      let parsedNodeDefault: (TreeNodeAppearance & { shape?: TreeShape }) | undefined;
      if (defObj.node !== undefined) {
        if (!defObj.node || typeof defObj.node !== "object" || Array.isArray(defObj.node)) {
          throw new Error("Invalid appearance defaults: node must be an object.");
        }
        for (const key of Object.keys(defObj.node)) {
          if (!["background", "outline", "outlineWidth", "width", "height", "shape"].includes(key)) {
            throw new Error(`Invalid appearance defaults for node: unexpected field '${key}'.`);
          }
        }
        const s = defObj.node as Record<string, unknown>;
        const nodeDef: TreeNodeAppearance & { shape?: TreeShape } = {};
        if (s.background !== undefined) {
          if (typeof s.background !== "string" || !HEX_COLOR_REGEX.test(s.background)) {
            throw new Error("Invalid appearance defaults for node: background must be a hex color in #RRGGBB format.");
          }
          nodeDef.background = s.background;
        }
        if (s.outline !== undefined) {
          if (typeof s.outline !== "string" || !HEX_COLOR_REGEX.test(s.outline)) {
            throw new Error("Invalid appearance defaults for node: outline must be a hex color in #RRGGBB format.");
          }
          nodeDef.outline = s.outline;
        }
        if (s.outlineWidth !== undefined) {
          if (typeof s.outlineWidth !== "number" || !Number.isFinite(s.outlineWidth) || s.outlineWidth < MIN_OUTLINE_WIDTH || s.outlineWidth > MAX_OUTLINE_WIDTH) {
            throw new Error(`Invalid appearance defaults for node: outlineWidth must be a finite number between ${MIN_OUTLINE_WIDTH} and ${MAX_OUTLINE_WIDTH}.`);
          }
          nodeDef.outlineWidth = s.outlineWidth;
        }
        if (s.width !== undefined) {
          if (typeof s.width !== "number" || !Number.isFinite(s.width) || s.width < MIN_NODE_WIDTH || s.width > MAX_NODE_WIDTH) {
            throw new Error(`Invalid appearance defaults for node: width must be a finite number between ${MIN_NODE_WIDTH} and ${MAX_NODE_WIDTH}.`);
          }
          nodeDef.width = s.width;
        }
        if (s.height !== undefined) {
          if (typeof s.height !== "number" || !Number.isFinite(s.height) || s.height < MIN_NODE_HEIGHT || s.height > MAX_NODE_HEIGHT) {
            throw new Error(`Invalid appearance defaults for node: height must be a finite number between ${MIN_NODE_HEIGHT} and ${MAX_NODE_HEIGHT}.`);
          }
          nodeDef.height = s.height;
        }
        if (s.shape !== undefined) {
          if (typeof s.shape !== "string" || !TREE_SHAPES.includes(s.shape as TreeShape)) {
            throw new Error("Invalid appearance defaults for node: unknown shape.");
          }
          nodeDef.shape = s.shape as TreeShape;
        }
        parsedNodeDefault = nodeDef;
      }
      let parsedEdgeDefault: TreeEdgeAppearance | undefined;
      if (defObj.edge !== undefined) {
        if (!defObj.edge || typeof defObj.edge !== "object" || Array.isArray(defObj.edge)) {
          throw new Error("Invalid appearance defaults: edge must be an object.");
        }
        for (const key of Object.keys(defObj.edge)) {
          if (!["color", "width", "pattern"].includes(key)) {
            throw new Error(`Invalid appearance defaults for edge: unexpected field '${key}'.`);
          }
        }
        const s = defObj.edge as Record<string, unknown>;
        const edgeDef: TreeEdgeAppearance = {};
        if (s.color !== undefined) {
          if (typeof s.color !== "string" || !HEX_COLOR_REGEX.test(s.color)) {
            throw new Error("Invalid appearance defaults for edge: color must be a hex color in #RRGGBB format.");
          }
          edgeDef.color = s.color;
        }
        if (s.width !== undefined) {
          if (typeof s.width !== "number" || !Number.isFinite(s.width) || s.width < MIN_WIRE_WIDTH || s.width > MAX_WIRE_WIDTH) {
            throw new Error(`Invalid appearance defaults for edge: width must be a finite number between ${MIN_WIRE_WIDTH} and ${MAX_WIRE_WIDTH}.`);
          }
          edgeDef.width = s.width;
        }
        if (s.pattern !== undefined) {
          if (typeof s.pattern !== "string" || !TREE_WIRE_PATTERNS.includes(s.pattern as TreeWirePattern)) {
            throw new Error("Invalid appearance defaults for edge: unknown wire pattern.");
          }
          edgeDef.pattern = s.pattern as TreeWirePattern;
        }
        parsedEdgeDefault = edgeDef;
      }
      let parsedCustomCss: string | undefined;
      if (defObj.customCss !== undefined) {
        if (typeof defObj.customCss !== "string") {
          throw new Error("Invalid appearance defaults: customCss must be a string.");
        }
        parsedCustomCss = defObj.customCss;
      }
      parsedDefaults = {
        ...(parsedNodeDefault !== undefined ? { node: parsedNodeDefault } : {}),
        ...(parsedEdgeDefault !== undefined ? { edge: parsedEdgeDefault } : {}),
        ...(parsedCustomCss !== undefined ? { customCss: parsedCustomCss } : {}),
      };
    }
    const rawNodes = appearanceMap.nodes;
    const parsedNodes: Record<string, TreeNodeAppearance> = Object.create(null);
    if (rawNodes !== undefined) {
      if (!rawNodes || typeof rawNodes !== "object" || Array.isArray(rawNodes)) throw new Error("Malformed appearance: nodes must be an object.");
      const nodeIds = new Set(nodes);
      for (const [id, style] of Object.entries(rawNodes)) {
        if (!nodeIds.has(id)) throw new Error(`Invalid appearance: unknown node '${id}'.`);
        if (!style || typeof style !== "object" || Array.isArray(style)) throw new Error(`Invalid appearance for node '${id}': expected an object.`);
        for (const key of Object.keys(style)) {
          if (!["background", "outline", "outlineWidth", "width", "height", "locked"].includes(key)) {
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
        if (s.locked !== undefined) {
          if (typeof s.locked !== "boolean") {
            throw new Error(`Invalid appearance for node '${id}': locked must be a boolean.`);
          }
          nodeAppearance.locked = s.locked;
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
          if (!["color", "width", "pattern"].includes(key)) {
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
        if (s.pattern !== undefined) {
          if (typeof s.pattern !== "string" || !TREE_WIRE_PATTERNS.includes(s.pattern as TreeWirePattern)) {
            throw new Error(`Invalid appearance for edge '${edgeKey}': unknown wire pattern.`);
          }
          edgeAppearance.pattern = s.pattern as TreeWirePattern;
        }
        parsedEdges[edgeKey] = edgeAppearance;
      }
    }
    const rawImages = appearanceMap.images;
    let parsedImages: TreeCanvasImage[] | undefined;
    if (rawImages !== undefined) {
      if (!Array.isArray(rawImages)) {
        throw new Error("Malformed appearance: images must be an array.");
      }
      const seenImageIds = new Set<string>();
      parsedImages = rawImages.map((rawImage, index) => {
        if (!rawImage || typeof rawImage !== "object" || Array.isArray(rawImage)) {
          throw new Error(`Invalid appearance image at index ${index}: expected an object.`);
        }
        for (const key of Object.keys(rawImage)) {
          if (!["id", "src", "x", "y", "width", "height", "locked"].includes(key)) {
            throw new Error(`Invalid appearance image at index ${index}: unexpected field '${key}'.`);
          }
        }
        const img = rawImage as Record<string, unknown>;
        if (typeof img.id !== "string" || !img.id.trim()) {
          throw new Error(`Invalid appearance image at index ${index}: id must be a nonempty string.`);
        }
        if (seenImageIds.has(img.id)) {
          throw new Error(`Invalid appearance image: duplicate image id '${img.id}'.`);
        }
        seenImageIds.add(img.id);
        if (typeof img.src !== "string" || !isValidImageSrc(img.src)) {
          throw new Error(`Invalid appearance image '${img.id}': src must be a valid image path or URL.`);
        }
        if (typeof img.x !== "number" || !Number.isFinite(img.x)) {
          throw new Error(`Invalid appearance image '${img.id}': x must be a finite number.`);
        }
        if (typeof img.y !== "number" || !Number.isFinite(img.y)) {
          throw new Error(`Invalid appearance image '${img.id}': y must be a finite number.`);
        }
        if (typeof img.width !== "number" || !Number.isFinite(img.width) || img.width <= 0) {
          throw new Error(`Invalid appearance image '${img.id}': width must be a positive number.`);
        }
        if (typeof img.height !== "number" || !Number.isFinite(img.height) || img.height <= 0) {
          throw new Error(`Invalid appearance image '${img.id}': height must be a positive number.`);
        }
        if (typeof img.locked !== "boolean") {
          throw new Error(`Invalid appearance image '${img.id}': locked must be a boolean.`);
        }
        return {
          id: img.id,
          src: img.src,
          x: img.x,
          y: img.y,
          width: img.width,
          height: img.height,
          locked: img.locked,
        };
      });
    }
    appearance = {
      nodes: parsedNodes,
      edges: parsedEdges,
      ...(canvasBackground !== undefined ? { canvasBackground: canvasBackground as string } : {}),
      ...(parsedDefaults !== undefined ? { defaults: parsedDefaults } : {}),
      ...(parsedImages !== undefined ? { images: parsedImages } : {}),
    };

  }
  return { nodes, edges, direction, positions, ...(shapes ? { shapes } : {}), ...(appearance ? { appearance } : {}) };
}

export function setTreeGraph(entity: AnyEntity, instance: TraitInstance, graph: TreeGraph): Partial<AnyEntity> {
  const encodedEdges = graph.edges.map((edge) => {
    if (typeof edge.relationship === "string" && edge.relationship.trim() !== "") {
      return JSON.stringify([edge.from, edge.to, edge.relationship]);
    }
    return JSON.stringify([edge.from, edge.to]);
  });
  return {
    traits: (entity.traits ?? []).map((trait) => {
      if (trait.id !== instance.id) {
        return trait;
      }

      return {
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
                  ...(graph.appearance?.canvasBackground !== undefined ? { canvasBackground: graph.appearance.canvasBackground } : {}),
                  ...(graph.appearance?.defaults !== undefined ? { defaults: graph.appearance.defaults } : {}),
                  nodes: graph.appearance?.nodes ?? {},
                  edges: graph.appearance?.edges ?? {},
                  ...(graph.appearance?.images !== undefined ? { images: graph.appearance.images } : {}),
                }),
              }
            : {}),
        },
      };
    }),
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

  if (graph.edges.some((edge) => edge.relationship !== undefined && (typeof edge.relationship !== "string" || !edge.relationship.trim()))) {
    errors.push("Edge relationship must be a nonempty string.");
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
    if (graph.appearance.canvasBackground !== undefined && !HEX_COLOR_REGEX.test(graph.appearance.canvasBackground)) {
      errors.push("Invalid canvas background color.");
    }
    if (graph.appearance.defaults) {
      if (graph.appearance.defaults.node) {
        const node = graph.appearance.defaults.node;
        if (node.background !== undefined && !HEX_COLOR_REGEX.test(node.background)) {
          errors.push("Invalid appearance default background for node.");
        }
        if (node.outline !== undefined && !HEX_COLOR_REGEX.test(node.outline)) {
          errors.push("Invalid appearance default outline for node.");
        }
        if (node.outlineWidth !== undefined && (!Number.isFinite(node.outlineWidth) || node.outlineWidth < MIN_OUTLINE_WIDTH || node.outlineWidth > MAX_OUTLINE_WIDTH)) {
          errors.push("Invalid appearance default outlineWidth for node.");
        }
        if (node.width !== undefined && (!Number.isFinite(node.width) || node.width < MIN_NODE_WIDTH || node.width > MAX_NODE_WIDTH)) {
          errors.push("Invalid appearance default width for node.");
        }
        if (node.height !== undefined && (!Number.isFinite(node.height) || node.height < MIN_NODE_HEIGHT || node.height > MAX_NODE_HEIGHT)) {
          errors.push("Invalid appearance default height for node.");
        }
        if (node.shape !== undefined && !TREE_SHAPES.includes(node.shape)) {
          errors.push("Invalid appearance default shape for node.");
        }
        if ((node as unknown as Record<string, unknown>).locked !== undefined) {
          errors.push("Appearance default node cannot define locked.");
        }
      }
      if (graph.appearance.defaults.edge) {
        const edge = graph.appearance.defaults.edge;
        if (edge.color !== undefined && !HEX_COLOR_REGEX.test(edge.color)) {
          errors.push("Invalid appearance default color for edge.");
        }
        if (edge.width !== undefined && (!Number.isFinite(edge.width) || edge.width < MIN_WIRE_WIDTH || edge.width > MAX_WIRE_WIDTH)) {
          errors.push("Invalid appearance default width for edge.");
        }
        if (edge.pattern !== undefined && !TREE_WIRE_PATTERNS.includes(edge.pattern)) {
          errors.push("Invalid appearance default pattern for edge.");
        }
      }
      if (graph.appearance.defaults.customCss !== undefined && typeof graph.appearance.defaults.customCss !== "string") {
        errors.push("Invalid appearance default customCss.");
      }
    }
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
        if (style.locked !== undefined && typeof style.locked !== "boolean") {
          errors.push(`Invalid appearance locked for node '${id}'.`);
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
        if (style.pattern !== undefined && !TREE_WIRE_PATTERNS.includes(style.pattern)) {
          errors.push(`Invalid appearance pattern for edge '${edgeKey}'.`);
        }
      }
    }
    if (graph.appearance.images !== undefined) {
      if (!Array.isArray(graph.appearance.images)) {
        errors.push("Appearance images must be an array.");
      } else {
        const seenImageIds = new Set<string>();
        for (const image of graph.appearance.images) {
          if (!image || typeof image !== "object" || Array.isArray(image)) {
            errors.push("Invalid appearance image.");
            continue;
          }
          if (typeof image.id !== "string" || !image.id.trim()) {
            errors.push("Appearance image ID must be a nonempty string.");
          } else if (seenImageIds.has(image.id)) {
            errors.push(`Duplicate appearance image ID '${image.id}'.`);
          } else {
            seenImageIds.add(image.id);
          }
          if (typeof image.src !== "string" || !isValidImageSrc(image.src)) {
            errors.push(`Invalid appearance image src '${image?.src}'.`);
          }
          if (typeof image.x !== "number" || !Number.isFinite(image.x) || typeof image.y !== "number" || !Number.isFinite(image.y)) {
            errors.push(`Appearance image '${image.id || "unknown"}' coordinates must be finite numbers.`);
          }
          if (typeof image.width !== "number" || !Number.isFinite(image.width) || image.width <= 0 ||
              typeof image.height !== "number" || !Number.isFinite(image.height) || image.height <= 0) {
            errors.push(`Appearance image '${image.id || "unknown"}' dimensions must be positive numbers.`);
          }
          if (typeof image.locked !== "boolean") {
            errors.push(`Appearance image '${image.id || "unknown"}' locked must be a boolean.`);
          }
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

  const defaultNodeWidth = graph.appearance?.defaults?.node?.width ?? CARD_WIDTH;
  const defaultNodeHeight = graph.appearance?.defaults?.node?.height ?? CARD_HEIGHT;

  let maxNodeWidth = CARD_WIDTH;
  let maxNodeHeight = CARD_HEIGHT;
  for (const node of graph.nodes) {
    const nodeWidth = graph.appearance?.nodes?.[node]?.width ?? defaultNodeWidth;
    const nodeHeight = graph.appearance?.nodes?.[node]?.height ?? defaultNodeHeight;
    if (nodeWidth > maxNodeWidth) maxNodeWidth = nodeWidth;
    if (nodeHeight > maxNodeHeight) maxNodeHeight = nodeHeight;
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
    const nodeWidth = graph.appearance?.nodes?.[id]?.width ?? defaultNodeWidth;
    const nodeHeight = graph.appearance?.nodes?.[id]?.height ?? defaultNodeHeight;
    if (pos.x + nodeWidth > maxX) maxX = pos.x + nodeWidth;
    if (pos.y + nodeHeight > maxY) maxY = pos.y + nodeHeight;
  }

  const width = Math.ceil(maxX + PADDING);
  const height = Math.ceil(maxY + PADDING);

  return { positions, width, height };
}
