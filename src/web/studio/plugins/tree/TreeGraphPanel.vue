<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { StudioApi } from "../../api.ts";
import type { EntityTemplateRecord } from "../../contracts.ts";
import { studioState } from "../../state.ts";
import { thumbnailUrl } from "../../thumbnails.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import type { AnyEntity } from "../runtime.ts";
import { TREE_EXAMPLES } from "./examples.ts";
import { TREE_GRAPH_KEY, TREE_ITEM_KEY, layoutTree, setTreeGraph, treeErrors, treeGraphOf, treeRoots, treeTrait, type TreeDirection, type TreeEdge, type TreeGraph, type TreePoint, type TreePositions } from "./model.ts";

const props = defineProps<{ entity: AnyEntity; readonly?: boolean }>();
const emit = defineEmits<{ (event: "patch", patch: Partial<AnyEntity>): void; (event: "save"): void }>();
const item = ref("");
const source = ref("");
const target = ref("");
const notice = ref("");
const status = ref("");
const example = ref("");
const creatingExample = ref(false);
let mounted = true;
onBeforeUnmount(() => { mounted = false; });
let drag: { id: string; pointer: number; startX: number; startY: number; point: TreePoint } | null = null;
const linkDrag = ref<{ from: string; pointer: number; startX: number; startY: number; x: number; y: number; moved: boolean } | null>(null);
const zoom = ref(100);
const treeScroll = ref<HTMLElement | null>(null);
const panX = ref(40);
const panY = ref(40);
let pan: { pointer: number; x: number; y: number } | null = null;
const markerId = `tree-arrow-${crypto.randomUUID()}`;
const owner = computed(() => treeTrait(props.entity, studioState.traitTemplates, TREE_GRAPH_KEY));
const parsed = computed<{ graph: TreeGraph | null; error: string }>(() => {
  if (!owner.value) return { graph: null, error: "" };
  try { return { graph: treeGraphOf(owner.value), error: "" }; }
  catch (error) { return { graph: null, error: error instanceof Error ? error.message : String(error) }; }
});
const graph = computed(() => parsed.value.graph);
const entities = computed(() => [...studioState.entities, ...studioState.entityTemplates] as AnyEntity[]);
const byId = computed(() => new Map(entities.value.map((entity) => [entity.id, entity])));
const eligible = computed(() => entities.value.filter((entity) => !!treeTrait(entity, studioState.traitTemplates, TREE_ITEM_KEY)));
const eligibleIds = computed(() => new Set(eligible.value.map((entity) => entity.id)));
const available = computed(() => eligible.value.filter((entity) => !graph.value?.nodes.includes(entity.id)));
const issues = computed(() => graph.value ? treeErrors(graph.value, eligibleIds.value) : []);
const roots = computed(() => new Set(graph.value ? treeRoots(graph.value) : []));
const layout = computed(() => graph.value ? layoutTree(graph.value) : null);
const zoomFactor = computed(() => zoom.value / 100);
const label = (id: string) => byId.value.get(id)?.name ?? id;
const description = (id: string) => byId.value.get(id)?.description ?? "";
async function fitGraph() {
  await nextTick();
  const viewport = treeScroll.value;
  const current = layout.value;
  if (!viewport || !current || !graph.value?.nodes.length) return;
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
  for (const point of Object.values(current.positions)) {
    left = Math.min(left, point.x - 40);
    top = Math.min(top, point.y - 40);
    right = Math.max(right, point.x + 204);
    bottom = Math.max(bottom, point.y + 108);
  }
  zoom.value = Math.max(10, Math.min(100, Math.floor(Math.min(viewport.clientWidth / (right - left), viewport.clientHeight / (bottom - top)) * 100)));
  panX.value = Math.round((viewport.clientWidth - (right - left) * zoomFactor.value) / 2 - left * zoomFactor.value);
  panY.value = Math.round((viewport.clientHeight - (bottom - top) * zoomFactor.value) / 2 - top * zoomFactor.value);
}
watch([() => graph.value?.direction, () => graph.value?.nodes.length], () => { void fitGraph(); }, { flush: "post", immediate: true });
watch(treeScroll, (element, _previous, onCleanup) => {
  if (!element) return;
  const observer = new ResizeObserver(() => { void fitGraph(); });
  observer.observe(element);
  onCleanup(() => observer.disconnect());
}, { flush: "post" });
function startPan(event: PointerEvent) {
  if (event.button !== 0 && event.button !== 1) return;
  if ((event.target as Element).closest(".tree-node")) return;
  pan = { pointer: event.pointerId, x: event.clientX - panX.value, y: event.clientY - panY.value };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}
