<script setup lang="ts">
import { computed } from "vue";
import { isGameplay, parseCanvas } from "./model.ts";

/** Property-level UI: replaces the raw JSON textbox for a `canvas_doc` property. */
const props = defineProps<{ modelValue: unknown; disabled?: boolean }>();
const doc = computed(() => parseCanvas(props.modelValue));
</script>

<template>
  <div class="cps" data-testid="canvas-property-summary">
    <span>{{ doc.layers.length }} layers · {{ doc.shapes.length }} shapes · {{ doc.shapes.filter(isGameplay).length }} markers &amp; references · {{ doc.revisions.length }} revisions</span>
    <small>Edit in the <strong>Canvas</strong> tab.</small>
  </div>
</template>

<style scoped>
.cps { display: flex; flex-direction: column; gap: 2px; padding: 6px 8px; border: 1px dashed var(--border, #333); border-radius: 8px; font-size: 13px; }
.cps small { color: var(--text-muted, #999); }
</style>
