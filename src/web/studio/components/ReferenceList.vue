<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import type { ContentReference } from "../contracts.ts";

const props = withDefaults(
  defineProps<{
    references: ContentReference[];
    loading?: boolean;
    targetName?: string;
  }>(),
  {
    targetName: "Current Record",
  },
);

const router = useRouter();

const GROUPS: Array<{ kind: string; label: string; color: string }> = [
  { kind: "entity_template", label: "Entity Templates", color: "#3b82f6" },
  { kind: "storylet", label: "Storylets", color: "#10b981" },
  { kind: "trait_template", label: "Traits", color: "#a855f7" },
];

const viewMode = ref<"split" | "graph" | "list">("split");

// Category filters
const activeCategories = ref<Set<string>>(new Set(GROUPS.map((g) => g.kind)));

// Sync active categories when references change
watch(
  () => props.references,
  () => {
    activeCategories.value = new Set(GROUPS.map((g) => g.kind));
  },
  { immediate: true },
);

const allCategoriesSelected = computed({
  get: () => GROUPS.every((g) => activeCategories.value.has(g.kind)),
  set: (val: boolean) => {
    if (val) {
      activeCategories.value = new Set(GROUPS.map((g) => g.kind));
    } else {
      activeCategories.value.clear();
    }
  },
});

function toggleCategory(kind: string) {
  if (activeCategories.value.has(kind)) {
    activeCategories.value.delete(kind);
  } else {
    activeCategories.value.add(kind);
  }
}

const categoryCounts = computed(() => {
  const counts: Record<string, number> = {};
  for (const g of GROUPS) counts[g.kind] = 0;
  for (const ref of props.references) {
    counts[ref.source_kind] = (counts[ref.source_kind] || 0) + 1;
  }
  return counts;
});

const activeGroupsWithCount = computed(() =>
  GROUPS.filter((g) => (categoryCounts.value[g.kind] || 0) > 0),
);

const grouped = computed(() =>
  GROUPS.map((group) => ({
    ...group,
    rows: props.references.filter((reference) => reference.source_kind === group.kind),
  })).filter((group) => group.rows.length > 0),
);

const unknown = computed(() =>
  props.references.filter((reference) => !GROUPS.some((group) => group.kind === reference.source_kind)),
);

function viaLabel(reference: ContentReference): string {
  if (reference.via === "role") return "by role";
  if (reference.via === "tag") return "by tag";
  return "direct";
}

function open(reference: ContentReference) {
  if (reference.source_kind === "storylet") {
    router.push({
      path: `/storylets/${encodeURIComponent(reference.source_id)}/edit`,
      query: reference.node_id ? { node: reference.node_id } : {},
    });
  } else if (reference.source_kind === "entity_template") {
    router.push(`/entity-templates/${encodeURIComponent(reference.source_id)}`);
  } else if (reference.source_kind === "trait_template") {
    router.push(`/trait-templates/${encodeURIComponent(reference.source_id)}`);
}
}

// Graph node representations & layout
interface GraphNode {
  id: string;
  label: string;
  subLabel: string;
  kind: string;
  color: string;
  x: number;
  y: number;
  reference?: ContentReference;
  isCenter?: boolean;
}

interface GraphEdge {
  id: string;
  source: GraphNode;
  target: GraphNode;
  field: string;
  via: string;
  color: string;
}

const hoveredNodeId = ref<string | null>(null);
const zoom = ref(1.0);
const panX = ref(0);
const panY = ref(0);
const isPanning = ref(false);
const startPan = ref({ x: 0, y: 0 });

const draggedNodeId = ref<string | null>(null);
const canvasWrapRef = ref<HTMLDivElement | null>(null);
const nodeDragOffset = ref({ x: 0, y: 0 });
const nodePositions = ref<Map<string, { x: number; y: number }>>(new Map());

