<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from "vue-router";
import type { EntityProperty, PropertyDefinition, TraitInstance } from "../contracts.ts";
import { traitEditProblem, traitLabels } from "../entityModel.ts";
import { notify, studioState } from "../state.ts";
import { useEntityEditor } from "../view-models/entityEditor.ts";
import PropertyValueInput from "../components/PropertyValueInput.vue";
import PropertyListEditor from "../components/PropertyListEditor.vue";
import ReferenceList from "../components/ReferenceList.vue";
import TagEditor from "../components/TagEditor.vue";
import ThumbnailPicker from "../components/ThumbnailPicker.vue";
import { entityPanelsFor, traitInspectorFor } from "../plugins/runtime.ts";
import { countLabel } from "../countLabel.ts";
import { thumbnailUrl } from "../thumbnails.ts";
import ReferencePicker from "../components/ReferencePicker.vue";
import { groupTraitProperties } from "../traitPropertyGroups.ts";
const route = useRoute(); const router = useRouter(); const entityId = typeof route.params.id === "string" ? route.params.id : "";
const editor = useEntityEditor(entityId); const { draft, dirty, loading, saving, activeTab, references, referencesLoading, isWorld, isEntityRoute, loadReferences, patch, addTrait, removeTrait, setTraitValue, traitPropertyDisplayValue, save, remove } = editor;
const intrinsic = computed(() => isWorld.value);
const pluginPanels = computed(() => draft.value ? entityPanelsFor(draft.value) : []);
const activePanel = computed(() => pluginPanels.value.find((panel) => activeTab.value === `plugin:${panel.id}`));
const templateId = computed(() => (draft.value && "template_id" in draft.value ? draft.value.template_id : null));
const parentTemplate = computed(() => {
  const tid = templateId.value;
  return tid ? studioState.entityTemplates.find((t) => t.id === tid) : undefined;
});
const pendingTraitId = ref("");
const traitPickerOpen = ref(false);
const traitPickerAnchor = ref<HTMLElement | null>(null);
const availableTemplates = computed(() => studioState.traitTemplates.filter((t) => {
  const traits = draft.value?.traits ?? [];
  if (!t.allow_multiple && traits.some((instance) => instance.template_id === t.id)) return false;
  return !t.unique || !traits.some((instance) => studioState.traitTemplates.some((applied) => applied.id === instance.template_id && applied.unique));
}));
const pendingTraitThumbnail = computed(() => thumbnailUrl(availableTemplates.value.find((t) => t.id === pendingTraitId.value)?.thumbnail));
const appliedTraits = computed(() => (draft.value?.traits ?? []).map((instance: TraitInstance) => ({ instance, template: studioState.traitTemplates.find((t) => t.id === instance.template_id) })).filter((e) => e.template));
const hasAppliedUniqueTrait = computed(() => appliedTraits.value.some((entry) => entry.template?.unique));
const appliedTraitLabels = computed(() => draft.value ? traitLabels(draft.value, studioState.traitTemplates) : []);
const characterRoleOptions = computed<string[]>(() => studioState.gameplayTags.map((tag) => tag.name));
function roleTagOptions(templateKey: string | undefined, propertyKey: string): string[] | undefined {
  return templateKey === "character_settings" && propertyKey === "role" ? characterRoleOptions.value : undefined;
}
function traitInspectorComponent(traitKey: string) { return traitInspectorFor(traitKey)?.component; }
function updateProperties(rows: Array<EntityProperty | PropertyDefinition>) { patch({ properties: rows as EntityProperty[] }); }
function updateTags(value: string | string[]) { patch({ tags: Array.isArray(value) ? value : value ? [value] : [] }); }
function addSelectedTrait() { const template = studioState.traitTemplates.find((t) => t.id === pendingTraitId.value); if (!template) return; if (addTrait(template)) pendingTraitId.value = ""; }
async function onDelete() {
  const targetLabel = isEntityRoute ? "entity" : "entity template";
  if (draft.value?.id === "world" || !window.confirm(`Delete this ${targetLabel}?`)) return;
  const targetRoot = isEntityRoute ? "entities" : "entity_templates";
  if (await remove()) await router.push(`/browse/${targetRoot}`);
}

