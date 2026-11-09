<script setup lang="ts">
import { computed, ref } from "vue";
import type { NarrativeGraph, NarrativeNode } from "../../narrativeGraphTypes.ts";
import { studioState } from "../../state.ts";
import { CARD_HEIGHT, CARD_WIDTH, TREE_GRAPH_KEY, layoutTree, treeGraphOf, treeTrait } from "./model.ts";
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
const position = (id: string) => layout.value!.positions[id]!;
const size = (id: string) => ({ width: parsed.value.tree?.appearance?.nodes[id]?.width ?? parsed.value.tree?.appearance?.defaults?.node?.width ?? CARD_WIDTH, height: parsed.value.tree?.appearance?.nodes[id]?.height ?? parsed.value.tree?.appearance?.defaults?.node?.height ?? CARD_HEIGHT });
const zoom = ref(1);
const clipId = `tree-pointer-${crypto.randomUUID()}`;
const bounds = computed(() => {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const id of parsed.value.tree?.nodes ?? []) {
    const point = position(id), dimensions = size(id);
    minX = Math.min(minX, point.x - 40); minY = Math.min(minY, point.y - 50);
    maxX = Math.max(maxX, point.x + dimensions.width + 20); maxY = Math.max(maxY, point.y + dimensions.height + 20);
  }
  if (!Number.isFinite(minX)) return { x: 0, y: 0, width: 320, height: 180 };
  return { x: minX, y: minY, width: Math.max(320, maxX - minX), height: Math.max(180, maxY - minY) };
});
const visibleEdges = computed(() => (parsed.value.tree?.edges ?? []).filter((edge) => layout.value?.positions[edge.from] && layout.value.positions[edge.to]));
const shape = (id: string) => parsed.value.tree?.shapes?.[id] ?? parsed.value.tree?.appearance?.defaults?.node?.shape ?? "card";
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
  const start = Math.min(dx ? sa.width / 2 / Math.abs(dx) : Infinity, dy ? sa.height / 2 / Math.abs(dy) : Infinity);
  const end = Math.min(dx ? sb.width / 2 / Math.abs(dx) : Infinity, dy ? sb.height / 2 / Math.abs(dy) : Infinity);
  return `M ${ax + dx * start} ${ay + dy * start} L ${bx - dx * end} ${by - dy * end}`;
}
const selected = (id: string) => props.config.entity_id === entityId.value && props.config.target_id === id;
function choose(id: string) { emit("update:config", { entity_id: entityId.value, target_id: id }); }
</script>

<template>
  <div class="tree-pointer-inspector">
    <p v-if="!entityId">Connect a Get Entity node to choose a tree.</p>
    <p v-else-if="!owner">The connected entity is unavailable.</p>
    <p v-else-if="!trait">{{ owner.name }} does not have the TreeGraph trait.</p>
    <p v-else-if="parsed.error" role="alert">{{ parsed.error }}</p>
    <p v-else-if="!parsed.tree?.nodes.length">This tree has no items.</p>
    <template v-else-if="layout && parsed.tree">
      <p>Choose a position in {{ owner?.name }}. The tree cannot be edited here.</p>
      <div class="tree-pointer-tools" aria-label="Tree preview zoom">
        <button type="button" aria-label="Zoom out tree preview" :disabled="zoom <= 1" @click="zoom = Math.max(1, zoom - 0.5)">−</button>
        <button type="button" aria-label="Fit tree preview" @click="zoom = 1">Fit</button>
        <button type="button" aria-label="Zoom in tree preview" :disabled="zoom >= 4" @click="zoom = Math.min(4, zoom + 0.5)">+</button>
      </div>
      <div class="tree-pointer-viewport" role="region" aria-label="Tree positions" tabindex="0">
        <svg :style="{ width: `${zoom * 100}%` }" :viewBox="`${bounds.x} ${bounds.y} ${bounds.width} ${bounds.height}`" aria-label="Choose a tree item">
          <defs>
            <marker :id="`${clipId}-arrow`" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M 0 0 L 10 5 L 0 10 z" fill="#6d8192" /></marker>
            <clipPath v-for="(id, index) in parsed.tree.nodes" :id="`${clipId}-${index}`" :key="id"><rect :x="position(id).x + 8" :y="position(id).y" :width="Math.max(0, size(id).width - 16)" :height="size(id).height" /></clipPath>
          </defs>
          <path v-for="edge in visibleEdges" :key="JSON.stringify([edge.from, edge.to])" :d="wire(edge.from, edge.to)" :marker-end="`url(#${clipId}-arrow)`" class="tree-pointer-wire" />
          <g v-for="(id, index) in parsed.tree.nodes" :key="id" role="button" tabindex="0" :aria-label="`Select ${names.get(id) ?? id}`" :aria-pressed="selected(id)" class="tree-pointer-node" :class="{ selected: selected(id) }" @click="choose(id)" @keydown.enter.prevent="choose(id)" @keydown.space.prevent="choose(id)">
            <title>{{ names.get(id) ?? id }}</title>
            <ellipse v-if="shape(id) === 'ellipse'" class="tree-pointer-shape" :fill="fill(id)" :cx="position(id).x + size(id).width / 2" :cy="position(id).y + size(id).height / 2" :rx="size(id).width / 2" :ry="size(id).height / 2" />
            <polygon v-else-if="shape(id) === 'diamond'" class="tree-pointer-shape" :fill="fill(id)" :points="diamond(id)" />
            <rect v-else class="tree-pointer-shape" :fill="fill(id)" :x="position(id).x" :y="position(id).y" :width="size(id).width" :height="size(id).height" :rx="shape(id) === 'rounded' ? 8 : 0" />
            <text :clip-path="`url(#${clipId}-${index})`" :x="position(id).x + size(id).width / 2" :y="position(id).y + size(id).height / 2" text-anchor="middle" dominant-baseline="middle">{{ names.get(id) ?? id }}</text>
          </g>
        </svg>
      </div>
      <p v-if="config.entity_id === entityId && config.target_id">Selected: <strong>{{ names.get(config.target_id) ?? config.target_id }}</strong></p>
    </template>
  </div>
</template>

<style scoped>
.tree-pointer-inspector p { margin: 0 0 8px; font-size: 13px; }
.tree-pointer-viewport { max-height: 320px; max-width: 100%; overflow: auto; border: 1px solid var(--border, #364052); background: var(--surface, #141923); }
.tree-pointer-viewport:focus-visible, .tree-pointer-node:focus-visible { outline: 2px solid var(--accent, #06b6d4); outline-offset: 2px; }
.tree-pointer-wire { fill: none; stroke: #6d8192; stroke-width: 2; }
.tree-pointer-node { cursor: pointer; }
.tree-pointer-tools { display: flex; gap: 4px; margin-bottom: 8px; }
.tree-pointer-tools button { min-width: 32px; height: 28px; padding: 0 8px; color: #e2e2e8; background: #1a202c; border: 1px solid #364052; font: inherit; cursor: pointer; }
.tree-pointer-tools button:hover:not(:disabled) { border-color: #06b6d4; }
.tree-pointer-tools button:disabled { opacity: 0.45; cursor: default; }
.tree-pointer-tools button:focus-visible { outline: 2px solid #06b6d4; outline-offset: 2px; }
.tree-pointer-viewport svg { display: block; height: auto; max-width: none; }
.tree-pointer-shape { stroke: #77869a; stroke-width: 2; }
.tree-pointer-node.selected .tree-pointer-shape { stroke: #06b6d4; stroke-width: 4; }
.tree-pointer-node text { fill: #eef2f7; font-size: 13px; pointer-events: none; }
</style>
