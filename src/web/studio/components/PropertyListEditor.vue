<script setup lang="ts">
import { computed, ref, nextTick, onBeforeUnmount, toRaw } from "vue";
import type { EntityProperty, PropertyDefinition, PropertyValue } from "../contracts.ts";
import {
  isValidPropertyKey,
  newEntityProperty,
  newPropertyDefinition,
  PROPERTY_TYPES,
  retypeValue,
  toggleArray,
} from "../propertyModel.ts";
import PropertyValueInput from "./PropertyValueInput.vue";
import { propertyEditorFor } from "../plugins/runtime.ts";
import StudioIcon from "./StudioIcon.vue";
import { reorderProperties } from "../traitPropertyGroups.ts";

type Row = EntityProperty | PropertyDefinition;

const props = defineProps<{
  modelValue: Row[];
  mode: "instance" | "definition";
  disabled?: boolean;
  categories?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: Row[]] }>();

const rows = computed(() => props.modelValue);
const valueKey = computed(() => (props.mode === "instance" ? "value" : "default"));

function valueOf(row: Row): PropertyValue {
  // Entity rows from the server carry `value`; template edits are staged under
  // `default` (which the backend maps onto `value`). Prefer whichever key the
  // row actually defines so saved values survive a reload in either shape.
  const asEntity = row as Partial<EntityProperty>;
  const asDef = row as Partial<PropertyDefinition>;
  if (props.mode === "instance") {
    return asEntity.value !== undefined ? asEntity.value : (asDef.default ?? null);
  }
  return asDef.default !== undefined ? asDef.default : (asEntity.value ?? null);
}

function duplicateKeys(): Set<string> {
  const seen = new Set<string>();
  const duplicates = new Set<string>();
  for (const row of rows.value) {
    const key = row.key.trim();
    if (!key) continue;
    if (seen.has(key)) duplicates.add(key);
    seen.add(key);
  }
  return duplicates;
}

const duplicates = computed(duplicateKeys);

function rowProblem(row: Row): string | null {
  const key = row.key.trim();
  if (!isValidPropertyKey(key)) return "Key must be an identifier";
  if (duplicates.value.has(key)) return "Key is duplicated";
  if (!row.name.trim()) return "Name is required";
  return null;
}

function patch(index: number, changes: Partial<Row>) {
  const next = rows.value.map((row, position) => {
    if (position !== index) return row;
    const updated = { ...row, ...changes } as Row & Record<string, unknown>;
    if (!row.id) rowKeys.set(toRaw(updated), rowKey(row) as number);
    // Keep a single canonical value key per mode: the backend rejects
    // payloads that carry both `value` and its `default` alias.
    if (props.mode === "definition") delete updated.value;
    else delete updated.default;
    return updated as Row;
  });
  emit("update:modelValue", next);
}

function patchValue(index: number, value: PropertyValue) {
  const row = rows.value[index];
  if (!row) return;
  patch(index, { [valueKey.value]: value } as Partial<Row>);
}

function changeType(index: number, event: Event) {
  const type = (event.target as HTMLSelectElement).value as Row["type"];
  const row = rows.value[index];
  if (!row) return;
  const isArray = row.is_array ?? false;
  patch(index, {
    type,
    [valueKey.value]: retypeValue(type, isArray, valueOf(row)),
  } as Partial<Row>);
}

function setArray(index: number, isArray: boolean) {
  const row = rows.value[index];
  if (!row) return;
  patch(index, {
    is_array: isArray,
    [valueKey.value]: toggleArray(valueOf(row), isArray, row.type),
  } as Partial<Row>);
}

function changeArray(index: number, event: Event) {
  setArray(index, (event.target as HTMLInputElement).checked);
}

function toggleArrayValue(index: number) {
  const row = rows.value[index];
  if (!row) return;
  setArray(index, !(row.is_array === true));
}