function onTemplateChange(newTemplateId: string) {
  if (!draft.value) return;
  if (!newTemplateId) {
    patch({ template_id: null });
    return;
  }
  const template = studioState.entityTemplates.find((t) => t.id === newTemplateId);
  if (!template) return;

  const hasExistingData = (draft.value.properties && draft.value.properties.length > 0) || (draft.value.traits && draft.value.traits.length > 0);
  if (hasExistingData && !window.confirm(`Apply template '${template.name}'? This will merge template properties and traits into this entity.`)) {
    return;
  }

  const mergedTags = Array.from(new Set([...(draft.value.tags ?? []), ...template.tags]));
  const existingProps = new Map((draft.value.properties ?? []).map((p) => [p.key, p]));
  const mergedProps: EntityProperty[] = template.properties.map((tp) => {
    const existing = existingProps.get(tp.key);
    const resolvedValue = existing?.value !== undefined ? existing.value : (tp.value !== undefined && tp.value !== null ? tp.value : tp.default);
    return { ...tp, value: resolvedValue };
  });
  for (const [key, prop] of existingProps.entries()) {
    if (!template.properties.some((tp) => tp.key === key)) {
      mergedProps.push(prop);
    }
  }

  const existingTraits = [...(draft.value.traits ?? [])];
  const consumed = new Set<number>();
  const mergedTraits = template.traits.map((tt) => {
    const existingIndex = existingTraits.findIndex((trait, index) => !consumed.has(index) && trait.template_id === tt.template_id);
    if (existingIndex >= 0) {
      consumed.add(existingIndex);
      const existing = existingTraits[existingIndex];
      return { ...tt, id: existing.id || crypto.randomUUID(), values: { ...tt.values, ...existing.values } };
    }
    return { ...JSON.parse(JSON.stringify(tt)), id: crypto.randomUUID() };
  });
  existingTraits.forEach((trait, index) => { if (!consumed.has(index)) mergedTraits.push(trait); });

  const traitProblem = traitEditProblem(mergedTraits, studioState.traitTemplates);
  if (traitProblem) {
    notify(`${traitProblem} The template was not applied.`, "error");
    return;
  }

  patch({
    template_id: template.id,
    tags: mergedTags,
    properties: mergedProps as EntityProperty[],
    traits: mergedTraits,
    thumbnail: draft.value.thumbnail || template.thumbnail || null,
  });
}

function reapplyTemplate() {
  if (templateId.value) {
    onTemplateChange(templateId.value);
  }
}
onMounted(() => { void loadReferences(); });
const confirmDiscard = () => !dirty.value || window.confirm("Discard unsaved changes?");
onBeforeRouteLeave(confirmDiscard);
onBeforeRouteUpdate(confirmDiscard);

