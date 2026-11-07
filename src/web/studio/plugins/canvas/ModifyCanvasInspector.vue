<script setup lang="ts">
import { computed, ref } from "vue";
import { studioState } from "../../state.ts";
import type { AnyEntity } from "../runtime.ts";
import {
  CANVAS_PROPERTY_KEY, MARKER_ICONS, actionsFor, applyOps, isGameplay, newId, opLabelText, parseCanvas, shapeDisplayName,
  type CanvasAction, type CanvasOp, type ModifyCanvasConfig,
} from "./model.ts";
import { refName } from "./refs.ts";
import CanvasShapeView from "./CanvasShapeView.vue";

const props = defineProps<{ config: ModifyCanvasConfig }>();
const emit = defineEmits<{ (e: "update:config", value: ModifyCanvasConfig): void }>();

const entities = computed(() => [...studioState.entities, ...studioState.entityTemplates] as AnyEntity[]);
const withCanvas = computed(() => entities.value.filter((entity) => entity.properties.some((property) => property.key === CANVAS_PROPERTY_KEY)));
const target = computed(() => entities.value.find((entity) => entity.id === props.config.canvas_entity));
const doc = computed(() => {
  const raw = target.value?.properties.find((property) => property.key === CANVAS_PROPERTY_KEY)?.value;
  return raw ? parseCanvas(raw) : null;
});
const elements = computed(() => doc.value?.shapes.filter(isGameplay) ?? []);
const uid = newId("mc");
const preview = ref<"before" | "after">("after");
const shown = computed(() => doc.value ? (preview.value === "after" ? applyOps(doc.value, props.config.ops) : doc.value) : null);
const viewBox = computed(() => {
  const shapes = doc.value?.shapes ?? [];
  if (!shapes.length) return "0 0 400 240";
  const minX = Math.min(...shapes.map((s) => s.x)) - 40; const minY = Math.min(...shapes.map((s) => s.y)) - 40;
  const maxX = Math.max(...shapes.map((s) => s.x + s.w)) + 40; const maxY = Math.max(...shapes.map((s) => s.y + s.h)) + 40;
  return `${minX} ${minY} ${maxX - minX} ${maxY - minY}`;
});
const pendingShape = ref("");
const pendingAction = ref<CanvasAction | "">("");
const pendingIcon = ref(MARKER_ICONS[0]!.id);
const pendingActions = computed(() => {
  const shape = elements.value.find((entry) => entry.id === pendingShape.value);
  return shape ? actionsFor(shape.kind) : [];
});

function update(patch: Partial<ModifyCanvasConfig>) { emit("update:config", { ...props.config, ...patch }); }
function setEntity(id: string) { update({ canvas_entity: id, ops: [] }); }
function addOp() {
  if (!pendingShape.value || !pendingAction.value) return;
  const op: CanvasOp = { shape_id: pendingShape.value, action: pendingAction.value };
  if (op.action === "set_icon") op.icon = pendingIcon.value;
  update({ ops: [...props.config.ops, op] });
  pendingShape.value = ""; pendingAction.value = "";
}
function removeOp(index: number) { update({ ops: props.config.ops.filter((_, i) => i !== index) }); }
function opLabel(op: CanvasOp) {
  const shape = elements.value.find((entry) => entry.id === op.shape_id);
  if (!shape) return `${op.action} (missing element)`;
  return `${opLabelText(op, shape)}: ${shapeDisplayName(shape, refName)}`;
}
function quickAdd(shapeId: string, action: CanvasAction) { update({ ops: [...props.config.ops, { shape_id: shapeId, action }] }); }
</script>

