<script setup lang="ts">
import { computed, ref, useId } from "vue";
import { normalizeTagName, uniqueTagNames } from "../tagData.ts";
import { addTagSelection, commitSingleTagEdit, nextTagSuggestionIndex, tagSuggestionQuery, tagSuggestions } from "../tagEditorModel.ts";

const props = withDefaults(defineProps<{
  modelValue?: string | string[] | null;
  options?: string[];
  multiple?: boolean;
  placeholder?: string;
  label?: string;
}>(), { options: () => [], multiple: true, placeholder: "Add or create a tag", label: "Tag" });
const emit = defineEmits<{ "update:modelValue": [value: string | string[]] }>();

const input = ref<HTMLInputElement | null>(null);
const query = ref("");
const isFocused = ref(false);
const isSingleEditing = ref(false);
const activeIndex = ref(-1);
const listboxId = "tag-editor-options-" + useId();
const selected = computed(() => uniqueTagNames(Array.isArray(props.modelValue) ? props.modelValue : [props.modelValue ?? ""]));
const options = computed(() => uniqueTagNames(props.options ?? []));
const suggestions = computed(() => tagSuggestions(options.value, selected.value, tagSuggestionQuery(query.value, selected.value, props.multiple)));
const exactOption = computed(() => options.value.find((option) => normalizeTagName(option) === normalizeTagName(query.value)));
const isListboxOpen = computed(() => isFocused.value && suggestions.value.length > 0);
const activeOptionId = computed(() => activeIndex.value >= 0 ? listboxId + "-" + activeIndex.value : undefined);
const inputValue = computed(() => !props.multiple && !isSingleEditing.value ? selected.value[0] ?? "" : query.value);

function publish(values: string[]) {
  const next = uniqueTagNames(values);
  emit("update:modelValue", props.multiple ? next : (next[0] ?? ""));
}
function finishEditing() {
  query.value = "";
  isSingleEditing.value = false;
  activeIndex.value = -1;
}
function add(raw: string) {
  publish(addTagSelection(selected.value, raw, options.value, props.multiple));
  finishEditing();
  isFocused.value = props.multiple && document.activeElement === input.value;
}
function chooseSuggestion(value: string) {
  add(value);
  if (props.multiple) input.value?.focus();
}
function remove(value: string) {
  publish(selected.value.filter((item) => normalizeTagName(item) !== normalizeTagName(value)));
  input.value?.focus();
}
function onFocus() {
  isFocused.value = true;
  if (!props.multiple) {
    isSingleEditing.value = true;
    query.value = selected.value[0] ?? "";
    input.value?.select();
  }
}
function onInput(event: Event) {
  query.value = (event.target as HTMLInputElement).value;
  if (!props.multiple) isSingleEditing.value = true;
  activeIndex.value = -1;
  isFocused.value = true;
}
function onBlur() {
  if (props.multiple) {
    if (query.value.trim()) add(query.value);
  } else if (isSingleEditing.value) {
    publish(commitSingleTagEdit(selected.value, query.value, true, options.value));
  }
  finishEditing();
  isFocused.value = false;
}
function moveActiveOption(direction: 1 | -1) {
  if (!suggestions.value.length) return;
  isFocused.value = true;
  activeIndex.value = nextTagSuggestionIndex(activeIndex.value, direction, suggestions.value.length);
}
function onKeydown(event: KeyboardEvent) {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    if (!suggestions.value.length) return;
    event.preventDefault();
    moveActiveOption(event.key === "ArrowDown" ? 1 : -1);
  } else if (event.key === "Enter") {
    event.preventDefault();
    const active = suggestions.value[activeIndex.value];
    if (!props.multiple && isSingleEditing.value && !query.value.trim() && !active) {
      publish([]);
      finishEditing();
      isFocused.value = false;
    } else add(active ?? query.value);
  } else if (event.key === ",") {
    event.preventDefault();
    add(query.value.replace(/,$/, ""));
  } else if (event.key === "Escape") {
    isFocused.value = false;
    activeIndex.value = -1;
  } else if (event.key === "Backspace" && props.multiple && !query.value && selected.value.length) {
    remove(selected.value[selected.value.length - 1]!);
  }
}
function focusInput() {
  input.value?.focus();
}
</script>

<template>
  <div class="tag-editor" :aria-label="label" @click="focusInput">
    <span v-if="multiple" v-for="tag in selected" :key="normalizeTagName(tag)" class="tag-editor__chip">
      {{ tag }}
      <button type="button" class="tag-editor__remove" :aria-label="'Remove ' + tag" @click.stop="remove(tag)">×</button>
    </span>
    <input
      ref="input"
      class="tag-editor__input"
      :value="inputValue"
      :placeholder="placeholder"
      :aria-label="label"
      role="combobox"
      aria-autocomplete="list"
      aria-haspopup="listbox"
      :aria-expanded="isListboxOpen"
      :aria-controls="listboxId"
      :aria-activedescendant="activeOptionId"
      @focus="onFocus"
      @input="onInput"
      @keydown="onKeydown"
      @blur="onBlur"
    />
    <div v-if="isListboxOpen" :id="listboxId" class="tag-editor__popup" role="listbox" :aria-label="label + ' suggestions'">
      <div
        v-for="(tag, index) in suggestions"
        :id="listboxId + '-' + index"
        :key="normalizeTagName(tag)"
        class="tag-editor__option"
        :class="{ 'tag-editor__option--active': index === activeIndex }"
        role="option"
        :aria-selected="index === activeIndex"
        @pointerdown.prevent
        @mouseenter="activeIndex = index"
        @click="chooseSuggestion(tag)"
      >{{ tag }}</div>
    </div>
    <small v-if="query.trim() && !exactOption" class="tag-editor__hint">Press Enter to add “{{ query.trim() }}”.</small>
  </div>
</template>

<style scoped>
.tag-editor {
  position: relative;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 5px;
  min-height: 34px;
  padding: 4px 7px;
  border: 1px solid var(--border-subtle);
  border-radius: 7px;
  background: var(--bg-input);
  cursor: text;
}
.tag-editor:focus-within {
  border-color: var(--border-focus);
  box-shadow: 0 0 0 2px var(--accent-subtle);
}
.tag-editor__chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
  background: var(--bg-badge);
  color: var(--text-main);
  font-size: 12px;
  line-height: 18px;
}
.tag-editor__remove {
  border: 0;
  padding: 0 1px;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 15px;
  line-height: 1;
}
.tag-editor__remove:hover { color: var(--status-danger); }
.tag-editor__input {
  flex: 1 1 90px;
  min-width: 80px;
  height: 24px;
  border: 0;
  outline: 0;
  padding: 0 2px;
  background: transparent;
  color: var(--text-main);
  box-shadow: none;
}
.tag-editor__input:focus { outline: 0; box-shadow: none; }
.tag-editor__popup {
  position: absolute;
  z-index: 50;
  top: calc(100% + 4px);
  right: 0;
  left: 0;
  max-height: 220px;
  overflow-y: auto;
  padding: 4px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: var(--bg-card);
  box-shadow: var(--shadow-md);
}
.tag-editor__option {
  padding: 6px 8px;
  border-radius: 5px;
  color: var(--text-main);
  cursor: pointer;
  font-size: 12px;
}
.tag-editor__option:hover, .tag-editor__option--active { background: var(--accent-subtle); }
.tag-editor__hint { flex-basis: 100%; color: var(--text-muted); font-size: 11px; }
</style>
