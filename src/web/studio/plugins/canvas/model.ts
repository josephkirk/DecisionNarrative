/**
 * Vector canvas document: layered shapes stored as JSON in an entity
 * property. Gameplay shapes (markers, entity references, storylet
 * references) also declare entity properties so storylets can change the
 * canvas at runtime through ordinary Apply Command nodes.
 */
import type { EntityProperty } from "../../contracts.ts";

export const CANVAS_PROPERTY_KEY = "canvas_doc";
export const STATE_PREFIX = "map_";

export type ShapeKind = "rect" | "ellipse" | "line" | "path" | "text" | "note" | "marker" | "entity" | "storylet";
export const GAMEPLAY_KINDS: ShapeKind[] = ["marker", "entity", "storylet"];

/** Sticky note paper colours. */
export const NOTE_COLORS = ["#ffe066", "#ffadad", "#a0e7a0", "#a0d8ff", "#e0bbff"];

/** Icons a marker can use. `id` is what gets stored; `glyph` is drawn. */
export const MARKER_ICONS: Array<{ id: string; glyph: string; label: string }> = [
  { id: "pin", glyph: "📍", label: "Pin" },
  { id: "door", glyph: "🚪", label: "Door (closed)" },
  { id: "door-open", glyph: "🔓", label: "Door (open)" },
  { id: "lock", glyph: "🔒", label: "Lock" },
  { id: "key", glyph: "🔑", label: "Key" },
  { id: "chest", glyph: "📦", label: "Chest" },
  { id: "gem", glyph: "💎", label: "Item" },
  { id: "coin", glyph: "🪙", label: "Coin" },
  { id: "scroll", glyph: "📜", label: "Fact / note" },
  { id: "event", glyph: "⚡", label: "Event" },
  { id: "info", glyph: "ℹ️", label: "Info" },
  { id: "question", glyph: "❓", label: "Question" },
  { id: "alert", glyph: "❗", label: "Alert" },
  { id: "flag", glyph: "🚩", label: "Flag" },
  { id: "star", glyph: "⭐", label: "Star" },
  { id: "fire", glyph: "🔥", label: "Fire" },
  { id: "skull", glyph: "💀", label: "Danger" },
  { id: "person", glyph: "🧍", label: "Person" },
  { id: "eye", glyph: "👁️", label: "Watch" },
  { id: "trap", glyph: "🪤", label: "Trap" },
];
export const DEFAULT_MARKER_ICON = "pin";
export function markerGlyph(icon: string | undefined): string {
  return MARKER_ICONS.find((entry) => entry.id === icon)?.glyph ?? icon ?? "📍";
}
export function markerIconLabel(icon: string | undefined): string {
  return MARKER_ICONS.find((entry) => entry.id === icon)?.label ?? icon ?? "";
}

export interface CanvasLayer { id: string; name: string; visible: boolean; locked: boolean; opacity: number }

export interface CanvasShape {
  id: string;
  layer: string;
  kind: ShapeKind;
  x: number; y: number; w: number; h: number;
  /** Polyline points relative to (x, y): [x0, y0, x1, y1, ...]. */
  points?: number[];
  /** Text shapes and sticky notes. */
  text?: string;
  /** Name shown on the canvas. */
  label?: string;
  /** Long text shown when hovering the shape. */
  description?: string;
  stroke: string; fill: string; strokeWidth: number;
  /** Marker only: icon id from MARKER_ICONS. */
  icon?: string;
  /** Entity this shape references (any entity, e.g. an item or character). */
  entity_id?: string;
  /** Storylet this shape references. */
  storylet_id?: string;
  /** Initial runtime state. */
  present: boolean;
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
    layers: [{ id: "ly_default", name: "Default", visible: true, locked: false, opacity: 1 }],
    shapes: [],
    revisions: [],
  };
}