<template>
  <div class="mc-root" data-testid="modify-canvas-inspector">
    <div class="form-group">
      <label class="field-label">Canvas entity</label>
      <select class="form-select" :value="config.canvas_entity" @change="setEntity(($event.target as HTMLSelectElement).value)">
        <option value="">Choose an entity with a canvas…</option>
        <option v-for="entity in withCanvas" :key="entity.id" :value="entity.id">{{ entity.name }}</option>
      </select>
      <small v-if="!withCanvas.length" class="field-sublabel">Give an entity the <strong>Canvas</strong> trait and draw on its Canvas tab first.</small>
    </div>

    <template v-if="doc">
      <div class="form-group">
        <label class="field-label">Changes, in order</label>
        <ol class="mc-ops">
          <li v-for="(op, index) in config.ops" :key="index">
            <span>{{ opLabel(op) }}</span>
            <button class="btn btn-small btn-secondary" :aria-label="`Remove change ${index + 1}`" @click="removeOp(index)">✕</button>
          </li>
          <li v-if="!config.ops.length" class="field-sublabel">No changes yet. Click an element on the canvas or pick one below.</li>
        </ol>
        <div class="mc-add">
          <select v-model="pendingShape" class="form-select" aria-label="Canvas element">
            <option value="">Element…</option>
            <option v-for="shape in elements" :key="shape.id" :value="shape.id">{{ shapeDisplayName(shape, refName) }} ({{ shape.kind }})</option>
          </select>
          <select v-model="pendingAction" class="form-select" aria-label="Change" :disabled="!pendingActions.length">
            <option value="">Change…</option>
            <option v-for="entry in pendingActions" :key="entry.action" :value="entry.action">{{ entry.label }}</option>
          </select>
          <button class="btn btn-secondary" :disabled="!pendingShape || !pendingAction" @click="addOp">Add</button>
          <select v-if="pendingAction === 'set_icon'" v-model="pendingIcon" class="form-select mc-icon" aria-label="New icon">
            <option v-for="icon in MARKER_ICONS" :key="icon.id" :value="icon.id">{{ icon.glyph }} {{ icon.label }}</option>
          </select>
        </div>
      </div>

      <div class="form-group">
        <div class="mc-preview-head">
          <label class="field-label">Preview</label>
          <div class="mc-seg">
            <button :class="preview === 'before' && 'is-active'" @click="preview = 'before'">Before</button>
            <button :class="preview === 'after' && 'is-active'" @click="preview = 'after'">After</button>
          </div>
        </div>
        <svg class="mc-preview" :viewBox="viewBox" preserveAspectRatio="xMidYMid meet" aria-label="Canvas preview">
          <template v-for="shape in shown?.shapes ?? []" :key="shape.id">
            <g v-if="doc.layers.find((layer) => layer.id === shape.layer)?.visible !== false" class="mc-shape"
              @click="isGameplay(shape) && quickAdd(shape.id, actionsFor(shape.kind)[0]!.action)">
              <title v-if="isGameplay(shape)">Click: {{ actionsFor(shape.kind)[0]!.label }}</title>
              <CanvasShapeView :shape="shape" :runtime="true" :uid="uid" />
            </g>
          </template>
        </svg>
      </div>
    </template>
  </div>
</template>

<style scoped>
.mc-root { display: flex; flex-direction: column; gap: 10px; }
.mc-ops { margin: 4px 0; padding-left: 18px; display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
.mc-ops li { display: flex; justify-content: space-between; align-items: center; gap: 6px; }
.mc-add { display: grid; grid-template-columns: 1fr 1fr auto; gap: 4px; }
.mc-icon { grid-column: 1 / -1; }
.mc-preview-head { display: flex; justify-content: space-between; align-items: center; }
.mc-seg { display: flex; gap: 2px; }
.mc-seg button { padding: 2px 8px; font-size: 12px; border: 1px solid var(--border, #333); background: transparent; color: var(--text, #eee); border-radius: 6px; cursor: pointer; }
.mc-seg button.is-active { border-color: var(--accent, #0a84ff); background: color-mix(in srgb, var(--accent, #0a84ff) 20%, transparent); }
.mc-preview { width: 100%; height: 220px; border: 1px solid var(--border, #333); border-radius: 8px; background: var(--surface-1, #111); }
.mc-shape { cursor: pointer; }
</style>
