import type { EntityProperty } from "../../contracts.ts";

export const CANVAS_PROPERTY_KEY = "canvas_doc";
export const CANVAS_VERSION_KEY = "canvas_version";
export const STATE_PREFIX = "map_";

export type ShapeKind = "rect" | "ellipse" | "line" | "path" | "text" | "note" | "marker" | "entity" | "storylet";
export const GAMEPLAY_KINDS: ShapeKind[] = ["marker", "entity", "storylet"];

export const NOTE_COLORS = ["#ffe066", "#ffadad", "#a0e7a0", "#a0d8ff", "#e0bbff"];

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
  points?: number[];
  text?: string;
  label?: string;
  description?: string;
  stroke: string; fill: string; strokeWidth: number;
  icon?: string;
  entity_id?: string;
  storylet_id?: string;
  present: boolean;
}

export interface CanvasSnapshot { layers: CanvasLayer[]; shapes: CanvasShape[] }
export interface CanvasVersionSource { storylet_id: string; storylet_title?: string; node_id: string }
export interface CanvasRevision {
  at: number; label: string; snapshot: CanvasSnapshot;
  id?: string;
  source?: CanvasVersionSource;
  draft?: CanvasSnapshot;
  uncommitted?: boolean;
  pending_delete?: boolean;
}

export function versionSnapshot(revision: CanvasRevision): CanvasSnapshot {
  return revision.draft ?? revision.snapshot;
}

export function versionTempState(revision: CanvasRevision): string {
  if (revision.pending_delete) return "removal pending";
  if (revision.uncommitted) return "new, storylet not saved";
  if (revision.draft) return "edited, storylet not saved";
  return "";
}

