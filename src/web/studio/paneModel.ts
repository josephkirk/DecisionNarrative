/**
 * Split-pane workspace layout. A layout is a binary tree: leaves are panes that
 * each show one studio route, splits divide their area between two children.
 * The pane with id MAIN_PANE follows the browser URL; other panes keep their own
 * route. All operations are pure and return a new tree.
 */
export const MAIN_PANE = "main";
export const MAX_PANES = 6;
export const MIN_RATIO = 0.15;
export const MAX_RATIO = 0.85;

export type SplitDirection = "row" | "column";
export type DropSide = "left" | "right" | "top" | "bottom";

export interface PaneLeaf { kind: "pane"; id: string; path: string }
export interface PaneSplit { kind: "split"; id: string; direction: SplitDirection; ratio: number; children: [PaneNode, PaneNode] }
export type PaneNode = PaneLeaf | PaneSplit;

export function singlePane(): PaneNode {
  return { kind: "pane", id: MAIN_PANE, path: "" };
}

export function panes(node: PaneNode): PaneLeaf[] {
  return node.kind === "pane" ? [node] : [...panes(node.children[0]), ...panes(node.children[1])];
}

export function findPane(node: PaneNode, id: string): PaneLeaf | undefined {
  return panes(node).find((pane) => pane.id === id);
}

export function clampRatio(ratio: number): number {
  if (!Number.isFinite(ratio)) return 0.5;
  return Math.min(MAX_RATIO, Math.max(MIN_RATIO, ratio));
}

/** Places a new pane showing `path` on one side of `targetId`. Returns the tree unchanged at the pane limit. */
export function splitPane(node: PaneNode, targetId: string, side: DropSide, path: string, newId: string): PaneNode {
  if (panes(node).length >= MAX_PANES || !findPane(node, targetId) || findPane(node, newId)) return node;
  const fresh: PaneLeaf = { kind: "pane", id: newId, path };
  const direction: SplitDirection = side === "left" || side === "right" ? "row" : "column";
  const first = side === "left" || side === "top";
  const visit = (current: PaneNode): PaneNode => {
    if (current.kind === "pane") {
      if (current.id !== targetId) return current;
      return { kind: "split", id: `split-${newId}`, direction, ratio: 0.5, children: first ? [fresh, current] : [current, fresh] };
    }
    return { ...current, children: [visit(current.children[0]), visit(current.children[1])] };
  };
  return visit(node);
}

/** Removes a pane and lets its sibling take the freed space. The last pane and the main pane stay. */
export function closePane(node: PaneNode, id: string): PaneNode {
  if (id === MAIN_PANE || node.kind === "pane") return node;
  const visit = (current: PaneNode): PaneNode => {
    if (current.kind === "pane") return current;
    const [a, b] = current.children;
    if (a.kind === "pane" && a.id === id) return b;
    if (b.kind === "pane" && b.id === id) return a;
    return { ...current, children: [visit(a), visit(b)] };
  };
  return visit(node);
}

export function setPanePath(node: PaneNode, id: string, path: string): PaneNode {
  if (node.kind === "pane") return node.id === id ? { ...node, path } : node;
  return { ...node, children: [setPanePath(node.children[0], id, path), setPanePath(node.children[1], id, path)] };
}

export function setRatio(node: PaneNode, splitId: string, ratio: number): PaneNode {
  if (node.kind === "pane") return node;
  if (node.id === splitId) return { ...node, ratio: clampRatio(ratio) };
  return { ...node, children: [setRatio(node.children[0], splitId, ratio), setRatio(node.children[1], splitId, ratio)] };
}

/** Accepts only well-formed stored layouts that still contain the main pane. */
export function parseLayout(raw: unknown): PaneNode | null {
  const ids = new Set<string>();
  const valid = (value: unknown): value is PaneNode => {
    if (!value || typeof value !== "object") return false;
    const node = value as Record<string, unknown>;
    if (typeof node.id !== "string" || ids.has(node.id)) return false;
    ids.add(node.id);
    if (node.kind === "pane") return typeof node.path === "string";
    if (node.kind !== "split" || (node.direction !== "row" && node.direction !== "column") || typeof node.ratio !== "number") return false;
    return Array.isArray(node.children) && node.children.length === 2 && valid(node.children[0]) && valid(node.children[1]);
  };
  if (!valid(raw) || !findPane(raw, MAIN_PANE) || panes(raw).length > MAX_PANES) return null;
  const normalise = (node: PaneNode): PaneNode => node.kind === "pane" ? node : { ...node, ratio: clampRatio(node.ratio), children: [normalise(node.children[0]), normalise(node.children[1])] };
  return normalise(raw);
}
export interface Box { x: number; y: number; w: number; h: number }
export interface PaneGeometry { panes: Array<{ id: string; box: Box }>; handles: Array<{ splitId: string; direction: SplitDirection; ratio: number; box: Box }> }

/**
 * Flattens the tree into fractional boxes (0..1 of the workspace) so panes can
 * render as stable siblings: splitting or closing never remounts a pane.
 */
export function paneGeometry(node: PaneNode, box: Box = { x: 0, y: 0, w: 1, h: 1 }): PaneGeometry {
  if (node.kind === "pane") return { panes: [{ id: node.id, box }], handles: [] };
  const row = node.direction === "row";
  const first: Box = row ? { ...box, w: box.w * node.ratio } : { ...box, h: box.h * node.ratio };
  const second: Box = row
    ? { ...box, x: box.x + first.w, w: box.w - first.w }
    : { ...box, y: box.y + first.h, h: box.h - first.h };
  const a = paneGeometry(node.children[0], first), b = paneGeometry(node.children[1], second);
  return {
    panes: [...a.panes, ...b.panes],
    handles: [{ splitId: node.id, direction: node.direction, ratio: node.ratio, box }, ...a.handles, ...b.handles],
  };
}

/** Which part of a pane a pointer at (x, y) inside a w × h box targets. Edges take the outer quarter. */
export function dropSide(x: number, y: number, width: number, height: number): DropSide | "center" {
  if (width <= 0 || height <= 0) return "center";
  const fx = x / width, fy = y / height;
  const distances: Array<[DropSide, number]> = [["left", fx], ["right", 1 - fx], ["top", fy], ["bottom", 1 - fy]];
  const [side, distance] = distances.reduce((best, entry) => entry[1] < best[1] ? entry : best);
  return distance < 0.25 ? side : "center";
}
