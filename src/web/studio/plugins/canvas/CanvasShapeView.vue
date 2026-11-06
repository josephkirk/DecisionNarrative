<script setup lang="ts">
import { computed } from "vue";
import { shapeDisplayName, type CanvasShape } from "./model.ts";

const props = defineProps<{
  shape: CanvasShape;
  /** Fade shapes that are absent at runtime and draw gate state. */
  runtime: boolean;
  entityName: (id: string) => string | undefined;
  entityColor: (id: string) => string;
}>();

const cx = computed(() => props.shape.x + props.shape.w / 2);
const cy = computed(() => props.shape.y + props.shape.h / 2);
const absent = computed(() => props.runtime && !props.shape.present);
const points = computed(() => {
  const pts = props.shape.points ?? [];
  const out: string[] = [];
  for (let i = 0; i + 1 < pts.length; i += 2) out.push(`${props.shape.x + pts[i]!},${props.shape.y + pts[i + 1]!}`);
  return out.join(" ");
});
const name = computed(() => shapeDisplayName(props.shape, props.entityName));
const GLYPH: Record<string, string> = { key: "⚷", item: "◆", fact: "!", event: "⚡" };
const COLOR: Record<string, string> = { key: "#ffd60a", item: "#64d2ff", fact: "#bf5af2", event: "#ff375f" };
const diamond = computed(() => {
  const { x, y, w, h } = props.shape;
  return `${x + w / 2},${y} ${x + w},${y + h / 2} ${x + w / 2},${y + h} ${x},${y + h / 2}`;
});
</script>

<template>
  <g :class="['vc-kind', `vc-kind-${shape.kind}`, absent && 'vc-absent']">
    <rect v-if="shape.kind === 'rect'" :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h" rx="4"
      :fill="shape.fill" :stroke="shape.stroke" :stroke-width="shape.strokeWidth" />
    <ellipse v-else-if="shape.kind === 'ellipse'" :cx="cx" :cy="cy" :rx="shape.w / 2" :ry="shape.h / 2"
      :fill="shape.fill" :stroke="shape.stroke" :stroke-width="shape.strokeWidth" />
    <polyline v-else-if="shape.kind === 'line' || shape.kind === 'path'" :points="points" fill="none"
      :stroke="shape.stroke" :stroke-width="shape.strokeWidth" stroke-linecap="round" stroke-linejoin="round" />
    <text v-else-if="shape.kind === 'text'" :x="shape.x" :y="shape.y + shape.h * 0.75" :fill="shape.stroke === '#8e8e93' ? 'currentColor' : shape.stroke"
      :font-size="Math.max(10, shape.h * 0.7)" class="vc-text">{{ shape.text }}</text>

    <g v-else-if="shape.kind === 'gate'">
      <rect :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h" rx="3"
        :fill="runtime && shape.open ? 'rgba(48,209,88,0.25)' : '#ff9f0a'" :stroke="runtime && shape.open ? '#30d158' : '#1c1c1e'" stroke-width="2"
        :stroke-dasharray="runtime && shape.open ? '6 4' : undefined" />
      <text :x="cx" :y="shape.y - 6" text-anchor="middle" class="vc-caption">{{ name }}{{ runtime ? (shape.open ? ' · open' : ' · closed') : '' }}</text>
    </g>

    <g v-else-if="shape.kind === 'entity'">
      <circle :cx="cx" :cy="cy" :r="Math.min(shape.w, shape.h) / 2" fill="var(--surface-2, #1c1c1e)" :stroke="entityColor(shape.entity_id ?? '')" stroke-width="3" />
      <text :x="cx" :y="cy + 6" text-anchor="middle" font-size="18" font-weight="700" :fill="entityColor(shape.entity_id ?? '')">{{ (name[0] ?? '@').toUpperCase() }}</text>
      <text :x="cx" :y="shape.y + shape.h + 14" text-anchor="middle" class="vc-caption">{{ name }}</text>
    </g>

    <g v-else>
      <polygon v-if="shape.kind === 'item'" :points="diamond" :fill="COLOR.item" stroke="#1c1c1e" stroke-width="2" />
      <rect v-else :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h" :rx="shape.kind === 'key' ? shape.w / 2 : 6"
        :fill="COLOR[shape.kind] ?? '#888'" stroke="#1c1c1e" stroke-width="2" />
      <text :x="cx" :y="cy + 6" text-anchor="middle" font-size="18" font-weight="700" fill="#1c1c1e">{{ GLYPH[shape.kind] }}</text>
      <text :x="cx" :y="shape.y + shape.h + 14" text-anchor="middle" class="vc-caption">{{ name }}</text>
    </g>

    <line v-if="absent" :x1="shape.x" :y1="shape.y" :x2="shape.x + shape.w" :y2="shape.y + shape.h" stroke="#ff453a" stroke-width="2" />
  </g>
</template>

<style scoped>
.vc-absent { opacity: .3; }
.vc-caption { font-size: 12px; fill: var(--text, #eee); paint-order: stroke; stroke: var(--surface-1, #111); stroke-width: 3px; pointer-events: none; }
.vc-text { font-family: inherit; }
</style>
