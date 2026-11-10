<script setup lang="ts">
import { computed, ref } from "vue";
import { findPane, paneGeometry, type Box } from "../paneModel.ts";
import { paneState, resizeSplit } from "../view-models/panes.ts";
import StudioPane from "./StudioPane.vue";

const workspace = ref<HTMLElement | null>(null);
const geometry = computed(() => paneGeometry(paneState.layout));
const resizing = ref<string | null>(null);

const pct = (value: number) => `${(value * 100).toFixed(4)}%`;
function paneStyle(box: Box) {
  return { left: pct(box.x), top: pct(box.y), width: pct(box.w), height: pct(box.h) };
}
/** Outer splits come first; they sit above inner handles where dividers meet. */
function handleStyle(handle: (typeof geometry.value.handles)[number], index: number) {
  const { box, ratio } = handle;
  const zIndex = 4 + geometry.value.handles.length - index;
  return handle.direction === "row"
    ? { left: pct(box.x + box.w * ratio), top: pct(box.y), height: pct(box.h), zIndex }
    : { top: pct(box.y + box.h * ratio), left: pct(box.x), width: pct(box.w), zIndex };
}

function startResize(splitId: string, event: PointerEvent) {
  if (event.button !== 0) return;
  resizing.value = splitId;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}
function moveResize(handle: (typeof geometry.value.handles)[number], event: PointerEvent) {
  const rect = workspace.value?.getBoundingClientRect();
  if (resizing.value !== handle.splitId || !rect) return;
  const { box } = handle;
  const ratio = handle.direction === "row"
    ? ((event.clientX - rect.left) / rect.width - box.x) / box.w
    : ((event.clientY - rect.top) / rect.height - box.y) / box.h;
  resizeSplit(handle.splitId, ratio);
}
function keyResize(handle: (typeof geometry.value.handles)[number], event: KeyboardEvent) {
  const back = handle.direction === "row" ? "ArrowLeft" : "ArrowUp";
  const forward = handle.direction === "row" ? "ArrowRight" : "ArrowDown";
  if (event.key !== back && event.key !== forward) return;
  event.preventDefault();
  resizeSplit(handle.splitId, handle.ratio + (event.key === forward ? 0.05 : -0.05));
}
</script>

<template>
  <div ref="workspace" class="pane-workspace" :class="{ 'is-resizing': resizing, 'has-maximized': paneState.maximizedId }">
    <div
      v-for="entry in geometry.panes"
      :key="entry.id"
      class="pane-workspace__slot"
      :class="{ 'is-maximized': paneState.maximizedId === entry.id }"
      :style="paneStyle(entry.box)"
    >
      <StudioPane :pane="findPane(paneState.layout, entry.id)!" />
    </div>
    <div
      v-for="(handle, index) in geometry.handles"
      :key="handle.splitId"
      class="pane-workspace__handle"
      :class="`is-${handle.direction}`"
      :style="handleStyle(handle, index)"
      role="separator"
      tabindex="0"
      :aria-orientation="handle.direction === 'row' ? 'vertical' : 'horizontal'"
      :aria-valuenow="Math.round(handle.ratio * 100)"
      aria-valuemin="15"
      aria-valuemax="85"
      aria-label="Resize panes"
      @pointerdown="startResize(handle.splitId, $event)"
      @pointermove="moveResize(handle, $event)"
      @pointerup="resizing = null"
      @pointercancel="resizing = null"
      @keydown="keyResize(handle, $event)"
    ></div>
  </div>
</template>
