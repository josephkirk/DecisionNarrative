<script setup lang="ts">
import { computed, nextTick, ref, useId } from "vue";
import type { NarrativeGraph, NarrativeNode } from "../../narrativeGraphTypes.ts";
import { studioState } from "../../state.ts";
import { thumbnailUrl } from "../../thumbnails.ts";
import { CARD_HEIGHT, CARD_WIDTH, TREE_GRAPH_KEY, layoutTree, treeGraphOf, treeTrait, type TreeEdge } from "./model.ts";
import { treeInputEntityId, type TreePointerConfig } from "./pointerNodes.ts";

const props = defineProps<{ node: NarrativeNode; config: TreePointerConfig; graph: NarrativeGraph }>();
const emit = defineEmits<{ (event: "update:config", value: TreePointerConfig): void }>();
const entityId = computed(() => treeInputEntityId(props.graph, props.node.id));
const entities = computed(() => [...studioState.entities, ...studioState.entityTemplates]);
const owner = computed(() => entities.value.find((entity) => entity.id === entityId.value));
const trait = computed(() => owner.value && treeTrait(owner.value, studioState.traitTemplates, TREE_GRAPH_KEY));
const parsed = computed(() => {
  if (!trait.value) return { tree: null, error: "" };
  try { return { tree: treeGraphOf(trait.value), error: "" }; }
  catch (error) { return { tree: null, error: error instanceof Error ? error.message : String(error) }; }
});
const layout = computed(() => parsed.value.tree ? layoutTree(parsed.value.tree) : null);
const names = computed(() => new Map(entities.value.map((entity) => [entity.id, entity.name])));
const thumbnails = computed(() => new Map(entities.value.map((entity) => [entity.id, thumbnailUrl(entity.thumbnail)])));
const position = (id: string) => layout.value!.positions[id]!;
const size = (id: string) => ({ width: parsed.value.tree?.appearance?.nodes[id]?.width ?? parsed.value.tree?.appearance?.defaults?.node?.width ?? CARD_WIDTH, height: parsed.value.tree?.appearance?.nodes[id]?.height ?? parsed.value.tree?.appearance?.defaults?.node?.height ?? CARD_HEIGHT });
const zoom = ref(1);
const viewport = ref<HTMLElement | null>(null);
const clipId = `tree-pointer-${useId()}`;
const bounds = computed(() => {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const id of parsed.value.tree?.nodes ?? []) {
    const point = position(id), dimensions = size(id);
    minX = Math.min(minX, point.x - 40); minY = Math.min(minY, point.y - 50);
    maxX = Math.max(maxX, point.x + dimensions.width + 20); maxY = Math.max(maxY, point.y + dimensions.height + 20);
  }
  for (const image of parsed.value.tree?.appearance?.images ?? []) {
    minX = Math.min(minX, image.x - 20); minY = Math.min(minY, image.y - 20);
    maxX = Math.max(maxX, image.x + image.width + 20); maxY = Math.max(maxY, image.y + image.height + 20);
  }
  if (!Number.isFinite(minX)) return { x: 0, y: 0, width: 320, height: 180 };
  return { x: minX, y: minY, width: Math.max(320, maxX - minX), height: Math.max(180, maxY - minY) };
});
const maxZoom = computed(() => Math.max(4, Math.ceil(bounds.value.width / 240)));
const visibleEdges = computed(() => (parsed.value.tree?.edges ?? []).filter((edge) => layout.value?.positions[edge.from] && layout.value.positions[edge.to]));
const wireStyle = (edge: TreeEdge) => ({ ...parsed.value.tree?.appearance?.defaults?.edge, ...parsed.value.tree?.appearance?.edges[JSON.stringify([edge.from, edge.to])] });
const shape = (id: string) => parsed.value.tree?.shapes?.[id] ?? parsed.value.tree?.appearance?.defaults?.node?.shape ?? "card";
const nodeStyle = (id: string) => {
  const appearance = { ...parsed.value.tree?.appearance?.defaults?.node, ...parsed.value.tree?.appearance?.nodes[id] };
  return { "--tree-outline": appearance.outline ?? "#77869a", "--tree-outline-width": appearance.outlineWidth ?? 2 };
};
const labelInset = (id: string) => shape(id) === "diamond" ? size(id).width * 0.16 : 8;
function textColor(id: string): string {
  const color = fill(id);
  const channel = (start: number) => {
    const value = parseInt(color.slice(start, start + 2), 16) / 255;
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5) > 0.179 ? "#111317" : "#ffffff";
}
const fill = (id: string) => parsed.value.tree?.appearance?.nodes[id]?.background ?? parsed.value.tree?.appearance?.defaults?.node?.background ?? "#1a202c";
const diamond = (id: string) => {
  const p = position(id), s = size(id);
  return `${p.x + s.width / 2},${p.y} ${p.x + s.width},${p.y + s.height / 2} ${p.x + s.width / 2},${p.y + s.height} ${p.x},${p.y + s.height / 2}`;
};
function wire(from: string, to: string): string {
  const a = position(from), b = position(to), sa = size(from), sb = size(to);
  if (from === to) {
    const x = a.x + sa.width / 2, y = a.y;
    return `M ${x - 12} ${y} C ${x - 40} ${y - 42}, ${x + 40} ${y - 42}, ${x + 12} ${y}`;
  }
  const ax = a.x + sa.width / 2, ay = a.y + sa.height / 2;
  const bx = b.x + sb.width / 2, by = b.y + sb.height / 2;
  const dx = bx - ax, dy = by - ay;
  if (dx === 0 && dy === 0) return "";
  const offset = (id: string, width: number, height: number) => {
    const x = Math.abs(dx) / (width / 2), y = Math.abs(dy) / (height / 2);
    if (shape(id) === "ellipse") return 1 / Math.hypot(x, y);
    if (shape(id) === "diamond") return 1 / (x + y);
    return 1 / Math.max(x, y);
  };
  const start = offset(from, sa.width, sa.height);
  const end = offset(to, sb.width, sb.height);
  return `M ${ax + dx * start} ${ay + dy * start} L ${bx - dx * end} ${by - dy * end}`;
}
const selected = (id: string) => props.config.entity_id === entityId.value && props.config.target_id === id;
const focusableId = computed(() => parsed.value.tree?.nodes.find((id) => selected(id)) ?? parsed.value.tree?.nodes[0]);
async function setZoom(value: number) {
  const region = viewport.value;
  if (!region) return;
  const centerX = region.scrollLeft + region.clientWidth / 2;
  const centerY = region.scrollTop + region.clientHeight / 2;
  const previousWidth = region.scrollWidth, previousHeight = region.scrollHeight;
  zoom.value = Math.min(maxZoom.value, Math.max(1, value));
  await nextTick();
  const selectedNode = region.querySelector(".tree-pointer-node[aria-checked='true']");
  if (selectedNode) {
    const target = selectedNode.getBoundingClientRect(), box = region.getBoundingClientRect();
    region.scrollLeft += target.left + target.width / 2 - box.left - box.width / 2;
    region.scrollTop += target.top + target.height / 2 - box.top - box.height / 2;
  } else {
    region.scrollLeft = centerX * region.scrollWidth / previousWidth - region.clientWidth / 2;
    region.scrollTop = centerY * region.scrollHeight / previousHeight - region.clientHeight / 2;
  }
}
function choose(id: string) { emit("update:config", { entity_id: entityId.value, target_id: id }); }
async function moveSelection(id: string, event: KeyboardEvent) {
  if (!["ArrowLeft", "ArrowUp", "ArrowRight", "ArrowDown"].includes(event.key)) return;
  event.preventDefault();
  const nodes = parsed.value.tree?.nodes ?? [];
  if (!nodes.length) return;
  const direction = event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 1;
  const next = nodes[(nodes.indexOf(id) + direction + nodes.length) % nodes.length]!;
  choose(next);
  await nextTick();
  const target = [...(viewport.value?.querySelectorAll(".tree-pointer-node") ?? [])]
    .find((element) => element.getAttribute("data-item-id") === next);
  if (target instanceof SVGElement) target.focus();
}
</script>

