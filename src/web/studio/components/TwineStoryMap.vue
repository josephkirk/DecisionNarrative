<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import { extractTwineLinks, calculateLinkCurve } from "../twineUtils.ts";
import type { StoryletRecord } from "../contracts.ts";
import { notify } from "../state.ts";

const props = defineProps<{
  storylets: StoryletRecord[];
}>();

const emit = defineEmits<{
  (e: "refresh"): void;
  (e: "open-storylet", id: string): void;
}>();

const router = useRouter();

// Canvas pan and zoom state
const zoom = ref(1.0);
const panX = ref(100);
const panY = ref(100);
const isPanning = ref(false);
const startPan = ref({ x: 0, y: 0 });

// Dragging individual passage tile
const draggingPassageId = ref<string | null>(null);
const dragStartOffset = ref({ x: 0, y: 0 });

// Active passage positions map (id -> { x, y, w, h })
const passagePositions = ref<Map<string, { x: number; y: number; w: number; h: number }>>(new Map());

function initializePositions() {
  const map = new Map<string, { x: number; y: number; w: number; h: number }>();
  props.storylets.forEach((st, idx) => {
    let x = (idx % 6) * 220 + 80;
    let y = Math.floor(idx / 6) * 180 + 80;

    if (st.position && Array.isArray(st.position) && st.position.length === 2) {
      x = st.position[0];
      y = st.position[1];
    }

    const w = st.size?.[0] ?? 140;
    const h = st.size?.[1] ?? 100;
    map.set(st.id, { x, y, w, h });
  });
  passagePositions.value = map;
}

onMounted(() => {
  initializePositions();
});

// Computed link arrows between passages
const linkConnections = computed(() => {
  const connections: Array<{ id: string; path: string; fromId: string; toId: string }> = [];
  const map = passagePositions.value;

  for (const st of props.storylets) {
    const fromPos = map.get(st.id);
    if (!fromPos) continue;

    // Collect target IDs from choices or inline links
    const targetSet = new Set<string>();
    for (const c of st.choices) {
      if (c.divert_target) targetSet.add(c.divert_target);
    }

    const text = (st.content.sceneOpening || "") + " " + (st.content.text_template || "");
    const inlineLinks = extractTwineLinks(text);
    for (const l of inlineLinks) {
      targetSet.add(l.target);
    }

    for (const tgt of targetSet) {
      // Target could be storylet ID or title
      const targetSt = props.storylets.find(
        (item) => item.id === tgt || item.title.toLowerCase() === tgt.toLowerCase()
      );
      if (!targetSt) continue;

      const toPos = map.get(targetSt.id);
      if (!toPos) continue;

      const curve = calculateLinkCurve(
        fromPos.x, fromPos.y, fromPos.w, fromPos.h,
        toPos.x, toPos.y, toPos.w, toPos.h
      );

      connections.push({
        id: `${st.id}->${targetSt.id}`,
        path: curve,
        fromId: st.id,
        toId: targetSt.id,
      });
    }
  }

  return connections;
});

// Canvas mouse events
function onMouseDownCanvas(e: MouseEvent) {
  if ((e.target as HTMLElement).closest(".passage-tile")) return;
  isPanning.value = true;
  startPan.value = { x: e.clientX - panX.value, y: e.clientY - panY.value };
}

function onMouseMoveCanvas(e: MouseEvent) {
  if (isPanning.value) {
    panX.value = e.clientX - startPan.value.x;
    panY.value = e.clientY - startPan.value.y;
  } else if (draggingPassageId.value) {
    const id = draggingPassageId.value;
    const current = passagePositions.value.get(id);
    if (current) {
      const newX = Math.round((e.clientX - panX.value) / zoom.value - dragStartOffset.value.x);
      const newY = Math.round((e.clientY - panY.value) / zoom.value - dragStartOffset.value.y);
      passagePositions.value.set(id, { ...current, x: newX, y: newY });
    }
  }
}

