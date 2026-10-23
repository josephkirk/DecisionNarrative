<script setup lang="ts">
import { computed } from "vue";
import type { EntityRecord, LocationSection } from "../contracts.ts";
import { studioState } from "../state.ts";
import TagEditor from "./TagEditor.vue";

const props = withDefaults(defineProps<{ modelValue: LocationSection; entityId?: string | null }>(), {
  entityId: null,
});
const emit = defineEmits<{ "update:modelValue": [value: LocationSection] }>();

const section = computed(() => props.modelValue);
const otherLocations = computed(() =>
  studioState.entities.filter(
    (entity: EntityRecord) => entity.type === "location" && entity.id !== props.entityId,
  ),
);
const tagOptions = computed(() =>
  studioState.entities
    .filter((entity) => entity.type === "location")
    .flatMap((entity) => entity.location?.tags ?? []),
);

function patch(changes: Partial<LocationSection>) {
  emit("update:modelValue", { ...section.value, ...changes });
}

function toggleZone(id: string, checked: boolean) {
  const zones = new Set(section.value.connected_zones ?? []);
  if (checked) zones.add(id);
  else zones.delete(id);
  patch({ connected_zones: [...zones] });
}
</script>

<template>
  <div class="settings-section">
    <div class="form-group">
      <label class="form-label">Location Tags</label>
      <TagEditor
        :model-value="section.tags ?? []"
        :options="tagOptions"
        label="Location tags"
        placeholder="Choose or create location tags"
        @update:model-value="(value) => patch({ tags: Array.isArray(value) ? value : [value] })"
      />
    </div>

    <section class="settings-block">
      <h3 class="form-label">Connected Zones</h3>
      <div v-if="!otherLocations.length" class="form-hint">Create another location to connect zones.</div>
      <label v-for="location in otherLocations" :key="location.id" class="settings-checkbox">
        <input
          type="checkbox"
          :checked="(section.connected_zones ?? []).includes(location.id)"
          @change="toggleZone(location.id, ($event.target as HTMLInputElement).checked)"
        />
        {{ location.name }}
      </label>
    </section>
  </div>
</template>