/** Mouse wheels scroll the tab strip sideways when plugin tabs overflow a narrow pane. */
function scrollTabs(event: WheelEvent) {
  const strip = event.currentTarget as HTMLElement;
  if (strip.scrollWidth <= strip.clientWidth || Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
  event.preventDefault();
  strip.scrollLeft += event.deltaY;
}
</script>
<template>
  <div v-if="!loading && draft" class="entity-page-surface scroll-fade-mask">
    <!-- Top Header Card matching Image #2 -->
    <header class="entity-hero-card">
      <div class="entity-hero-left">
        <ThumbnailPicker
          :model-value="draft.thumbnail ?? null"
          :label="draft.name"
          :size="56"
          :disabled="intrinsic"
          @update:model-value="patch({ thumbnail: $event })"
        />

        <div class="entity-hero-meta">
          <div class="entity-title-row">
            <input
              v-if="!intrinsic"
              class="entity-hero-input"
              :value="draft.name"
              placeholder="Entity name…"
              aria-label="Entity name"
              @input="patch({ name: ($event.target as HTMLInputElement).value })"
            />
            <h1 v-else class="entity-hero-heading">{{ draft.name }}</h1>
            <span class="entity-id-tag">#{{ (draft.id || '').slice(-6) }}</span>
          </div>

          <div class="entity-subtitle-row">
            <span v-if="appliedTraitLabels.length" class="entity-type-label">{{ appliedTraitLabels.join(" · ") }}</span>
            <span v-else class="entity-standalone-text">No traits</span>
            <span v-if="parentTemplate || templateId || (!intrinsic && isEntityRoute)" class="entity-bullet-sep" aria-hidden="true">•</span>
            <span v-if="parentTemplate" class="entity-parent-link">
              Template: <router-link :to="`/entity-templates/${parentTemplate.id}`">{{ parentTemplate.name }}</router-link>
            </span>
            <span v-else-if="templateId" class="entity-parent-link">
              Template: {{ templateId }}
            </span>
            <span v-else-if="!intrinsic && isEntityRoute" class="entity-standalone-text">Standalone Entity</span>
            <span v-if="dirty" class="entity-unsaved-pill">Unsaved</span>
          </div>
        </div>
      </div>

      <div class="entity-hero-right">
        <div v-if="draft.game_id || draft.object_id" class="entity-runtime-badge">
          <span class="runtime-key">Runtime ID:</span>
          <code class="runtime-val" :title="draft.game_id || draft.object_id">{{ draft.game_id || draft.object_id }}</code>
        </div>
        <div class="entity-hero-buttons">
          <span v-if="intrinsic" class="badge">Intrinsic · Read-Only</span>
          <button v-else class="btn btn-primary" :disabled="saving || !dirty" @click="save">Save</button>
          <button v-if="!intrinsic" class="btn btn-secondary btn-danger-hover" :disabled="saving" @click="onDelete">Delete</button>
        </div>
      </div>
    </header>

    <!-- Tab Navigation matching Image #2 -->
    <nav class="entity-tabs" aria-label="Entity sections" @wheel="scrollTabs">
      <button :class="['entity-tab', activeTab === 'properties' && 'is-active']" @click="activeTab = 'properties'">
        <svg aria-hidden="true" class="tab-glyph" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="3" y1="6" x2="21" y2="6"/>
          <line x1="3" y1="12" x2="21" y2="12"/>
          <line x1="3" y1="18" x2="21" y2="18"/>
        </svg>
        Properties
      </button>
      <button :class="['entity-tab', activeTab === 'traits' && 'is-active']" @click="activeTab = 'traits'">
        Traits
        <span class="tab-badge">{{ appliedTraits.length }}</span>
      </button>
      <button :class="['entity-tab', activeTab === 'references' && 'is-active']" @click="activeTab = 'references'; loadReferences()">
        References
        <span class="tab-badge">{{ references.length }}</span>
      </button>
      <button v-for="panel in pluginPanels" :key="panel.id" :class="['entity-tab', activeTab === `plugin:${panel.id}` && 'is-active']" @click="activeTab = `plugin:${panel.id}`">
        {{ panel.label }}
      </button>
    </nav>

    <!-- Properties Content Stack -->
    <section v-if="activeTab === 'properties'" class="entity-content-stack">
      <!-- Archetype template card (when not intrinsic) -->
      <!-- Archetype template card (when not intrinsic) -->
      <div v-if="isEntityRoute && !intrinsic" class="content-block archetype-block">
        <div class="content-block-head">
          <span class="block-title">Archetype template</span>
          <button
            v-if="templateId"
            type="button"
            class="btn btn-secondary btn-compact"
            title="Re-copy defaults and traits from the template"
            @click="reapplyTemplate"
          >
            Re-apply defaults
          </button>
        </div>
        <div class="archetype-control-row">
          <select
            aria-label="Entity template"
            class="form-select archetype-select"
            :value="templateId ?? ''"
            @change="onTemplateChange(($event.target as HTMLSelectElement).value)"
          >
            <option value="">(None - Standalone Entity)</option>
            <option
              v-for="template in studioState.entityTemplates"
              :key="template.id"
              :value="template.id"
            >
              {{ template.name }}
            </option>
          </select>
          <span class="archetype-hint">
            {{ parentTemplate ? `Subclassing archetype '${parentTemplate.name}'. Copies properties and trait schemas.` : 'Choose an Entity Template to seed traits and properties.' }}
          </span>
        </div>
      </div>

      <!-- 1. Description -->
      <div class="content-block lore-block">
        <div class="content-block-head">
          <span class="block-title">Description</span>
          <span class="char-count-tag">{{ countLabel((draft.description || '').length, "character") }}</span>
        </div>
        <textarea
          class="lore-textarea"
          :value="draft.description"
          :disabled="intrinsic"
          rows="4"
          placeholder="Enter background story, character lore, or descriptive traits…"
          @input="patch({ description: ($event.target as HTMLTextAreaElement).value })"
        ></textarea>
      </div>

      <!-- 2. Gameplay Tags -->
      <div class="content-block tags-block">
        <div class="content-block-head">
          <span class="block-title">Gameplay tags</span>
          <span class="block-hint">Choose registered tags</span>
        </div>
        <TagEditor
          v-if="!intrinsic"
          :model-value="draft.tags ?? []"
          :options="studioState.gameplayTags.map((tag) => tag.name)"
          :allow-create="false"
          placeholder="Choose gameplay tags…"
          @update:model-value="updateTags"
        />
        <span v-else class="intrinsic-tags-text" :class="{ 'is-empty': !draft.tags.length }">{{ draft.tags.length ? draft.tags.join(', ') : 'No gameplay tags' }}</span>
      </div>

      <!-- 3. Typed Properties -->
      <div class="content-block properties-block">
        <div class="content-block-head">
          <span class="block-title">Typed properties</span>
          <span class="defined-pill">{{ draft.properties.length }} defined</span>
        </div>
        <PropertyListEditor
          :model-value="draft.properties"
          mode="definition"
          :disabled="intrinsic"
          @update:model-value="updateProperties"
        />
      </div>
    </section>

    <section v-else-if="activeTab === 'traits'" class="editor-column">
      <div v-if="!intrinsic" class="trait-add">
        <img v-if="pendingTraitThumbnail" class="trait-thumbnail" :src="pendingTraitThumbnail" alt="" width="32" height="32" />
        <select v-model="pendingTraitId" class="form-select" aria-label="Trait to add">
          <option value="">Choose trait…</option>
          <option v-for="template in availableTemplates" :key="template.id" :value="template.id">{{ template.name }}</option>
        </select>
        <button type="button" class="btn btn-secondary" @click="traitPickerAnchor = $event.currentTarget as HTMLElement; traitPickerOpen = true">Browse</button>
        <button class="btn btn-secondary" :disabled="!pendingTraitId" @click="addSelectedTrait">Add</button>
      </div>
      <ReferencePicker :open="traitPickerOpen" :anchor="traitPickerAnchor" title="Choose trait" :items="availableTemplates.map(t => ({ id: t.id, title: t.name, thumbnail: t.thumbnail }))" :selected-id="pendingTraitId || null" @select="pendingTraitId = $event; traitPickerOpen = false" @close="traitPickerOpen = false" />
      <p v-if="hasAppliedUniqueTrait" class="form-hint" style="margin:0 0 12px;">This entity already has a unique trait. Remove it before adding another unique trait.</p>
      <div v-if="intrinsic && !appliedTraits.length" class="form-hint" style="color:var(--text-muted); margin-bottom:12px;">
        No traits applied to intrinsic entity.
      </div>
      <details v-for="entry in appliedTraits" :key="entry.instance.id" class="trait-card" open>
        <summary><img v-if="thumbnailUrl(entry.template?.thumbnail)" class="trait-thumbnail" :src="thumbnailUrl(entry.template?.thumbnail) ?? undefined" alt="" width="32" height="32" /><span>{{ entry.template?.name }}</span><span class="trait-count">{{ countLabel((entry.template?.properties ?? []).length, 'property', 'properties') }}</span></summary>
        <component
          v-if="traitInspectorComponent(entry.template?.key ?? '')"
          :is="traitInspectorComponent(entry.template?.key ?? '')"
          :entity="draft"
          :readonly="intrinsic"
          @patch="patch"
        />
        <div v-else class="trait-groups">
          <section v-for="group in groupTraitProperties(entry.template?.properties ?? [])" :key="group.category" class="trait-group" :class="{ 'is-categorized': group.category }">
          <h3 v-if="group.category" class="trait-group-title">{{ group.category }}</h3>
          <div class="trait-fields">
          <div v-for="property in group.properties" :key="property.key" class="trait-row">
            <label>
              <span class="field-label">{{ property.name }}</span>
              <PropertyValueInput
                :model-value="traitPropertyDisplayValue(entry.instance, property)"
                :type="property.type"
                :is-array="property.is_array"
                :tag-options="roleTagOptions(entry.template?.key ?? entry.template?.id, property.key)"
                :selection-only="entry.template?.key === 'character_settings' && property.key === 'role'"
                :trait-key-filter="entry.template?.key === 'character_settings' && (property.key === 'home_location' || property.key === 'current_location') ? 'location' : undefined"
                :disabled="intrinsic"
                @update:model-value="setTraitValue(entry.instance.id, property.key, $event)"
              />
            </label>
          </div>
          </div>
          </section>
        </div>
        <div v-if="!intrinsic" class="trait-footer"><button class="btn btn-danger" @click="removeTrait(entry.instance.id)">Remove trait</button></div>
      </details>
    </section>

    <section v-else-if="activePanel" class="entity-content-stack" :data-plugin-panel="activePanel.id">
      <component :is="activePanel.component" :entity="draft" :readonly="intrinsic" @patch="patch" @save="save" />
    </section>

    <ReferenceList v-else :references="references" :loading="referencesLoading" />
  </div>
  <div v-else class="entity-page-surface" aria-label="Loading entity">
    <div class="skeleton-hero-card">
      <div class="skeleton-avatar"></div>
      <div style="flex:1; display:flex; flex-direction:column; gap:8px;">
        <div class="skeleton-line skeleton-hero-title"></div>
        <div class="skeleton-line skeleton-hero-meta"></div>
      </div>
    </div>
    <div class="skeleton-tabs">
      <div class="skeleton-tab-item"></div>
      <div class="skeleton-tab-item"></div>
      <div class="skeleton-tab-item"></div>
    </div>
    <div class="skeleton-block" style="height: 120px;"></div>
    <div class="skeleton-block" style="height: 160px;"></div>
  </div>
</template>

<style scoped>
.intrinsic-tags-text { font-size: 13px; color: var(--text-main); }
.intrinsic-tags-text.is-empty { color: var(--text-muted); font-weight: 400; }

.archetype-control-row {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-wrap: wrap;
}

.archetype-select {
  max-width: 320px;
}

.archetype-hint {
  font-size: 12px;
  color: var(--text-dim);
  flex: 1;
  min-width: 220px;
}

/* Traits tab */
.editor-column { display: flex; flex-direction: column; gap: 12px; min-width: 0; container-type: inline-size; }
.trait-add { display: flex; gap: 8px; margin: 0; }
.trait-add select { flex: 1; min-width: 0; }
.trait-card { min-width: 0; overflow: hidden; margin: 0; padding: 0; border: 1px solid var(--border-subtle); border-radius: var(--radius-md, 8px); background: var(--bg-card); }
.trait-card > summary { display: flex; align-items: center; gap: 10px; padding: 14px 16px; cursor: pointer; font-size: 14px; font-weight: 600; list-style: none; }
.trait-card > summary::-webkit-details-marker { display: none; }
.trait-card > summary::before { content: '›'; color: var(--text-muted); font-size: 18px; line-height: 1; }
.trait-card[open] > summary::before { transform: rotate(90deg); }
.trait-card[open] > summary { border-bottom: 1px solid var(--border-subtle); }
.trait-count { margin-left: auto; color: var(--text-muted); font-size: 11px; font-weight: 400; white-space: nowrap; }
.trait-thumbnail { flex: none; width: 32px; height: 32px; object-fit: cover; border-radius: var(--radius-sm, 4px); }
.trait-fields { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 220px), 1fr)); column-gap: 16px; row-gap: 24px; padding: 16px; }
.trait-groups { display: flex; flex-direction: column; gap: 12px; padding: 16px; }
.trait-groups:empty { display: none; }
.trait-group { min-width: 0; }
.trait-group.is-categorized { border: 1px solid var(--border-subtle); border-radius: var(--radius-md, 8px); background: var(--bg-input); }
.trait-group-title { margin: 0; padding: 12px 16px 0; color: var(--text-main); font-size: 12px; font-weight: 600; overflow-wrap: anywhere; }
.trait-group .trait-fields { padding: 16px; }
.trait-group:not(.is-categorized) .trait-fields { padding: 0; }
.trait-row { display: block; min-width: 0; padding: 0; border: 0; }
.trait-row label { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
.trait-row .field-label { color: var(--text-muted); font-size: 12px; font-weight: 500; overflow-wrap: anywhere; }
.trait-row :deep(.form-input), .trait-row :deep(.form-select), .trait-row :deep(.form-textarea) { width: 100%; min-width: 0; }
.trait-footer { display: flex; justify-content: flex-end; padding: 0 16px 16px; }
.trait-groups:empty + .trait-footer { padding-top: 16px; }
.traits-container {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.trait-card-summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  padding: 10px 14px;
  font-weight: 600;
  font-size: 13px;
  background: var(--surface-container-high, #212735);
  border-radius: var(--radius-md, 8px);
  user-select: none;
}

.trait-card-body {
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: var(--bg-card);
  border: 1px solid var(--border-subtle);
  border-top: 0;
  border-radius: 0 0 var(--radius-md, 8px) var(--radius-md, 8px);
}

.loading-view {
  padding: 60px;
  text-align: center;
  color: var(--text-muted);
}
</style>
