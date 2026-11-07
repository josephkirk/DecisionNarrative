/**
 * Vector canvas document: layered shapes stored as JSON in an entity
 * property. Gameplay shapes (gate, key, item, fact, event, entity reference)
 * also declare boolean entity properties so storylets can change the map at
 * runtime through ordinary Apply Command nodes.
 */
import type { EntityProperty } from "../../contracts.ts";

export const CANVAS_PROPERTY_KEY = "canvas_doc";
export const STATE_PREFIX = "map_";

export type ShapeKind = "rect" | "ellipse" | "line" | "path" | "text" | "entity" | "gate" | "key" | "item" | "fact" | "event";
export const GAMEPLAY_KINDS: ShapeKind[] = ["entity", "gate", "key", "item", "fact", "event"];

export interface CanvasLayer { id: string; name: string; visible: boolean; locked: boolean; opacity: number }

export interface CanvasShape {
  id: string;
  layer: string;
  kind: ShapeKind;
  x: number; y: number; w: number; h: number;
  /** Polyline points relative to (x, y): [x0, y0, x1, y1, ...]. */
  points?: number[];
  text?: string;
  label?: string;
  stroke: string; fill: string; strokeWidth: number;
  /** Entity this shape references (any entity, e.g. an item or character). */
  entity_id?: string;
  /** Gate only: key shape that opens it. */
  key_id?: string;
  /** Initial runtime state. */
  present: boolean;
  open?: boolean;
}

export interface CanvasSnapshot { layers: CanvasLayer[]; shapes: CanvasShape[] }
export interface CanvasRevision { at: number; label: string; snapshot: CanvasSnapshot }

export interface CanvasDoc extends CanvasSnapshot {
  version: 1;
  width: number;
  height: number;
  /** Persisted history, newest last. */
  revisions: CanvasRevision[];
}

export const MAX_REVISIONS = 40;
export const MAX_UNDO = 100;

export function newId(prefix: string): string {
  return `${prefix}${Math.random().toString(36).slice(2, 8)}`;
}

export function emptyCanvas(): CanvasDoc {
  return {
    version: 1, width: 1600, height: 1000,
    layers: [
      { id: "ly_ground", name: "Ground", visible: true, locked: false, opacity: 1 },
      { id: "ly_walls", name: "Walls", visible: true, locked: false, opacity: 1 },
      { id: "ly_logic", name: "Gates & Keys", visible: true, locked: false, opacity: 1 },
    ],
    shapes: [],
    revisions: [],
  };
}

export function parseCanvas(raw: unknown): CanvasDoc {
  if (typeof raw !== "string" || !raw.trim()) return emptyCanvas();
  try {
    const doc = JSON.parse(raw) as Partial<CanvasDoc>;
    if (!Array.isArray(doc.layers) || !Array.isArray(doc.shapes)) return emptyCanvas();
    return { ...emptyCanvas(), ...doc, revisions: Array.isArray(doc.revisions) ? doc.revisions : [] } as CanvasDoc;
  } catch {
    return emptyCanvas();
  }
}

export function serializeCanvas(doc: CanvasDoc): string { return JSON.stringify(doc); }

export function snapshotOf(doc: CanvasSnapshot): CanvasSnapshot {
  return JSON.parse(JSON.stringify({ layers: doc.layers, shapes: doc.shapes }));
}

export function pushRevision(doc: CanvasDoc, label: string, at = Date.now()): CanvasDoc {
  const revisions = [...doc.revisions, { at, label, snapshot: snapshotOf(doc) }].slice(-MAX_REVISIONS);
  return { ...doc, revisions };
}

/** Load a saved revision. Does not add a revision: history stays unchanged. */
export function restoreRevision(doc: CanvasDoc, index: number): CanvasDoc {
  const revision = doc.revisions[index];
  if (!revision) return doc;
  return { ...doc, ...snapshotOf(revision.snapshot) };
}

/** In-memory undo/redo over snapshots. */
export class UndoStack {
  private past: CanvasSnapshot[] = [];
  private future: CanvasSnapshot[] = [];
  record(current: CanvasSnapshot) {
    this.past.push(snapshotOf(current));
    if (this.past.length > MAX_UNDO) this.past.shift();
    this.future = [];
  }
  undo(current: CanvasSnapshot): CanvasSnapshot | null {
    const previous = this.past.pop();
    if (!previous) return null;
    this.future.push(snapshotOf(current));
    return previous;
  }
  redo(current: CanvasSnapshot): CanvasSnapshot | null {
    const next = this.future.pop();
    if (!next) return null;
    this.past.push(snapshotOf(current));
    return next;
  }
  get canUndo() { return this.past.length > 0; }
  get canRedo() { return this.future.length > 0; }
}

export function isGameplay(shape: CanvasShape): boolean { return GAMEPLAY_KINDS.includes(shape.kind); }

