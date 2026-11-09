<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import ColorWheelPicker from "../../components/ColorWheelPicker.vue";
import TreeImageLayer from "./TreeImageLayer.vue";
import { StudioApi } from "../../api.ts";
import type { EntityTemplateRecord } from "../../contracts.ts";
import { studioState } from "../../state.ts";
import { thumbnailUrl } from "../../thumbnails.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import type { AnyEntity } from "../runtime.ts";
import { TREE_EXAMPLES } from "./examples.ts";
import { TREE_GRAPH_KEY, TREE_ITEM_KEY, TREE_SHAPES, layoutTree, setTreeGraph, treeErrors, treeGraphOf, treeRoots, treeTrait, type TreeCanvasImage, type TreeDirection, type TreeEdge, type TreeGraph, type TreePoint, type TreePositions, type TreeShape } from "./model.ts";

const props = defineProps<{ entity: AnyEntity; readonly?: boolean }>();
const emit = defineEmits<{ (event: "patch", patch: Partial<AnyEntity>): void; (event: "save"): void }>();
const wireOptions = [
  { id: "solid", label: "Normal wire" },
  { id: "dash", label: "Dashed wire" },
  { id: "dot", label: "Dotted wire" },
  { id: "brush", label: "Brush wire" },
] as const;
const source = ref("");
const notice = ref("");
const status = ref("");
const creatingExample = ref(false);
let mounted = true;
onBeforeUnmount(() => { mounted = false; });
let drag: { id: string; pointer: number; startX: number; startY: number; point: TreePoint } | null = null;
const linkDrag = ref<{ from: string; edge?: number; pointer: number; startX: number; startY: number; x: number; y: number; moved: boolean } | null>(null);
const resizeDrag = ref<{ id: string; pointer: number; x: number; y: number; width: number; height: number; left: number; top: number; direction: string } | null>(null);
const cutDrag = ref<{ pointer: number; start: TreePoint; end: TreePoint } | null>(null);
const zoom = ref(100);
const treeScroll = ref<HTMLElement | null>(null);
const imageInput = ref<HTMLInputElement | null>(null);
let imagePoint: TreePoint | null = null;
const panX = ref(40);
const panY = ref(40);
let pan: { pointer: number; x: number; y: number } | null = null;
const menu = ref<{ x: number; y: number; node?: string; edge?: number; image?: string; reconnect?: number; point: TreePoint } | null>(null);
const menuSearch = ref("");
const editingRelationship = ref(false);
const relationshipDraft = ref("");
const showGrid = ref(true);
const showThumbnails = ref(true);
try {
  showGrid.value = localStorage.getItem("studio.tree.showGrid") !== "false";
  showThumbnails.value = localStorage.getItem("studio.tree.showThumbnails") !== "false";
} catch { /* Preferences remain available for this session. */ }
watch([showGrid, showThumbnails], ([grid, thumbnails]) => {
  try {
    localStorage.setItem("studio.tree.showGrid", String(grid));
    localStorage.setItem("studio.tree.showThumbnails", String(thumbnails));
  } catch { /* Storage may be unavailable. */ }
});
function dismissMenu(event: Event) {
  if (event instanceof KeyboardEvent && event.key !== "Escape") return;
  if (event instanceof PointerEvent && (event.target as Element)?.closest(".tree-context")) return;
  menu.value = null;
}
window.addEventListener("pointerdown", dismissMenu);
window.addEventListener("keydown", dismissMenu);
onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", dismissMenu);
  window.removeEventListener("keydown", dismissMenu);
});
const markerId = `tree-arrow-${crypto.randomUUID()}`;
const owner = computed(() => treeTrait(props.entity, studioState.traitTemplates, TREE_GRAPH_KEY));
const parsed = computed<{ graph: TreeGraph | null; error: string }>(() => {
  if (!owner.value) return { graph: null, error: "" };
  try { return { graph: treeGraphOf(owner.value), error: "" }; }
  catch (error) { return { graph: null, error: error instanceof Error ? error.message : String(error) }; }
});
const graph = computed(() => parsed.value.graph);
const canvasId = `tree-canvas-${crypto.randomUUID()}`;
const customCanvasCss = computed(() => {
  const css = graph.value?.appearance?.defaults?.customCss;
  if (!css) return "";
  try {
    const sheet = new CSSStyleSheet();
    sheet.replaceSync(css);
    return `@scope (#${canvasId}) { :scope { ${Array.from(sheet.cssRules, (rule) => rule.cssText).join("\n")} } }`;
  } catch {
    return "";
  }
});
const entities = computed(() => [...studioState.entities, ...studioState.entityTemplates] as AnyEntity[]);
const byId = computed(() => new Map(entities.value.map((entity) => [entity.id, entity])));
const eligible = computed(() => entities.value.filter((entity) => !!treeTrait(entity, studioState.traitTemplates, TREE_ITEM_KEY)));
const eligibleIds = computed(() => new Set(eligible.value.map((entity) => entity.id)));
const available = computed(() => eligible.value.filter((entity) => !graph.value?.nodes.includes(entity.id)));
const matchingItems = computed(() => {
  const query = menuSearch.value.trim().toLocaleLowerCase();
  return available.value.filter((entity) => `${entity.name} ${entity.id}`.toLocaleLowerCase().includes(query));
});
const issues = computed(() => graph.value ? treeErrors(graph.value, eligibleIds.value) : []);
const roots = computed(() => new Set(graph.value ? treeRoots(graph.value) : []));
const layout = computed(() => graph.value ? layoutTree(graph.value) : null);
const zoomFactor = computed(() => zoom.value / 100);
const label = (id: string) => byId.value.get(id)?.name ?? id;
const description = (id: string) => byId.value.get(id)?.description ?? "";
const matchingNodes = computed(() => {
  const query = menuSearch.value.trim().toLocaleLowerCase();
  return (graph.value?.nodes ?? []).filter((id) => `${label(id)} ${id}`.toLocaleLowerCase().includes(query));
});
const edgeKey = (edge: TreeEdge) => JSON.stringify([edge.from, edge.to]);
const menuImage = computed(() => graph.value?.appearance?.images?.find((image) => image.id === menu.value?.image));
const nodeAppearance = (id: string) => ({ ...graph.value?.appearance?.defaults?.node, ...graph.value?.appearance?.nodes[id] });
const edgeAppearance = (edge: TreeEdge) => ({ ...graph.value?.appearance?.defaults?.edge, ...graph.value?.appearance?.edges[edgeKey(edge)] });
const nodeShape = (id: string) => graph.value?.shapes?.[id] ?? graph.value?.appearance?.defaults?.node?.shape ?? "card";
const nodeSize = (id: string) => ({ width: nodeAppearance(id).width ?? 164, height: nodeAppearance(id).height ?? 68 });
function worldPoint(event: { clientX: number; clientY: number }): TreePoint {
  const rect = treeScroll.value!.getBoundingClientRect();
  return { x: (event.clientX - rect.left - panX.value) / zoomFactor.value, y: (event.clientY - rect.top - panY.value) / zoomFactor.value };
}
function updateNodeAppearance(id: string, update: Record<string, string | number | boolean>) {
  if (!graph.value || props.readonly) return;
  const appearance = graph.value.appearance ?? { nodes: {}, edges: {} };
  if (appearance.nodes[id]?.locked && ("width" in update || "height" in update)) return;
  const positions = ("width" in update || "height" in update || update.locked === true) && layout.value
    ? { ...graph.value.positions, [graph.value.direction]: { ...layout.value.positions } }
    : graph.value.positions;
  patch({ ...graph.value, positions, appearance: { ...appearance, nodes: { ...appearance.nodes, [id]: { ...appearance.nodes[id], ...update } } } });
}
function resetNodeAppearance(id: string) {
  const current = graph.value;
  if (!current || props.readonly) return;
  const shapes = { ...current.shapes };
  const appearance = current.appearance && { ...current.appearance, nodes: { ...current.appearance.nodes } };
  delete shapes[id];
  if (appearance) {
    const locked = appearance.nodes[id]?.locked;
    if (locked) appearance.nodes[id] = { locked: true };
    else delete appearance.nodes[id];
  }
  const positions = { ...current.positions, [current.direction]: { ...layout.value?.positions } };
  patch({ ...current, positions, shapes, appearance });
}
function resetEdgeAppearance(index: number) {
  const current = graph.value, edge = current?.edges[index];
  if (!current?.appearance || !edge || props.readonly) return;
  const appearance = { ...current.appearance, edges: { ...current.appearance.edges } };
  delete appearance.edges[edgeKey(edge)];
  patch({ ...current, appearance });
}
function updateEdgeAppearance(index: number, update: Record<string, string | number>) {
  const edge = graph.value?.edges[index];
  if (!graph.value || !edge || props.readonly) return;
  const appearance = graph.value.appearance ?? { nodes: {}, edges: {} }, key = edgeKey(edge);
  patch({ ...graph.value, appearance: { ...appearance, edges: { ...appearance.edges, [key]: { ...appearance.edges[key], ...update } } } });
}
function setCanvasBackground(color?: string) {
  if (!graph.value || props.readonly) return;
  const appearance = { ...(graph.value.appearance ?? { nodes: {}, edges: {} }) };
  if (color === undefined) delete appearance.canvasBackground;
  else appearance.canvasBackground = color;
  patch({ ...graph.value, appearance });
}
async function fitGraph() {
  await nextTick();
  const viewport = treeScroll.value;
  const current = layout.value;
  if (!viewport || !current || !graph.value || (!graph.value.nodes.length && !graph.value.appearance?.images?.length)) return;
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
  for (const [id, point] of Object.entries(current.positions)) {
    const size = nodeSize(id);
    left = Math.min(left, point.x - 40);
    top = Math.min(top, point.y - 40);
    right = Math.max(right, point.x + size.width + 40);
    bottom = Math.max(bottom, point.y + size.height + 40);
  }
  for (const image of graph.value.appearance?.images ?? []) {
    left = Math.min(left, image.x - 40);
    top = Math.min(top, image.y - 40);
    right = Math.max(right, image.x + image.width + 40);
    bottom = Math.max(bottom, image.y + image.height + 40);
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
  if (event.altKey && !props.readonly) {
    const point = worldPoint(event);
    cutDrag.value = { pointer: event.pointerId, start: point, end: point };
    treeScroll.value?.setPointerCapture(event.pointerId);
    event.preventDefault();
    return;
  }
  if ((event.target as Element).closest(".tree-node")) return;
  pan = { pointer: event.pointerId, x: event.clientX - panX.value, y: event.clientY - panY.value };
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}
function movePan(event: PointerEvent) {
  if (cutDrag.value?.pointer === event.pointerId) { cutDrag.value.end = worldPoint(event); return; }
  if (resizeDrag.value?.pointer === event.pointerId) {
    const active = resizeDrag.value;
    const dx = Math.round((event.clientX - active.x) / zoomFactor.value), dy = Math.round((event.clientY - active.y) / zoomFactor.value);
    const west = active.direction.includes("w"), east = active.direction.includes("e");
    const north = active.direction.includes("n"), south = active.direction.includes("s");
    const width = Math.max(100, Math.min(600, active.width + (west ? -dx : east ? dx : 0)));
    const height = Math.max(60, Math.min(400, active.height + (north ? -dy : south ? dy : 0)));
    if (graph.value) {
      const appearance = graph.value.appearance ?? { nodes: {}, edges: {} };
      const positions = { ...graph.value.positions, [graph.value.direction]: { ...layout.value?.positions, [active.id]: { x: active.left + (west ? active.width - width : 0), y: active.top + (north ? active.height - height : 0) } } };
      patch({ ...graph.value, positions, appearance: { ...appearance, nodes: { ...appearance.nodes, [active.id]: { ...appearance.nodes[active.id], width, height } } } });
    }
    return;
  }
  if (linkDrag.value?.pointer === event.pointerId) { moveLink(event); return; }
  if (!pan || event.pointerId !== pan.pointer) return;
  panX.value = event.clientX - pan.x;
  panY.value = event.clientY - pan.y;
}
function endPan(event: PointerEvent) {
  if (event.type === "pointercancel") { cutDrag.value = null; resizeDrag.value = null; linkDrag.value = null; pan = null; return; }
  if (cutDrag.value?.pointer === event.pointerId) { finishCut(); return; }
  if (resizeDrag.value?.pointer === event.pointerId) { resizeDrag.value = null; return; }
  if (linkDrag.value?.pointer === event.pointerId) { finishLink(event); return; }
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
function openContext(event: MouseEvent, node?: string, edge?: number) {
  if (props.readonly || creatingExample.value || !treeScroll.value) return;
  event.preventDefault();
  event.stopPropagation();
  const rect = treeScroll.value.getBoundingClientRect();
  const anchor = event.currentTarget instanceof Element ? event.currentTarget.getBoundingClientRect() : rect;
  const x = (event.clientX || anchor.left + anchor.width / 2) - rect.left, y = (event.clientY || anchor.bottom) - rect.top;
  menuSearch.value = "";
  editingRelationship.value = false;
  menu.value = {
    x: Math.max(0, Math.min(x, rect.width - 260)),
    y: Math.max(0, Math.min(y, rect.height - Math.min(420, rect.height))),
    node,
    edge,
    point: event.currentTarget instanceof HTMLElement && event.currentTarget.closest(".tree-canvas-toolbar")
      ? { x: Math.round((rect.width / 2 - panX.value) / zoomFactor.value), y: Math.round((rect.height / 2 - panY.value) / zoomFactor.value) }
      : { x: Math.round((x - panX.value) / zoomFactor.value), y: Math.round((y - panY.value) / zoomFactor.value) },
  };
  void nextTick(() => treeScroll.value?.querySelector<HTMLElement>(".tree-context input, .tree-context select, .tree-context button")?.focus());
}
function openEdgeKeyboard(event: KeyboardEvent, index: number) {
  const bounds = (event.currentTarget as Element).getBoundingClientRect();
  openContext(new MouseEvent("contextmenu", { clientX: bounds.left + bounds.width / 2, clientY: bounds.top + bounds.height / 2 }), undefined, index);
}
function editRelationship(index: number) {
  relationshipDraft.value = graph.value?.edges[index]?.relationship ?? "";
  editingRelationship.value = true;
  void nextTick(() => treeScroll.value?.querySelector<HTMLInputElement>(".tree-relationship-name")?.focus());
}
function cancelRelationshipEdit() {
  editingRelationship.value = false;
  void nextTick(() => treeScroll.value?.querySelector<HTMLButtonElement>(".tree-relationship-toggle")?.focus());
}
function wireAtPoint(x: number, y: number): SVGElement | undefined {
  return document.elementsFromPoint(x, y).find((element): element is SVGElement =>
    element instanceof SVGElement && element.matches(".tree-edge-hit, .tree-edge-label.is-editable") && !!treeScroll.value?.contains(element));
}
function openZoneContext(id: string, event: MouseEvent) {
  const hit = wireAtPoint(event.clientX, event.clientY);
  openContext(event, hit ? undefined : id, hit ? Number(hit.dataset.index) : undefined);
}
function wireDoubleClick(event: MouseEvent) {
  const top = document.elementFromPoint(event.clientX, event.clientY);
  if (!top?.closest(".tree-edge-hit, .tree-edge-label, .tree-connect-zone")) return;
  const hit = wireAtPoint(event.clientX, event.clientY);
  if (!hit) return;
  const index = Number(hit.dataset.index);
  if (!Number.isInteger(index) || !graph.value?.edges[index] || props.readonly) return;
  openContext(event, undefined, index);
  editRelationship(index);
}
function setRelationship(index: number, name?: string) {
  const current = graph.value;
  const edge = current?.edges[index];
  if (!current || !edge || props.readonly) return;
  const relationship = name?.trim();
  if (name !== undefined && !relationship) return;
  const { relationship: _previous, ...unnamed } = edge;
  const replacement = relationship ? { ...unnamed, relationship } : unnamed;
  patch({ ...current, edges: current.edges.map((other, i) => i === index ? replacement : other) });
  editingRelationship.value = false;
  menu.value = null;
}
function addAtContext(id: string) {
  const current = graph.value, position = menu.value?.point;
  if (!current || !position || !available.value.some((entity) => entity.id === id)) return;
  const positions = { ...current.positions, [current.direction]: { ...current.positions?.[current.direction], [id]: position } };
  patch({ ...current, nodes: [...current.nodes, id], positions });
  menu.value = null;
}
function openImageContext(id: string, event: MouseEvent) {
  openContext(event);
  if (menu.value) menu.value.image = id;
}
function changeImage(id: string, update: Partial<TreeCanvasImage>) {
  const current = graph.value;
  if (!current || props.readonly) return;
  const appearance = current.appearance ?? { nodes: {}, edges: {} };
  const existing = appearance.images?.find((image) => image.id === id);
  if (!existing || (existing.locked && ("x" in update || "y" in update || "width" in update || "height" in update))) return;
  const images = (appearance.images ?? []).map((image) => image.id === id ? { ...image, ...update } : image);
  patch({ ...current, appearance: { ...appearance, images } });
}
function removeImage(id: string) {
  const current = graph.value;
  if (!current?.appearance || props.readonly) return;
  patch({ ...current, appearance: { ...current.appearance, images: (current.appearance.images ?? []).filter((image) => image.id !== id) } });
  menu.value = null;
}
function chooseImage() {
  if (props.readonly || !menu.value) return;
  imagePoint = { ...menu.value.point };
  menu.value = null;
  imageInput.value?.click();
}
async function addImage(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0], point = imagePoint, entityId = props.entity.id;
  input.value = "";
  imagePoint = null;
  if (!file || !point || props.readonly) return;
  const url = URL.createObjectURL(file);
  try {
    const image = new Image();
    image.src = url;
    await image.decode();
    if (!image.naturalWidth || !image.naturalHeight) throw new Error("The selected image has no dimensions.");
    const uploaded = await StudioApi.uploadThumbnail(file);
    if (!mounted || props.entity.id !== entityId || !graph.value || props.readonly) return;
    const scale = Math.min(1, 320 / Math.max(image.naturalWidth, image.naturalHeight));
    const appearance = graph.value.appearance ?? { nodes: {}, edges: {} };
    const width = Math.max(24, Math.round(image.naturalWidth * scale)), height = Math.max(24, Math.round(image.naturalHeight * scale));
    const entry: TreeCanvasImage = { id: crypto.randomUUID(), src: uploaded.path, x: Math.round(point.x - width / 2), y: Math.round(point.y - height / 2), width, height, locked: false };
    patch({ ...graph.value, appearance: { ...appearance, images: [...(appearance.images ?? []), entry] } });
  } catch (error) {
    notice.value = `Could not add image: ${error instanceof Error ? error.message : String(error)}`;
  } finally {
    URL.revokeObjectURL(url);
  }
}

function patch(next: TreeGraph) {
  if (!owner.value || props.readonly) return;
  emit("patch", setTreeGraph(props.entity, owner.value, next));
  notice.value = "";
  status.value = "";
}
function removeItem(id: string) {
  if (!graph.value) return;
  const positions: TreePositions = {};
  for (const [view, points] of Object.entries(graph.value.positions ?? {})) {
    positions[view as TreeDirection] = Object.fromEntries(Object.entries(points).filter(([node]) => node !== id));
  }
  const shapes = graph.value.shapes ? Object.fromEntries(Object.entries(graph.value.shapes).filter(([node]) => node !== id)) : undefined;
  const edges = graph.value.edges.filter((edge) => edge.from !== id && edge.to !== id);
  const appearance = graph.value.appearance && {
    ...graph.value.appearance,
    nodes: Object.fromEntries(Object.entries(graph.value.appearance.nodes).filter(([node]) => node !== id)),
    edges: Object.fromEntries(edges.map((edge) => [edgeKey(edge), graph.value!.appearance!.edges[edgeKey(edge)]]).filter(([, value]) => value !== undefined)),
  };
  patch({ ...graph.value, nodes: graph.value.nodes.filter((node) => node !== id), edges, positions, shapes, appearance });
  if (source.value === id) source.value = "";
}
function setShape(id: string, event: Event) {
  if (!graph.value || props.readonly) return;
  const shape = (event.target as HTMLSelectElement).value as TreeShape;
  if (!TREE_SHAPES.includes(shape)) return;
  patch({ ...graph.value, shapes: { ...graph.value.shapes, [id]: shape } });
}
function connect(from: string, to: string) {
  if (!graph.value || props.readonly) return;
  if (!graph.value.nodes.includes(from) || !graph.value.nodes.includes(to)) { notice.value = "Choose both connection endpoints."; return; }
  if (graph.value.edges.some((edge) => edge.from === from && edge.to === to)) { notice.value = "That connection already exists."; return; }
  const positions = { ...graph.value.positions, [graph.value.direction]: { ...layout.value?.positions } };
  patch({ ...graph.value, positions, edges: [...graph.value.edges, { from, to }] });
  source.value = "";
}
function beginLink(id: string, event: PointerEvent) {
  if (event.altKey) { startPan(event); return; }
  if (event.button !== 0 || props.readonly || creatingExample.value) return;
  const hit = wireAtPoint(event.clientX, event.clientY);
  if (hit) { wireDown(Number(hit.dataset.index), event); return; }
  source.value = id;
  linkDrag.value = { from: id, pointer: event.pointerId, startX: event.clientX, startY: event.clientY, x: event.clientX, y: event.clientY, moved: false };
  treeScroll.value?.setPointerCapture(event.pointerId);
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
  if (node?.dataset.treeId) {
    if (active.edge === undefined) connect(active.from, node.dataset.treeId);
    else reconnectEdge(active.edge, node.dataset.treeId);
  } else if (active.edge !== undefined) {
    openContext(event);
    if (menu.value) menu.value.reconnect = active.edge;
  }
}
const pendingLink = computed(() => {
  const active = linkDrag.value, viewport = treeScroll.value;
  if (!active?.moved || !viewport) return null;
  const point = layout.value?.positions[active.from];
  if (!point) return null;
  const rect = viewport.getBoundingClientRect();
  const size = nodeSize(active.from);
  return `M ${point.x + size.width / 2} ${point.y + size.height / 2} L ${(active.x - rect.left - panX.value) / zoomFactor.value} ${(active.y - rect.top - panY.value) / zoomFactor.value}`;
});
function removeEdges(indices: ReadonlySet<number>) {
  if (!graph.value || props.readonly) return;
  const edges = graph.value.edges.filter((_, index) => !indices.has(index));
  const appearance = graph.value.appearance && { ...graph.value.appearance, edges: Object.fromEntries(edges.map((edge) => [edgeKey(edge), graph.value!.appearance!.edges[edgeKey(edge)]]).filter(([, value]) => value !== undefined)) };
  const positions = { ...graph.value.positions, [graph.value.direction]: { ...layout.value?.positions } };
  patch({ ...graph.value, positions, edges, appearance });
}
function removeEdge(index: number) { removeEdges(new Set([index])); }
function reconnectEdge(index: number, to: string) {
  const current = graph.value, edge = current?.edges[index];
  if (!current || !edge || !current.nodes.includes(to) || props.readonly) return;
  if (current.edges.some((other, i) => i !== index && other.from === edge.from && other.to === to)) { notice.value = "That connection already exists."; return; }
  const replacement = { ...edge, to };
  const edges = current.edges.map((other, i) => i === index ? replacement : other);
  let appearance = current.appearance;
  if (appearance) {
    const styles = { ...appearance.edges }, previous = styles[edgeKey(edge)];
    delete styles[edgeKey(edge)];
    if (previous) styles[edgeKey(replacement)] = previous;
    appearance = { ...appearance, edges: styles };
  }
  const positions = { ...current.positions, [current.direction]: { ...layout.value?.positions } };
  patch({ ...current, positions, edges, appearance });
  menu.value = null;
}
function wireDown(index: number, event: PointerEvent) {
  if (event.altKey) { startPan(event); return; }
  const edge = graph.value?.edges[index];
  if (!edge || props.readonly || event.button !== 0) return;
  linkDrag.value = { from: edge.from, edge: index, pointer: event.pointerId, startX: event.clientX, startY: event.clientY, x: event.clientX, y: event.clientY, moved: false };
  (event.currentTarget as Element).setPointerCapture(event.pointerId);
  event.preventDefault();
  event.stopPropagation();
}
function finishCut() {
  const cut = cutDrag.value;
  cutDrag.value = null;
  if (!cut || Math.hypot(cut.end.x - cut.start.x, cut.end.y - cut.start.y) < 2) return;
  const cross = (a: TreePoint, b: TreePoint, c: TreePoint) => (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
  const hit = (a: TreePoint, b: TreePoint) =>
    Math.max(Math.min(cut.start.x, cut.end.x), Math.min(a.x, b.x)) <= Math.min(Math.max(cut.start.x, cut.end.x), Math.max(a.x, b.x)) &&
    Math.max(Math.min(cut.start.y, cut.end.y), Math.min(a.y, b.y)) <= Math.min(Math.max(cut.start.y, cut.end.y), Math.max(a.y, b.y)) &&
    cross(cut.start, cut.end, a) * cross(cut.start, cut.end, b) <= 0 && cross(a, b, cut.start) * cross(a, b, cut.end) <= 0;
  const removed = new Set<number>();
  treeScroll.value?.querySelectorAll(".tree-edge-hit").forEach((element) => {
    const path = element as SVGPathElement;
    const length = path.getTotalLength();
    let previous = path.getPointAtLength(0);
    for (let i = 1; i <= 48; i++) {
      const next = path.getPointAtLength(length * i / 48);
      if (hit(previous, next)) { removed.add(Number(path.dataset.index)); break; }
      previous = next;
    }
  });
  if (removed.size) removeEdges(removed);
}
function startResize(id: string, direction: string, event: PointerEvent) {
  if (event.altKey) { startPan(event); return; }
  if (props.readonly || creatingExample.value || nodeAppearance(id).locked || event.button !== 0) return;
  const point = layout.value?.positions[id];
  if (!point) return;
  const size = nodeSize(id);
  resizeDrag.value = { id, direction, pointer: event.pointerId, x: event.clientX, y: event.clientY, left: point.x, top: point.y, ...size };
  treeScroll.value?.setPointerCapture(event.pointerId);
  event.stopPropagation();
  event.preventDefault();
}
function changeDirection(event: Event) {
  if (!graph.value) return;
  patch({ ...graph.value, direction: (event.target as HTMLSelectElement).value as TreeDirection });
}
function moveNode(id: string, point: TreePoint) {
  if (!graph.value || props.readonly || nodeAppearance(id).locked) return;
  const direction = graph.value.direction;
  const positions = { ...graph.value.positions, [direction]: { ...graph.value.positions?.[direction], [id]: point } };
  patch({ ...graph.value, positions });
}
function startDrag(id: string, event: PointerEvent) {
  if (event.altKey && event.button === 0) { startPan(event); event.stopPropagation(); return; }
  if (props.readonly || creatingExample.value || nodeAppearance(id).locked || event.button !== 0 || (event.target as Element).closest("button")) return;
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
  if (props.readonly || nodeAppearance(id).locked || event.target !== event.currentTarget) return;
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
  const locked = Object.fromEntries(graph.value.nodes.filter((id) => nodeAppearance(id).locked && layout.value?.positions[id]).map((id) => [id, layout.value!.positions[id]!]));
  if (Object.keys(locked).length) positions[graph.value.direction] = locked;
  else delete positions[graph.value.direction];
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
async function createExample(id: string) {
  const selected = TREE_EXAMPLES.find((entry) => entry.id === id);
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
    source.value = ""; menu.value = null;
    status.value = `Created ${created.length} TreeItem entities. Save this entity to keep the example graph.`;
  } catch (error) {
    const cleanup = await Promise.allSettled(created.map((id) => StudioApi.deleteContent("entity_templates", id)));
    await refreshWorkspace(["entity_templates"]).catch(() => {});
    const remaining = cleanup.filter((result) => result.status === "rejected").length;
    notice.value = `Could not create example: ${error instanceof Error ? error.message : String(error)}.${remaining ? ` ${remaining} created entities could not be removed.` : ""}`;
  } finally { creatingExample.value = false; }
}
function edgeGeometry(edge: TreeEdge): { path: string; label: TreePoint } | null {
  const a = layout.value?.positions[edge.from];
  const b = layout.value?.positions[edge.to];
  if (!a || !b) return null;
  const sizeA = nodeSize(edge.from), sizeB = nodeSize(edge.to);
  if (edge.from === edge.to) {
    const halfWidth = sizeA.width / 2, halfHeight = sizeA.height / 2, offset = Math.min(26, halfWidth / 2);
    const inset = nodeShape(edge.from) === "diamond" ? halfHeight * offset / halfWidth : nodeShape(edge.from) === "ellipse" ? halfHeight * (1 - Math.sqrt(1 - (offset / halfWidth) ** 2)) : 0;
    return {
      path: `M ${a.x + halfWidth + offset} ${a.y + inset} C ${a.x + halfWidth + offset + 24} ${a.y - 38}, ${a.x + halfWidth - offset - 24} ${a.y - 38}, ${a.x + halfWidth - offset} ${a.y + inset}`,
      label: { x: a.x + halfWidth, y: (a.y + inset) * .25 + (a.y - 38) * .75 },
    };
  }
  const ax = a.x + sizeA.width / 2, ay = a.y + sizeA.height / 2, bx = b.x + sizeB.width / 2, by = b.y + sizeB.height / 2;
  const dx = bx - ax, dy = by - ay;
  if (dx === 0 && dy === 0) return null;
  const clip = (id: string, size: { width: number; height: number }) => {
    const x = size.width / 2, y = size.height / 2, shape = nodeShape(id);
    if (shape === "diamond") return 1 / (Math.abs(dx) / x + Math.abs(dy) / y);
    if (shape === "ellipse") return 1 / Math.hypot(dx / x, dy / y);
    return Math.min(dx === 0 ? Infinity : x / Math.abs(dx), dy === 0 ? Infinity : y / Math.abs(dy));
  };
  const fromClip = clip(edge.from, sizeA), toClip = clip(edge.to, sizeB);
  const x1 = ax + dx * fromClip, y1 = ay + dy * fromClip, x2 = bx - dx * toClip, y2 = by - dy * toClip;
  const reciprocal = graph.value?.edges.some((other) => other.from === edge.to && other.to === edge.from);
  const midpoint = { x: (x1 + x2) / 2, y: (y1 + y2) / 2 };
  if (!reciprocal) return { path: `M ${x1} ${y1} L ${x2} ${y2}`, label: midpoint };
  const length = Math.hypot(dx, dy);
  const cx = midpoint.x - dy / length * 32, cy = midpoint.y + dx / length * 32;
  return { path: `M ${x1} ${y1} Q ${cx} ${cy} ${x2} ${y2}`, label: { x: (midpoint.x + cx) / 2, y: (midpoint.y + cy) / 2 } };
}
function edgePath(edge: TreeEdge): string | null { return edgeGeometry(edge)?.path ?? null; }
</script>

<template>
  <section class="tree-panel" aria-label="Tree graph">
    <header class="tree-header">
      <div><h2>Tree graph</h2><p class="field-sublabel">Drag outside a node to connect; drag its edge or corner to resize. Double-click a wire to name its relationship. Alt-drag across wires to cut. Right-click for options.</p></div>
    </header>
    <p v-if="parsed.error" class="tree-error" role="alert">Tree data cannot be read: {{ parsed.error }}</p>
    <template v-if="graph && layout">
      <p v-if="notice || issues.length" class="tree-error" role="alert">{{ notice || issues.join(' ') }}</p>
      <p v-if="status" class="field-sublabel" role="status">{{ status }}</p>
      <p v-if="source && !readonly" class="tree-link-hint" role="status">Connecting from {{ label(source) }}. Open another node's options to connect. <button type="button" @click="source = ''">Cancel</button></p>
      <div class="tree-meta"><span>{{ graph.nodes.length }} items</span><span>{{ graph.edges.length }} connections</span><span>{{ roots.size }} roots</span></div>
      <div :id="canvasId" ref="treeScroll" class="tree-scroll" :class="{ 'hide-grid': !showGrid }" :style="{ backgroundColor: graph.appearance?.canvasBackground, backgroundPosition: `${panX}px ${panY}px`, backgroundSize: `${24 * zoomFactor}px ${24 * zoomFactor}px` }" tabindex="0" aria-label="Tree graph canvas; drag background to pan, use mouse wheel to zoom" @contextmenu="openContext($event)" @dblclick="wireDoubleClick" @pointerdown="startPan" @pointermove="movePan" @pointerup="endPan" @pointercancel="endPan" @wheel.prevent="wheelZoom" @keydown="keyboardPan">
        <component :is="'style'" v-if="customCanvasCss" v-text="customCanvasCss" />
        <input ref="imageInput" type="file" accept=".png,.jpg,.jpeg,image/png,image/jpeg" hidden @change="addImage" />
        <strong class="tree-canvas-title" :title="entity.name">{{ entity.name }}</strong>
        <div class="tree-canvas-toolbar" @pointerdown.stop @wheel.stop>
          <button v-if="!readonly" type="button" class="btn btn-secondary" @click.stop="openContext($event)">Canvas actions</button>
          <label class="tree-direction">View
            <select class="form-select" aria-label="Graph direction" :value="graph.direction" :disabled="readonly || creatingExample" @change="changeDirection">
              <option value="right">Left to right</option><option value="left">Right to left</option>
              <option value="down">Top to bottom</option><option value="up">Bottom to top</option><option value="radial">Radial</option>
            </select>
          </label>
          <button type="button" class="btn btn-secondary" :disabled="!graph.nodes.length && !graph.appearance?.images?.length" @click="fitGraph">Fit graph</button>
          <button v-if="!readonly" type="button" class="btn btn-primary" :disabled="creatingExample || !!issues.length" title="Saves all entity changes" @click="emit('save')">Save layout</button>
          <details class="tree-options">
            <summary>View options</summary>
            <div class="tree-options-panel">
            <div class="tree-settings">
              <label class="zoom-label">Zoom <output>{{ zoom }}%</output><input v-model.number="zoom" type="range" min="10" max="200" step="1" aria-label="Graph zoom" /></label>
              <label><input v-model="showGrid" type="checkbox" /> Show grid</label>
              <label><input v-model="showThumbnails" type="checkbox" /> Show thumbnails</label>
              <button v-if="!readonly" type="button" class="btn btn-secondary" :disabled="creatingExample" @click="resetLayout">Reset view layout</button>
              <button type="button" class="btn btn-secondary" :disabled="!graph.nodes.length && !graph.appearance?.images?.length" @click="exportJson">Export JSON</button>
            </div>
            <p class="field-sublabel">Grid and thumbnail preferences are saved on this device. Saving layout also saves other entity changes.</p>
            </div>
          </details>
        </div>
        <div v-if="!graph.nodes.length && !graph.appearance?.images?.length" class="tree-empty"><strong>Start with an item or image</strong><span>Right-click the canvas or use Canvas actions to add content.</span></div>
        <div class="tree-world" :style="{ transform: `translate(${panX}px, ${panY}px) scale(${zoomFactor})` }">
            <TreeImageLayer :images="graph.appearance?.images ?? []" :readonly="readonly" :zoom="zoomFactor" @change="changeImage" @context="openImageContext" />
            <svg class="tree-edges" aria-label="Tree connections">
              <defs><marker :id="markerId" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="10" markerHeight="10" markerUnits="userSpaceOnUse" orient="auto"><path d="M 0 0 L 8 4 L 0 8 z" /></marker></defs>
              <template v-for="(edge, index) in graph.edges" :key="`${edge.from}-${edge.to}-${index}`">
                <path v-if="edgePath(edge) && edgeAppearance(edge).pattern === 'brush'" :d="edgePath(edge) ?? ''" class="tree-edge" :style="{ stroke: edgeAppearance(edge).color, strokeWidth: Math.max(1, (edgeAppearance(edge).width ?? 2) * .45) }" />
                <path v-if="edgePath(edge)" :d="edgePath(edge) ?? ''" class="tree-edge" :class="`wire-${edgeAppearance(edge).pattern ?? 'solid'}`" :style="{ stroke: edgeAppearance(edge).color, strokeWidth: edgeAppearance(edge).width }" :marker-end="`url(#${markerId})`" />
                <text v-if="edge.relationship && edgeGeometry(edge)" class="tree-edge-label" :class="{ 'is-editable': !readonly }" :data-index="index" :x="edgeGeometry(edge)!.label.x" :y="edgeGeometry(edge)!.label.y" dy="-6" @pointerdown.stop="wireDown(index, $event)" @contextmenu.stop.prevent="openContext($event, undefined, index)">{{ edge.relationship }}</text>
                <path v-if="edgePath(edge) && !readonly" :d="edgePath(edge) ?? ''" class="tree-edge-hit" :data-index="index" :aria-label="`Connection from ${label(edge.from)} to ${label(edge.to)}${edge.relationship ? `, ${edge.relationship}` : ''}; press Enter for options`" role="button" tabindex="0" @pointerdown.stop="wireDown(index, $event)" @contextmenu.stop.prevent="openContext($event, undefined, index)" @keydown.enter.prevent.stop="openEdgeKeyboard($event, index)" />
              </template>
              <path v-if="pendingLink" :d="pendingLink" class="tree-edge tree-edge-pending" />
              <path v-if="cutDrag" :d="`M ${cutDrag.start.x} ${cutDrag.start.y} L ${cutDrag.end.x} ${cutDrag.end.y}`" class="tree-cut-line" />
            </svg>
            <article v-for="id in graph.nodes" :key="id" class="tree-node" :data-tree-id="id" :class="[`shape-${nodeShape(id)}`, { 'is-movable': !readonly && !nodeAppearance(id).locked, 'is-locked': nodeAppearance(id).locked }]" :style="{ left: `${layout.positions[id]?.x ?? 0}px`, top: `${layout.positions[id]?.y ?? 0}px`, width: `${nodeSize(id).width}px`, height: `${nodeSize(id).height}px`, '--node-bg': nodeAppearance(id).background, '--node-outline': nodeAppearance(id).outline, '--node-outline-width': `${nodeAppearance(id).outlineWidth ?? 1}px` }" :tabindex="readonly ? undefined : 0" :aria-label="`${label(id)}${roots.has(id) ? ', root' : ''}${nodeAppearance(id).locked ? ', locked' : '. Use arrow keys to move'}.`" @contextmenu.stop="openContext($event, id)" @pointerdown="startDrag(id, $event)" @pointermove="dragNode" @pointerup="endDrag" @pointercancel="endDrag" @keydown="keyboardMove(id, $event)">
              <template v-if="!readonly"><span v-for="side in ['top', 'right', 'bottom', 'left']" :key="side" class="tree-connect-zone" :class="`zone-${side}`" title="Drag to connect" @pointerdown.stop="beginLink(id, $event)" @contextmenu.stop.prevent="openZoneContext(id, $event)" /></template>
              <span class="tree-node-art" aria-hidden="true"><img v-if="showThumbnails && thumbnailUrl(byId.get(id)?.thumbnail)" :src="thumbnailUrl(byId.get(id)?.thumbnail) ?? ''" width="164" height="68" alt="" /></span>
              <span class="tree-node-copy"><strong :title="label(id)">{{ label(id) }}</strong><small :title="description(id)">{{ roots.has(id) ? 'Root' : 'Tree item' }}{{ nodeAppearance(id).locked ? ' · Locked' : '' }}</small></span>
              <button v-if="!readonly" type="button" class="tree-remove" :disabled="creatingExample" :aria-label="`Options for ${label(id)}`" @click.stop="openContext($event, id)">⋯</button>
              <template v-if="!readonly && !nodeAppearance(id).locked"><span v-for="direction in ['n','ne','e','se','s','sw','w','nw']" :key="direction" class="tree-resize" :class="`resize-${direction}`" :title="`Resize ${label(id)}`" aria-hidden="true" @pointerdown.stop="startResize(id, direction, $event)" /></template>
            </article>
          </div>
        <div v-if="menu" class="tree-context" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }" @pointerdown.stop @wheel.stop @keydown.esc.stop="menu = null">
          <template v-if="menu.node">
            <strong>{{ label(menu.node) }}</strong>
            <button type="button" @click="updateNodeAppearance(menu.node, { locked: !nodeAppearance(menu.node).locked })">{{ nodeAppearance(menu.node).locked ? 'Unlock node' : 'Lock node' }}</button>
            <label :for="`shape-${menu.node}`">Node shape</label>
            <select :id="`shape-${menu.node}`" class="form-select" aria-label="Node shape" :value="nodeShape(menu.node)" @change="setShape(menu.node, $event)">
              <option value="card">Card</option><option value="rounded">Rounded</option><option value="ellipse">Ellipse</option><option value="diamond">Diamond</option>
            </select>
            <label>Background <ColorWheelPicker label="Node background" :model-value="nodeAppearance(menu.node).background ?? '#1a1c20'" @update:model-value="updateNodeAppearance(menu.node, { background: $event })" /></label>
            <label>Outline <ColorWheelPicker label="Node outline" :model-value="nodeAppearance(menu.node).outline ?? '#06b6d4'" @update:model-value="updateNodeAppearance(menu.node, { outline: $event })" /></label>
            <label>Outline size <input type="number" min="1" max="12" :value="nodeAppearance(menu.node).outlineWidth ?? 1" @change="updateNodeAppearance(menu.node, { outlineWidth: Math.max(1, Math.min(12, Number(($event.target as HTMLInputElement).value) || 1)) })" /></label>
            <label>Width <input type="number" min="100" max="600" :disabled="nodeAppearance(menu.node).locked" :value="nodeSize(menu.node).width" @change="updateNodeAppearance(menu.node, { width: Math.max(100, Math.min(600, Number(($event.target as HTMLInputElement).value) || 100)) })" /></label>
            <label>Height <input type="number" min="60" max="400" :disabled="nodeAppearance(menu.node).locked" :value="nodeSize(menu.node).height" @change="updateNodeAppearance(menu.node, { height: Math.max(60, Math.min(400, Number(($event.target as HTMLInputElement).value) || 60)) })" /></label>
            <button v-if="graph.shapes?.[menu.node] || graph.appearance?.nodes[menu.node]" type="button" :disabled="nodeAppearance(menu.node).locked" @click="resetNodeAppearance(menu.node)">Use tree defaults</button>
            <button v-if="source && source !== menu.node" type="button" @click="connect(source, menu.node); menu = null">Connect from {{ label(source) }}</button>
            <button type="button" @click="source = menu.node; menu = null">Start connection</button>
            <button type="button" class="tree-context-danger" @click="removeItem(menu.node); menu = null">Remove from tree</button>
          </template>
          <template v-else-if="menuImage">
            <strong>Image{{ menuImage.locked ? ' (locked)' : '' }}</strong>
            <button type="button" @click="changeImage(menuImage.id, { locked: !menuImage.locked }); menu = null">{{ menuImage.locked ? 'Unlock image' : 'Lock image' }}</button>
            <button type="button" class="tree-context-danger" @click="removeImage(menuImage.id)">Remove image</button>
          </template>
          <template v-else-if="menu.edge !== undefined && graph.edges[menu.edge]">
            <strong>{{ label(graph.edges[menu.edge]!.from) }} → {{ label(graph.edges[menu.edge]!.to) }}</strong>
            <template v-if="editingRelationship">
              <label for="tree-relationship-name">Relationship name</label>
              <input id="tree-relationship-name" v-model="relationshipDraft" class="form-input tree-relationship-name" aria-label="Relationship name" placeholder="Relationship name" @keydown.enter.prevent.stop="setRelationship(menu.edge, relationshipDraft)" />
              <button type="button" :disabled="!relationshipDraft.trim()" @click="setRelationship(menu.edge, relationshipDraft)">Save relationship</button>
              <button type="button" @click="cancelRelationshipEdit">Cancel</button>
            </template>
            <button v-else type="button" class="tree-relationship-toggle" @click="editRelationship(menu.edge)">{{ graph.edges[menu.edge]!.relationship ? 'Edit relationship' : 'Add relationship' }}</button>
            <button v-if="graph.edges[menu.edge]!.relationship" type="button" @click="setRelationship(menu.edge)">Remove relationship</button>
            <label>Wire color <ColorWheelPicker label="Wire color" :model-value="edgeAppearance(graph.edges[menu.edge]!).color ?? '#06b6d4'" @update:model-value="updateEdgeAppearance(menu.edge, { color: $event })" /></label>
            <label>Wire thickness <input type="number" min="1" max="12" :value="edgeAppearance(graph.edges[menu.edge]!).width ?? 2" @change="updateEdgeAppearance(menu.edge, { width: Math.max(1, Math.min(12, Number(($event.target as HTMLInputElement).value) || 1)) })" /></label>
            <span class="tree-pattern-label">Wire style</span>
            <div class="tree-patterns" role="group" aria-label="Wire style">
              <button v-for="option in wireOptions" :key="option.id" type="button" :title="option.label" :aria-label="option.label" :aria-pressed="(edgeAppearance(graph.edges[menu.edge]!).pattern ?? 'solid') === option.id" @click="updateEdgeAppearance(menu.edge, { pattern: option.id })">
                <svg viewBox="0 0 42 16" aria-hidden="true" width="42" height="16">
                  <template v-if="option.id === 'brush'">
                    <path d="M 5 11 L 12 4 M 4 12 C 3 12 3 14 2 14 C 5 14 7 14 7 11" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" />
                    <path d="M 19 8 H 39" fill="none" stroke="currentColor" stroke-width="1" />
                    <path d="M 19 8 H 39" class="wire-brush" fill="none" stroke="currentColor" stroke-width="2" />
                  </template>
                  <path v-else d="M 3 8 H 39" :class="`wire-${option.id}`" fill="none" stroke="currentColor" stroke-width="2" />
                </svg>
              </button>
            </div>
            <button v-if="graph.appearance?.edges[edgeKey(graph.edges[menu.edge]!)]" type="button" @click="resetEdgeAppearance(menu.edge)">Use tree defaults</button>
            <button type="button" @click="menu.reconnect = menu.edge; menu.edge = undefined; menuSearch = ''">Reconnect target</button>
            <button type="button" class="tree-context-danger" @click="removeEdge(menu.edge); menu = null">Remove wire</button>
          </template>
          <template v-else-if="menu.reconnect !== undefined">
            <strong>Choose connection target</strong>
            <input v-model="menuSearch" class="form-input" aria-label="Search connection targets" placeholder="Search nodes…" />
            <div class="tree-context-results">
              <button v-for="id in matchingNodes" :key="id" type="button" @click="reconnectEdge(menu.reconnect, id)">{{ label(id) }}</button>
              <p v-if="!matchingNodes.length" class="field-sublabel">No matching nodes.</p>
            </div>
          </template>
          <template v-else>
            <label>Canvas background <ColorWheelPicker label="Canvas background" :model-value="graph.appearance?.canvasBackground ?? '#1a1c20'" @update:model-value="setCanvasBackground($event)" /></label>
            <button v-if="graph.appearance?.canvasBackground" type="button" @click="setCanvasBackground()">Reset background</button>
            <button type="button" @click="chooseImage">Add image…</button>
            <button v-for="(image, index) in graph.appearance?.images ?? []" :key="image.id" type="button" @click="menu.image = image.id">Image {{ index + 1 }}{{ image.locked ? ' (locked)' : '' }} options</button>
            <label for="tree-context-search">Add TreeItem</label>
            <input id="tree-context-search" v-model="menuSearch" class="form-input" aria-label="Search TreeItem entities" placeholder="Search items…" />
            <div class="tree-context-results">
              <button v-for="candidate in matchingItems" :key="candidate.id" type="button" @click="addAtContext(candidate.id)">{{ candidate.name }} <small>{{ candidate.id }}</small></button>
              <p v-if="!matchingItems.length" class="field-sublabel">{{ available.length ? 'No matching items.' : 'No available TreeItem entities.' }}</p>
            </div>
            <strong>Examples</strong>
            <button v-for="entry in TREE_EXAMPLES" :key="entry.id" type="button" @click="createExample(entry.id)">{{ entry.name }}</button>
          </template>
        </div>
      </div>
    </template>
  </section>
</template>

<style scoped>
.tree-panel { display: grid; gap: 12px; min-width: 0; }
.tree-header, .tree-meta { display: flex; align-items: end; gap: 12px; flex-wrap: wrap; }
.tree-header { justify-content: space-between; }
.tree-header h2 { margin: 0 0 4px; }
.tree-header p { margin: 0; }
.zoom-label { grid-template-columns: auto auto; }
.zoom-label input { grid-column: 1 / -1; width: 110px; }
.tree-options { position: relative; margin-left: auto; color: var(--text-primary); font-size: 13px; }
.tree-options summary { display: flex; align-items: center; box-sizing: border-box; min-height: 36px; cursor: pointer; border: 1px solid var(--border-subtle); background: var(--bg-card); border-radius: 5px; padding: 6px 12px; }
.tree-options summary::before { content: "▸"; margin-right: 5px; }
.tree-options[open] summary::before { content: "▾"; }
.tree-options-panel { position: absolute; top: calc(100% + 4px); right: 0; width: min(240px, calc(100vw - 48px)); max-height: 290px; overflow: auto; box-sizing: border-box; padding: 10px; background: var(--bg-card); border: 1px solid var(--border-subtle); box-shadow: 0 8px 24px rgb(0 0 0 / 35%); }
.tree-settings { display: grid; gap: 8px; padding: 8px 0; }
.tree-settings label:not(.zoom-label) { display: flex; align-items: center; min-height: 32px; gap: 8px; }
.tree-settings .zoom-label { display: grid; gap: 4px; }
.tree-error { color: var(--status-danger); margin: 0; }
.tree-link-hint { margin: 0; color: var(--text-muted); font-size: 13px; }
.tree-link-hint button { border: 0; background: transparent; color: var(--accent); cursor: pointer; text-decoration: underline; }
.tree-meta { color: var(--text-muted); font-size: 12px; gap: 16px; }
.tree-canvas-toolbar { position: absolute; top: 10px; left: 10px; right: 10px; z-index: 3; display: flex; align-items: center; flex-wrap: wrap; gap: 6px; pointer-events: none; }
.tree-canvas-toolbar > button, .tree-canvas-toolbar select, .tree-settings button { box-sizing: border-box; min-height: 36px; }
.tree-canvas-toolbar select { height: 36px; }
.tree-canvas-toolbar > * { pointer-events: auto; }
.tree-direction { display: flex; align-items: center; gap: 6px; color: var(--text-muted); background: var(--bg-card); font-size: 12px; }
.tree-scroll { position: relative; container-type: inline-size; overflow: hidden; height: min(65vh, 620px); min-height: 360px; border: 1px solid var(--border-subtle); background-color: var(--bg-card); background-image: radial-gradient(var(--border-subtle) 1px, transparent 1px); background-size: 24px 24px; cursor: grab; touch-action: none; }
.tree-canvas-title { position: absolute; z-index: 2; left: 16px; bottom: 12px; max-width: calc(100% - 32px); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--text-primary); font-size: clamp(20px, 4cqw, 48px); line-height: 1.15; text-shadow: 0 1px 6px var(--bg-card); pointer-events: none; }
.tree-scroll.hide-grid { background-image: none; }
.tree-context { position: absolute; z-index: 4; width: min(250px, 100%); max-height: min(420px, 100%); overflow: auto; display: grid; gap: 6px; box-sizing: border-box; padding: 10px; color: var(--text-primary); background: var(--bg-card); border: 1px solid var(--border-subtle); box-shadow: 0 8px 24px rgb(0 0 0 / 35%); cursor: default; touch-action: auto; }
.tree-context :deep(.color-trigger), .tree-context input, .tree-context select { box-sizing: border-box; min-height: 36px; }
.tree-context label, .tree-context strong { font-size: 12px; }
.tree-context-results { display: grid; gap: 2px; overflow-y: auto; }
.tree-context button { min-height: 36px; border: 0; background: transparent; color: var(--text-primary); text-align: left; padding: 8px 10px; cursor: pointer; }
.tree-context button:hover, .tree-context button:focus-visible { background: var(--bg-hover, #30343b); outline: 1px solid var(--accent); }
.tree-context button small { display: block; color: var(--text-muted); overflow-wrap: anywhere; }
.tree-context .tree-context-danger { color: var(--status-danger); }
.tree-scroll:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.tree-empty { position: absolute; inset: 0; display: grid; place-content: center; text-align: center; gap: 8px; padding: 16px; color: var(--text-muted); pointer-events: none; }
.tree-empty strong { color: var(--text-primary); }
.tree-world { position: absolute; z-index: 1; left: 0; top: 0; transform-origin: top left; }
.tree-edges { position: absolute; z-index: 1; width: 1px; height: 1px; overflow: visible; pointer-events: none; }
.tree-edges .tree-edge { fill: none; stroke: var(--accent); stroke-width: 2; }
.wire-dash { stroke-dasharray: 10 6; }
.wire-dot { stroke-dasharray: 1 6; stroke-linecap: round; }
.wire-brush { stroke-dasharray: 18 2 5 2 1 2; stroke-linecap: round; }
.tree-pattern-label { font-size: 12px; }
.tree-patterns { display: flex; gap: 4px; }
.tree-patterns button { padding: 4px; border: 1px solid var(--border-subtle); }
.tree-patterns button[aria-pressed="true"] { border-color: var(--accent); color: var(--accent); }
.tree-edges .tree-edge-hit { fill: none; stroke: transparent; stroke-width: 18; pointer-events: stroke; cursor: grab; }
.tree-edges .tree-edge-label { fill: var(--text-main); stroke: var(--bg-card); stroke-width: 4px; paint-order: stroke; font-size: 14px; font-weight: 600; text-anchor: middle; pointer-events: none; }
.tree-edges .tree-edge-label.is-editable { pointer-events: bounding-box; cursor: grab; }
.tree-edges .tree-edge-hit:focus-visible { stroke: var(--accent); stroke-opacity: .35; }
.tree-edges .tree-cut-line { fill: none; stroke: var(--status-danger); stroke-width: 2; stroke-dasharray: 5 4; pointer-events: none; }
.tree-edges .tree-edge-pending { stroke-dasharray: 5 4; }
.tree-edges marker path { fill: context-stroke; }
.tree-node { position: absolute; z-index: 2; box-sizing: border-box; width: 164px; height: 68px; display: flex; align-items: center; gap: 8px; padding: 8px; }
.tree-node-art { position: absolute; inset: 0; overflow: hidden; background: var(--node-bg, var(--bg-card)); border: var(--node-outline-width, 1px) solid var(--node-outline, var(--accent)); pointer-events: none; }
.tree-node-art img { width: 100%; height: 100%; object-fit: cover; opacity: .75; }
.tree-node-art:has(img)::after { content: ""; position: absolute; inset: 0; background: rgb(0 0 0 / 30%); }
.shape-rounded .tree-node-art { border-radius: 14px; }
.shape-ellipse .tree-node-art { border-radius: 50%; }
.shape-diamond .tree-node-art { border: 0; background: var(--node-outline, var(--accent)); clip-path: polygon(50% 0, 100% 50%, 50% 100%, 0 50%); }
.shape-diamond .tree-node-art::before { content: ""; position: absolute; inset: var(--node-outline-width, 1px); background: var(--node-bg, var(--bg-card)); clip-path: polygon(50% 0, 100% 50%, 50% 100%, 0 50%); }
.shape-diamond .tree-node-art img { position: absolute; inset: var(--node-outline-width, 1px); width: calc(100% - 2 * var(--node-outline-width, 1px)); height: calc(100% - 2 * var(--node-outline-width, 1px)); clip-path: polygon(50% 0, 100% 50%, 50% 100%, 0 50%); }
.shape-diamond .tree-node-copy { padding-left: 27px; }
.tree-connect-zone { position: absolute; z-index: 2; cursor: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='24' height='24' viewBox='0 0 24 24'%3E%3Cpath d='M9 15l6-6M7 11l-2 2a4 4 0 005.7 5.7l2-2M17 13l2-2a4 4 0 00-5.7-5.7l-2 2' fill='none' stroke='%2306b6d4' stroke-width='2.5'/%3E%3C/svg%3E") 12 12, crosshair; }
.zone-top, .zone-bottom { left: 0; right: 0; height: 14px; }
.zone-left, .zone-right { top: 0; bottom: 0; width: 14px; }
.zone-top { top: -14px; } .zone-bottom { bottom: -14px; }
.zone-left { left: -14px; } .zone-right { right: -14px; }
.tree-node:has(.tree-connect-zone:hover)::after { content: ""; position: absolute; inset: -14px; border: 1px solid var(--accent); pointer-events: none; }
.tree-resize { position: absolute; z-index: 3; }
.resize-n, .resize-s { left: 10px; right: 10px; height: 6px; cursor: ns-resize; }
.resize-e, .resize-w { top: 10px; bottom: 10px; width: 6px; cursor: ew-resize; }
.resize-n { top: 0; } .resize-s { bottom: 0; } .resize-e { right: 0; } .resize-w { left: 0; }
.resize-ne, .resize-se, .resize-sw, .resize-nw { width: 10px; height: 10px; }
.resize-ne { top: 0; right: 0; cursor: nesw-resize; } .resize-sw { bottom: 0; left: 0; cursor: nesw-resize; }
.resize-nw { top: 0; left: 0; cursor: nwse-resize; } .resize-se { bottom: 0; right: 0; cursor: nwse-resize; }
.tree-node.is-movable { cursor: grab; touch-action: none; }
.tree-node.is-movable:active { cursor: grabbing; }
.tree-node:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.tree-node-copy { position: relative; min-width: 0; flex: 1; display: grid; gap: 3px; }
.tree-node-copy strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
.tree-node-copy small { color: var(--text-muted); font-size: 10px; }
.tree-remove { position: relative; border: 0; background: transparent; color: var(--text-primary); cursor: pointer; padding: 4px; }
.tree-remove:hover, .tree-remove:focus-visible { color: var(--status-danger); }
.tree-context label { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.tree-context input[type=number] { width: 70px; padding: 6px 8px; color: var(--text-primary); background: var(--bg-input); border: 1px solid var(--border-subtle); border-radius: 4px; }
</style>
