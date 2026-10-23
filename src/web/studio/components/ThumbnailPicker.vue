<script setup lang="ts">
import { computed, ref } from "vue";
import { StudioApi, ApiError } from "../api.ts";
import { notify } from "../state.ts";
import { thumbnailUrl } from "../thumbnails.ts";

const props = defineProps<{ modelValue: string | null; label?: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string | null] }>();

const fileInput = ref<HTMLInputElement | null>(null);
const uploading = ref(false);
const broken = ref(false);
const preview = computed(() => thumbnailUrl(props.modelValue));

function pick() {
  fileInput.value?.click();
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
  } catch (error) {
    notify(error instanceof ApiError ? error.message : String(error), "error");
  } finally {
    uploading.value = false;
  }
}

function setPath(event: Event) {
  const value = (event.target as HTMLInputElement).value.trim();
  broken.value = false;
  emit("update:modelValue", value ? value : null);
}
</script>

<template>
  <div class="thumb-picker">
    <div class="thumb-picker__preview">
      <img
        v-if="preview && !broken"
        :src="preview"
        :alt="props.label ?? 'Thumbnail'"
        @error="broken = true"
      />
      <span v-else class="thumb-picker__placeholder">No image</span>
    </div>
    <div class="thumb-picker__controls">
      <input
        class="form-input"
        :value="props.modelValue ?? ''"
        placeholder="Path or https:// URL"
        @change="setPath"
      />
      <div style="display:flex; gap:6px;">
        <button class="btn btn-secondary" type="button" :disabled="uploading" @click="pick">
          {{ uploading ? "Uploading…" : "Upload" }}
        </button>
        <button
          class="btn btn-secondary"
          type="button"
          :disabled="!props.modelValue"
          @click="emit('update:modelValue', null)"
        >
          Clear
        </button>
      </div>
      <input ref="fileInput" type="file" accept=".png,.jpg,.jpeg" hidden @change="onFile" />
    </div>
  </div>
</template>
