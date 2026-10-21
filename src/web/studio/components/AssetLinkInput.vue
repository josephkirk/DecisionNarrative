<script setup lang="ts">
import { computed } from "vue";
import type { AssetLinkValue } from "../contracts.ts";
import { studioState } from "../state.ts";

const props = defineProps<{ modelValue: AssetLinkValue | null }>();
const emit = defineEmits<{ "update:modelValue": [value: AssetLinkValue | null] }>();

const assets = computed(() => studioState.assets);
const selectedAsset = computed(() =>
  props.modelValue ? assets.value.find((asset) => asset.id === props.modelValue?.asset_id) : undefined,
);
const versions = computed(() => [...(selectedAsset.value?.versions ?? [])].reverse());

function selectAsset(event: Event) {
  const assetId = (event.target as HTMLSelectElement).value;
  if (!assetId) {
    emit("update:modelValue", null);
    return;
  }
  emit("update:modelValue", { asset_id: assetId, version: "latest" });
}

function selectVersion(event: Event) {
  const version = (event.target as HTMLSelectElement).value;
  if (!props.modelValue) return;
  emit("update:modelValue", { asset_id: props.modelValue.asset_id, version });
}
</script>

<template>
  <div class="asset-link">
    <select class="form-select" :value="props.modelValue?.asset_id ?? ''" @change="selectAsset">
      <option value="">(none)</option>
      <option v-for="asset in assets" :key="asset.id" :value="asset.id">{{ asset.name }}</option>
    </select>
    <select
      class="form-select"
      :value="props.modelValue?.version ?? 'latest'"
      :disabled="!props.modelValue"
      @change="selectVersion"
    >
      <option value="latest">Latest</option>
      <option v-for="version in versions" :key="version.id" :value="version.id">
        {{ version.tool_version }} · {{ version.creator }}
      </option>
    </select>
  </div>
</template>
