<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { AssetLinkValue, PropertyScalar, PropertyType, PropertyValue } from "../contracts.ts";
import { defaultPropertyValue } from "../propertyModel.ts";
import { ENTITY_TYPES } from "../entityModel.ts";
import { studioState } from "../state.ts";
import AssetLinkInput from "./AssetLinkInput.vue";

const props = defineProps<{
  modelValue: PropertyValue;
  type: PropertyType;
  isArray?: boolean;
  multiline?: boolean;
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: PropertyValue] }>();

// Local mirror so two clicks inside one render tick compose instead of both
// reading the stale prop (Vue applies the parent update on the next tick).
const local = ref<PropertyValue>(props.modelValue);
watch(
  () => [props.modelValue, props.type, props.isArray],
  () => {
    local.value = props.modelValue;
  },
);

const isArray = computed(() => props.isArray ?? false);

const entitiesByType = computed(() =>
  ENTITY_TYPES.map((definition) => ({
    type: definition.value,
    label: definition.label,
    entities: studioState.entities.filter((entity) => entity.type === definition.value),
  })).filter((group) => group.entities.length > 0),
);

function commit(value: PropertyValue) {
  local.value = value;
  emit("update:modelValue", value);
}

function emitScalar(value: PropertyScalar) {
  emit("update:modelValue", value);
}
function emitArray(index: number, value: PropertyScalar) {
  const next = Array.isArray(local.value) ? [...local.value] : [];
  next[index] = value;
  commit(next);
}

function defaultScalar(type: PropertyType): PropertyScalar {
  const value = defaultPropertyValue(type, false);
  return Array.isArray(value) ? null : value;
}

function addElement() {
  const next = Array.isArray(local.value) ? [...local.value] : [];
  next.push(defaultScalar(props.type));
  commit(next);
}

function removeElement(index: number) {
  const next = Array.isArray(local.value) ? [...local.value] : [];
  next.splice(index, 1);
  commit(next);
}

function textValue(value: PropertyScalar): string {
  if (typeof value === "string") return value;
  if (typeof value === "number") return String(value);
  return "";
}
function onNumber(event: Event, write: (value: number) => void, integer: boolean) {
  const raw = (event.target as HTMLInputElement).value.trim();
  const parsed = Number(raw);
  if (!Number.isFinite(parsed)) return; // Partial input such as "-" or "1e"
  write(integer ? Math.trunc(parsed) : parsed);
}

function onText(event: Event, write: (value: string) => void) {
  write((event.target as HTMLTextAreaElement).value);
}

function onEntity(event: Event, write: (value: string | null) => void) {
  const value = (event.target as HTMLSelectElement).value;
  write(value ? value : null);
}

function elementAt(index: number): PropertyScalar {
  return Array.isArray(local.value) ? local.value[index] ?? null : null;
}
</script>

<template>
  <div v-if="!isArray" class="property-value">
    <input
      v-if="props.type === 'bool'"
      key="scalar-bool"
      type="checkbox"
      :checked="props.modelValue === true"
      :disabled="props.disabled"
      @change="emitScalar(($event.target as HTMLInputElement).checked)"
    />
    <textarea
      v-else-if="props.type === 'string' && props.multiline"
      key="scalar-string-multiline"
      class="form-textarea"
      style="min-height:70px;"
      :value="textValue(props.modelValue as PropertyScalar)"
      :disabled="props.disabled"
      @input="onText($event, emitScalar)"
    ></textarea>
    <input
      v-else-if="props.type === 'string'"
      key="scalar-string"
      class="form-input"
      :value="textValue(props.modelValue as PropertyScalar)"
      :disabled="props.disabled"
      @input="onText($event, emitScalar)"
    />
    <input
      v-else-if="props.type === 'float'"
      key="scalar-float"
      class="form-input"
      type="number"
      step="any"
      :value="textValue(props.modelValue as PropertyScalar)"
      :disabled="props.disabled"
      @input="onNumber($event, emitScalar, false)"
    />
    <input
      v-else-if="props.type === 'int'"
      key="scalar-int"
      class="form-input"
      type="number"
      step="1"
      :value="textValue(props.modelValue as PropertyScalar)"
      :disabled="props.disabled"
      @input="onNumber($event, emitScalar, true)"
    />
    <AssetLinkInput
      v-else-if="props.type === 'asset'"
      key="scalar-asset"
      :model-value="(props.modelValue as AssetLinkValue | null)"
      @update:model-value="emitScalar"
    />
    <select
      v-else-if="props.type === 'entity'"
      key="scalar-entity"
      class="form-select"
      :value="textValue(props.modelValue as PropertyScalar)"
      :disabled="props.disabled"
      @change="onEntity($event, emitScalar)"
    >
      <option value="">(none)</option>
      <optgroup
        v-for="group in entitiesByType"
        :key="group.type"
        :label="group.label"
      >
        <option v-for="entity in group.entities" :key="entity.id" :value="entity.id">
          {{ entity.name }}
        </option>
      </optgroup>
    </select>
  </div>

  <div v-else class="property-array">
    <div v-for="(_element, index) in (local as PropertyScalar[])" :key="index" class="property-array__row">
      <input
        v-if="props.type === 'bool'"
        :key="'array-bool-' + index"
        type="checkbox"
        :checked="elementAt(index) === true"
        :disabled="props.disabled"
        @change="emitArray(index, ($event.target as HTMLInputElement).checked)"
      />
      <textarea
        v-else-if="props.type === 'string' && props.multiline"
        :key="'array-string-multiline-' + index"
        class="form-textarea"
        :value="textValue(elementAt(index))"
        :disabled="props.disabled"
        @input="onText($event, (value) => emitArray(index, value))"
      ></textarea>
      <input
        v-else-if="props.type === 'string'"
        :key="'array-string-' + index"
        class="form-input"
        :value="textValue(elementAt(index))"
        :disabled="props.disabled"
        @input="onText($event, (value) => emitArray(index, value))"
      />
      <input
        v-else-if="props.type === 'float' || props.type === 'int'"
        :key="'array-num-' + index"
        class="form-input"
        type="number"
        :step="props.type === 'int' ? '1' : 'any'"
        :value="textValue(elementAt(index))"
        :disabled="props.disabled"
        @input="onNumber($event, (value) => emitArray(index, value), props.type === 'int')"
      />
      <AssetLinkInput
        v-else-if="props.type === 'asset'"
        :key="'array-asset-' + index"
        :model-value="(elementAt(index) as AssetLinkValue | null)"
        @update:model-value="(value) => emitArray(index, value)"
      />
      <select
        v-else-if="props.type === 'entity'"
        :key="'array-entity-' + index"
        class="form-select"
        :value="textValue(elementAt(index))"
        :disabled="props.disabled"
        @change="onEntity($event, (value) => emitArray(index, value))"
      >
        <option value="">(none)</option>
        <optgroup v-for="group in entitiesByType" :key="group.type" :label="group.label">
          <option v-for="entity in group.entities" :key="entity.id" :value="entity.id">{{ entity.name }}</option>
        </optgroup>
      </select>
      <button class="btn btn-secondary" type="button" :disabled="props.disabled" @click="removeElement(Number(index))">✕</button>
    </div>
    <button class="btn btn-secondary" type="button" :disabled="props.disabled" @click="addElement">+ Add element</button>
  </div>
</template>
