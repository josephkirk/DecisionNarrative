<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from "vue";
import { THEME_PRESETS } from "../theme.ts";
import { appearance, backdropFromFile } from "../view-models/appearance.ts";
import { notify } from "../state.ts";
import StudioIcon from "./StudioIcon.vue";
import ToggleSwitch from "./ui/ToggleSwitch.vue";

const open = ref(false);
const root = ref<HTMLElement | null>(null);
const trigger = ref<HTMLButtonElement | null>(null);
const fileInput = ref<HTMLInputElement | null>(null);
const busy = ref(false);

function toggle() {
  open.value = !open.value;
}
function close(restoreFocus = false) {
  open.value = false;
  if (restoreFocus) trigger.value?.focus();
}
function onDocumentPointer(event: PointerEvent) {
  if (open.value && !root.value?.contains(event.target as Node)) close();
}
function onKey(event: KeyboardEvent) {
  if (open.value && event.key === "Escape") close(true);
}
watch(open, (value) => {
  if (value) {
    document.addEventListener("pointerdown", onDocumentPointer);
    document.addEventListener("keydown", onKey);
    void nextTick(() => root.value?.querySelector<HTMLElement>(".appearance__preset.is-selected")?.focus());
  } else {
    document.removeEventListener("pointerdown", onDocumentPointer);
    document.removeEventListener("keydown", onKey);
  }
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocumentPointer);
  document.removeEventListener("keydown", onKey);
});

async function chooseImage(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  busy.value = true;
  try {
    appearance.backgroundImage = await backdropFromFile(file);
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  } finally {
    busy.value = false;
  }
}

/** Arrow keys move through the 4-column preset grid, as in native radio groups. */
const PRESET_COLUMNS = 4;
function presetKey(event: KeyboardEvent, index: number) {
  const steps: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: PRESET_COLUMNS, ArrowUp: -PRESET_COLUMNS };
  const delta = steps[event.key] ?? 0;
  if (!delta) return;
  event.preventDefault();
  const next = THEME_PRESETS[(index + delta + THEME_PRESETS.length) % THEME_PRESETS.length]!;
  appearance.preset = next.id;
  void nextTick(() => root.value?.querySelector<HTMLElement>(`[data-preset="${next.id}"]`)?.focus());
}
</script>

<template>
  <div ref="root" class="appearance">
    <button
      ref="trigger"
      type="button"
      class="sidebar-toggle-btn"
      aria-label="Appearance"
      title="Appearance"
      aria-haspopup="dialog"
      :aria-expanded="open"
      @click="toggle"
    >
      <StudioIcon name="palette" />
    </button>

    <section v-if="open" class="appearance__panel" role="dialog" aria-label="Appearance">
      <h2 class="appearance__title">Appearance</h2>

      <div class="appearance__group">
        <span id="appearance-theme-label" class="appearance__label">Theme</span>
        <div class="appearance__presets" role="radiogroup" aria-labelledby="appearance-theme-label">
          <button
            v-for="(preset, index) in THEME_PRESETS"
            :key="preset.id"
            type="button"
            role="radio"
            class="appearance__preset"
            :class="{ 'is-selected': appearance.preset === preset.id }"
            :data-preset="preset.id"
            :aria-checked="appearance.preset === preset.id"
            :tabindex="appearance.preset === preset.id ? 0 : -1"
            @click="appearance.preset = preset.id"
            @keydown="presetKey($event, index)"
          >
            <span
              class="appearance__swatch"
              :style="{ background: `radial-gradient(circle at 30% 25%, ${preset.swatch[2]} 0, transparent 60%), ${preset.swatch[0]}` }"
              aria-hidden="true"
            >
              <span class="appearance__dot" :style="{ background: preset.swatch[1] }"></span>
              <StudioIcon v-if="appearance.preset === preset.id" name="check" class="appearance__check" />
            </span>
            <span class="appearance__preset-name">{{ preset.label }}</span>
          </button>
        </div>
      </div>

      <div class="appearance__group">
        <span class="appearance__label">Background image</span>
        <div class="appearance__backdrop">
          <span class="appearance__thumb" :class="{ 'is-empty': !appearance.backgroundImage }" aria-hidden="true">
            <img v-if="appearance.backgroundImage" :src="appearance.backgroundImage" alt="" width="72" height="48" />
            <StudioIcon v-else name="assets" />
          </span>
          <div class="appearance__backdrop-actions">
            <button type="button" class="btn btn-secondary" :disabled="busy" @click="fileInput?.click()">
              {{ busy ? "Processing…" : appearance.backgroundImage ? "Replace image…" : "Choose image…" }}
            </button>
            <button v-if="appearance.backgroundImage" type="button" class="btn btn-secondary" @click="appearance.backgroundImage = null">Remove</button>
          </div>
          <input ref="fileInput" type="file" accept="image/*" class="sr-only" tabindex="-1" aria-label="Background image file" @change="chooseImage" />
        </div>
      </div>

      <ToggleSwitch
        v-model="appearance.blur"
        label="Blur background"
        :hint="appearance.backgroundImage ? 'Softens the image so panes stay readable.' : 'Add a background image to use blur.'"
        :disabled="!appearance.backgroundImage"
      />
    </section>
  </div>
</template>