function toggleMultiline(index: number) {
  const row = rows.value[index];
  if (!row) return;
  patch(index, { multiline: !(row.multiline === true) });
}
const dragIndex = ref<number | null>(null);
const pointerReordering = ref(false);
let resetMotion: ReturnType<typeof setTimeout> | undefined;
const rowKeys = new WeakMap<object, number>();
let nextRowKey = 0;
function rowKey(row: Row): string | number {
  if (row.id) return row.id;
  const raw = toRaw(row);
  if (!rowKeys.has(raw)) rowKeys.set(raw, ++nextRowKey);
  return rowKeys.get(raw)!;
}
onBeforeUnmount(() => clearTimeout(resetMotion));
function move(from: number, to: number) {
  if (props.disabled || from < 0 || to < 0 || from >= rows.value.length || to >= rows.value.length) return;
  emit("update:modelValue", reorderProperties(rows.value, from, to));
}
function onDragStart(event: DragEvent, index: number) {
  const handle = event.currentTarget as HTMLElement;
  const row = handle.closest<HTMLElement>(".property-list__row");
  if (event.dataTransfer && row) {
    const preview = row.cloneNode(true) as HTMLElement;
    const originalFields = row.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("input, select, textarea");
    preview.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>("input, select, textarea").forEach((field, i) => {
      const original = originalFields[i];
      if (original) field.value = original.value;
    });
    const bounds = row.getBoundingClientRect();
    Object.assign(preview.style, { position: "fixed", left: "-10000px", top: "0", width: `${bounds.width}px`, boxSizing: "border-box", opacity: "1", pointerEvents: "none", background: "var(--bg-elevated)", zIndex: "9999" });
    preview.setAttribute("aria-hidden", "true");
    row.parentElement?.appendChild(preview);
    event.dataTransfer.setDragImage(preview, Math.min(event.clientX - bounds.left, bounds.width), event.clientY - bounds.top);
    requestAnimationFrame(() => preview.remove());
  }
  clearTimeout(resetMotion);
  pointerReordering.value = true;
  dragIndex.value = index;
  if (event.dataTransfer) { event.dataTransfer.effectAllowed = "move"; event.dataTransfer.setData("text/plain", String(index)); }
}
function onDragEnd() {
  dragIndex.value = null;
  resetMotion = setTimeout(() => { pointerReordering.value = false; }, 200);
}
function onDrop(event: DragEvent, index: number) {
  event.preventDefault();
  if (dragIndex.value !== null) move(dragIndex.value, index);
  dragIndex.value = null;
}
function onHandleKey(event: KeyboardEvent, index: number) {
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
  event.preventDefault();
  const target = index + (event.key === "ArrowUp" ? -1 : 1);
  if (target < 0 || target >= rows.value.length) return;
  const handle = event.currentTarget as HTMLElement;
  move(index, target);
  void nextTick(() => handle.focus());
}

function remove(index: number) {
  emit(
    "update:modelValue",
    rows.value.filter((_row, position) => position !== index),
  );
}

function add() {
  const next =
    props.mode === "instance"
      ? newEntityProperty(rows.value as EntityProperty[])
      : newPropertyDefinition(rows.value as PropertyDefinition[]);
  emit("update:modelValue", [...rows.value, next] as Row[]);
}
</script>

