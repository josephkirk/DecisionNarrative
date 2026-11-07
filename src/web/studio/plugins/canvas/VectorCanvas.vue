<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { studioState } from "../../state.ts";
import type { AnyEntity } from "../runtime.ts";
import {
  CANVAS_PROPERTY_KEY, DEFAULT_MARKER_ICON, MARKER_ICONS, NOTE_COLORS, UndoStack, isGameplay, newId, parseCanvas,
  pushRevision, restoreRevision, shapeDisplayName, snapshotOf, syncCanvasProperties, versionTempState,
  type CanvasDoc, type CanvasLayer, type CanvasShape, type ShapeKind,
} from "./model.ts";
import { allEntities as listEntities, refInfo, refName, STORYLET_COLOR } from "./refs.ts";
import CanvasShapeView from "./CanvasShapeView.vue";

/**
 * `entity` mode (the entity's Canvas tab): every edit is a history revision
 * saved on the entity. `node` mode (a Modify Canvas node): edits a working
 * copy with undo only, no history; the caller saves it as one version.
 */
const props = withDefaults(defineProps<{ entity?: AnyEntity; readonly?: boolean; mode?: "entity" | "node"; initialDoc?: CanvasDoc }>(), { mode: "entity" });
const emit = defineEmits<{ (e: "patch", value: Partial<AnyEntity>): void; (e: "change", value: CanvasDoc): void }>();
const nodeMode = computed(() => props.mode === "node");
const router = useRouter();

type Tool = "select" | "pan" | ShapeKind;
const TOOLS: Array<{ id: Tool; label: string; glyph: string; group: "draw" | "logic" }> = [
  { id: "select", label: "Select (V)", glyph: "↖", group: "draw" },
  { id: "pan", label: "Pan (H)", glyph: "✋", group: "draw" },
  { id: "rect", label: "Room / Rectangle (R)", glyph: "▭", group: "draw" },
  { id: "ellipse", label: "Ellipse (O)", glyph: "◯", group: "draw" },
  { id: "line", label: "Wall / Polyline (L) — click points, double-click to finish", glyph: "╱", group: "draw" },
  { id: "text", label: "Text (T)", glyph: "T", group: "draw" },
  { id: "note", label: "Sticky note (N)", glyph: "🗒", group: "draw" },
  { id: "marker", label: "Marker (M)", glyph: "📍", group: "logic" },
  { id: "entity", label: "Entity reference", glyph: "@", group: "logic" },
  { id: "storylet", label: "Storylet reference", glyph: "📖", group: "logic" },
];
const DEFAULT_SIZE: Partial<Record<ShapeKind, [number, number]>> = { marker: [40, 40], entity: [56, 56], storylet: [72, 56], note: [160, 120], text: [160, 28] };
const uid = newId("vc");

const doc = ref<CanvasDoc>(props.mode === "node" && props.initialDoc ? JSON.parse(JSON.stringify(props.initialDoc)) : parseCanvas(propertyValue()));
const undo = new UndoStack();
const undoVersion = ref(0);
const tool = ref<Tool>("select");
const activeLayer = ref(doc.value.layers.at(-1)?.id ?? "");
const selectedId = ref<string | null>(null);
const pendingEntityId = ref("");
const pendingStoryletId = ref("");
const pendingIcon = ref(DEFAULT_MARKER_ICON);
const showRuntime = ref(props.mode === "node");
const hover = ref<{ id: string; x: number; y: number } | null>(null);
const sidePanel = ref<"layers" | "history">("layers");
const zoom = ref(0.8); const panX = ref(20); const panY = ref(20);
const svgRef = ref<SVGSVGElement | null>(null);
const draft = ref<CanvasShape | null>(null);
type Drag = { mode: "move" | "resize" | "pan" | "create"; startX: number; startY: number; origin: CanvasShape | null; panX: number; panY: number; before: ReturnType<typeof snapshotOf> };
const drag = ref<Drag | null>(null);

function propertyValue(): unknown { return props.entity?.properties.find((property) => property.key === CANVAS_PROPERTY_KEY)?.value; }
watch(() => props.entity?.id, () => { if (nodeMode.value) return; doc.value = parseCanvas(propertyValue()); selectedId.value = null; });

