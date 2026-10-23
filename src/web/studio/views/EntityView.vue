<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from "vue-router";
import type { EntityProperty, EntityType, PropertyDefinition, TraitInstance } from "../contracts.ts";
import { ENTITY_TYPES, entityTypeColor, entityTypeLabel } from "../entityModel.ts";
import { effectiveTraitValue, hasTraitOverride, propertyListProblem } from "../propertyModel.ts";
import { studioState } from "../state.ts";
import { thumbnailInitial, thumbnailUrl } from "../thumbnails.ts";
import { useEntityEditor } from "../view-models/entityEditor.ts";
import CharacterSettings from "../components/CharacterSettings.vue";
import LocationSettings from "../components/LocationSettings.vue";
import PropertyListEditor from "../components/PropertyListEditor.vue";
import PropertyValueInput from "../components/PropertyValueInput.vue";
import ReferenceList from "../components/ReferenceList.vue";
import ThumbnailPicker from "../components/ThumbnailPicker.vue";

const route = useRoute();
const router = useRouter();
const entityId = typeof route.params.id === "string" ? route.params.id : "";

const {
  draft,
  dirty,
  loading,
  saving,
  activeTab,
  references,
  referencesLoading,
  load,
  patch,
  changeType,
  addTrait,
  removeTrait,
  setTraitValue,
  resetTraitValue,
  save,
  remove,
  loadReferences,
} = useEntityEditor(entityId);

const pendingTraitId = ref("");

const availableTemplates = computed(() => {
  const applied = new Set((draft.value?.traits ?? []).map((trait) => trait.template_id));
  return studioState.traitTemplates.filter((template) => !applied.has(template.id));
});

const appliedTraits = computed(() =>
  (draft.value?.traits ?? []).map((instance: TraitInstance) => ({
    instance,
    template: studioState.traitTemplates.find((template) => template.id === instance.template_id),
  })).filter((entry) => entry.template !== undefined),
);

const propertyProblem = computed(() =>
  draft.value ? propertyListProblem(draft.value.properties) : null,
);

function templateFor(templateId: string) {
  return studioState.traitTemplates.find((template) => template.id === templateId);
}

function onTypeChange(event: Event) {
  const select = event.target as HTMLSelectElement;
  const requested = select.value as EntityType;
  changeType(requested, true);
  // A cancelled (or refused) switch leaves draft.type untouched; the select must
  // follow the draft back instead of showing a type the record does not have.
  if (draft.value && draft.value.type !== requested) {
    select.value = draft.value.type;
  }
}

function definitionValue(definition: PropertyDefinition, instance: TraitInstance) {
  return effectiveTraitValue(definition, instance);
}

function addSelectedTrait() {
  if (!pendingTraitId.value) return;
  addTrait(pendingTraitId.value);
  pendingTraitId.value = "";
}

function removeAppliedTrait(templateId: string, name: string, hasOverrides: boolean) {
  if (hasOverrides && !window.confirm(`Remove trait '${name}' and its overrides?`)) return;
  removeTrait(templateId);
}

async function onSave() {
  await save();
}

async function onDelete() {
  if (await remove()) {
    const folderId = draft.value?.folder_id ?? null;
    router.push(folderId ? `/browse/entities/${folderId}` : "/browse/entities");
  }
}

watch(activeTab, (tab) => {
  if (tab === "references") void loadReferences();
});

onMounted(async () => {
  await load();
  await loadReferences();
});

function confirmDiscard(): boolean {
  if (!dirty.value) return true;
  return window.confirm("Discard unsaved changes?");
}

