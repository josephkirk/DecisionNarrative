<script setup lang="ts">
import { computed } from "vue";
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

type Row = EntityProperty | PropertyDefinition;

const props = defineProps<{
  modelValue: Row[];
  mode: "instance" | "definition";
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: Row[]] }>();

const rows = computed(() => props.modelValue);
const valueKey = computed(() => (props.mode === "instance" ? "value" : "default"));

function valueOf(row: Row): PropertyValue {
  return props.mode === "instance"
    ? ((row as EntityProperty).value ?? null)
    : ((row as PropertyDefinition).default ?? null);
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
  const next = rows.value.map((row, position) =>
    position === index ? ({ ...row, ...changes } as Row) : row,
  );
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

function changeArray(index: number, event: Event) {
  const isArray = (event.target as HTMLInputElement).checked;
  const row = rows.value[index];
  if (!row) return;
  patch(index, {
    is_array: isArray,
    [valueKey.value]: toggleArray(valueOf(row), isArray, row.type),
  } as Partial<Row>);
}

function move(index: number, delta: number) {
  const target = index + delta;
  if (target < 0 || target >= rows.value.length) return;
  const next = [...rows.value];
  const [row] = next.splice(index, 1);
  next.splice(target, 0, row);
  emit("update:modelValue", next);
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
  <div class="property-list">
    <div class="property-list__head">
      <span>Name</span>
      <span>Key</span>
      <span>Type</span>
      <span>Array</span>
      <span>{{ props.mode === "instance" ? "Value" : "Default" }}</span>
      <span></span>
    </div>

    <div v-if="!rows.length" class="property-list__empty">No properties yet.</div>

    <div v-for="(row, index) in rows" :key="`${row.id || index}-${row.type}-${row.is_array}-${row.multiline}`" class="property-list__row">
      <div class="property-list__cell">
        <input
          class="form-input"
          :value="row.name"
          :disabled="props.disabled"
          @input="patch(index, { name: ($event.target as HTMLInputElement).value })"
        />
      </div>
      <div class="property-list__cell">
        <input
          class="form-input"
          :class="{ 'is-invalid': rowProblem(row) }"
          :value="row.key"
          :disabled="props.disabled"
          @input="patch(index, { key: ($event.target as HTMLInputElement).value })"
        />
        <span v-if="rowProblem(row)" class="property-list__error">{{ rowProblem(row) }}</span>
        <label v-if="row.type === 'string'" class="property-list__inline">
          <input
            type="checkbox"
            :checked="row.multiline === true"
            :disabled="props.disabled"
            @change="patch(index, { multiline: ($event.target as HTMLInputElement).checked })"
          />
          Multiline
        </label>
      </div>
      <div class="property-list__cell">
        <select class="form-select" :value="row.type" :disabled="props.disabled" @change="changeType(index, $event)">
          <option v-for="option in PROPERTY_TYPES" :key="option.value" :value="option.value">{{ option.label }}</option>
        </select>
      </div>
      <div class="property-list__cell">
        <input
          type="checkbox"
          :checked="row.is_array === true"
          :disabled="props.disabled"
          @change="changeArray(index, $event)"
        />
      </div>
      <div class="property-list__cell">
        <PropertyValueInput
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
        <button class="btn btn-secondary" type="button" :disabled="props.disabled || index === 0" @click="move(index, -1)">↑</button>
        <button class="btn btn-secondary" type="button" :disabled="props.disabled || index === rows.length - 1" @click="move(index, 1)">↓</button>
        <button class="btn btn-secondary" type="button" :disabled="props.disabled" @click="remove(index)">✕</button>
      </div>
    </div>

    <button class="btn btn-secondary" type="button" :disabled="props.disabled" @click="add">+ Add Property</button>
  </div>
</template>