const allEntities = computed<AnyEntity[]>(() => listEntities());
const storylets = computed(() => studioState.storylets);
const layerById = computed(() => new Map(doc.value.layers.map((layer) => [layer.id, layer])));
const orderedShapes = computed(() => {
  const order = new Map(doc.value.layers.map((layer, index) => [layer.id, index]));
  return [...doc.value.shapes].sort((a, b) => (order.get(a.layer) ?? 0) - (order.get(b.layer) ?? 0));
});
const selected = computed(() => doc.value.shapes.find((shape) => shape.id === selectedId.value) ?? null);
const hovered = computed(() => {
  const shape = hover.value && !drag.value ? doc.value.shapes.find((entry) => entry.id === hover.value!.id) : undefined;
  if (!shape) return null;
  const description = shape.description?.trim() || refInfo(shape)?.description?.trim() || "";
  return description ? { name: shapeDisplayName(shape, refName), description, x: hover.value!.x, y: hover.value!.y } : null;
});
function onShapeHover(event: MouseEvent, shape: CanvasShape) {
  const rect = svgRef.value!.getBoundingClientRect();
  hover.value = { id: shape.id, x: event.clientX - rect.left + 14, y: event.clientY - rect.top + 14 };
}
const canUndo = computed(() => (undoVersion.value, undo.canUndo));
const canRedo = computed(() => (undoVersion.value, undo.canRedo));
const isLocked = (shape: CanvasShape) => props.readonly || !!layerById.value.get(shape.layer)?.locked;

/* ---------- history ---------- */
function persist(next: CanvasDoc) {
  doc.value = next;
  if (nodeMode.value) { emit("change", next); return; }
  emit("patch", { properties: syncCanvasProperties(props.entity?.properties ?? [], next) });
}
function commit(label: string, before: ReturnType<typeof snapshotOf>, mutate?: (doc: CanvasDoc) => void) {
  if (props.readonly) return;
  undo.record(before); undoVersion.value++;
  const next = JSON.parse(JSON.stringify(doc.value)) as CanvasDoc;
  mutate?.(next);
  persist(nodeMode.value ? next : pushRevision(next, label));
}
function change(label: string, mutate: (doc: CanvasDoc) => void) { commit(label, snapshotOf(doc.value), mutate); }
function doUndo() { const prev = undo.undo(doc.value); if (prev) { undoVersion.value++; persist({ ...doc.value, ...prev }); } }
function doRedo() { const next = undo.redo(doc.value); if (next) { undoVersion.value++; persist({ ...doc.value, ...next }); } }
function restore(index: number) { undo.record(doc.value); undoVersion.value++; persist(restoreRevision(doc.value, index)); }

/* ---------- layers ---------- */
function addLayer() {
  const id = newId("ly_");
  change("Add layer", (d) => d.layers.push({ id, name: `Layer ${d.layers.length + 1}`, visible: true, locked: false, opacity: 1 }));
  activeLayer.value = id;
}
function updateLayer(id: string, patch: Partial<CanvasLayer>, label = "Edit layer") {
  change(label, (d) => { const layer = d.layers.find((entry) => entry.id === id); if (layer) Object.assign(layer, patch); });
}
function moveLayer(id: string, delta: number) {
  change("Reorder layers", (d) => {
    const index = d.layers.findIndex((layer) => layer.id === id); const target = index + delta;
    if (index < 0 || target < 0 || target >= d.layers.length) return;
    const [layer] = d.layers.splice(index, 1); d.layers.splice(target, 0, layer!);
  });
}
function removeLayer(id: string) {
  if (doc.value.layers.length <= 1) return;
  const count = doc.value.shapes.filter((shape) => shape.layer === id).length;
  if (count && !window.confirm(`Delete layer and its ${count} shape(s)?`)) return;
  change("Delete layer", (d) => { d.layers = d.layers.filter((layer) => layer.id !== id); d.shapes = d.shapes.filter((shape) => shape.layer !== id); });
  if (activeLayer.value === id) activeLayer.value = doc.value.layers.at(-1)?.id ?? "";
}

/* ---------- shapes ---------- */
function updateSelected(patch: Partial<CanvasShape>, label = "Edit shape") {
  const id = selectedId.value; if (!id) return;
  change(label, (d) => { const shape = d.shapes.find((entry) => entry.id === id); if (shape) Object.assign(shape, patch); });
}
function deleteSelected() {
  const id = selectedId.value; if (!id || !selected.value || isLocked(selected.value)) return;
  change(`Delete ${selected.value.kind}`, (d) => { d.shapes = d.shapes.filter((shape) => shape.id !== id); });
  selectedId.value = null;
}
function duplicateSelected() {
  const source = selected.value; if (!source) return;
  const id = newId("s_");
  change(`Duplicate ${source.kind}`, (d) => d.shapes.push({ ...JSON.parse(JSON.stringify(source)), id, x: source.x + 20, y: source.y + 20 }));
  selectedId.value = id;
}
function openReference(shape: CanvasShape) {
  const route = refInfo(shape)?.route;
  if (route) void router.push(route);
}