<template>
  <div class="property-list" :class="{ 'has-categories': categories, 'is-pointer-reordering': pointerReordering }">
    <!-- Column heads only label rows; an empty list shows just its empty state, so it never scrolls sideways. -->
    <div v-if="rows.length" class="property-list__head">
      <span></span>
      <span>Name</span>
      <span>Key</span>
      <span v-if="categories">Category</span>
      <span>Type</span>
      <span>Array</span>
      <span>{{ props.mode === "instance" ? "Value" : "Default" }}</span>
      <span style="text-align:right;">Actions</span>
    </div>

    <div v-if="!rows.length" class="property-list__empty" style="display:flex; flex-direction:column; align-items:center; gap:12px; padding:28px;">
      <span>No typed properties defined yet.</span>
      <button v-if="!props.disabled" class="btn btn-primary" type="button" @click="add"><StudioIcon name="plus" /> Add property</button>
    </div>

    <TransitionGroup name="property-reorder" tag="div">
    <div v-for="(row, index) in rows" :key="rowKey(row)" class="property-list__row" :class="{ 'is-dragging': dragIndex === index }" @dragover.prevent @drop="onDrop($event, index)">
      <button class="property-drag" type="button" :draggable="!props.disabled" :disabled="props.disabled" :aria-label="`Reorder ${row.name || row.key || 'property'}; use arrow keys or drag`" title="Drag to reorder; use arrow keys to move" @dragstart="onDragStart($event, index)" @dragend="onDragEnd" @keydown="onHandleKey($event, index)"><svg viewBox="0 0 12 16" aria-hidden="true"><circle cx="4" cy="3" r="1"/><circle cx="8" cy="3" r="1"/><circle cx="4" cy="8" r="1"/><circle cx="8" cy="8" r="1"/><circle cx="4" cy="13" r="1"/><circle cx="8" cy="13" r="1"/></svg></button>
      <div class="property-list__cell property-list__cell--name">
        <span class="property-type-dot" :class="`type-${row.type}`" :title="`Type: ${row.type}`"></span>
        <input
          class="form-input property-name-input"
          :value="row.name"
          placeholder="Property name"
          :aria-label="`Name for ${row.name || row.key || 'property'}`"
          :disabled="props.disabled"
          @input="patch(index, { name: ($event.target as HTMLInputElement).value })"
        />
      </div>
      <div class="property-list__cell property-list__cell--key">
        <input
          class="form-input property-key-input"
          :class="{ 'is-invalid': rowProblem(row) }"
          :value="row.key"
          placeholder="key_name"
          :aria-label="`Key for ${row.name || row.key || 'property'}`"
          :disabled="props.disabled"
          @input="patch(index, { key: ($event.target as HTMLInputElement).value })"
        />
        <span v-if="rowProblem(row)" class="property-list__error">{{ rowProblem(row) }}</span>
      </div>
      <div v-if="categories" class="property-list__cell"><input class="form-input" :value="(row as any).category ?? ''" :aria-label="`Category for ${row.name || row.key || 'property'}`" placeholder="Uncategorized" :disabled="props.disabled" @input="patch(index, { category: ($event.target as HTMLInputElement).value } as Partial<Row>)" /></div>
      <div class="property-list__cell property-list__cell--type">
        <select class="form-select property-type-select" :aria-label="`Type for ${row.name || row.key || 'property'}`" :value="row.type" :disabled="props.disabled" @change="changeType(index, $event)">
          <option v-for="option in PROPERTY_TYPES" :key="option.value" :value="option.value">{{ option.label }}</option>
        </select>
      </div>
      <div class="property-list__cell property-list__cell--array">
        <button
          type="button"
          class="array-switch"
          role="switch"
          :aria-checked="row.is_array === true"
          :aria-label="`Multiple values for ${row.name || row.key || 'property'}`"
          :title="row.is_array ? 'Multiple values on' : 'Multiple values off'"
          :disabled="props.disabled"
          @click="toggleArrayValue(index)"
        >
          <svg class="array-switch__icon" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M1.5 2.5h9M1.5 6h9M1.5 9.5h9"/></svg>
          <span class="array-switch__state">{{ row.is_array ? 'On' : 'Off' }}</span>
        </button>
      </div>
      <div class="property-list__cell property-list__cell--value">
        <component
          :is="propertyEditorFor(row)!.component"
          v-if="propertyEditorFor(row)"
          :property="row"
          :model-value="valueOf(row)"
          :disabled="props.disabled"
          @update:model-value="patchValue(index, $event)"
        />
        <PropertyValueInput
          v-else
          :key="`${row.id}-${row.type}-${row.is_array}-${row.multiline}`"
          :model-value="valueOf(row)"
          :type="row.type"
          :is-array="row.is_array === true"
          :multiline="row.multiline === true"
          :disabled="props.disabled"
          @update:model-value="patchValue(index, $event)"
        />
      </div>
      <div class="property-list__actions">
        <button v-if="row.type === 'string'" class="btn-micro" type="button" title="Multiline text" :aria-label="`Multiline text for ${row.name || row.key || 'property'}`" :aria-pressed="row.multiline === true" :class="{ 'is-active': row.multiline === true }" :disabled="props.disabled" @click="toggleMultiline(index)">Aa</button>
        <button class="btn-micro danger" type="button" title="Delete property" aria-label="Delete property" :disabled="props.disabled" @click="remove(index)"><StudioIcon name="close" /></button>
      </div>
    </div>
    </TransitionGroup>

    <div v-if="rows.length" class="property-list__footer">
      <button v-if="!props.disabled" class="btn btn-secondary property-add-btn" type="button" @click="add">
        <StudioIcon name="plus" /> Add property
      </button>
    </div>
  </div>
</template>

<style scoped>
.property-list {
  background: rgba(0, 0, 0, 0.18);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md, 9px);
  /* The table keeps its columns and scrolls sideways inside narrow panes. */
  overflow-x: auto;
  overflow-y: hidden;
}

.property-list__head {
  display: grid;
  grid-template-columns: 24px 1.2fr 1fr 115px 84px 1.2fr 64px;
  gap: 8px;
  min-width: 620px;
  padding: 8px 12px;
  background: rgba(255, 255, 255, 0.03);
  border-bottom: 1px solid var(--border-subtle);
  font-size: 11px;
  font-weight: 600;
  color: var(--text-muted);
  line-height: 1.2;
}