const graphData = computed(() => {
  const visibleRefs = props.references.filter((r) => activeCategories.value.has(r.source_kind));

  const centerNode: GraphNode = {
    id: "__center__",
    label: props.targetName,
    subLabel: "Target Record",
    kind: "center",
    color: "#f59e0b",
    x: nodePositions.value.get("__center__")?.x ?? 320,
    y: nodePositions.value.get("__center__")?.y ?? 260,
    isCenter: true,
  };

  const nodes: GraphNode[] = [centerNode];
  const edges: GraphEdge[] = [];

  const count = visibleRefs.length;
  const radius = Math.min(220, Math.max(140, count * 18));

  visibleRefs.forEach((ref, index) => {
    const refIndex = props.references.indexOf(ref);
    const nodeId = `${ref.source_kind}:${ref.source_id}:${ref.node_id ?? ""}:${ref.field}:${refIndex}`;
    const groupDef = GROUPS.find((g) => g.kind === ref.source_kind);
    const color = groupDef?.color ?? "#94a3b8";

    // Radial layout with gentle spiral offset
    const angle = (index / Math.max(1, count)) * 2 * Math.PI - Math.PI / 2;
    const dist = radius + (index % 2 === 0 ? 0 : 35);
    const defaultX = centerNode.x + Math.cos(angle) * dist;
    const defaultY = centerNode.y + Math.sin(angle) * dist;

    const savedPos = nodePositions.value.get(nodeId);
    const node: GraphNode = {
      id: nodeId,
      label: ref.node_label || ref.source_label,
      subLabel: ref.source_kind,
      kind: ref.source_kind,
      color,
      x: savedPos?.x ?? defaultX,
      y: savedPos?.y ?? defaultY,
      reference: ref,
    };

    nodes.push(node);

    edges.push({
      id: `edge:${nodeId}`,
      source: node,
      target: centerNode,
      field: ref.field,
      via: ref.via,
      color,
    });
  });

  return { nodes, edges };
});

function startPanCanvas(e: MouseEvent) {
  if ((e.target as HTMLElement)?.closest(".graph-node")) return;
  isPanning.value = true;
  startPan.value = { x: e.clientX - panX.value, y: e.clientY - panY.value };
}

const dragStartClient = ref({ x: 0, y: 0 });

function onPanMove(e: MouseEvent) {
  if (draggedNodeId.value && canvasWrapRef.value) {
    if (Math.hypot(e.clientX - dragStartClient.value.x, e.clientY - dragStartClient.value.y) > 3) {
      dragMoved.value = true;
    }
    const node = graphData.value.nodes.find((n) => n.id === draggedNodeId.value);
    if (node) {
      const rect = canvasWrapRef.value.getBoundingClientRect();
      const mouseGraphX = (e.clientX - rect.left - panX.value) / zoom.value;
      const mouseGraphY = (e.clientY - rect.top - panY.value) / zoom.value;
      nodePositions.value.set(node.id, {
        x: mouseGraphX + nodeDragOffset.value.x,
        y: mouseGraphY + nodeDragOffset.value.y,
      });
    }
    return;
  }
  if (!isPanning.value) return;
  panX.value = e.clientX - startPan.value.x;
  panY.value = e.clientY - startPan.value.y;
}

function stopPan() {
  isPanning.value = false;
  draggedNodeId.value = null;
}

const dragMoved = ref(false);

function onNodeClick(node: GraphNode) {
  if (dragMoved.value) {
    dragMoved.value = false;
    return;
  }
  if (node.reference) open(node.reference);
}

function startNodeDrag(nodeId: string, e: MouseEvent) {
  dragMoved.value = false;
  dragStartClient.value = { x: e.clientX, y: e.clientY };
  draggedNodeId.value = nodeId;
  const node = graphData.value.nodes.find((n) => n.id === nodeId);
  if (node && canvasWrapRef.value) {
    const rect = canvasWrapRef.value.getBoundingClientRect();
    const mouseGraphX = (e.clientX - rect.left - panX.value) / zoom.value;
    const mouseGraphY = (e.clientY - rect.top - panY.value) / zoom.value;
    nodeDragOffset.value = {
      x: node.x - mouseGraphX,
      y: node.y - mouseGraphY,
    };
  }
}

function resetGraphView() {
  zoom.value = 1.0;
  panX.value = 0;
  panY.value = 0;
  nodePositions.value.clear();
}
</script>