/* ---------- pointer ---------- */
function toWorld(event: MouseEvent) {
  const rect = svgRef.value!.getBoundingClientRect();
  return { x: (event.clientX - rect.left - panX.value) / zoom.value, y: (event.clientY - rect.top - panY.value) / zoom.value };
}
const snap = (value: number) => Math.round(value / 8) * 8;
function baseShape(kind: ShapeKind, x: number, y: number): CanvasShape {
  const [w, h] = DEFAULT_SIZE[kind] ?? [0, 0];
  const logic = isGameplay({ kind } as CanvasShape);
  const fill = kind === "rect" ? "rgba(142,142,147,0.18)" : kind === "ellipse" ? "rgba(100,210,255,0.18)" : kind === "note" ? NOTE_COLORS[0]! : "none";
  return {
    id: newId("s_"), layer: activeLayer.value, kind, x, y, w, h,
    stroke: kind === "line" ? "#d8dee9" : kind === "marker" ? "#ff9f0a" : logic ? "#1c1c1e" : "#8e8e93", fill,
    strokeWidth: kind === "line" ? 6 : 2, present: true,
    ...(kind === "text" ? { text: "Label" } : {}),
    ...(kind === "note" ? { text: "Note" } : {}),
    ...(kind === "marker" ? { icon: pendingIcon.value } : {}),
  };
}
function onPointerDown(event: MouseEvent) {
  if (event.button === 1 || tool.value === "pan" || (event.button === 0 && event.altKey)) {
    drag.value = { mode: "pan", startX: event.clientX, startY: event.clientY, origin: null, panX: panX.value, panY: panY.value, before: snapshotOf(doc.value) };
    return;
  }
  if (event.button !== 0 || props.readonly) return;
  const layer = layerById.value.get(activeLayer.value);
  const point = toWorld(event); const x = snap(point.x); const y = snap(point.y);
  if (tool.value === "select") { selectedId.value = null; return; }
  if (!layer || layer.locked || !layer.visible) return;
  const kind = tool.value as ShapeKind;
  if (kind === "line") {
    if (draft.value?.kind === "line" && event.detail >= 2) { onDoubleClick(); return; }
    if (draft.value?.kind === "line") { draft.value.points!.push(x - draft.value.x, y - draft.value.y); return; }
    draft.value = { ...baseShape("line", x, y), points: [0, 0, 0, 0] };
    return;
  }
  if (kind === "entity" && !pendingEntityId.value) return;
  if (kind === "storylet" && !pendingStoryletId.value) return;
  if (DEFAULT_SIZE[kind]) {
    const shape = baseShape(kind, x, y);
    if (kind === "entity") shape.entity_id = pendingEntityId.value;
    if (kind === "storylet") { shape.storylet_id = pendingStoryletId.value; shape.stroke = STORYLET_COLOR; }
    shape.x -= shape.w / 2; shape.y -= shape.h / 2;
    change(`Add ${kind}`, (d) => d.shapes.push(shape));
    selectedId.value = shape.id;
    return;
  }
  draft.value = baseShape(kind, x, y);
  drag.value = { mode: "create", startX: x, startY: y, origin: null, panX: 0, panY: 0, before: snapshotOf(doc.value) };
}
function onShapeDown(event: MouseEvent, shape: CanvasShape, mode: "move" | "resize" = "move") {
  if (tool.value !== "select" || event.button !== 0) return;
  event.stopPropagation();
  selectedId.value = shape.id;
  if (isLocked(shape)) return;
  const point = toWorld(event);
  drag.value = { mode, startX: point.x, startY: point.y, origin: { ...shape }, panX: 0, panY: 0, before: snapshotOf(doc.value) };
}
function onPointerMove(event: MouseEvent) {
  const active = drag.value;
  if (draft.value?.kind === "line" && !active) {
    const point = toWorld(event); const pts = draft.value.points!;
    pts[pts.length - 2] = snap(point.x) - draft.value.x; pts[pts.length - 1] = snap(point.y) - draft.value.y;
    return;
  }
  if (!active) return;
  if (active.mode === "pan") { panX.value = active.panX + event.clientX - active.startX; panY.value = active.panY + event.clientY - active.startY; return; }
  const point = toWorld(event);
  if (active.mode === "create" && draft.value) {
    const x = snap(point.x);
    const y = snap(point.y);
    draft.value.x = Math.min(x, active.startX); draft.value.y = Math.min(y, active.startY);
    draft.value.w = Math.abs(x - active.startX); draft.value.h = Math.abs(y - active.startY);
    return;
  }
  const origin = active.origin!; const shape = doc.value.shapes.find((entry) => entry.id === origin.id); if (!shape) return;
  const dx = snap(point.x - active.startX); const dy = snap(point.y - active.startY);
  if (active.mode === "move") { shape.x = origin.x + dx; shape.y = origin.y + dy; }
  else { shape.w = Math.max(8, origin.w + dx); shape.h = Math.max(8, origin.h + dy); }
}
function onPointerUp() {
  const active = drag.value; drag.value = null;
  if (!active || active.mode === "pan") return;
  if (active.mode === "create" && draft.value) {
    const shape = draft.value; draft.value = null;
    if (shape.w < 4 || shape.h < 4) return;
    change(`Add ${shape.kind}`, (d) => d.shapes.push(shape));
    selectedId.value = shape.id;
    return;
  }
  const moved = doc.value.shapes.find((shape) => shape.id === active.origin?.id);
  if (moved && (moved.x !== active.origin!.x || moved.y !== active.origin!.y || moved.w !== active.origin!.w || moved.h !== active.origin!.h)) {
    commit(active.mode === "move" ? `Move ${moved.kind}` : `Resize ${moved.kind}`, active.before);
  }
}
function onDoubleClick() {
  const shape = draft.value;
  if (shape?.kind !== "line") return;
  draft.value = null;
  const points = shape.points!.slice(0, -2);
  if (points.length < 4) return;
  change("Draw wall", (d) => d.shapes.push({ ...shape, points }));
  selectedId.value = shape.id;
}
function onWheel(event: WheelEvent) {
  event.preventDefault();
  const rect = svgRef.value!.getBoundingClientRect();
  const mx = event.clientX - rect.left; const my = event.clientY - rect.top;
  const next = Math.min(4, Math.max(0.2, zoom.value * (event.deltaY < 0 ? 1.1 : 1 / 1.1)));
  panX.value = mx - ((mx - panX.value) / zoom.value) * next; panY.value = my - ((my - panY.value) / zoom.value) * next;
  zoom.value = next;
}
function onKey(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null;
  if (target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName)) return;
  if (!svgRef.value?.isConnected) return;
  const mod = event.ctrlKey || event.metaKey;
  if (mod && event.key.toLowerCase() === "z") { event.preventDefault(); event.shiftKey ? doRedo() : doUndo(); return; }
  if (mod && event.key.toLowerCase() === "y") { event.preventDefault(); doRedo(); return; }
  if (mod && event.key.toLowerCase() === "d") { event.preventDefault(); duplicateSelected(); return; }
  if (event.key === "Delete" || event.key === "Backspace") { deleteSelected(); return; }
  if (event.key === "Escape") { draft.value = null; selectedId.value = null; return; }
  const shortcuts: Record<string, Tool> = { v: "select", h: "pan", r: "rect", o: "ellipse", l: "line", t: "text", n: "note", m: "marker" };
  const next = shortcuts[event.key.toLowerCase()];
  if (next && !mod) tool.value = next;
}
onMounted(() => window.addEventListener("keydown", onKey));
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));

