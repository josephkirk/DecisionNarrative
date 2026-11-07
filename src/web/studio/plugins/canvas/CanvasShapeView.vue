<script setup lang="ts">
import { computed } from "vue";
import { markerGlyph, shapeDisplayName, type CanvasShape } from "./model.ts";
import { refInfo, refName } from "./refs.ts";

const props = defineProps<{
  shape: CanvasShape;
  /** Fade shapes that are absent at runtime. */
  runtime: boolean;
  /** Unique per canvas instance, so clip-path ids do not collide. */
  uid: string;
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
const name = computed(() => shapeDisplayName(props.shape, refName));
const ref = computed(() => refInfo(props.shape));
const clipId = computed(() => `vc-clip-${props.uid}-${props.shape.id}`);
const initial = computed(() => (ref.value?.name ?? name.value).trim().charAt(0).toUpperCase() || "?");
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

    <g v-else-if="shape.kind === 'note'">
      <rect :x="shape.x + 3" :y="shape.y + 4" :width="shape.w" :height="shape.h" rx="2" fill="rgba(0,0,0,0.25)" />
      <rect :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h" rx="2" :fill="shape.fill" stroke="rgba(0,0,0,0.15)" />
      <foreignObject :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h">
        <div xmlns="http://www.w3.org/1999/xhtml" class="vc-note-text">{{ shape.text }}</div>
      </foreignObject>
    </g>

    <g v-else-if="shape.kind === 'marker'">
      <circle :cx="cx" :cy="cy" :r="Math.min(shape.w, shape.h) / 2" fill="var(--surface-2, #1c1c1e)" :stroke="shape.stroke" stroke-width="2" />
      <text :x="cx" :y="cy" text-anchor="middle" dominant-baseline="central" :font-size="Math.min(shape.w, shape.h) * 0.55">{{ markerGlyph(shape.icon) }}</text>
      <text :x="cx" :y="shape.y + shape.h + 14" text-anchor="middle" class="vc-caption">{{ name }}</text>
    </g>

    <g v-else-if="shape.kind === 'entity' || shape.kind === 'storylet'">
      <defs>
        <clipPath :id="clipId">
          <circle v-if="shape.kind === 'entity'" :cx="cx" :cy="cy" :r="Math.min(shape.w, shape.h) / 2" />
          <rect v-else :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h" rx="6" />
        </clipPath>
      </defs>
      <circle v-if="shape.kind === 'entity'" :cx="cx" :cy="cy" :r="Math.min(shape.w, shape.h) / 2" fill="var(--surface-2, #1c1c1e)" />
      <rect v-else :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h" rx="6" fill="var(--surface-2, #1c1c1e)" />
      <image v-if="ref?.thumb" :href="ref.thumb" :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h"
        preserveAspectRatio="xMidYMid slice" :clip-path="`url(#${clipId})`" class="vc-thumb" />
      <text v-else :x="cx" :y="cy" text-anchor="middle" dominant-baseline="central" :font-size="Math.min(shape.w, shape.h) * 0.4" font-weight="700" :fill="ref?.color ?? '#888'">{{ initial }}</text>
      <circle v-if="shape.kind === 'entity'" :cx="cx" :cy="cy" :r="Math.min(shape.w, shape.h) / 2" fill="none" :stroke="ref?.color ?? '#888'" stroke-width="3" />
      <rect v-else :x="shape.x" :y="shape.y" :width="shape.w" :height="shape.h" rx="6" fill="none" :stroke="ref?.color ?? '#888'" stroke-width="3" />
      <text v-if="shape.kind === 'storylet'" :x="shape.x + shape.w - 2" :y="shape.y + 2" text-anchor="end" dominant-baseline="hanging" font-size="14">📖</text>
      <text :x="cx" :y="shape.y + shape.h + 14" text-anchor="middle" class="vc-caption">{{ name }}</text>
    </g>

    <line v-if="absent" :x1="shape.x" :y1="shape.y" :x2="shape.x + shape.w" :y2="shape.y + shape.h" stroke="#ff453a" stroke-width="2" />
  </g>
</template>

<style scoped>
.vc-absent { opacity: .3; }
.vc-caption { font-size: 12px; fill: var(--text, #eee); paint-order: stroke; stroke: var(--surface-1, #111); stroke-width: 3px; pointer-events: none; }
.vc-text { font-family: inherit; }
.vc-thumb { pointer-events: none; }
.vc-note-text { box-sizing: border-box; width: 100%; height: 100%; padding: 8px 10px; overflow: hidden; color: #1c1c1e; font-size: 13px; line-height: 1.35; white-space: pre-wrap; word-break: break-word; pointer-events: none; }
</style>