function movePan(event: PointerEvent) {
  if (!pan || event.pointerId !== pan.pointer) return;
  panX.value = event.clientX - pan.x;
  panY.value = event.clientY - pan.y;
}
function endPan(event: PointerEvent) {
  if (pan?.pointer === event.pointerId) pan = null;
}
function wheelZoom(event: WheelEvent) {
  const viewport = treeScroll.value;
  if (!viewport) return;
  const rect = viewport.getBoundingClientRect();
  const x = event.clientX - rect.left, y = event.clientY - rect.top;
  const next = Math.max(10, Math.min(200, Math.round(zoom.value * (event.deltaY > 0 ? 0.9 : 1.1))));
  const ratio = next / zoom.value;
  panX.value = x - (x - panX.value) * ratio;
  panY.value = y - (y - panY.value) * ratio;
  zoom.value = next;
}
function keyboardPan(event: KeyboardEvent) {
  if (event.target !== event.currentTarget) return;
  const delta: Record<string, TreePoint> = { ArrowLeft: { x: 40, y: 0 }, ArrowRight: { x: -40, y: 0 }, ArrowUp: { x: 0, y: 40 }, ArrowDown: { x: 0, y: -40 } };
  const move = delta[event.key];
  if (!move) return;
  event.preventDefault();
  panX.value += move.x;
  panY.value += move.y;
}

function patch(next: TreeGraph) {
  if (!owner.value || props.readonly) return;
  emit("patch", setTreeGraph(props.entity, owner.value, next));
  notice.value = "";
  status.value = "";
}
function addItem() {
  if (!graph.value || !available.value.some((entity) => entity.id === item.value)) {
    notice.value = "Choose an entity with the TreeItem trait.";
    return;
  }
  patch({ ...graph.value, nodes: [...graph.value.nodes, item.value] });
  item.value = "";
}
function removeItem(id: string) {
  if (!graph.value) return;
  const positions: TreePositions = {};
  for (const [view, points] of Object.entries(graph.value.positions ?? {})) {
    positions[view as TreeDirection] = Object.fromEntries(Object.entries(points).filter(([node]) => node !== id));
  }
  patch({ ...graph.value, nodes: graph.value.nodes.filter((node) => node !== id), edges: graph.value.edges.filter((edge) => edge.from !== id && edge.to !== id), positions });
  if (source.value === id) source.value = "";
  if (target.value === id) target.value = "";
}
function connect(from: string, to: string) {
  if (!graph.value || props.readonly) return;
  if (!graph.value.nodes.includes(from) || !graph.value.nodes.includes(to)) { notice.value = "Choose both connection endpoints."; return; }
  if (graph.value.edges.some((edge) => edge.from === from && edge.to === to)) { notice.value = "That connection already exists."; return; }
  patch({ ...graph.value, edges: [...graph.value.edges, { from, to }] });
}
function addEdge() { connect(source.value, target.value); }
function beginLink(id: string, event: PointerEvent) {
  if (event.button !== 0 || props.readonly || creatingExample.value) return;
  source.value = id;
  linkDrag.value = { from: id, pointer: event.pointerId, startX: event.clientX, startY: event.clientY, x: event.clientX, y: event.clientY, moved: false };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.stopPropagation();
  event.preventDefault();
}
function moveLink(event: PointerEvent) {
  const active = linkDrag.value;
  if (!active || active.pointer !== event.pointerId) return;
  linkDrag.value = { ...active, x: event.clientX, y: event.clientY, moved: active.moved || Math.hypot(event.clientX - active.startX, event.clientY - active.startY) > 4 };
  event.stopPropagation();
}
function finishLink(event: PointerEvent) {
  const active = linkDrag.value;
  if (!active || active.pointer !== event.pointerId) return;
  linkDrag.value = null;
  event.stopPropagation();
  if (!active.moved) return;
  const node = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-tree-id]");
  if (node?.dataset.treeId) connect(active.from, node.dataset.treeId);
}
const pendingLink = computed(() => {
  const active = linkDrag.value, viewport = treeScroll.value;
  if (!active?.moved || !viewport) return null;
  const point = layout.value?.positions[active.from];
  if (!point) return null;
  const rect = viewport.getBoundingClientRect();
  return `M ${point.x + 164} ${point.y + 34} L ${(active.x - rect.left - panX.value) / zoomFactor.value} ${(active.y - rect.top - panY.value) / zoomFactor.value}`;
});
function removeEdge(index: number) {
  if (graph.value) patch({ ...graph.value, edges: graph.value.edges.filter((_, i) => i !== index) });
}
function changeDirection(event: Event) {
  if (!graph.value) return;
  patch({ ...graph.value, direction: (event.target as HTMLSelectElement).value as TreeDirection });
}
function moveNode(id: string, point: TreePoint) {
  if (!graph.value || props.readonly) return;
  const direction = graph.value.direction;
  const positions = { ...graph.value.positions, [direction]: { ...graph.value.positions?.[direction], [id]: point } };
  patch({ ...graph.value, positions });
}
function startDrag(id: string, event: PointerEvent) {
  if (props.readonly || creatingExample.value || event.button !== 0 || (event.target as Element).closest("button")) return;
  const point = layout.value?.positions[id];
  if (!point) return;
  drag = { id, pointer: event.pointerId, startX: event.clientX, startY: event.clientY, point: { ...point } };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}