function formatTime(at: number) { return new Date(at).toLocaleString(); }
const gameplayCount = computed(() => doc.value.shapes.filter(isGameplay).length);
</script>

<template>
  <div class="vc-root" data-testid="vector-canvas">
    <div class="vc-toolbar" role="toolbar" aria-label="Canvas tools">
      <button v-for="entry in TOOLS" :key="entry.id" :class="['vc-tool', tool === entry.id && 'is-active', entry.group === 'logic' && 'is-logic']"
        :title="entry.label" :aria-label="entry.label" :aria-pressed="tool === entry.id" :disabled="readonly && entry.id !== 'select' && entry.id !== 'pan'"
        @click="tool = entry.id">{{ entry.glyph }}</button>
      <select v-if="tool === 'entity'" v-model="pendingEntityId" class="form-select vc-entity-pick" aria-label="Entity to place">
        <option value="">Choose entity to place…</option>
        <option v-for="entity in allEntities" :key="entity.id" :value="entity.id">{{ entity.name }} ({{ entity.root_tag }})</option>
      </select>
      <select v-if="tool === 'storylet'" v-model="pendingStoryletId" class="form-select vc-entity-pick" aria-label="Storylet to place">
        <option value="">Choose storylet to place…</option>
        <option v-for="storylet in storylets" :key="storylet.id" :value="storylet.id">{{ storylet.title }}</option>
      </select>
      <select v-if="tool === 'marker'" v-model="pendingIcon" class="form-select vc-entity-pick" aria-label="Icon for new markers">
        <option v-for="icon in MARKER_ICONS" :key="icon.id" :value="icon.id">{{ icon.glyph }} {{ icon.label }}</option>
      </select>
      <span class="vc-sep"></span>
      <button class="vc-tool" title="Undo (Ctrl+Z)" aria-label="Undo" :disabled="!canUndo" @click="doUndo">↶</button>
      <button class="vc-tool" title="Redo (Ctrl+Y)" aria-label="Redo" :disabled="!canRedo" @click="doRedo">↷</button>
      <label class="vc-toggle"><input v-model="showRuntime" type="checkbox" /> Show runtime state</label>
      <span class="vc-status">{{ doc.shapes.length }} shapes · {{ gameplayCount }} markers &amp; references · {{ Math.round(zoom * 100) }}%</span>
    </div>

    <div class="vc-body">
      <div class="vc-stage-wrap">
      <svg ref="svgRef" class="vc-stage" :class="`tool-${tool}`" @mousedown="onPointerDown" @mousemove="onPointerMove" @mouseup="onPointerUp"
        @mouseleave="onPointerUp" @dblclick="onDoubleClick" @wheel="onWheel">
        <defs>
          <pattern id="vc-grid" width="32" height="32" patternUnits="userSpaceOnUse">
            <path d="M 32 0 L 0 0 0 32" fill="none" stroke="var(--vc-grid)" stroke-width="1" />
          </pattern>
        </defs>
        <g :transform="`translate(${panX} ${panY}) scale(${zoom})`">
          <rect x="0" y="0" :width="doc.width" :height="doc.height" fill="url(#vc-grid)" class="vc-board" />
          <template v-for="shape in orderedShapes" :key="shape.id">
            <g v-if="layerById.get(shape.layer)?.visible !== false" :opacity="layerById.get(shape.layer)?.opacity ?? 1"
              :class="['vc-shape', selectedId === shape.id && 'is-selected']" :data-kind="shape.kind" @mousedown="onShapeDown($event, shape)" @dblclick.stop="openReference(shape)"
              @mousemove="onShapeHover($event, shape)" @mouseleave="hover = null">
              <CanvasShapeView :shape="shape" :runtime="showRuntime" :uid="uid" />
            </g>
          </template>
          <CanvasShapeView v-if="draft" :shape="draft" :runtime="false" :uid="uid" class="vc-draft" />
          <template v-if="selected && layerById.get(selected.layer)?.visible !== false">
            <rect class="vc-selection" :x="selected.x - 4" :y="selected.y - 4" :width="Math.max(selected.w, 1) + 8" :height="Math.max(selected.h, 1) + 8" />
            <rect v-if="selected.kind !== 'line' && !isLocked(selected)" class="vc-handle" :x="selected.x + selected.w - 5" :y="selected.y + selected.h - 5" width="10" height="10"
              @mousedown="onShapeDown($event, selected, 'resize')" />
          </template>
        </g>
      </svg>
      <div v-if="hovered" class="vc-tooltip" role="tooltip" :style="{ left: `${hovered.x}px`, top: `${hovered.y}px` }">
        <strong>{{ hovered.name }}</strong>
        <p>{{ hovered.description }}</p>
      </div>
      </div>

      <aside class="vc-side">
        <section v-if="selected" class="vc-card" aria-label="Selected shape">
          <header class="vc-card-head">
            <strong>{{ shapeDisplayName(selected, refName) }}</strong>
            <span class="badge">{{ selected.kind }}</span>
          </header>
          <label v-if="selected.kind !== 'note'" class="vc-field"><span>Name on canvas</span>
            <input class="form-input" :value="selected.label ?? ''" :placeholder="refInfo(selected)?.name ?? ''" :disabled="isLocked(selected)" @change="updateSelected({ label: ($event.target as HTMLInputElement).value }, 'Rename')" /></label>
          <label v-if="selected.kind === 'text'" class="vc-field"><span>Text</span>
            <input class="form-input" :value="selected.text ?? ''" :disabled="isLocked(selected)" @change="updateSelected({ text: ($event.target as HTMLInputElement).value }, 'Edit text')" /></label>
          <template v-if="selected.kind === 'note'">
            <label class="vc-field"><span>Note</span>
              <textarea class="form-input" rows="4" :value="selected.text ?? ''" :disabled="isLocked(selected)" @change="updateSelected({ text: ($event.target as HTMLTextAreaElement).value }, 'Edit note')"></textarea></label>
            <div class="vc-swatches" role="group" aria-label="Note colour">
              <button v-for="color in NOTE_COLORS" :key="color" :class="['vc-swatch', selected.fill === color && 'is-active']" :style="{ background: color }"
                :aria-label="`Note colour ${color}`" :disabled="isLocked(selected)" @click="updateSelected({ fill: color }, 'Note colour')"></button>
            </div>
          </template>
          <div v-if="selected.kind === 'marker'" class="vc-field"><span>Icon</span>
            <div class="vc-icons" role="group" aria-label="Marker icons">
              <button v-for="icon in MARKER_ICONS" :key="icon.id" :class="['vc-icon-pick', selected.icon === icon.id && 'is-active']" :title="icon.label" :aria-label="icon.label"
                :aria-pressed="selected.icon === icon.id" :disabled="isLocked(selected)" @click="updateSelected({ icon: icon.id }, 'Change icon')">{{ icon.glyph }}</button>
            </div>
          </div>
          <label v-if="isGameplay(selected)" class="vc-field"><span>Description (shown on hover)</span>
            <textarea class="form-input" rows="3" :value="selected.description ?? ''" :placeholder="refInfo(selected)?.description ?? ''" :disabled="isLocked(selected)"
              @change="updateSelected({ description: ($event.target as HTMLTextAreaElement).value }, 'Edit description')"></textarea></label>
          <label class="vc-field"><span>Layer</span>
            <select class="form-select" :value="selected.layer" :disabled="isLocked(selected)" @change="updateSelected({ layer: ($event.target as HTMLSelectElement).value }, 'Move to layer')">
              <option v-for="layer in doc.layers" :key="layer.id" :value="layer.id">{{ layer.name }}</option>
            </select></label>
          <label v-if="selected.kind === 'entity'" class="vc-field"><span>Entity</span>
            <select class="form-select" :value="selected.entity_id ?? ''" :disabled="isLocked(selected)" @change="updateSelected({ entity_id: ($event.target as HTMLSelectElement).value || undefined }, 'Link entity')">
              <option value="">None</option>
              <option v-for="entity in allEntities" :key="entity.id" :value="entity.id">{{ entity.name }} ({{ entity.root_tag }})</option>
            </select></label>
          <label v-if="selected.kind === 'storylet'" class="vc-field"><span>Storylet</span>
            <select class="form-select" :value="selected.storylet_id ?? ''" :disabled="isLocked(selected)" @change="updateSelected({ storylet_id: ($event.target as HTMLSelectElement).value || undefined }, 'Link storylet')">
              <option value="">None</option>
              <option v-for="storylet in storylets" :key="storylet.id" :value="storylet.id">{{ storylet.title }}</option>
            </select></label>
          <button v-if="refInfo(selected)?.route" class="btn btn-secondary vc-wide" @click="openReference(selected)">Open {{ refInfo(selected)?.name }} →</button>
          <div v-if="isGameplay(selected)" class="vc-state">
            <label><input type="checkbox" :checked="selected.present" :disabled="isLocked(selected)" @change="updateSelected({ present: ($event.target as HTMLInputElement).checked }, 'Set presence')" /> {{ nodeMode ? "On canvas after this node" : "On canvas at start" }}</label>
            <small v-if="nodeMode" class="form-hint">Unchecking or deleting removes it when the node runs; new markers and references appear when it runs.</small>
            <small v-else class="form-hint">Storylets change the canvas with a <strong>Modify Canvas</strong> node.</small>
          </div>
          <div v-if="!isGameplay(selected) && selected.kind !== 'note'" class="vc-row">
            <label class="vc-field"><span>Stroke</span><input type="color" :value="selected.stroke.startsWith('#') ? selected.stroke : '#888888'" :disabled="isLocked(selected)" @change="updateSelected({ stroke: ($event.target as HTMLInputElement).value }, 'Stroke color')" /></label>
            <label class="vc-field"><span>Fill</span><input type="color" :value="selected.fill.startsWith('#') ? selected.fill : '#444444'" :disabled="isLocked(selected)" @change="updateSelected({ fill: ($event.target as HTMLInputElement).value }, 'Fill color')" /></label>
            <label class="vc-field"><span>Width</span><input class="form-input" type="number" min="1" max="40" :value="selected.strokeWidth" :disabled="isLocked(selected)" @change="updateSelected({ strokeWidth: Number(($event.target as HTMLInputElement).value) || 1 }, 'Stroke width')" /></label>
          </div>
          <div class="vc-row">
            <button class="btn btn-secondary" :disabled="isLocked(selected)" @click="duplicateSelected">Duplicate</button>
            <button class="btn btn-danger" :disabled="isLocked(selected)" @click="deleteSelected">Delete</button>
          </div>
        </section>

        <nav class="vc-tabs">
          <button :class="['vc-tab', sidePanel === 'layers' && 'is-active']" @click="sidePanel = 'layers'">Layers</button>
          <button v-if="!nodeMode" :class="['vc-tab', sidePanel === 'history' && 'is-active']" @click="sidePanel = 'history'">History · {{ doc.revisions.length }}</button>
        </nav>

        <section v-if="sidePanel === 'layers'" class="vc-card" aria-label="Layers">
          <ul class="vc-layers">
            <li v-for="layer in [...doc.layers].reverse()" :key="layer.id" :class="['vc-layer', activeLayer === layer.id && 'is-active']" @click="activeLayer = layer.id">
              <button class="vc-icon" :title="layer.visible ? 'Hide layer' : 'Show layer'" :aria-label="layer.visible ? 'Hide layer' : 'Show layer'" @click.stop="updateLayer(layer.id, { visible: !layer.visible }, 'Toggle layer')">{{ layer.visible ? '👁' : '—' }}</button>
              <button class="vc-icon" :title="layer.locked ? 'Unlock' : 'Lock'" :aria-label="layer.locked ? 'Unlock layer' : 'Lock layer'" @click.stop="updateLayer(layer.id, { locked: !layer.locked }, 'Lock layer')">{{ layer.locked ? '🔒' : '🔓' }}</button>
              <input class="vc-layer-name" :value="layer.name" :disabled="readonly" aria-label="Layer name" @change="updateLayer(layer.id, { name: ($event.target as HTMLInputElement).value }, 'Rename layer')" />
              <input class="vc-opacity" type="range" min="0.1" max="1" step="0.1" :value="layer.opacity" aria-label="Layer opacity" @click.stop @change="updateLayer(layer.id, { opacity: Number(($event.target as HTMLInputElement).value) }, 'Layer opacity')" />
              <button class="vc-icon" title="Move up" aria-label="Move layer up" @click.stop="moveLayer(layer.id, 1)">▲</button>
              <button class="vc-icon" title="Move down" aria-label="Move layer down" @click.stop="moveLayer(layer.id, -1)">▼</button>
              <button class="vc-icon" title="Delete layer" aria-label="Delete layer" :disabled="doc.layers.length <= 1" @click.stop="removeLayer(layer.id)">✕</button>
            </li>
          </ul>
          <button class="btn btn-secondary vc-wide" :disabled="readonly" @click="addLayer">+ Add layer</button>
          <small class="form-hint">New shapes go on the highlighted layer.</small>
        </section>

        <section v-else-if="!nodeMode" class="vc-card" aria-label="History">
          <p v-if="!doc.revisions.length" class="form-hint">No changes yet. Every edit is recorded here and saved with the entity.</p>
          <ol class="vc-history">
            <li v-for="(revision, index) in [...doc.revisions].reverse()" :key="revision.at + '-' + index" :class="revision.source && 'is-node-version'">
              <span>{{ revision.label }} <span v-if="revision.source" class="badge vc-version-badge">{{ versionTempState(revision) || "storylet version" }}</span></span>
              <small>{{ formatTime(revision.at) }}</small>
              <button class="btn btn-secondary btn-small" :disabled="readonly || index === 0" @click="restore(doc.revisions.length - 1 - index)">Restore</button>
            </li>
          </ol>
        </section>
      </aside>
    </div>
  </div>
