<script setup lang="ts">
import { ref } from "vue";
import { thumbnailUrl } from "../../thumbnails.ts";
import type { TreeCanvasImage } from "./model.ts";

const props = withDefaults(
  defineProps<{
    images: TreeCanvasImage[];
    readonly?: boolean;
    zoom: number;
  }>(),
  {
    images: () => [],
    readonly: false,
    zoom: 1,
  },
);

const emit = defineEmits<{
  (e: "change", id: string, update: Partial<TreeCanvasImage>): void;
  (e: "context", id: string, event: MouseEvent): void;
}>();

const RESIZE_HANDLES = ["nw", "n", "ne", "e", "se", "s", "sw", "w"] as const;
type ResizeDirection = (typeof RESIZE_HANDLES)[number];

interface DragState {
  id: string;
  pointerId: number;
  startX: number;
  startY: number;
  origX: number;
  origY: number;
}

interface ResizeState {
  id: string;
  direction: ResizeDirection;
  pointerId: number;
  startX: number;
  startY: number;
  origX: number;
  origY: number;
  origWidth: number;
  origHeight: number;
}

const activeDrag = ref<DragState | null>(null);
const activeResize = ref<ResizeState | null>(null);


function zoomFactor(): number {
  return typeof props.zoom === "number" && props.zoom > 0 ? props.zoom : 1;
}

function startImageDrag(image: TreeCanvasImage, event: PointerEvent) {
  // Ignore alt so canvas alt-cut / alt-pan works
  if (event.altKey) return;

  // Locked or readonly images never move or resize; left pointer bubbles to pan
  if (image.locked || props.readonly) return;

  // Only primary (left) button starts move drag
  if (event.button !== 0) return;

  event.stopPropagation();
  event.preventDefault();

  const target = event.currentTarget as HTMLElement | null;
  target?.setPointerCapture(event.pointerId);

  activeDrag.value = {
    id: image.id,
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    origX: image.x,
    origY: image.y,
  };
}

function onImagePointerMove(event: PointerEvent) {
  if (!activeDrag.value || activeDrag.value.pointerId !== event.pointerId) return;

  const z = zoomFactor();
  const dx = Math.round((event.clientX - activeDrag.value.startX) / z);
  const dy = Math.round((event.clientY - activeDrag.value.startY) / z);

  emit("change", activeDrag.value.id, {
    x: Math.round(activeDrag.value.origX + dx),
    y: Math.round(activeDrag.value.origY + dy),
  });
}

function onImagePointerUp(event: PointerEvent) {
  if (activeDrag.value && activeDrag.value.pointerId === event.pointerId) {
    try {
      (event.currentTarget as HTMLElement | null)?.releasePointerCapture(event.pointerId);
    } catch {
      // pointer capture might have already been released
    }
    activeDrag.value = null;
  }
}

function startResize(image: TreeCanvasImage, direction: ResizeDirection, event: PointerEvent) {
  if (event.altKey) return;
  if (image.locked || props.readonly) return;
  if (event.button !== 0) return;

  event.stopPropagation();
  event.preventDefault();

  const target = event.currentTarget as HTMLElement | null;
  target?.setPointerCapture(event.pointerId);

  activeResize.value = {
    id: image.id,
    direction,
    pointerId: event.pointerId,
    startX: event.clientX,
    startY: event.clientY,
    origX: image.x,
    origY: image.y,
    origWidth: image.width,
    origHeight: image.height,
  };
}

function onResizePointerMove(event: PointerEvent) {
  if (!activeResize.value || activeResize.value.pointerId !== event.pointerId) return;

  const z = zoomFactor();
  const dx = Math.round((event.clientX - activeResize.value.startX) / z);
  const dy = Math.round((event.clientY - activeResize.value.startY) / z);

  const west = activeResize.value.direction.includes("w");
  const east = activeResize.value.direction.includes("e");
  const north = activeResize.value.direction.includes("n");
  const south = activeResize.value.direction.includes("s");

  const rawWidth = activeResize.value.origWidth + (west ? -dx : east ? dx : 0);
  const rawHeight = activeResize.value.origHeight + (north ? -dy : south ? dy : 0);

  // Clamping >= 24px
  const width = Math.max(24, Math.round(rawWidth));
  const height = Math.max(24, Math.round(rawHeight));

  // Anchoring opposite edge
  const x = Math.round(activeResize.value.origX + (west ? activeResize.value.origWidth - width : 0));
  const y = Math.round(activeResize.value.origY + (north ? activeResize.value.origHeight - height : 0));

  emit("change", activeResize.value.id, { x, y, width, height });
}

function onResizePointerUp(event: PointerEvent) {
  if (activeResize.value && activeResize.value.pointerId === event.pointerId) {
    try {
      (event.currentTarget as HTMLElement | null)?.releasePointerCapture(event.pointerId);
    } catch {
      // pointer capture might have already been released
    }
    activeResize.value = null;
  }
}

function onContextMenu(id: string, event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
  emit("context", id, event);
}