function slug(id: string): string { return id.toLowerCase().replace(/[^a-z0-9_]/g, "_"); }
export function presentKey(shape: Pick<CanvasShape, "id">): string { return `${STATE_PREFIX}${slug(shape.id)}_present`; }
export function openKey(shape: Pick<CanvasShape, "id">): string { return `${STATE_PREFIX}${slug(shape.id)}_open`; }

export function shapeDisplayName(shape: CanvasShape, entityName?: (id: string) => string | undefined): string {
  if (shape.label?.trim()) return shape.label.trim();
  if (shape.entity_id) return entityName?.(shape.entity_id) ?? shape.entity_id;
  return `${shape.kind} ${shape.id.slice(-4)}`;
}

/**
 * Rebuild the entity properties for a canvas: the document itself plus one
 * boolean per gameplay shape state. Unrelated properties are kept; state
 * properties for deleted shapes are removed.
 */
export function syncCanvasProperties(properties: EntityProperty[], doc: CanvasDoc): EntityProperty[] {
  const wanted = new Map<string, EntityProperty>();
  for (const shape of doc.shapes.filter(isGameplay)) {
    const name = shape.label?.trim() || shape.id;
    wanted.set(presentKey(shape), { id: presentKey(shape), key: presentKey(shape), name: `Map: ${name} present`, type: "bool", value: shape.present });
    if (shape.kind === "gate") {
      wanted.set(openKey(shape), { id: openKey(shape), key: openKey(shape), name: `Map: ${name} open`, type: "bool", value: !!shape.open });
    }
  }
  const rest = properties.filter((property) => property.key !== CANVAS_PROPERTY_KEY && !property.key.startsWith(STATE_PREFIX));
  const docProperty: EntityProperty = { id: CANVAS_PROPERTY_KEY, key: CANVAS_PROPERTY_KEY, name: "Map Canvas", type: "string", multiline: true, value: serializeCanvas(doc) };
  return [...rest, docProperty, ...wanted.values()];
}

/* ---------- Modify Canvas operations ---------- */

export type CanvasAction = "remove" | "add" | "open" | "close";
export interface CanvasOp { shape_id: string; action: CanvasAction }
export interface ModifyCanvasConfig { canvas_entity: string; ops: CanvasOp[] }

/** Author-facing verbs for each gameplay kind. */
export function actionsFor(kind: ShapeKind): Array<{ action: CanvasAction; label: string }> {
  const noun = kind === "entity" ? "reference" : kind;
  const base = kind === "fact" || kind === "event"
    ? [{ action: "add" as const, label: `Add ${noun}` }, { action: "remove" as const, label: `Remove ${noun}` }]
    : [{ action: "remove" as const, label: `Remove ${noun}` }, { action: "add" as const, label: `Add ${noun}` }];
  return kind === "gate" ? [{ action: "open", label: "Open gate" }, { action: "close", label: "Close gate" }, ...base] : base;
}

export function opProperty(op: CanvasOp, shape: CanvasShape): { key: string; value: boolean } {
  if (op.action === "open" || op.action === "close") return { key: openKey(shape), value: op.action === "open" };
  return { key: presentKey(shape), value: op.action === "add" };
}

export function validateOps(config: ModifyCanvasConfig, doc: CanvasDoc | null): string[] {
  const issues: string[] = [];
  if (!config.canvas_entity) return ["Modify Canvas must choose an entity with a map canvas."];
  if (!doc) return ["Modify Canvas entity has no map canvas."];
  if (!config.ops.length) issues.push("Modify Canvas must apply at least one change.");
  for (const op of config.ops) {
    const shape = doc.shapes.find((candidate) => candidate.id === op.shape_id);
    if (!shape || !isGameplay(shape)) { issues.push(`Modify Canvas targets a missing map element (${op.shape_id}).`); continue; }
    if (!actionsFor(shape.kind).some((entry) => entry.action === op.action)) issues.push(`Map element ${shape.id} does not support '${op.action}'.`);
  }
  return issues;
}

/** Pure preview of the runtime state after the operations run. */
export function applyOps(doc: CanvasDoc, ops: CanvasOp[]): CanvasDoc {
  const shapes = doc.shapes.map((shape) => {
    let next = shape;
    for (const op of ops.filter((candidate) => candidate.shape_id === shape.id)) {
      if (op.action === "open" || op.action === "close") next = { ...next, open: op.action === "open" };
      else next = { ...next, present: op.action === "add" };
    }
    return next;
  });
  return { ...doc, shapes };
}

/** Apply runtime property values (e.g. from a run snapshot) to the canvas. */
export function applyState(doc: CanvasDoc, values: Record<string, unknown>): CanvasDoc {
  return {
    ...doc,
    shapes: doc.shapes.map((shape) => ({
      ...shape,
      present: typeof values[presentKey(shape)] === "boolean" ? values[presentKey(shape)] as boolean : shape.present,
      open: typeof values[openKey(shape)] === "boolean" ? values[openKey(shape)] as boolean : shape.open,
    })),
  };
}