export function parseCanvas(raw: unknown): CanvasDoc {
  if (typeof raw !== "string" || !raw.trim()) return emptyCanvas();
  try {
    const doc = JSON.parse(raw) as Partial<CanvasDoc>;
    if (!Array.isArray(doc.layers) || !Array.isArray(doc.shapes)) return emptyCanvas();
    return {
      ...emptyCanvas(), ...doc,
      layers: doc.layers.length ? doc.layers : emptyCanvas().layers,
      revisions: Array.isArray(doc.revisions) ? doc.revisions : [],
    } as CanvasDoc;
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

export function isGameplay(shape: Pick<CanvasShape, "kind">): boolean { return GAMEPLAY_KINDS.includes(shape.kind); }

function slug(id: string): string { return id.toLowerCase().replace(/[^a-z0-9_]/g, "_"); }
export function presentKey(shape: Pick<CanvasShape, "id">): string { return `${STATE_PREFIX}${slug(shape.id)}_present`; }
export function iconKey(shape: Pick<CanvasShape, "id">): string { return `${STATE_PREFIX}${slug(shape.id)}_icon`; }
/** Typed reference property: the canvas owner references the placed entity or storylet. */
export function refKey(shape: Pick<CanvasShape, "id">): string { return `${STATE_PREFIX}${slug(shape.id)}_ref`; }

/** Resolves the name of the entity or storylet a shape references. */
export type RefName = (shape: CanvasShape) => string | undefined;

export function shapeDisplayName(shape: CanvasShape, refName?: RefName): string {
  if (shape.label?.trim()) return shape.label.trim();
  const ref = refName?.(shape);
  if (ref) return ref;
  if (shape.kind === "marker") return markerIconLabel(shape.icon) || "Marker";
  if (shape.kind === "note") return (shape.text ?? "").split("\n")[0]!.slice(0, 24) || "Note";
  return shape.entity_id ?? shape.storylet_id ?? `${shape.kind} ${shape.id.slice(-4)}`;
}

/**
 * Rebuild the entity properties for a canvas: the document itself plus the
 * runtime state of each gameplay shape. Unrelated properties are kept; state
 * properties for deleted shapes are removed.
 */
export function syncCanvasProperties(properties: EntityProperty[], doc: CanvasDoc): EntityProperty[] {
  const wanted = new Map<string, EntityProperty>();
  for (const shape of doc.shapes.filter(isGameplay)) {
    const name = shape.label?.trim() || shape.id;
    wanted.set(presentKey(shape), { id: presentKey(shape), key: presentKey(shape), name: `Canvas: ${name} present`, type: "bool", value: shape.present });
    if (shape.kind === "marker") {
      wanted.set(iconKey(shape), { id: iconKey(shape), key: iconKey(shape), name: `Canvas: ${name} icon`, type: "string", value: shape.icon ?? DEFAULT_MARKER_ICON });
    }
    const refLabel = shape.label?.trim() ? `Canvas reference (${shape.label.trim()})` : "Canvas reference";
    if (shape.kind === "entity" && shape.entity_id) {
      wanted.set(refKey(shape), { id: refKey(shape), key: refKey(shape), name: refLabel, type: "entity", value: shape.entity_id });
    }
    if (shape.kind === "storylet" && shape.storylet_id) {
      wanted.set(refKey(shape), { id: refKey(shape), key: refKey(shape), name: refLabel, type: "storylet", value: shape.storylet_id });
    }
  }
  const rest = properties.filter((property) => property.key !== CANVAS_PROPERTY_KEY && !property.key.startsWith(STATE_PREFIX));
  const docProperty: EntityProperty = { id: CANVAS_PROPERTY_KEY, key: CANVAS_PROPERTY_KEY, name: "Canvas", type: "string", multiline: true, value: serializeCanvas(doc) };
  return [...rest, docProperty, ...wanted.values()];
}

/* ---------- Modify Canvas operations ---------- */

export type CanvasAction = "remove" | "add" | "set_icon";
export interface CanvasOp { shape_id: string; action: CanvasAction; /** set_icon only. */ icon?: string }
export interface ModifyCanvasConfig { canvas_entity: string; ops: CanvasOp[] }

/** Author-facing verbs for each gameplay kind. */
export function actionsFor(kind: ShapeKind): Array<{ action: CanvasAction; label: string }> {
  const noun = kind === "marker" ? "marker" : "reference";
  const base = [{ action: "remove" as const, label: `Remove ${noun}` }, { action: "add" as const, label: `Add ${noun}` }];
  return kind === "marker" ? [...base, { action: "set_icon", label: "Change icon" }] : base;
}

export function opLabelText(op: CanvasOp, shape: CanvasShape): string {
  if (op.action === "set_icon") return `Change icon to ${markerGlyph(op.icon)} ${markerIconLabel(op.icon)}`;
  return actionsFor(shape.kind).find((entry) => entry.action === op.action)?.label ?? op.action;
}

export function opProperty(op: CanvasOp, shape: CanvasShape): { key: string; value: boolean | string; type: "bool" | "string" } {
  if (op.action === "set_icon") return { key: iconKey(shape), value: op.icon ?? DEFAULT_MARKER_ICON, type: "string" };
  return { key: presentKey(shape), value: op.action === "add", type: "bool" };
}

export function validateOps(config: ModifyCanvasConfig, doc: CanvasDoc | null): string[] {
  const issues: string[] = [];
  if (!config.canvas_entity) return ["Modify Canvas must choose an entity with a canvas."];
  if (!doc) return ["Modify Canvas entity has no canvas."];
  if (!config.ops.length) issues.push("Modify Canvas must apply at least one change.");
  for (const op of config.ops) {
    const shape = doc.shapes.find((candidate) => candidate.id === op.shape_id);
    if (!shape || !isGameplay(shape)) { issues.push(`Modify Canvas targets a missing canvas element (${op.shape_id}).`); continue; }
    if (!actionsFor(shape.kind).some((entry) => entry.action === op.action)) issues.push(`Canvas element ${shape.id} does not support '${op.action}'.`);
    if (op.action === "set_icon" && !op.icon) issues.push(`Change icon on ${shape.id} must choose an icon.`);
  }
  return issues;
}

/** Pure preview of the runtime state after the operations run. */
export function applyOps(doc: CanvasDoc, ops: CanvasOp[]): CanvasDoc {
  const shapes = doc.shapes.map((shape) => {
    let next = shape;
    for (const op of ops.filter((candidate) => candidate.shape_id === shape.id)) {
      if (op.action === "set_icon") next = { ...next, icon: op.icon ?? next.icon };
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
      icon: shape.kind === "marker" && typeof values[iconKey(shape)] === "string" ? values[iconKey(shape)] as string : shape.icon,
    })),
  };
}