function onKeyDown(image: TreeCanvasImage, event: KeyboardEvent) {
  // Support context menu key
  if (event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) {
    event.preventDefault();
    event.stopPropagation();
    const target = event.currentTarget as HTMLElement | null;
    const rect = target?.getBoundingClientRect();
    const mouseEvent = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: rect ? Math.round(rect.left + rect.width / 2) : 0,
      clientY: rect ? Math.round(rect.top + rect.height / 2) : 0,
    });
    emit("context", image.id, mouseEvent);
    return;
  }

  // Locked or readonly images never move or resize via keyboard
  if (props.readonly || image.locked) return;

  const arrowKeys = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"];
  if (!arrowKeys.includes(event.key)) return;

  event.preventDefault();
  event.stopPropagation();

  const step = 10;
  if (event.shiftKey) {
    // shift+arrows resize, clamping >= 24px
    let { width, height } = image;
    if (event.key === "ArrowRight") width = Math.max(24, width + step);
    else if (event.key === "ArrowLeft") width = Math.max(24, width - step);
    else if (event.key === "ArrowDown") height = Math.max(24, height + step);
    else if (event.key === "ArrowUp") height = Math.max(24, height - step);

    emit("change", image.id, { width, height });
  } else {
    // arrows move unlocked image
    let { x, y } = image;
    if (event.key === "ArrowLeft") x -= step;
    else if (event.key === "ArrowRight") x += step;
    else if (event.key === "ArrowUp") y -= step;
    else if (event.key === "ArrowDown") y += step;

    emit("change", image.id, { x, y });
  }
}
</script>

<template>
  <div class="tree-image-layer" aria-label="Tree canvas images">
    <div
      v-for="image in images"
      :key="image.id"
      class="tree-canvas-image"
      :class="{
        'is-locked': image.locked,
        'is-dragging': activeDrag?.id === image.id,
        'is-resizing': activeResize?.id === image.id,
      }"
      :style="{
        left: `${image.x}px`,
        top: `${image.y}px`,
        width: `${image.width}px`,
        height: `${image.height}px`,
      }"
      tabindex="0"
      role="img"
      :aria-label="image.locked ? 'Tree canvas image (locked)' : 'Tree canvas image'"
      @pointerdown="startImageDrag(image, $event)"
      @pointermove="onImagePointerMove"
      @pointerup="onImagePointerUp"
      @pointercancel="onImagePointerUp"
      @contextmenu="onContextMenu(image.id, $event)"
      @keydown="onKeyDown(image, $event)"
    >
      <img
        :src="thumbnailUrl(image.src) ?? undefined"
        :width="image.width"
        :height="image.height"
        alt=""
        class="tree-canvas-image__content"
        draggable="false"
        aria-hidden="true"
        @dragstart.prevent
      />

      <template v-if="!readonly && !image.locked">
        <span
          v-for="dir in RESIZE_HANDLES"
          :key="dir"
          class="tree-image-resize"
          :class="`resize-${dir}`"
          :title="`Resize image ${dir.toUpperCase()}`"
          :aria-label="`Resize image ${dir.toUpperCase()}`"
          @pointerdown="startResize(image, dir, $event)"
          @pointermove="onResizePointerMove"
          @pointerup="onResizePointerUp"
          @pointercancel="onResizePointerUp"
        />
      </template>
    </div>
  </div>
</template>

<style scoped>
.tree-image-layer {
  position: absolute;
  inset: 0;
  pointer-events: none;
  z-index: 0;
  isolation: isolate;
}

.tree-canvas-image {
  position: absolute;
  box-sizing: border-box;
  touch-action: none;
  cursor: grab;
  user-select: none;
  pointer-events: auto;
}

.tree-canvas-image.is-dragging {
  cursor: grabbing;
}

.tree-canvas-image.is-locked {
  cursor: default;
}

.tree-canvas-image__content {
  width: 100%;
  height: 100%;
  object-fit: fill;
  display: block;
  user-select: none;
  -webkit-user-drag: none;
  pointer-events: none;
}

/* Hover and focus indicators without obscuring canvas */
.tree-canvas-image:hover,
.tree-canvas-image:focus-visible,
.tree-canvas-image:focus-within,
.tree-canvas-image.is-dragging,
.tree-canvas-image.is-resizing {
  outline: 1px dashed var(--accent, #3b82f6);
  outline-offset: 1px;
}

.tree-canvas-image.is-locked:hover,
.tree-canvas-image.is-locked:focus-visible {
  outline: 1px dashed var(--border-subtle, #94a3b8);
  outline-offset: 1px;
}

/* Resize affordances hidden by default, shown on hover/focus */
.tree-canvas-image .tree-image-resize {
  position: absolute;
  z-index: 2;
  opacity: 0;
  pointer-events: none;
  box-sizing: border-box;
}

.tree-canvas-image:hover .tree-image-resize,
.tree-canvas-image:focus-visible .tree-image-resize,
.tree-canvas-image:focus-within .tree-image-resize,
.tree-canvas-image.is-resizing .tree-image-resize {
  opacity: 1;
  pointer-events: auto;
}

/* Edge handles */
.resize-n,
.resize-s {
  left: 8px;
  right: 8px;
  height: 6px;
  cursor: ns-resize;
}

.resize-e,
.resize-w {
  top: 8px;
  bottom: 8px;
  width: 6px;
  cursor: ew-resize;
}

.resize-n {
  top: -3px;
}

.resize-s {
  bottom: -3px;
}

.resize-e {
  right: -3px;
}

.resize-w {
  left: -3px;
}

/* Corner handles */
.resize-ne,
.resize-se,
.resize-sw,
.resize-nw {
  width: 8px;
  height: 8px;
  background: var(--bg-surface, #ffffff);
  border: 1px solid var(--accent, #3b82f6);
  border-radius: 2px;
}

.resize-ne {
  top: -4px;
  right: -4px;
  cursor: nesw-resize;
}

.resize-sw {
  bottom: -4px;
  left: -4px;
  cursor: nesw-resize;
}

.resize-nw {
  top: -4px;
  left: -4px;
  cursor: nwse-resize;
}

.resize-se {
  bottom: -4px;
  right: -4px;
  cursor: nwse-resize;
}
</style>