<template>
  <div class="reference-container">
    <!-- TOOLBAR WITH VIEW MODE SELECTOR -->
    <header class="reference-toolbar">
      <div style="display:flex; align-items:center; gap:10px;">
        <span class="reference-toolbar__title">Reference Explorer</span>
        <span class="badge" style="font-size:11px;">{{ props.references.length }} references</span>
      </div>

      <div class="reference-view-toggle">
        <button
          type="button"
          :class="['btn', viewMode === 'split' ? 'btn-primary' : 'btn-secondary']"
          style="padding:4px 10px; font-size:12px;"
          @click="viewMode = 'split'"
        >
          ◫ Split
        </button>
        <button
          type="button"
          :class="['btn', viewMode === 'graph' ? 'btn-primary' : 'btn-secondary']"
          style="padding:4px 10px; font-size:12px;"
          @click="viewMode = 'graph'"
        >
          🕸️ Graph
        </button>
        <button
          type="button"
          :class="['btn', viewMode === 'list' ? 'btn-primary' : 'btn-secondary']"
          style="padding:4px 10px; font-size:12px;"
          @click="viewMode = 'list'"
        >
          ☰ List
        </button>
      </div>
    </header>

    <div v-if="props.loading" class="reference-list__empty">Loading references…</div>
    <div v-else-if="!props.references.length" class="reference-list__empty">No references found for this record.</div>

    <!-- MAIN BODY -->
    <div v-else :class="['reference-body', `is-${viewMode}`]">
      <!-- 1. GRAPHIFY NODE-LINK VIEW -->
      <section v-if="viewMode === 'graph' || viewMode === 'split'" class="reference-graph-pane">
        <div
          ref="canvasWrapRef"
          class="reference-graph-canvas-wrap"
          @mousedown="startPanCanvas"
          @mousemove="onPanMove"
          @mouseup="stopPan"
          @mouseleave="stopPan"
        >
          <!-- SVG Graph -->
          <svg
            class="reference-svg"
            width="100%"
            height="100%"
            xmlns="http://www.w3.org/2000/svg"
          >
            <defs>
              <filter id="node-glow" x="-20%" y="-20%" width="140%" height="140%">
                <feGaussianBlur stdDeviation="3" result="blur" />
                <feComposite in="SourceGraphic" in2="blur" operator="over" />
              </filter>
            </defs>

            <g :transform="`translate(${panX}, ${panY}) scale(${zoom})`">
              <!-- Background grid dots -->
              <pattern id="ref-dots" width="24" height="24" patternUnits="userSpaceOnUse">
                <circle cx="12" cy="12" r="1" fill="rgba(255,255,255,0.08)" />
              </pattern>
              <rect x="-2000" y="-2000" width="5000" height="5000" fill="url(#ref-dots)" />

              <!-- Edges -->
              <g class="graph-edges">
                <g v-for="edge in graphData.edges" :key="edge.id" class="graph-edge-group">
                  <line
                    :x1="edge.source.x"
                    :y1="edge.source.y"
                    :x2="edge.target.x"
                    :y2="edge.target.y"
                    :stroke="hoveredNodeId === edge.source.id ? '#fff' : edge.color"
                    :stroke-width="hoveredNodeId === edge.source.id ? 2.5 : 1.2"
                    :stroke-opacity="hoveredNodeId && hoveredNodeId !== edge.source.id ? 0.2 : 0.65"
                    stroke-dasharray="4,2"
                  />
                  <!-- Edge label at midpoint -->
                  <text
                    v-if="viewMode === 'graph' || hoveredNodeId === edge.source.id"
                    :x="(edge.source.x + edge.target.x) / 2"
                    :y="(edge.source.y + edge.target.y) / 2 - 4"
                    fill="var(--text-muted, #94a3b8)"
                    font-size="9"
                    font-weight="500"
                    text-anchor="middle"
                    class="edge-label"
                  >
                    {{ edge.field }}
                  </text>
                </g>
              </g>

              <!-- Nodes -->
              <g class="graph-nodes">
                <g
                  v-for="node in graphData.nodes"
                  :key="node.id"
                  :class="['graph-node', node.isCenter ? 'is-center' : '', hoveredNodeId === node.id ? 'is-hovered' : '']"
                  :transform="`translate(${node.x}, ${node.y})`"
                  @mousedown.stop="startNodeDrag(node.id, $event)"
                  @mouseenter="hoveredNodeId = node.id"
                  @mouseleave="hoveredNodeId = null"
                  @click.stop="onNodeClick(node)"
                >
                  <!-- Outer glow ring on hover/center -->
                  <circle
                    :r="node.isCenter ? 26 : 16"
                    :fill="node.color"
                    :fill-opacity="node.isCenter ? 0.25 : 0.15"
                    filter="url(#node-glow)"
                  />
                  <!-- Main Node Circle -->
                  <circle
                    :r="node.isCenter ? 18 : 10"
                    :fill="node.color"
                    stroke="#0f1117"
                    :stroke-width="node.isCenter ? 3 : 2"
                  />
                  <!-- Center icon or node dot -->
                  <text
                    v-if="node.isCenter"
                    text-anchor="middle"
                    dy="4"
                    font-size="12"
                    fill="#fff"
                  >
                    🎯
                  </text>

                  <!-- Node Text Label -->
                  <text
                    text-anchor="middle"
                    :dy="node.isCenter ? 36 : 22"
                    :fill="hoveredNodeId === node.id ? '#fff' : 'var(--text-main, #e2e8f0)'"
                    font-size="11"
                    font-weight="600"
                    class="node-text"
                  >
                    {{ node.label.length > 20 ? node.label.slice(0, 18) + '…' : node.label }}
                  </text>
                  <!-- Sublabel / Category -->
                  <text
                    text-anchor="middle"
                    :dy="node.isCenter ? 48 : 32"
                    fill="var(--text-muted, #64748b)"
                    font-size="9"
                    class="node-subtext"
                  >
                    {{ node.subLabel }}
                  </text>
                </g>
              </g>
            </g>
          </svg>

          <!-- Graph Controls -->
          <div class="graph-zoom-controls">
            <button class="btn btn-secondary" title="Zoom In" @click="zoom = Math.min(2.5, zoom + 0.2)">+</button>
            <button class="btn btn-secondary" title="Zoom Out" @click="zoom = Math.max(0.4, zoom - 0.2)">−</button>
            <button class="btn btn-secondary" title="Reset View" @click="resetGraphView">↺</button>
          </div>

          <!-- COMMUNITIES SIDEBAR (Matching Image #1) -->
          <aside class="graph-communities-panel">
            <div class="communities-header">COMMUNITIES</div>

            <label class="community-row select-all">
              <input type="checkbox" v-model="allCategoriesSelected" />
              <span class="community-name">Select All</span>
              <span class="community-count">{{ props.references.length }}</span>
            </label>

            <div class="community-divider"></div>

            <label
              v-for="group in activeGroupsWithCount"
              :key="group.kind"
              class="community-row"
            >
              <input
                type="checkbox"
                :checked="activeCategories.has(group.kind)"
                @change="toggleCategory(group.kind)"
              />
              <span class="community-dot" :style="{ background: group.color }"></span>
              <span class="community-name">{{ group.label }}</span>
              <span class="community-count">{{ categoryCounts[group.kind] }}</span>
            </label>
          </aside>
        </div>
      </section>

      <!-- 2. TRADITIONAL LIST VIEW (Matching Image #2) -->
      <section v-if="viewMode === 'list' || viewMode === 'split'" class="reference-list-pane">
        <div class="reference-list">
          <section v-for="group in grouped" :key="group.kind" class="reference-list__group">
            <h4 class="reference-list__title" :style="{ color: group.color }">{{ group.label }}</h4>
            <button
              v-for="(reference, index) in group.rows"
              :key="`${reference.source_id}-${reference.field}-${index}`"
              :class="['reference-list__row', hoveredNodeId?.includes(reference.source_id) ? 'is-highlighted' : '']"
              type="button"
              @mouseenter="hoveredNodeId = `${reference.source_kind}:${reference.source_id}:${reference.node_id ?? ''}:${index}`"
              @mouseleave="hoveredNodeId = null"
              @click="open(reference)"
            >
              <span class="reference-list__label">{{ reference.node_label || reference.source_label }}</span>
              <span class="reference-list__field">{{ reference.field }}</span>
              <span class="reference-list__via">{{ viaLabel(reference) }}</span>
            </button>
          </section>

          <section v-if="unknown.length" class="reference-list__group">
            <h4 class="reference-list__title">Other</h4>
            <button
              v-for="(reference, index) in unknown"
              :key="`other-${reference.source_id}-${index}`"
              class="reference-list__row"
              type="button"
              @click="open(reference)"
            >
              <span class="reference-list__label">{{ reference.source_label }}</span>
              <span class="reference-list__field">{{ reference.field }}</span>
              <span class="reference-list__via">{{ viaLabel(reference) }}</span>
            </button>
          </section>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.reference-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 480px;
}

.reference-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  background: var(--bg-surface, rgba(0, 0, 0, 0.2));
  border-bottom: 1px solid var(--border-subtle, #333);
}

.reference-toolbar__title {
  font-weight: 700;
  font-size: 13px;
  color: var(--text-muted, #94a3b8);
  line-height: 1.2;
}

.reference-view-toggle {
  display: flex;
  gap: 4px;
}

.reference-body {
  display: flex;
  flex: 1;
  overflow: hidden;
}

.reference-body.is-split {
  display: grid;
  grid-template-columns: 1.4fr 1fr;
  height: calc(100% - 45px);
}

.reference-body.is-graph {
  display: flex;
  height: calc(100% - 45px);
}

.reference-body.is-list {
  display: flex;
  height: calc(100% - 45px);
}

/* Graph Canvas Pane */
.reference-graph-pane {
  position: relative;
  flex: 1;
  height: 100%;
  background: #0b0d13;
  overflow: hidden;
  border-right: 1px solid var(--border-subtle, #333);
}

.reference-graph-canvas-wrap {
  position: relative;
  width: 100%;
  height: 100%;
  cursor: grab;
  user-select: none;
}

.reference-graph-canvas-wrap:active {
  cursor: grabbing;
}

.reference-svg {
  display: block;
}

.graph-zoom-controls {
  position: absolute;
  bottom: 16px;
  left: 16px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  z-index: 10;
}

.graph-zoom-controls .btn {
  width: 28px;
  height: 28px;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 14px;
  font-weight: 700;
}

/* Communities Sidebar (Matching Image #1) */
.graph-communities-panel {
  position: absolute;
  top: 16px;
  right: 16px;
  width: 210px;
  background: var(--surface-container-high, #212735);
  border: 1px solid var(--border-subtle);
  border-radius: 0;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  z-index: 10;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
}

.communities-header {
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0;
  color: var(--text-muted, #94a3b8);
  line-height: 1.2;
}

.community-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  cursor: pointer;
  user-select: none;
}

.community-row.select-all {
  font-weight: 600;
  color: var(--text-main, #f1f5f9);
}

.community-dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.community-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.community-count {
  font-size: 11px;
  color: var(--text-muted, #64748b);
  font-variant-numeric: tabular-nums;
}

.community-divider {
  height: 1px;
  background: rgba(255, 255, 255, 0.1);
  margin: 2px 0;
}

/* Nodes & Edges */
.graph-node {
  cursor: pointer;
  transition: transform 0.05s ease;
}

.graph-node text {
  pointer-events: none;
}

.edge-label {
  pointer-events: none;
  text-shadow: 0 1px 3px rgba(0,0,0,0.8);
}

/* List View Pane */
.reference-list-pane {
  flex: 1;
  height: 100%;
  overflow-y: auto;
  padding: 16px;
  background: var(--bg-card, rgba(0,0,0,0.1));
}

.reference-list {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.reference-list__empty {
  padding: 40px;
  text-align: center;
  color: var(--text-muted);
}

.reference-list__group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.reference-list__title {
  margin: 0;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0;
  line-height: 1.2;
  color: var(--text-muted);
}

.reference-list__row {
  display: grid;
  grid-template-columns: 1.5fr 1fr auto;
  gap: 12px;
  align-items: center;
  padding: 8px 12px;
  background: var(--bg-surface, rgba(0,0,0,0.2));
  border: 1px solid var(--border-subtle, #333);
  border-radius: 6px;
  color: var(--text-main);
  text-align: left;
  cursor: pointer;
}

.reference-list__row:hover, .reference-list__row.is-highlighted {
  background: var(--accent-subtle, rgba(56, 189, 248, 0.15));
  border-color: var(--accent, #38bdf8);
}

.reference-list__label {
  font-weight: 600;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.reference-list__field {
  font-size: 12px;
  color: var(--text-muted);
}

.reference-list__via {
  font-size: 11px;
  color: var(--accent, #38bdf8);
}
</style>