export interface CanvasDoc extends CanvasSnapshot {
  version: 1;
  width: number;
  height: number;
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

function trimRevisions(revisions: CanvasRevision[]): CanvasRevision[] {
  let extra = revisions.filter((revision) => !revision.source).length - MAX_REVISIONS;
  return revisions.filter((revision) => revision.source || extra-- <= 0);
}

export function pushRevision(doc: CanvasDoc, label: string, at = Date.now()): CanvasDoc {
  return { ...doc, revisions: trimRevisions([...doc.revisions, { at, label, snapshot: snapshotOf(doc) }]) };
}

export function removeNodeVersion(doc: CanvasDoc, id: string): CanvasDoc {
  return { ...doc, revisions: doc.revisions.filter((revision) => revision.id !== id) };
}

export function stageNodeVersion(doc: CanvasDoc, version: { id: string; label: string; source: CanvasVersionSource; snapshot: CanvasSnapshot }, at = Date.now()): CanvasDoc {
  const existing = nodeVersion(doc, version.id);
  const snapshot = snapshotOf(version.snapshot);
  const entry: CanvasRevision = existing && !existing.uncommitted
    ? { ...existing, at, label: version.label, source: version.source, draft: snapshot, pending_delete: undefined }
    : { at, label: version.label, snapshot, id: version.id, source: version.source, uncommitted: true };
  return { ...doc, revisions: trimRevisions([...doc.revisions.filter((revision) => revision.id !== version.id), entry]) };
}

export function stageNodeVersionRemoval(doc: CanvasDoc, id: string): CanvasDoc {
  const existing = nodeVersion(doc, id);
  if (!existing) return doc;
  if (existing.uncommitted) return removeNodeVersion(doc, id);
  return { ...doc, revisions: doc.revisions.map((revision) => revision.id === id ? { ...revision, pending_delete: true } : revision) };
}

function ownedBy(revision: CanvasRevision, storyletId: string, nodeIds?: Set<string>): boolean {
  const source = revision.source;
  if (!source) return false;
  if (source.storylet_id === storyletId) return true;
  return source.storylet_id === "" && (!nodeIds || nodeIds.has(source.node_id));
}

export function commitStoryletVersions(doc: CanvasDoc, storyletId: string, nodeIds: Set<string>): CanvasDoc {
  const revisions: CanvasRevision[] = [];
  for (const revision of doc.revisions) {
    if (!ownedBy(revision, storyletId, nodeIds)) { revisions.push(revision); continue; }
    if (revision.pending_delete || !nodeIds.has(revision.source!.node_id)) continue;
    const { draft, uncommitted: _new, pending_delete: _gone, ...rest } = revision;
    revisions.push({ ...rest, snapshot: draft ?? revision.snapshot, source: { ...revision.source!, storylet_id: storyletId } });
  }
  return { ...doc, revisions };
}

export function discardStoryletVersions(doc: CanvasDoc, storyletId: string): CanvasDoc {
  const revisions: CanvasRevision[] = [];
  for (const revision of doc.revisions) {
    if (!ownedBy(revision, storyletId)) { revisions.push(revision); continue; }
    if (revision.uncommitted) continue;
    const { draft: _draft, pending_delete: _gone, ...rest } = revision;
    revisions.push(rest);
  }
  return { ...doc, revisions };
}

export function hasTempVersions(doc: CanvasDoc, storyletId: string): boolean {
  return doc.revisions.some((revision) => ownedBy(revision, storyletId) && !!versionTempState(revision));
}

export function nodeVersion(doc: CanvasDoc, id: string | undefined): CanvasRevision | undefined {
  return id ? doc.revisions.find((revision) => revision.id === id) : undefined;
}

export function upsertNodeVersion(doc: CanvasDoc, version: { id: string; label: string; source: CanvasVersionSource; snapshot: CanvasSnapshot }, at = Date.now()): CanvasDoc {
  const entry: CanvasRevision = { at, label: version.label, snapshot: snapshotOf(version.snapshot), id: version.id, source: version.source };
  return { ...doc, revisions: trimRevisions([...doc.revisions.filter((revision) => revision.id !== version.id), entry]) };
}

export function restoreRevision(doc: CanvasDoc, index: number): CanvasDoc {
  const revision = doc.revisions[index];
  if (!revision) return doc;
  return { ...doc, ...snapshotOf(revision.snapshot) };
}

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
export function refKey(shape: Pick<CanvasShape, "id">): string { return `${STATE_PREFIX}${slug(shape.id)}_ref`; }

export type RefName = (shape: CanvasShape) => string | undefined;

export function shapeDisplayName(shape: CanvasShape, refName?: RefName): string {
  if (shape.label?.trim()) return shape.label.trim();
  const ref = refName?.(shape);
  if (ref) return ref;
  if (shape.kind === "marker") return markerIconLabel(shape.icon) || "Marker";
  if (shape.kind === "note") return (shape.text ?? "").split("\n")[0]!.slice(0, 24) || "Note";
  return shape.entity_id ?? shape.storylet_id ?? `${shape.kind} ${shape.id.slice(-4)}`;
}

export function gameplayUniverse(doc: CanvasDoc): CanvasShape[] {
  const byId = new Map<string, CanvasShape>();
  for (const shape of doc.shapes.filter(isGameplay)) byId.set(shape.id, shape);
  for (const revision of doc.revisions.filter((entry) => entry.source)) {
    for (const shape of [...revision.snapshot.shapes, ...(revision.draft?.shapes ?? [])].filter(isGameplay)) {
      if (!byId.has(shape.id)) byId.set(shape.id, { ...shape, present: false });
    }
  }
  return [...byId.values()];
}

export function syncCanvasProperties(properties: EntityProperty[], doc: CanvasDoc): EntityProperty[] {
  const wanted = new Map<string, EntityProperty>();
  for (const shape of gameplayUniverse(doc)) {
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
  const rest = properties.filter((property) => property.key !== CANVAS_PROPERTY_KEY && property.key !== CANVAS_VERSION_KEY && !property.key.startsWith(STATE_PREFIX));
  const docProperty: EntityProperty = { id: CANVAS_PROPERTY_KEY, key: CANVAS_PROPERTY_KEY, name: "Canvas", type: "string", multiline: true, value: serializeCanvas(doc) };
  const versionProperty: EntityProperty = { id: CANVAS_VERSION_KEY, key: CANVAS_VERSION_KEY, name: "Canvas version", type: "string", value: "" };
  return [...rest, docProperty, versionProperty, ...wanted.values()];
}


export interface ModifyCanvasConfig {
  canvas_entity: string;
  version_id: string;
  saved_at?: number;
}

export interface StateChange { shape: CanvasShape; key: string; value: boolean | string; type: "bool" | "string"; text: string }

export function versionChanges(doc: CanvasDoc, version: CanvasSnapshot, refName?: RefName): StateChange[] {
  const base = new Map(doc.shapes.filter(isGameplay).map((shape) => [shape.id, shape]));
  const after = new Map(version.shapes.filter(isGameplay).map((shape) => [shape.id, shape]));
  const changes: StateChange[] = [];
  for (const id of new Set([...base.keys(), ...after.keys()])) {
    const before = base.get(id); const next = after.get(id);
    const shape = next ?? before!;
    const name = shapeDisplayName(shape, refName);
    const noun = shape.kind === "marker" ? "marker" : "reference";
    const wasPresent = before?.present ?? false; const isPresent = next?.present ?? false;
    if (wasPresent !== isPresent) {
      changes.push({ shape, key: presentKey(shape), value: isPresent, type: "bool", text: `${isPresent ? "Add" : "Remove"} ${noun}: ${name}` });
    }
    if (shape.kind === "marker" && before && next && isPresent) {
      const icon = next.icon ?? DEFAULT_MARKER_ICON;
      if (icon !== (before.icon ?? DEFAULT_MARKER_ICON)) {
        changes.push({ shape, key: iconKey(shape), value: icon, type: "string", text: `Change icon to ${markerGlyph(icon)} ${markerIconLabel(icon)}: ${name}` });
      }
    }
  }
  return changes;
}

export function validateModifyCanvas(config: ModifyCanvasConfig, doc: CanvasDoc | null): string[] {
  if (!config.canvas_entity) return ["Modify Canvas must choose an entity with a canvas."];
  if (!doc) return ["Modify Canvas entity has no canvas."];
  if (!config.version_id) return ["Modify Canvas has no canvas version yet: open the node and save the canvas."];
  const version = nodeVersion(doc, config.version_id);
  if (!version) return [`Modify Canvas version ${config.version_id} is missing from the entity's canvas history.`];
  if (version.pending_delete) return [`Modify Canvas version ${config.version_id} is marked for removal.`];
  return [];
}

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