<template>
  <div class="tree-pointer-inspector">
    <p v-if="!entityId">Connect a Get Entity node to choose a tree.</p>
    <p v-else-if="!owner">The connected entity is unavailable.</p>
    <p v-else-if="!trait">{{ owner.name }} does not have the TreeGraph trait.</p>
    <p v-else-if="owner.id === 'world'">The intrinsic World cannot own a TreeGraph.</p>
    <p v-else-if="parsed.error" role="alert">{{ parsed.error }}</p>
    <p v-else-if="!parsed.tree?.nodes.length">This tree has no items.</p>
    <template v-else-if="layout && parsed.tree">
      <p>Choose a position in {{ owner?.name }}. The tree cannot be edited here.</p>
      <div class="tree-pointer-tools" aria-label="Tree preview zoom">
        <button type="button" aria-label="Zoom out tree preview" :disabled="zoom <= 1" @click="setZoom(zoom / 2)">−</button>
        <button type="button" aria-label="Fit tree preview" @click="setZoom(1)">Fit</button>
        <button type="button" aria-label="Zoom in tree preview" :disabled="zoom >= maxZoom" @click="setZoom(zoom * 2)">+</button>
      </div>
      <div ref="viewport" class="tree-pointer-viewport" role="region" aria-label="Tree positions" tabindex="0" :style="{ background: parsed.tree.appearance?.canvasBackground }">
        <svg role="radiogroup" :style="{ width: `${zoom * 100}%` }" :viewBox="`${bounds.x} ${bounds.y} ${bounds.width} ${bounds.height}`" aria-label="Choose a tree item">
          <defs>
            <marker :id="`${clipId}-arrow`" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="context-stroke" /></marker>
            <clipPath v-for="(id, index) in parsed.tree.nodes" :id="`${clipId}-${index}`" :key="id">
              <ellipse v-if="shape(id) === 'ellipse'" :cx="position(id).x + size(id).width / 2" :cy="position(id).y + size(id).height / 2" :rx="size(id).width / 2" :ry="size(id).height / 2" />
              <polygon v-else-if="shape(id) === 'diamond'" :points="diamond(id)" />
              <rect v-else :x="position(id).x" :y="position(id).y" :width="size(id).width" :height="size(id).height" :rx="shape(id) === 'rounded' ? 8 : 0" />
            </clipPath>
          </defs>
          <image v-for="image in parsed.tree.appearance?.images ?? []" :key="image.id" :href="thumbnailUrl(image.src) ?? ''" :x="image.x" :y="image.y" :width="image.width" :height="image.height" preserveAspectRatio="none" pointer-events="none" />
          <path v-for="edge in visibleEdges" :key="JSON.stringify([edge.from, edge.to])" :d="wire(edge.from, edge.to)" :marker-end="`url(#${clipId}-arrow)`" class="tree-pointer-wire" :class="`wire-${wireStyle(edge).pattern ?? 'solid'}`" :style="{ stroke: wireStyle(edge).color, strokeWidth: wireStyle(edge).width }" />
          <g v-for="(id, index) in parsed.tree.nodes" :key="id" role="radio" :tabindex="focusableId === id ? 0 : -1" :data-item-id="id" :aria-label="`Select ${names.get(id) || id}`" :aria-checked="selected(id)" :style="nodeStyle(id)" class="tree-pointer-node" :class="{ selected: selected(id) }" @click="choose(id)" @keydown="moveSelection(id, $event)" @keydown.enter.prevent="choose(id)" @keydown.space.prevent="choose(id)">
            <title>{{ names.get(id) || id }}</title>
            <ellipse v-if="shape(id) === 'ellipse'" class="tree-pointer-shape" :fill="fill(id)" :cx="position(id).x + size(id).width / 2" :cy="position(id).y + size(id).height / 2" :rx="size(id).width / 2" :ry="size(id).height / 2" />
            <polygon v-else-if="shape(id) === 'diamond'" class="tree-pointer-shape" :fill="fill(id)" :points="diamond(id)" />
            <rect v-else class="tree-pointer-shape" :fill="fill(id)" :x="position(id).x" :y="position(id).y" :width="size(id).width" :height="size(id).height" :rx="shape(id) === 'rounded' ? 8 : 0" />
            <image v-if="thumbnails.get(id)" :href="thumbnails.get(id) ?? ''" :x="position(id).x" :y="position(id).y" :width="size(id).width" :height="size(id).height" :clip-path="`url(#${clipId}-${index})`" preserveAspectRatio="xMidYMid slice" pointer-events="none" />
            <foreignObject :x="position(id).x + labelInset(id)" :y="position(id).y" :width="Math.max(0, size(id).width - labelInset(id) * 2)" :height="size(id).height" aria-hidden="true">
              <div xmlns="http://www.w3.org/1999/xhtml" class="tree-pointer-label" :class="{ 'has-thumbnail': thumbnails.get(id) }" :style="{ color: thumbnails.get(id) ? '#ffffff' : textColor(id) }"><span>{{ names.get(id) || id }}</span></div>
            </foreignObject>
          </g>
        </svg>
      </div>
      <p v-if="config.entity_id === entityId && config.target_id && !parsed.tree.nodes.includes(config.target_id)" role="alert">The selected item is no longer in this tree. Choose another position.</p>
      <p v-else-if="config.entity_id === entityId && config.target_id">Selected: <strong>{{ names.get(config.target_id) || config.target_id }}</strong></p>
    </template>
  </div>