onBeforeRouteLeave(confirmDiscard);
onBeforeRouteUpdate(confirmDiscard);
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header
      class="entity-banner"
      :style="{ borderLeftColor: draft ? entityTypeColor(draft.type) : 'var(--border-subtle)' }"
    >
      <div class="entity-banner__thumb">
        <img
          v-if="draft && thumbnailUrl(draft.thumbnail)"
          :src="thumbnailUrl(draft.thumbnail) ?? ''"
          :alt="draft.name"
        />
        <span v-else>{{ draft ? thumbnailInitial(draft.name) : "?" }}</span>
      </div>
      <div style="flex:1;">
        <div class="entity-banner__name">{{ draft?.name ?? "Loading…" }}</div>
        <div class="entity-banner__meta">
          {{ draft ? entityTypeLabel(draft.type) : "" }}
          <span v-if="draft?.game_id"> · {{ draft.game_id }}</span>
          <span v-if="dirty" class="badge">Unsaved</span>
        </div>
      </div>
      <div style="display:flex; gap:8px;">
        <button class="btn btn-secondary" :disabled="saving || !dirty || !!propertyProblem" :title="propertyProblem ?? 'Save'" @click="onSave">Save</button>
        <button class="btn btn-danger" :disabled="saving || loading" @click="onDelete">Delete</button>
      </div>
    </header>

    <nav class="tab-nav">
      <button :class="['tab-btn', activeTab === 'properties' ? 'active' : '']" @click="activeTab = 'properties'">Properties</button>
      <button :class="['tab-btn', activeTab === 'traits' ? 'active' : '']" @click="activeTab = 'traits'">Traits</button>
      <button :class="['tab-btn', activeTab === 'references' ? 'active' : '']" @click="activeTab = 'references'">References</button>
    </nav>

    <div v-if="loading || !draft" class="view-content" style="padding:40px; color:var(--text-muted);">
      Loading entity…
    </div>

    <div v-else class="view-content editor-surface">
      <section v-if="activeTab === 'properties'" class="editor-column">
        <h3 class="editor-heading">General</h3>
        <div class="form-group">
          <label class="form-label">Name</label>
          <input class="form-input" :value="draft.name" @input="patch({ name: ($event.target as HTMLInputElement).value })" />
        </div>
        <div class="form-group">
          <label class="form-label">Type</label>
          <select class="form-select" :value="draft.type" @change="onTypeChange($event)">
            <option v-for="type in ENTITY_TYPES" :key="type.value" :value="type.value">{{ type.label }}</option>
          </select>
        </div>
        <div class="form-group">
          <label class="form-label">Thumbnail</label>
          <ThumbnailPicker :model-value="draft.thumbnail ?? null" :label="draft.name" @update:model-value="patch({ thumbnail: $event })" />
        </div>
        <div class="form-group">
          <label class="form-label">Description</label>
          <textarea
            class="form-textarea"
            style="min-height:90px;"
            :value="draft.description ?? ''"
            @input="patch({ description: ($event.target as HTMLTextAreaElement).value })"
          ></textarea>
        </div>
        <div class="form-group">
          <label class="form-label">Game ID</label>
          <input
            class="form-input"
            :value="draft.game_id"
            placeholder="Generated on save"
            @input="patch({ game_id: ($event.target as HTMLInputElement).value })"
          />
          <span class="form-hint">Identifier used by Unreal; blank values are generated from the name and type, and a generated ID follows later renames until you edit it.</span>
        </div>
        <div class="form-group">
          <label class="form-label">Object ID</label>
          <input class="form-input" :value="draft.object_id" readonly />
          <span class="form-hint">{{ draft.object_id ? "Assigned and immutable" : "Assigned on first save" }}</span>
        </div>

        <h3 class="editor-heading">{{ entityTypeLabel(draft.type) }} Settings</h3>
        <CharacterSettings
          v-if="draft.type === 'character' && draft.character"
          :model-value="draft.character"
          :entity-id="draft.id"
          @update:model-value="patch({ character: $event })"
        />
        <LocationSettings
          v-else-if="draft.type === 'location' && draft.location"
          :model-value="draft.location"
          :entity-id="draft.id"
          @update:model-value="patch({ location: $event })"
        />
        <div v-else class="form-hint">This entity kind has no typed settings.</div>

        <h3 class="editor-heading">Custom Properties</h3>
        <PropertyListEditor
          :model-value="draft.properties as EntityProperty[]"
          mode="instance"
          @update:model-value="patch({ properties: $event as EntityProperty[] })"
        />
      </section>

      <section v-else-if="activeTab === 'traits'" class="editor-column">
        <div class="trait-add">
          <select v-model="pendingTraitId" class="form-select">
            <option value="">Choose a trait template…</option>
            <option v-for="template in availableTemplates" :key="template.id" :value="template.id">{{ template.name }}</option>
          </select>
          <button class="btn btn-secondary" :disabled="!pendingTraitId" @click="addSelectedTrait">Add</button>
        </div>

        <div v-if="!appliedTraits.length" class="form-hint">No trait templates applied yet.</div>

        <details v-for="entry in appliedTraits" :key="entry.instance.template_id" class="trait-card" open>
          <summary class="trait-card__head">
            <span>{{ entry.template?.name }}</span>
            <span style="display:flex; gap:8px; align-items:center;">
              <router-link :to="`/trait-templates/${entry.instance.template_id}`" @click.stop>Edit template</router-link>
              <button
                class="btn btn-danger"
                type="button"
                @click.stop.prevent="removeAppliedTrait(entry.instance.template_id, entry.template?.name ?? '', Object.keys(entry.instance.values).length > 0)"
              >
                Remove
              </button>
            </span>
          </summary>
          <div v-for="definition in entry.template?.properties ?? []" :key="definition.id" class="trait-row">
            <label class="trait-row__label">{{ definition.name }}</label>
            <PropertyValueInput
              :key="`${definition.id}-${definition.type}-${definition.is_array}-${definition.multiline}`"
              :model-value="definitionValue(definition, entry.instance)"
              :type="definition.type"
              :is-array="definition.is_array === true"
              :multiline="definition.multiline === true"
              @update:model-value="setTraitValue(entry.instance.template_id, definition.id, $event)"
            />
            <button
              class="btn btn-secondary"
              type="button"
              title="Reset to template default"
              :disabled="!hasTraitOverride(entry.instance, definition.id)"
              @click="resetTraitValue(entry.instance.template_id, definition.id)"
            >
              ↶
            </button>
          </div>
        </details>
      </section>

      <section v-else class="editor-column">
        <ReferenceList :references="references" :loading="referencesLoading" />
      </section>
    </div>
  </div>
</template>