</template>

<style scoped>
.vc-root { --vc-grid: color-mix(in srgb, var(--text-muted, #888) 18%, transparent); display: flex; flex-direction: column; gap: 8px; min-height: 620px; }
.vc-toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; padding: 6px; border: 1px solid var(--border, #333); border-radius: 10px; background: var(--surface-2, #1c1c1e); }
.vc-tool { min-width: 34px; height: 34px; border-radius: 8px; border: 1px solid transparent; background: transparent; color: var(--text, #eee); font-size: 16px; cursor: pointer; }
.vc-tool:hover:not(:disabled) { background: var(--surface-3, #2c2c2e); }
.vc-tool.is-active { border-color: var(--accent, #0a84ff); background: color-mix(in srgb, var(--accent, #0a84ff) 20%, transparent); }
.vc-tool.is-logic { color: var(--type-location, #ff9f0a); }
.vc-tool:disabled { opacity: .4; cursor: default; }
.vc-entity-pick { max-width: 240px; }
.vc-sep { width: 1px; height: 24px; background: var(--border, #333); margin: 0 6px; }
.vc-toggle { display: flex; gap: 6px; align-items: center; font-size: 13px; margin-left: 8px; }
.vc-status { margin-left: auto; font-size: 12px; color: var(--text-muted, #999); }
.vc-body { display: grid; grid-template-columns: minmax(0, 1fr) 300px; gap: 8px; flex: 1; }
@media (max-width: 900px) { .vc-body { grid-template-columns: 1fr; } }
.vc-stage { width: 100%; height: 640px; border: 1px solid var(--border, #333); border-radius: 10px; background: var(--surface-1, #111); user-select: none; }
.vc-stage.tool-pan { cursor: grab; }
.vc-stage:not(.tool-select):not(.tool-pan) { cursor: crosshair; }
.vc-board { stroke: var(--border, #333); }
.vc-shape { cursor: pointer; }
.vc-selection { fill: none; stroke: var(--accent, #0a84ff); stroke-width: 1.5; stroke-dasharray: 4 3; pointer-events: none; }
.vc-handle { fill: var(--accent, #0a84ff); cursor: nwse-resize; }
.vc-draft { opacity: .6; pointer-events: none; }
.vc-stage-wrap { position: relative; min-width: 0; }
.vc-tooltip { position: absolute; z-index: 5; max-width: 320px; padding: 8px 10px; border-radius: 8px; border: 1px solid var(--border, #333); background: var(--surface-3, #2c2c2e); color: var(--text, #eee); font-size: 13px; box-shadow: 0 6px 20px rgba(0,0,0,.35); pointer-events: none; }
.vc-tooltip p { margin: 4px 0 0; white-space: pre-wrap; line-height: 1.4; }
.vc-icons { display: grid; grid-template-columns: repeat(auto-fill, minmax(32px, 1fr)); gap: 4px; }
.vc-icon-pick { height: 32px; border-radius: 6px; border: 1px solid var(--border, #333); background: transparent; font-size: 16px; cursor: pointer; }
.vc-icon-pick.is-active { border-color: var(--accent, #0a84ff); background: color-mix(in srgb, var(--accent, #0a84ff) 20%, transparent); }
.vc-swatches { display: flex; gap: 6px; }
.vc-swatch { width: 26px; height: 26px; border-radius: 6px; border: 2px solid transparent; cursor: pointer; }
.vc-swatch.is-active { border-color: var(--accent, #0a84ff); }
.vc-side { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
.vc-card { display: flex; flex-direction: column; gap: 8px; padding: 10px; border: 1px solid var(--border, #333); border-radius: 10px; background: var(--surface-2, #1c1c1e); }
.vc-card-head { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
.vc-field { display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--text-muted, #aaa); min-width: 0; }
.vc-row { display: flex; gap: 8px; align-items: flex-end; }
.vc-row > * { flex: 1; }
.vc-wide { width: 100%; }
.vc-state { display: flex; flex-direction: column; gap: 4px; font-size: 13px; padding: 8px; border-radius: 8px; background: color-mix(in srgb, var(--type-location, #ff9f0a) 10%, transparent); }
.vc-tabs { display: flex; gap: 4px; }
.vc-tab { flex: 1; padding: 6px; border-radius: 8px; border: 1px solid var(--border, #333); background: transparent; color: var(--text, #eee); cursor: pointer; }
.vc-tab.is-active { background: var(--surface-3, #2c2c2e); border-color: var(--accent, #0a84ff); }
.vc-layers { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; }
.vc-layer { display: flex; align-items: center; gap: 2px; padding: 4px; border-radius: 8px; border: 1px solid transparent; }
.vc-layer.is-active { border-color: var(--accent, #0a84ff); background: color-mix(in srgb, var(--accent, #0a84ff) 12%, transparent); }
.vc-layer-name { flex: 1; min-width: 0; background: transparent; border: none; color: var(--text, #eee); font-size: 13px; }
.vc-opacity { width: 48px; }
.vc-icon { width: 24px; height: 24px; border: none; background: transparent; color: var(--text, #eee); cursor: pointer; font-size: 12px; border-radius: 6px; }
.vc-icon:hover:not(:disabled) { background: var(--surface-3, #2c2c2e); }
.vc-history { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 4px; max-height: 360px; overflow: auto; }
.vc-history li { display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; align-items: center; font-size: 13px; padding: 4px 0; border-bottom: 1px solid var(--border, #333); }
.vc-history small { grid-column: 1; color: var(--text-muted, #999); }
.vc-history li.is-node-version span:first-child { color: var(--type-location, #ff9f0a); }
.vc-version-badge { font-size: 10px; margin-left: 4px; }
.vc-history button { grid-row: 1 / 3; grid-column: 2; }
</style>