.property-list__empty {
  padding: 24px;
  text-align: center;
  color: var(--text-dim);
  font-size: 12px;
}

.property-list__row {
  display: grid;
  grid-template-columns: 24px 1.2fr 1fr 115px 84px 1.2fr 64px;
  gap: 8px;
  min-width: 620px;
  align-items: center;
  padding: 6px 12px;
  border-bottom: 1px solid var(--border-subtle);
  transition: background-color 0.12s ease-out;
}
.has-categories .property-list__head, .has-categories .property-list__row { grid-template-columns: 24px 1.2fr 1fr 1fr 115px 84px 1.2fr 64px; min-width: 820px; }
.property-drag { width: 24px; height: 32px; padding: 5px; border: 0; border-radius: 4px; background: transparent; color: var(--text-muted); cursor: grab; }
.property-drag:active { cursor: grabbing; }
.property-drag:hover, .property-drag:focus-visible { color: var(--text-main); background: var(--bg-badge); }
.property-drag:disabled { opacity: .35; cursor: default; }
.property-drag svg { width: 12px; height: 16px; fill: currentColor; }
.property-list__row.is-dragging { opacity: .5; }
.is-pointer-reordering .property-reorder-move { transition: transform 180ms ease-in-out; }
@media (prefers-reduced-motion: reduce) { .is-pointer-reordering .property-reorder-move { transition: none; } }

.property-list__row:hover {
  background: var(--bg-card-hover, rgba(255, 255, 255, 0.02));
}

.property-list__cell--name {
  display: flex;
  align-items: center;
  gap: 8px;
}

.property-type-dot {
  width: 7px;
  height: 7px;
  border-radius: 9999px;
  flex-shrink: 0;
}
.property-type-dot.type-int,
.property-type-dot.type-float { background-color: var(--status-success, #10b981); }
.property-type-dot.type-string { background-color: var(--tertiary, #ffb95f); }
.property-type-dot.type-bool { background-color: var(--secondary, #bdc2ff); }
.property-type-dot.type-entity { background-color: var(--accent, #06b6d4); }
.property-type-dot.type-asset { background-color: var(--status-danger, #f43f5e); }
.property-type-dot.type-json { background-color: #a855f7; }

.property-key-input {
  font-family: var(--font-mono);
  color: var(--accent);
  font-size: 12px;
}

.property-type-select {
  font-size: 11px;
  padding: 4px 6px;
  height: 28px;
  background: var(--bg-badge, #212735);
  border-color: var(--border-subtle);
  border-radius: var(--radius-sm, 4px);
}

.property-list__cell--array {
  display: flex;
  align-items: center;
  justify-content: center;
}

.array-switch {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 28px;
  padding: 2px 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm, 4px);
  background: transparent;
  color: var(--text-muted);
  font-size: 11px;
  cursor: pointer;
  transition: background-color 0.12s ease-out, border-color 0.12s ease-out, color 0.12s ease-out;
}
.array-switch:hover:not(:disabled) {
  background: var(--bg-badge);
  color: var(--text-main);
  border-color: var(--border-muted);
}
.array-switch[aria-checked="true"] {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-subtle);
}
.array-switch:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.array-switch__icon {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
}
.array-switch__state {
  font-size: 11px;
  font-weight: 600;
}

.property-list__actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.btn-micro {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs, 3px);
  background: transparent;
  color: var(--text-muted);
  font-size: 10px;
  font-weight: 700;
  cursor: pointer;
  transition: background-color 0.12s ease-out, color 0.12s ease-out, border-color 0.12s ease-out;
}
.btn-micro__icon {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
}
.btn-micro.is-active {
  border-color: var(--accent);
  color: var(--accent);
  background: var(--accent-subtle);
}

.btn-micro:hover:not(:disabled) {
  background: var(--bg-badge);
  color: var(--text-main);
  border-color: var(--border-muted);
}

.btn-micro.danger:hover:not(:disabled) {
  background: var(--status-danger-subtle);
  color: var(--status-danger);
  border-color: var(--status-danger);
}

.btn-micro:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}

.property-list__footer {
  padding: 8px 12px;
  background: var(--surface-container-low, #1a1c20);
}

.property-add-btn {
  font-size: 12px;
  padding: 4px 10px;
  gap: 6px;
}


.property-list__error {
  display: block;
  font-size: 10px;
  color: var(--status-danger);
  margin-top: 2px;
}
</style>