function onMouseUpCanvas() {
  if (draggingPassageId.value) {
    const id = draggingPassageId.value;
    const pos = passagePositions.value.get(id);
    if (pos) {
      // Save updated position
      const st = props.storylets.find((s) => s.id === id);
      if (st) {
        st.position = [pos.x, pos.y];
        StudioApi.saveContent("storylets", st);
      }
    }
    draggingPassageId.value = null;
  }
  isPanning.value = false;
}

function onWheelCanvas(e: WheelEvent) {
  e.preventDefault();
  const delta = e.deltaY > 0 ? -0.1 : 0.1;
  zoom.value = Math.max(0.25, Math.min(2.0, zoom.value + delta));
}

function onDoubleClickCanvas(e: MouseEvent) {
  if ((e.target as HTMLElement).closest(".passage-tile")) return;
  const clickX = Math.round((e.clientX - panX.value) / zoom.value);
  const clickY = Math.round((e.clientY - panY.value) / zoom.value);
  createPassageAt(clickX, clickY);
}

// Passage Tile dragging
function onMouseDownPassage(e: MouseEvent, id: string) {
  e.stopPropagation();
  draggingPassageId.value = id;
  const pos = passagePositions.value.get(id);
  if (pos) {
    dragStartOffset.value = {
      x: (e.clientX - panX.value) / zoom.value - pos.x,
      y: (e.clientY - panY.value) / zoom.value - pos.y,
    };
  }
}

async function createPassageAt(x: number, y: number) {
  const nextIdx = props.storylets.length + 1;
  const newSt: StoryletRecord = {
    id: `stl_passage_${Date.now().toString(36)}`,
    revision: 1,
    approved_revision: null,
    title: `Passage ${nextIdx}`,
    ending: false,
    location_tags: [],
    cluster_type: "player_passage",
    urgency_weight: 50,
    repeatability: "always",
    cooldown_turns: 0,
    parameters: [],
    preconditions: {},
    content: {
      text_template: "Double-click to write passage prose and add [[Links->Target]].",
    },
    choices: [],
    effects: {},
    position: [x, y],
    size: [140, 100],
    created_at: Date.now(),
  };

  try {
    await StudioApi.saveContent("storylets", newSt);
    notify(`Created passage '${newSt.title}'`, "success");
    emit("refresh");
  } catch (e) {
    notify(e instanceof Error ? e.message : String(e), "error");
  }
}

function zoomIn() { zoom.value = Math.min(2.0, zoom.value + 0.15); }
function zoomOut() { zoom.value = Math.max(0.25, zoom.value - 0.15); }
function resetZoom() { zoom.value = 1.0; panX.value = 100; panY.value = 100; }
</script>