function dragNode(event: PointerEvent) {
  if (!drag || event.pointerId !== drag.pointer) return;
  moveNode(drag.id, { x: Math.round(drag.point.x + (event.clientX - drag.startX) / zoomFactor.value), y: Math.round(drag.point.y + (event.clientY - drag.startY) / zoomFactor.value) });
}
function endDrag(event: PointerEvent) {
  if (drag?.pointer === event.pointerId) drag = null;
}
function keyboardMove(id: string, event: KeyboardEvent) {
  if (props.readonly || event.target !== event.currentTarget) return;
  const delta: Record<string, TreePoint> = { ArrowLeft: { x: -10, y: 0 }, ArrowRight: { x: 10, y: 0 }, ArrowUp: { x: 0, y: -10 }, ArrowDown: { x: 0, y: 10 } };
  const change = delta[event.key];
  const point = layout.value?.positions[id];
  if (!change || !point) return;
  event.preventDefault();
  moveNode(id, { x: point.x + change.x, y: point.y + change.y });
}
function resetLayout() {
  if (!graph.value) return;
  const positions = { ...graph.value.positions };
  delete positions[graph.value.direction];
  patch({ ...graph.value, positions });
}
function exportJson() {
  if (!graph.value || !layout.value) return;
  const data = {
    format: "decision-narrative-tree-graph", version: 1, name: props.entity.name,
    graph: graph.value, layout: layout.value.positions,
    items: graph.value.nodes.map((id) => ({ id, name: label(id), description: description(id), thumbnail: byId.value.get(id)?.thumbnail ?? null })),
  };
  const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], { type: "application/json" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = `${props.entity.name.replace(/[\\/:*?"<>|\x00-\x1f]/g, "_") || "tree-graph"}.json`;
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
async function createExample() {
  const selected = TREE_EXAMPLES.find((entry) => entry.id === example.value);
  const marker = studioState.traitTemplates.find((entry) => entry.key === TREE_ITEM_KEY);
  if (!selected || !marker || !graph.value || props.readonly || creatingExample.value) return;
  if (!window.confirm(`Create ${selected.items.length} TreeItem entity templates and replace this graph with ${selected.name}? Save the entity to keep the graph.`)) return;
  creatingExample.value = true;
  const created: string[] = [];
  const ids = new Map<string, string>();
  try {
    for (const entry of selected.items) {
      const result = await StudioApi.saveContent<EntityTemplateRecord>("entity_templates", {
        id: crypto.randomUUID(), name: entry.name,
        traits: [{ id: crypto.randomUUID(), template_id: marker.id, values: {} }],
      });
      created.push(result.id);
      ids.set(entry.key, result.id);
    }
    if (!mounted) throw new Error("The editor was closed.");
    await refreshWorkspace(["entity_templates"]);
    if (!mounted) throw new Error("The editor was closed.");
    patch({ nodes: selected.items.map((entry) => ids.get(entry.key)!), edges: selected.edges.map((edge) => ({ from: ids.get(edge.from)!, to: ids.get(edge.to)! })), direction: selected.direction, positions: {} });
    source.value = ""; target.value = ""; example.value = "";
    status.value = `Created ${created.length} TreeItem entities. Save this entity to keep the example graph.`;
  } catch (error) {
    const cleanup = await Promise.allSettled(created.map((id) => StudioApi.deleteContent("entity_templates", id)));
    await refreshWorkspace(["entity_templates"]).catch(() => {});
    const remaining = cleanup.filter((result) => result.status === "rejected").length;
    notice.value = `Could not create example: ${error instanceof Error ? error.message : String(error)}.${remaining ? ` ${remaining} created entities could not be removed.` : ""}`;
  } finally { creatingExample.value = false; }
}
function edgePath(edge: TreeEdge): string | null {
  const a = layout.value?.positions[edge.from];
  const b = layout.value?.positions[edge.to];
  if (!a || !b) return null;
  if (edge.from === edge.to) {
    return `M ${a.x + 108} ${a.y} C ${a.x + 130} ${a.y - 38}, ${a.x + 34} ${a.y - 38}, ${a.x + 56} ${a.y}`;
  }
  const ax = a.x + 82, ay = a.y + 34, bx = b.x + 82, by = b.y + 34;
  const dx = bx - ax, dy = by - ay;
  if (dx === 0 && dy === 0) return null;
  const clip = (x: number, y: number) => Math.min(x === 0 ? Infinity : 82 / Math.abs(x), y === 0 ? Infinity : 34 / Math.abs(y));
  const t = clip(dx, dy);
  const x1 = ax + dx * t, y1 = ay + dy * t, x2 = bx - dx * t, y2 = by - dy * t;
  const reciprocal = graph.value?.edges.some((other) => other.from === edge.to && other.to === edge.from);
  if (!reciprocal) return `M ${x1} ${y1} L ${x2} ${y2}`;
  const length = Math.hypot(dx, dy);
  return `M ${x1} ${y1} Q ${(x1 + x2) / 2 - dy / length * 32} ${(y1 + y2) / 2 + dx / length * 32} ${x2} ${y2}`;
}
</script>

<template>
  <section class="tree-panel" aria-label="Tree graph">
    <header class="tree-header">
      <div><h2>Tree graph</h2><p class="field-sublabel">Drag items or use arrow keys to adjust the layout. Saving also saves other entity changes.</p></div>
      <div class="tree-view-controls">
        <label>View
          <select class="form-select" aria-label="Graph direction" :value="graph?.direction" :disabled="readonly || !graph || creatingExample" @change="changeDirection">
            <option value="right">Left to right</option><option value="left">Right to left</option>
            <option value="down">Top to bottom</option><option value="up">Bottom to top</option><option value="radial">Radial</option>
          </select>
        </label>
        <label class="zoom-label">Zoom <output>{{ zoom }}%</output><input v-model.number="zoom" type="range" min="10" max="200" step="1" aria-label="Graph zoom" /></label>
        <button type="button" class="btn btn-secondary" :disabled="!graph?.nodes.length" @click="fitGraph">Fit graph</button>
        <button v-if="!readonly" type="button" class="btn btn-secondary" :disabled="!graph || creatingExample" @click="resetLayout">Reset view layout</button>
        <button v-if="!readonly" type="button" class="btn btn-primary" :disabled="!graph || creatingExample || !!issues.length" @click="emit('save')">Save layout</button>
        <button type="button" class="btn btn-secondary" :disabled="!graph || !graph.nodes.length" @click="exportJson">Export JSON</button>
      </div>
    </header>
    <p v-if="parsed.error" class="tree-error" role="alert">Tree data cannot be read: {{ parsed.error }}</p>
    <template v-if="graph && layout">
      <fieldset v-if="!readonly" class="tree-tools" :disabled="creatingExample">
        <legend>Tree connections</legend>
        <label>Tree item
          <select v-model="item" class="form-select" aria-label="Tree item to add">
            <option value="">Choose a TreeItem entity…</option>
            <option v-for="candidate in available" :key="candidate.id" :value="candidate.id">{{ candidate.name }} ({{ candidate.id }})</option>
          </select>
        </label>
        <button type="button" class="btn btn-primary" :disabled="!item" @click="addItem">Add item</button>
        <div class="tree-tool-divider" aria-hidden="true"></div>
        <label>From
          <select v-model="source" class="form-select" aria-label="Connection source"><option value="">Choose item…</option><option v-for="id in graph.nodes" :key="id" :value="id">{{ label(id) }} ({{ id }})</option></select>
        </label>
        <label>To
          <select v-model="target" class="form-select" aria-label="Connection target"><option value="">Choose item…</option><option v-for="id in graph.nodes" :key="id" :value="id">{{ label(id) }} ({{ id }})</option></select>
        </label>
        <button type="button" class="btn btn-secondary" :disabled="!source || !target" @click="addEdge">Connect</button>
      </fieldset>
      <div v-if="!readonly" class="tree-examples">
        <label>Example layout
          <select v-model="example" class="form-select" aria-label="Example layout" :disabled="creatingExample">
            <option value="">Choose an example…</option>
            <option v-for="entry in TREE_EXAMPLES" :key="entry.id" :value="entry.id">{{ entry.name }}</option>
          </select>
        </label>
        <button type="button" class="btn btn-secondary" :disabled="!example || creatingExample" @click="createExample">{{ creatingExample ? 'Creating…' : 'Create example' }}</button>
        <span class="field-sublabel">Creates TreeItem entities in this project.</span>
      </div>
      <p v-if="notice || issues.length" class="tree-error" role="alert">{{ notice || issues.join(' ') }}</p>
      <p v-if="status" class="field-sublabel" role="status">{{ status }}</p>
      <div class="tree-meta"><span>{{ graph.nodes.length }} items</span><span>{{ graph.edges.length }} connections</span><span>{{ roots.size }} roots</span></div>
      <div ref="treeScroll" class="tree-scroll" tabindex="0" aria-label="Tree graph canvas; drag background to pan, use mouse wheel to zoom" @pointerdown="startPan" @pointermove="movePan" @pointerup="endPan" @pointercancel="endPan" @wheel.prevent="wheelZoom" @keydown="keyboardPan">
        <div v-if="!graph.nodes.length" class="tree-empty"><strong>Start with an item</strong><span>Add an entity with the TreeItem trait, then connect it to others.</span></div>
        <div v-else class="tree-world" :style="{ transform: `translate(${panX}px, ${panY}px) scale(${zoomFactor})` }">
            <svg class="tree-edges" aria-hidden="true">
              <defs><marker :id="markerId" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="8" markerHeight="8" orient="auto"><path d="M 0 0 L 8 4 L 0 8 z" /></marker></defs>
              <template v-for="(edge, index) in graph.edges" :key="`${edge.from}-${edge.to}-${index}`"><path v-if="edgePath(edge)" :d="edgePath(edge) ?? ''" class="tree-edge" :marker-end="`url(#${markerId})`" /></template>
              <path v-if="pendingLink" :d="pendingLink" class="tree-edge tree-edge-pending" />
            </svg>
            <article v-for="id in graph.nodes" :key="id" class="tree-node" :data-tree-id="id" :class="{ 'is-movable': !readonly }" :style="{ left: `${layout.positions[id]?.x ?? 0}px`, top: `${layout.positions[id]?.y ?? 0}px` }" :tabindex="readonly ? undefined : 0" :aria-label="`${label(id)}${roots.has(id) ? ', root' : ''}. Use arrow keys to move.`" @pointerdown="startDrag(id, $event)" @pointermove="dragNode" @pointerup="endDrag" @pointercancel="endDrag" @keydown="keyboardMove(id, $event)">
              <button v-if="!readonly" type="button" class="tree-port tree-port-in" :disabled="!source || creatingExample" :aria-label="`Connect to ${label(id)}`" :title="`Connect to ${label(id)}`" @click.stop="connect(source, id)"></button>
              <button v-if="!readonly" type="button" class="tree-port tree-port-out" :disabled="creatingExample" :aria-label="`Connect from ${label(id)}`" :title="`Drag from ${label(id)} to another item, or click then choose a target`" @pointerdown.stop="beginLink(id, $event)" @pointermove.stop="moveLink" @pointerup.stop="finishLink" @pointercancel.stop="linkDrag = null" @click.stop="source = id"></button>
              <img v-if="thumbnailUrl(byId.get(id)?.thumbnail)" :src="thumbnailUrl(byId.get(id)?.thumbnail) ?? ''" alt="" width="36" height="36" />
              <span v-else class="tree-node-placeholder" aria-hidden="true">◇</span>
              <span class="tree-node-copy"><strong :title="label(id)">{{ label(id) }}</strong><small :title="description(id)">{{ roots.has(id) ? 'Root' : 'Tree item' }}</small></span>
              <button v-if="!readonly" type="button" class="tree-remove" :disabled="creatingExample" :aria-label="`Remove ${label(id)} from tree`" @click="removeItem(id)">×</button>
            </article>
          </div>
      </div>
      <details v-if="graph.edges.length" class="tree-connection-list"><summary>Connections ({{ graph.edges.length }})</summary>
        <ul><li v-for="(edge, index) in graph.edges" :key="`${edge.from}-${edge.to}-${index}`"><span>{{ label(edge.from) }} → {{ label(edge.to) }}</span><button v-if="!readonly" type="button" class="btn btn-secondary" :aria-label="`Remove connection ${label(edge.from)} to ${label(edge.to)}`" @click="removeEdge(index)">Remove</button></li></ul>
      </details>
    </template>
  </section>
</template>

<style scoped>
.tree-panel { display: grid; gap: 12px; min-width: 0; }
.tree-header, .tree-view-controls, .tree-tools, .tree-meta { display: flex; align-items: end; gap: 12px; flex-wrap: wrap; }
.tree-examples { display: flex; align-items: end; gap: 12px; flex-wrap: wrap; }
.tree-examples label { display: grid; gap: 4px; font-size: 12px; color: var(--text-muted); }
.tree-header { justify-content: space-between; }
.tree-header h2 { margin: 0 0 4px; }
.tree-header p { margin: 0; }
.tree-view-controls label, .tree-tools label { display: grid; gap: 4px; font-size: 12px; color: var(--text-muted); }
.tree-tools { border: 1px solid var(--border-subtle); background: var(--bg-card); padding: 12px; }
.tree-tools select { min-width: 150px; max-width: 250px; }
.tree-tool-divider { width: 1px; height: 32px; background: var(--border-subtle); }
.zoom-label { grid-template-columns: auto auto; }
.zoom-label input { grid-column: 1 / -1; width: 110px; }
.tree-error { color: var(--status-danger); margin: 0; }
.tree-meta { color: var(--text-muted); font-size: 12px; gap: 16px; }
.tree-scroll { position: relative; overflow: hidden; height: min(65vh, 620px); min-height: 360px; border: 1px solid var(--border-subtle); background-color: var(--bg-card); background-image: radial-gradient(var(--border-subtle) 1px, transparent 1px); background-size: 24px 24px; cursor: grab; touch-action: none; }
.tree-scroll:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.tree-empty { position: absolute; inset: 0; display: grid; place-content: center; text-align: center; gap: 8px; padding: 16px; color: var(--text-muted); pointer-events: none; }
.tree-empty strong { color: var(--text-primary); }
.tree-world { position: absolute; left: 0; top: 0; transform-origin: top left; }
.tree-edges { position: absolute; width: 1px; height: 1px; overflow: visible; pointer-events: none; }
.tree-edges .tree-edge { fill: none; stroke: var(--accent); stroke-width: 2; }
.tree-edges .tree-edge-pending { stroke-dasharray: 5 4; }
.tree-edges marker path { fill: var(--accent); }
.tree-node { position: absolute; box-sizing: border-box; width: 164px; height: 68px; display: flex; align-items: center; gap: 8px; padding: 8px; background: var(--bg-card); border: 1px solid var(--border-subtle); border-top: 2px solid var(--accent); }
.tree-port { position: absolute; top: 27px; width: 12px; height: 12px; padding: 0; border: 1px solid var(--accent); background: var(--bg-card); cursor: crosshair; z-index: 2; }
.tree-port-in { left: -7px; }
.tree-port-out { right: -7px; }
.tree-port:hover, .tree-port:focus-visible { background: var(--accent); outline: 2px solid var(--accent); outline-offset: 2px; }
.tree-port:disabled { opacity: 0.4; cursor: default; }
.tree-node.is-movable { cursor: grab; touch-action: none; }
.tree-node.is-movable:active { cursor: grabbing; }
.tree-node:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.tree-node img, .tree-node-placeholder { width: 36px; height: 36px; flex: none; object-fit: cover; }
.tree-node-placeholder { display: grid; place-items: center; color: var(--accent); border: 1px solid var(--border-subtle); }
.tree-node-copy { min-width: 0; flex: 1; display: grid; gap: 3px; }
.tree-node-copy strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.tree-node-copy small { color: var(--text-muted); font-size: 10px; }
.tree-remove { border: 0; background: transparent; color: var(--text-muted); cursor: pointer; padding: 2px 4px; }
.tree-remove:hover, .tree-remove:focus-visible { color: var(--status-danger); }
.tree-connection-list { border: 1px solid var(--border-subtle); padding: 8px 12px; }
.tree-connection-list summary { cursor: pointer; }
.tree-connection-list ul { list-style: none; padding: 0; margin: 8px 0 0; display: grid; gap: 4px; }
.tree-connection-list li { display: flex; justify-content: space-between; gap: 12px; align-items: center; }
@media (max-width: 700px) { .tree-tool-divider { display: none; } .tree-tools label { flex: 1 1 160px; } .tree-tools select { max-width: none; width: 100%; } }
</style>