</template>

<style scoped>
.tree-pointer-inspector p { margin: 0 0 8px; font-size: 13px; }
.tree-pointer-viewport { max-height: 320px; max-width: 100%; overflow: auto; border: 1px solid var(--border, #364052); background: var(--surface, #141923); }
.tree-pointer-viewport:focus-visible, .tree-pointer-node:focus-visible { outline: 2px solid var(--accent, #06b6d4); outline-offset: 2px; }
.tree-pointer-wire { fill: none; stroke: #6d8192; stroke-width: 2; }
.tree-pointer-wire.wire-dash { stroke-dasharray: 10 6; }
.tree-pointer-wire.wire-dot { stroke-dasharray: 1 6; stroke-linecap: round; }
.tree-pointer-wire.wire-brush { stroke-dasharray: 18 2 5 2 1 2; stroke-linecap: round; }
.tree-pointer-node { cursor: pointer; }
.tree-pointer-tools { display: flex; gap: 4px; margin-bottom: 8px; }
.tree-pointer-tools button { min-width: 44px; height: 44px; padding: 0 8px; color: #e2e2e8; background: #1a202c; border: 1px solid #364052; font: inherit; cursor: pointer; }
.tree-pointer-tools button:hover:not(:disabled) { border-color: #06b6d4; }
.tree-pointer-tools button:disabled { opacity: 0.45; cursor: default; }
.tree-pointer-tools button:focus-visible { outline: 2px solid #06b6d4; outline-offset: 2px; }
.tree-pointer-viewport svg { display: block; height: auto; max-width: none; }
.tree-pointer-shape { stroke: var(--tree-outline); stroke-width: var(--tree-outline-width); }
.tree-pointer-node.selected .tree-pointer-shape { stroke: #06b6d4; stroke-width: 4; }
.tree-pointer-label { display: flex; align-items: center; justify-content: center; width: 100%; height: 100%; color: #eef2f7; font-size: 13px; pointer-events: none; }
.tree-pointer-label span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tree-pointer-label.has-thumbnail span { padding: 2px 4px; background: rgba(0, 0, 0, 0.8); }
</style>