<template>
  <div
    class="twine-map-wrapper"
    @mousedown="onMouseDownCanvas"
    @mousemove="onMouseMoveCanvas"
    @mouseup="onMouseUpCanvas"
    @wheel="onWheelCanvas"
    @dblclick="onDoubleClickCanvas"
  >
    <!-- Map Control HUD -->
    <div class="twine-hud">
      <div style="display:flex; align-items:center; gap:6px;">
        <button class="btn btn-secondary" style="padding:4px 8px;" @click="zoomOut">-</button>
        <span style="font-size:11px; font-weight:600; width:45px; text-align:center;">
          {{ Math.round(zoom * 100) }}%
        </span>
        <button class="btn btn-secondary" style="padding:4px 8px;" @click="zoomIn">+</button>
        <button class="btn btn-secondary" style="font-size:11px; padding:4px 8px;" @click="resetZoom">
          Reset
        </button>
      </div>

      <div style="display:flex; align-items:center; gap:8px;">
        <span style="font-size:11px; color:var(--text-muted);">
          Double-click canvas to create passage · Drag to reposition
        </span>
        <button class="btn btn-primary" style="font-size:12px;" @click="createPassageAt(200, 200)">
          + New Passage
        </button>
      </div>
    </div>

    <!-- Scalable / Pannable Workspace Surface -->
    <div
      class="twine-canvas-surface"
      :style="{
        transform: `translate(${panX}px, ${panY}px) scale(${zoom})`,
        transformOrigin: '0 0',
      }"
    >
      <!-- SVG Directed Link Layer -->
      <svg class="twine-svg-links">
        <defs>
          <marker
            id="arrow"
            viewBox="0 0 10 10"
            refX="6"
            refY="5"
            markerWidth="6"
            markerHeight="6"
            orient="auto-start-reverse"
          >
            <path d="M 0 1 L 8 5 L 0 9 z" fill="var(--accent)" />
          </marker>
        </defs>

        <path
          v-for="link in linkConnections"
          :key="link.id"
          :d="link.path"
          class="twine-link-arrow"
          marker-end="url(#arrow)"
        />
      </svg>

      <!-- Passage Tiles Layer -->
      <div
        v-for="st in props.storylets"
        :key="st.id"
        class="passage-tile"
        :style="{
          left: `${passagePositions.get(st.id)?.x ?? 100}px`,
          top: `${passagePositions.get(st.id)?.y ?? 100}px`,
          width: `${passagePositions.get(st.id)?.w ?? 140}px`,
          minHeight: `${passagePositions.get(st.id)?.h ?? 100}px`,
        }"
        @mousedown="onMouseDownPassage($event, st.id)"
        @dblclick.stop="emit('open-storylet', st.id)"
      >
        <div class="passage-tile-header">
          <strong class="passage-tile-title">{{ st.title }}</strong>
          <span v-if="st.ending" class="badge ending" style="font-size:9px; padding:1px 4px;">End</span>
        </div>

        <p class="passage-tile-excerpt">
          {{ st.content.sceneOpening || st.content.text_template || '(Empty passage)' }}
        </p>

        <div class="passage-tile-footer">
          <span v-if="st.location_tags?.length" style="font-size:10px; color:var(--text-dim);">📍 {{ st.location_tags.join(", ") }}</span>
          <span style="font-size:10px; color:var(--text-dim); margin-left:auto;">
            {{ st.choices.length }} links
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.twine-map-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background-color: var(--bg-canvas);
  background-image: radial-gradient(var(--border-subtle) 1px, transparent 1px);
  background-size: 24px 24px;
  cursor: grab;
  user-select: none;
}

.twine-map-wrapper:active {
  cursor: grabbing;
}

.twine-hud {
  position: absolute;
  top: 14px;
  left: 20px;
  right: 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 14px;
  background: var(--bg-card);
  border: 1px solid var(--border-subtle);
  border-radius: 10px;
  box-shadow: var(--shadow-sm);
  z-index: 100;
}

.twine-canvas-surface {
  position: absolute;
  inset: 0;
  width: 10000px;
  height: 10000px;
  pointer-events: none;
}

.twine-svg-links {
  position: absolute;
  inset: 0;
  width: 10000px;
  height: 10000px;
  pointer-events: none;
}

.twine-link-arrow {
  fill: none;
  stroke: var(--accent);
  stroke-width: 2.2;
  stroke-opacity: 0.75;
  transition: stroke-opacity 0.15s ease;
}

.twine-link-arrow:hover {
  stroke-opacity: 1.0;
  stroke-width: 3;
}

.passage-tile {
  position: absolute;
  background: var(--bg-card);
  border: 1.5px solid var(--border-subtle);
  border-radius: 10px;
  padding: 10px;
  box-shadow: var(--shadow-sm);
  cursor: pointer;
  pointer-events: auto;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  transition: box-shadow 0.12s ease, border-color 0.12s ease;
}

.passage-tile:hover {
  border-color: var(--border-focus);
  box-shadow: var(--shadow-md);
}

.passage-tile-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  margin-bottom: 6px;
}

.passage-tile-title {
  font-size: 12px;
  font-weight: 700;
  line-height: 1.3;
  color: var(--text-main);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.passage-tile-excerpt {
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.passage-tile-footer {
  display: flex;
  align-items: center;
  border-top: 1px solid var(--border-subtle);
  padding-top: 6px;
  margin-top: 6px;
}
</style>
