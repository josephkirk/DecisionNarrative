<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { AssetLinkValue, EntityRecord, EntityTemplateRecord, PropertyScalar, PropertyType, PropertyValue } from "../contracts.ts";
import { defaultPropertyValue } from "../propertyModel.ts";
import { traitLabels, hasTraitKey } from "../entityModel.ts";
import { studioState } from "../state.ts";
import ReferencePicker, { type PickerItem } from "./ReferencePicker.vue";
import TagEditor from "./TagEditor.vue";
const props = defineProps<{
  modelValue: PropertyValue;
  type: PropertyType;
  isArray?: boolean;
  multiline?: boolean;
  disabled?: boolean;
  tagOptions?: string[];
  selectionOnly?: boolean;
  traitKeyFilter?: string;
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

const entitiesByType = computed(() => {
  const entities = [...studioState.entities, ...studioState.entityTemplates];
  return [{ type: "entities", label: "Entities", entities: entities.filter((entity) =>
    !props.traitKeyFilter || hasTraitKey(entity, studioState.traitTemplates, props.traitKeyFilter),
  ) }];
});

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
function stepNumber(current: PropertyScalar, delta: number, integer: boolean): number {
  const base = typeof current === "number" && Number.isFinite(current) ? current : 0;
  const next = base + delta;
  return integer ? Math.trunc(next) : next;
}

let repeatDelay: ReturnType<typeof setTimeout> | null = null;
let repeatInterval: ReturnType<typeof setInterval> | null = null;
function stopRepeat() {
  if (repeatDelay !== null) { clearTimeout(repeatDelay); repeatDelay = null; }
  if (repeatInterval !== null) { clearInterval(repeatInterval); repeatInterval = null; }
}
onBeforeUnmount(stopRepeat);
function beginRepeat(getCurrent: () => PropertyScalar, write: (value: number) => void, delta: number, integer: boolean) {
  if (props.disabled) return;
  stopRepeat();
  write(stepNumber(getCurrent(), delta, integer));
  repeatDelay = setTimeout(() => {
    repeatInterval = setInterval(() => write(stepNumber(getCurrent(), delta, integer)), 70);
  }, 380);
}
function onNumKey(event: KeyboardEvent, write: (value: number) => void, integer: boolean) {
  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
  event.preventDefault();
  const parsed = Number((event.target as HTMLInputElement).value.trim());
  const base = Number.isFinite(parsed) ? parsed : 0;
  const delta = event.key === "ArrowUp" ? 1 : -1;
  write(integer ? Math.trunc(base + delta) : base + delta);
}

// Tag properties use the same multi-chip editor as entity Gameplay tags.
// Scalar slots wrap the single value so chips, `#` prefix, remove buttons
// and the suggestion popup match the rest of the studio.
function tagAsArray(value: PropertyScalar): string[] {
  return typeof value === "string" && value ? [value] : [];
}
function tagFromArray(value: string | string[]): string {
  const list = Array.isArray(value) ? value : [value];
  return list.length ? list[list.length - 1]! : "";
}
function elementAt(index: number): PropertyScalar {
  return Array.isArray(local.value) ? local.value[index] ?? null : null;
}

// Tag arrays are edited as one multi-chip field (like Gameplay tags):
// each chip carries its own remove control, so no per-element inputs
// or remove buttons are needed.
const tagArrayValue = computed((): string[] => {
  if (!Array.isArray(local.value)) return [];
  return local.value.filter((item): item is string => typeof item === "string" && item.length > 0);
});
function commitTagArray(next: string | string[]) {
  const list = Array.isArray(next) ? next.filter((item) => typeof item === "string" && item.length > 0) : [];
  commit(list);
}
function onText(event: Event, write: (value: string) => void) {
  write((event.target as HTMLTextAreaElement).value);
}

function onEntity(event: Event, write: (value: string | null) => void) {
  const value = (event.target as HTMLSelectElement).value;
  write(value ? value : null);
}

// Generic link picker (assets, entities, storylets)
const pickerOpen = ref(false);
const pickerTarget = ref<number | null>(null);

const pickerKind = computed(() => props.type);
const pickerTitle = computed(() =>
  props.type === "asset" ? "Choose asset" : props.type === "storylet" ? "Choose storylet" : "Choose entity",
);
const linkItems = computed<PickerItem[]>(() => {
  if (props.type === "asset") {
    return studioState.assets.map((asset) => ({
      id: asset.id,
      title: asset.name,
      subtitle: asset.versions.length ? `${asset.versions.length} version${asset.versions.length === 1 ? "" : "s"}` : undefined,
      thumbnail: asset.thumbnail ?? null,
    }));
  }
  if (props.type === "storylet") {
    return studioState.storylets.map((storylet) => ({
      id: storylet.id,
      title: storylet.title,
      subtitle: storylet.ending ? "Ending" : `v${storylet.revision}`,
      thumbnail: storylet.thumbnail ?? null,
    }));
  }
  return entitiesByType.value.flatMap((group) =>
    group.entities.map((entity) => ({
      id: entity.id,
      title: entity.name,
      subtitle: traitLabels(entity, studioState.traitTemplates).join(" · ") || group.label,
    })),
  );
});
const pickerSelectedId = computed(() => {
  const raw = pickerTarget.value === null ? props.modelValue : elementAt(pickerTarget.value);
  if (props.type === "asset") {
    const link = raw as AssetLinkValue | null;
    return link && typeof link === "object" ? link.asset_id ?? null : null;
  }
  return typeof raw === "string" ? raw : null;
});
const gameplayTagOptions = computed(() => studioState.gameplayTags.map((tag) => tag.name));
const pickerAnchor = ref<HTMLElement | null>(null);
function openPicker(target: number | null, event: MouseEvent) {
  pickerAnchor.value = event.currentTarget as HTMLElement;
  pickerTarget.value = target;
  pickerOpen.value = true;
}
function closePicker() {
  pickerOpen.value = false;
  pickerTarget.value = null;
}
function applyPickerSelection(id: string) {
  const target = pickerTarget.value;
  if (props.type === "asset") {
    const link = { asset_id: id };
    if (target === null) emit("update:modelValue", link);
    else emitArray(target, link);
  } else if (target === null) {
    emit("update:modelValue", id);
  } else {
    emitArray(target, id);
  }
  closePicker();
}
function onAssetSelect(event: Event, write: (value: AssetLinkValue | null) => void) {
  const assetId = (event.target as HTMLSelectElement).value;
  write(assetId ? { asset_id: assetId } : null);
}
</script>

<template>
  <div v-if="!isArray" class="property-value">
    <button
      v-if="props.type === 'bool'"
      key="scalar-bool"
      type="button"
      class="bool-switch"
      role="switch"
      :aria-checked="props.modelValue === true"
      aria-label="Boolean value"
      :disabled="props.disabled"
      @click="emitScalar(!(props.modelValue === true))"
    >
      <span class="bool-switch__track" aria-hidden="true"><span class="bool-switch__knob"></span></span>
      <span class="bool-switch__state">{{ props.modelValue === true ? 'True' : 'False' }}</span>
    </button>
    <textarea
      v-else-if="props.type === 'string' && props.multiline"
      key="scalar-string-multiline"
      class="form-textarea"
      style="min-height:70px;"
      :value="textValue(props.modelValue as PropertyScalar)"
      :disabled="props.disabled"
      @input="onText($event, emitScalar)"
    ></textarea>
    <select v-else-if="props.type === 'string' && props.selectionOnly" class="form-select" aria-label="Value" :value="textValue(props.modelValue as PropertyScalar)" :disabled="props.disabled" @change="emitScalar(($event.target as HTMLSelectElement).value)"><option value="">(none)</option><option v-for="option in (props.tagOptions ?? [])" :key="option" :value="option">{{ option }}</option></select>
    <input
      v-else-if="props.type === 'string'"
      key="scalar-string"
      class="form-input"
      :value="textValue(props.modelValue as PropertyScalar)"
      :disabled="props.disabled"
      @input="onText($event, emitScalar)"
    />
    <div
      v-else-if="props.type === 'float' || props.type === 'int'"
      key="scalar-num"
      class="num-stepper"
      :class="{ 'num-stepper--disabled': props.disabled }"
    >
      <input
        class="num-stepper__input"
        type="text"
        inputmode="decimal"
        :value="textValue(props.modelValue as PropertyScalar)"
        :disabled="props.disabled"
        aria-label="Numeric value"
        @input="onNumber($event, emitScalar, props.type === 'int')"
        @keydown="onNumKey($event, emitScalar, props.type === 'int')"
      />
      <div class="num-stepper__btns">
        <button class="num-stepper__btn" type="button" tabindex="-1" title="Increase value" aria-label="Increase value" :disabled="props.disabled" @pointerdown.prevent="beginRepeat(() => props.modelValue as PropertyScalar, emitScalar, 1, props.type === 'int')" @pointerup="stopRepeat" @pointerleave="stopRepeat" @pointercancel="stopRepeat"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M2.5 7.5 6 4l3.5 3.5"/></svg></button>
        <button class="num-stepper__btn" type="button" tabindex="-1" title="Decrease value" aria-label="Decrease value" :disabled="props.disabled" @pointerdown.prevent="beginRepeat(() => props.modelValue as PropertyScalar, emitScalar, -1, props.type === 'int')" @pointerup="stopRepeat" @pointerleave="stopRepeat" @pointercancel="stopRepeat"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M2.5 4.5 6 8l3.5-3.5"/></svg></button>
      </div>
    </div>
    <div v-else-if="props.type === 'asset'" key="scalar-asset" class="link-row">
      <select class="form-select link-row__select" :value="(props.modelValue as AssetLinkValue | null)?.asset_id ?? ''" aria-label="Asset" :disabled="props.disabled" @change="onAssetSelect($event, emitScalar)">
        <option value="">(none)</option>
        <option v-for="asset in studioState.assets" :key="asset.id" :value="asset.id">{{ asset.name }}</option>
      </select>
      <button class="link-browse-btn" type="button" title="Browse assets" aria-label="Browse assets" :disabled="props.disabled" @click="openPicker(null, $event)"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="1.5" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="1.5" y="7" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="7" width="3.5" height="3.5" rx="0.5"/></svg></button>
    </div>
    <div v-else-if="props.type === 'entity'" key="scalar-entity" class="link-row">
      <select class="form-select link-row__select" :value="textValue(props.modelValue as PropertyScalar)" aria-label="Entity" :disabled="props.disabled" @change="onEntity($event, emitScalar)">
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
      <button class="link-browse-btn" type="button" title="Browse entities" aria-label="Browse entities" :disabled="props.disabled" @click="openPicker(null, $event)"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="1.5" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="1.5" y="7" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="7" width="3.5" height="3.5" rx="0.5"/></svg></button>
    </div>
    <div v-else-if="props.type === 'storylet'" key="scalar-storylet" class="link-row">
      <select class="form-select link-row__select" :value="textValue(props.modelValue as PropertyScalar)" aria-label="Storylet" :disabled="props.disabled" @change="onEntity($event, emitScalar)">
        <option value="">(none)</option>
        <option v-for="storylet in studioState.storylets" :key="storylet.id" :value="storylet.id">{{ storylet.title }}</option>
      </select>
      <button class="link-browse-btn" type="button" title="Browse storylets" aria-label="Browse storylets" :disabled="props.disabled" @click="openPicker(null, $event)"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="1.5" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="1.5" y="7" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="7" width="3.5" height="3.5" rx="0.5"/></svg></button>
    </div>
    <TagEditor
      v-else-if="props.type === 'tag' && !props.disabled"
      key="scalar-tag"
      class="tag-single"
      :model-value="tagAsArray(props.modelValue as PropertyScalar)"
      :options="gameplayTagOptions"
      :multiple="true"
      label="Gameplay tag"
      placeholder="Choose tag"
      :allow-create="false"
      @update:model-value="emitScalar(tagFromArray($event as string | string[]))"
    />
    <span v-else-if="props.type === 'tag'" key="scalar-tag-readonly" class="tag-readonly">{{ (props.modelValue as string) || "(none)" }}</span>
  </div>

  <div v-else class="property-array">
    <TagEditor
      v-if="props.type === 'tag' && !props.disabled"
      key="array-tag"
      :model-value="tagArrayValue"
      :options="gameplayTagOptions"
      :multiple="true"
      label="Gameplay tag"
      placeholder="Choose tag"
      :allow-create="false"
      @update:model-value="commitTagArray($event as string | string[])"
    />
    <span v-else-if="props.type === 'tag'" key="array-tag-readonly" class="tag-readonly">{{ tagArrayValue.join(", ") || "(none)" }}</span>
    <template v-else>
    <div v-for="(_element, index) in (local as PropertyScalar[])" :key="index" class="property-array__row">
      <button
        v-if="props.type === 'bool'"
        :key="'array-bool-' + index"
        type="button"
        class="bool-switch"
        role="switch"
        :aria-checked="elementAt(index) === true"
        aria-label="Boolean value"
        :disabled="props.disabled"
        @click="emitArray(index, !(elementAt(index) === true))"
      >
        <span class="bool-switch__track" aria-hidden="true"><span class="bool-switch__knob"></span></span>
        <span class="bool-switch__state">{{ elementAt(index) === true ? 'True' : 'False' }}</span>
      </button>
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
      <div
        v-else-if="props.type === 'float' || props.type === 'int'"
        :key="'array-num-' + index"
        class="num-stepper"
        :class="{ 'num-stepper--disabled': props.disabled }"
      >
        <input
          class="num-stepper__input"
          type="text"
          inputmode="decimal"
          :value="textValue(elementAt(index))"
          :disabled="props.disabled"
          aria-label="Numeric value"
          @input="onNumber($event, (value) => emitArray(index, value), props.type === 'int')"
          @keydown="onNumKey($event, (value) => emitArray(index, value), props.type === 'int')"
        />
        <div class="num-stepper__btns">
          <button class="num-stepper__btn" type="button" tabindex="-1" title="Increase value" aria-label="Increase value" :disabled="props.disabled" @pointerdown.prevent="beginRepeat(() => elementAt(index), (value) => emitArray(index, value), 1, props.type === 'int')" @pointerup="stopRepeat" @pointerleave="stopRepeat" @pointercancel="stopRepeat"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M2.5 7.5 6 4l3.5 3.5"/></svg></button>
          <button class="num-stepper__btn" type="button" tabindex="-1" title="Decrease value" aria-label="Decrease value" :disabled="props.disabled" @pointerdown.prevent="beginRepeat(() => elementAt(index), (value) => emitArray(index, value), -1, props.type === 'int')" @pointerup="stopRepeat" @pointerleave="stopRepeat" @pointercancel="stopRepeat"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><path d="M2.5 4.5 6 8l3.5-3.5"/></svg></button>
        </div>
      </div>
      <div v-else-if="props.type === 'asset'" :key="'array-asset-' + index" class="link-row">
        <select class="form-select link-row__select" :value="(elementAt(index) as AssetLinkValue | null)?.asset_id ?? ''" aria-label="Asset" :disabled="props.disabled" @change="onAssetSelect($event, (value) => emitArray(index, value))">
          <option value="">(none)</option>
          <option v-for="asset in studioState.assets" :key="asset.id" :value="asset.id">{{ asset.name }}</option>
        </select>
        <button class="link-browse-btn" type="button" title="Browse assets" aria-label="Browse assets" :disabled="props.disabled" @click="openPicker(index, $event)"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="1.5" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="1.5" y="7" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="7" width="3.5" height="3.5" rx="0.5"/></svg></button>
      </div>
      <div v-else-if="props.type === 'entity'" :key="'array-entity-' + index" class="link-row">
        <select class="form-select link-row__select" :value="textValue(elementAt(index))" aria-label="Entity" :disabled="props.disabled" @change="onEntity($event, (value) => emitArray(index, value))">
          <option value="">(none)</option>
          <optgroup v-for="group in entitiesByType" :key="group.type" :label="group.label">
            <option v-for="entity in group.entities" :key="entity.id" :value="entity.id">{{ entity.name }}</option>
          </optgroup>
        </select>
        <button class="link-browse-btn" type="button" title="Browse entities" aria-label="Browse entities" :disabled="props.disabled" @click="openPicker(index, $event)"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="1.5" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="1.5" y="7" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="7" width="3.5" height="3.5" rx="0.5"/></svg></button>
      </div>
      <div v-else-if="props.type === 'storylet'" :key="'array-storylet-' + index" class="link-row">
        <select class="form-select link-row__select" :value="textValue(elementAt(index))" aria-label="Storylet" :disabled="props.disabled" @change="onEntity($event, (value) => emitArray(index, value))">
          <option value="">(none)</option>
          <option v-for="storylet in studioState.storylets" :key="storylet.id" :value="storylet.id">{{ storylet.title }}</option>
        </select>
        <button class="link-browse-btn" type="button" title="Browse storylets" aria-label="Browse storylets" :disabled="props.disabled" @click="openPicker(index, $event)"><svg viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5" aria-hidden="true"><rect x="1.5" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="1.5" width="3.5" height="3.5" rx="0.5"/><rect x="1.5" y="7" width="3.5" height="3.5" rx="0.5"/><rect x="7" y="7" width="3.5" height="3.5" rx="0.5"/></svg></button>
      </div>
      <button class="btn btn-secondary" type="button" :disabled="props.disabled" @click="removeElement(Number(index))">Remove</button>
    </div>
    <button class="btn btn-secondary" type="button" :disabled="props.disabled" @click="addElement">+ Add element</button>
    </template>
  </div>
  <ReferencePicker
    :open="pickerOpen"
    :anchor="pickerAnchor"
    :title="pickerTitle"
    :items="linkItems"
    :selected-id="pickerSelectedId"
    @select="applyPickerSelection"
    @close="closePicker"
  />
</template>

<style scoped>
.num-stepper {
  display: flex;
  align-items: stretch;
  min-width: 0;
  width: 100%;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--bg-input);
  overflow: hidden;
  transition: border-color 0.15s ease;
}
.num-stepper:focus-within {
  border-color: var(--border-focus);
}
.num-stepper--disabled {
  opacity: 0.6;
}
.num-stepper__input {
  flex: 1;
  min-width: 0;
  border: 0;
  background: transparent;
  color: var(--text-main);
  padding: 8px 12px;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
}
.num-stepper__input:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: -2px;
}
.num-stepper__input:disabled {
  cursor: not-allowed;
}
.num-stepper__btns {
  display: flex;
  flex-direction: column;
  flex: none;
  width: 28px;
  border-left: 1px solid var(--border-subtle);
}
.num-stepper__btn {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: background-color 0.12s ease-out, color 0.12s ease-out;
}
.num-stepper__btn:first-child {
  border-bottom: 1px solid var(--border-subtle);
}
.num-stepper__btn:hover:not(:disabled) {
  background: var(--bg-card-hover);
  color: var(--text-main);
}
.num-stepper__btn:active:not(:disabled) {
  background: var(--accent-subtle);
  color: var(--accent);
}
.num-stepper__btn:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
.num-stepper__btn svg {
  width: 12px;
  height: 12px;
  flex-shrink: 0;
}
.bool-switch {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 28px;
  padding: 2px 4px;
  border: 0;
  background: transparent;
  cursor: pointer;
}
.bool-switch:hover:not(:disabled) .bool-switch__track {
  border-color: var(--border-muted);
}
.bool-switch:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.bool-switch__track {
  position: relative;
  flex: none;
  width: 40px;
  height: 22px;
  border-radius: 9999px;
  border: 1px solid var(--border-subtle);
  background: var(--bg-badge);
  transition: background-color 0.12s ease-out, border-color 0.12s ease-out;
}
.bool-switch__knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 9999px;
  background: var(--text-dim);
  transition: transform 0.12s ease-out, background-color 0.12s ease-out;
}
.bool-switch[aria-checked="true"] .bool-switch__track {
  border-color: var(--accent);
  background: var(--accent-subtle);
}
.bool-switch[aria-checked="true"] .bool-switch__knob {
  transform: translateX(18px);
  background: var(--accent);
}
.bool-switch__state {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-muted);
  min-width: 20px;
  text-align: left;
}
.bool-switch[aria-checked="true"] .bool-switch__state {
  color: var(--text-main);
}
.link-row {
  display: flex;
  align-items: stretch;
  gap: 6px;
  min-width: 0;
  width: 100%;
}
.link-row__select {
  flex: 1;
  min-width: 0;
}
.link-browse-btn {
  flex: none;
  width: 36px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--bg-input);
  color: var(--text-muted);
  cursor: pointer;
  transition: background-color 0.12s ease-out, color 0.12s ease-out, border-color 0.12s ease-out;
}
.link-browse-btn svg {
  width: 14px;
  height: 14px;
}
.link-browse-btn:hover:not(:disabled) {
  background: var(--bg-card-hover);
  color: var(--accent);
  border-color: var(--accent);
}
.link-browse-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.tag-readonly {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-muted);
}
</style>
