<script setup lang="ts">
import { computed, ref } from "vue";
import { StudioApi, ApiError } from "../api.ts";
import { notify } from "../state.ts";
import { thumbnailUrl } from "../thumbnails.ts";

const props = withDefaults(
  defineProps<{
    modelValue: string | null;
    label?: string;
    size?: number;
    disabled?: boolean;
  }>(),
  { size: 58, disabled: false }
);
const emit = defineEmits<{ "update:modelValue": [value: string | null] }>();

const fileInput = ref<HTMLInputElement | null>(null);
const uploading = ref(false);
const broken = ref(false);
const showMenu = ref(false);
const preview = computed(() => thumbnailUrl(props.modelValue));

function onButtonClick(e: MouseEvent) {
  if (props.disabled || uploading.value) return;
  if (!preview.value || broken.value) {
    // If empty, directly open file selector for fastest workflow
    fileInput.value?.click();
  } else {
    // If already has an image, toggle the quick action popover
    showMenu.value = !showMenu.value;
  }
}

function pickFile() {
  showMenu.value = false;
  fileInput.value?.click();
}

function promptUrl() {
  showMenu.value = false;
  const current = props.modelValue ?? "";
  const next = window.prompt("Enter image URL or project-relative path:", current);
  if (next === null) return;
  broken.value = false;
  emit("update:modelValue", next.trim() || null);
}

function clearImage() {
  showMenu.value = false;
  broken.value = false;
  emit("update:modelValue", null);
}

async function onFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  uploading.value = true;
  try {
    const uploaded = await StudioApi.uploadThumbnail(file);
    broken.value = false;
    emit("update:modelValue", uploaded.path);
    notify("Thumbnail updated", "success");
  } catch (error) {
    notify(error instanceof ApiError ? error.message : String(error), "error");
  } finally {
    uploading.value = false;
  }
}
</script>

<template>
  <div class="thumb-avatar-wrapper" @mouseleave="showMenu = false">
    <button
      type="button"
      class="thumb-avatar-btn"
      :class="{ 'has-image': preview && !broken, 'is-disabled': props.disabled }"
      :style="{ width: `${size}px`, height: `${size}px` }"
      :title="props.disabled ? (props.label ?? 'Thumbnail') : (preview ? 'Click to change thumbnail' : 'Click to upload image')"
      :aria-label="props.label ?? 'Thumbnail'"
      :disabled="props.disabled || uploading"
      @click="onButtonClick"
    >
      <img
        v-if="preview && !broken"
        :src="preview"
        :alt="props.label ?? 'Thumbnail'"
        class="thumb-avatar-img"
        @error="broken = true"
      />
      <div v-else class="thumb-avatar-empty">
        <span v-if="uploading" class="thumb-spinner"></span>
        <svg v-else class="thumb-placeholder-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"/>
          <circle cx="12" cy="13" r="4"/>
        </svg>
      </div>

      <!-- Edit badge overlay with pencil icon in bottom right (matching Image #2) -->
      <div v-if="!props.disabled" class="thumb-avatar-badge" aria-hidden="true">
        <svg class="thumb-pencil-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
          <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
        </svg>
      </div>
    </button>

    <!-- Minimalist context menu if image is present -->
    <div v-if="showMenu" class="thumb-popover-menu" @click.stop>
      <button type="button" class="thumb-popover-item" @click="pickFile">
        <span>📁</span> Upload new image…
      </button>
      <button type="button" class="thumb-popover-item" @click="promptUrl">
        <span>🔗</span> Enter image URL…
      </button>
      <div class="thumb-popover-divider"></div>
      <button type="button" class="thumb-popover-item thumb-popover-item--danger" @click="clearImage">
        <span>🗑️</span> Remove image
      </button>
    </div>

    <input ref="fileInput" type="file" accept=".png,.jpg,.jpeg,.webp,.gif,.svg" hidden @change="onFile" />
  </div>
</template>

<style scoped>
.thumb-avatar-wrapper {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.thumb-avatar-btn {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  aspect-ratio: 1 / 1;
  border-radius: var(--radius-lg, 10px);
  border: 1px solid var(--border-subtle);
  background: var(--bg-card);
  padding: 0;
  cursor: pointer;
  overflow: hidden;
  transition: border-color 0.15s ease, transform 0.06s ease-out, box-shadow 0.15s ease;
}

.thumb-avatar-btn:hover:not(.is-disabled) {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent-subtle);
}

.thumb-avatar-btn:active:not(.is-disabled) {
  transform: scale(0.97);
}

.thumb-avatar-btn.is-disabled {
  cursor: default;
  opacity: 0.85;
}

.thumb-avatar-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  aspect-ratio: 1 / 1;
}

.thumb-avatar-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  background: var(--bg-input);
  color: var(--text-dim);
}

.thumb-placeholder-icon {
  width: 40%;
  height: 40%;
  opacity: 0.6;
}

/* Bottom-right pencil badge matching Image #2 */
.thumb-avatar-badge {
  position: absolute;
  bottom: 2px;
  right: 2px;
  width: 18px;
  height: 18px;
  border-radius: 9999px;
  background: var(--bg-surface-high, #212735);
  border: 1px solid var(--border-subtle);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  opacity: 0.85;
  transition: opacity 0.15s ease, background-color 0.15s ease, color 0.15s ease;
}

.thumb-avatar-btn:hover .thumb-avatar-badge {
  opacity: 1;
  background: var(--accent);
  color: #003640;
  border-color: var(--accent);
}

.thumb-pencil-svg {
  width: 10px;
  height: 10px;
}

/* Context Popover */
.thumb-popover-menu {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 60;
  min-width: 160px;
  background: var(--bg-card);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md, 8px);
  box-shadow: var(--shadow-lg);
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.thumb-popover-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 6px 10px;
  border: 0;
  border-radius: var(--radius-sm, 4px);
  background: transparent;
  color: var(--text-main);
  font-size: 12px;
  cursor: pointer;
  text-align: left;
}

.thumb-popover-item:hover {
  background: var(--bg-card-hover);
  color: var(--accent);
}

.thumb-popover-item--danger {
  color: var(--status-danger);
}

.thumb-popover-item--danger:hover {
  background: var(--status-danger-subtle);
  color: var(--status-danger);
}

.thumb-popover-divider {
  height: 1px;
  background: var(--border-subtle);
  margin: 3px 0;
}

.thumb-spinner {
  width: 16px;
  height: 16px;
  border: 2px solid var(--border-subtle);
  border-top-color: var(--accent);
  border-radius: 9999px;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
