<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { onBeforeRouteLeave, useRoute, useRouter } from "vue-router";
import type { EntityProperty, PropertyDefinition, TraitInstance } from "../contracts.ts";
import { rootTagColor, rootTagLabel } from "../entityModel.ts";
import { studioState } from "../state.ts";
import { useEntityEditor } from "../view-models/entityEditor.ts";
import PropertyValueInput from "../components/PropertyValueInput.vue";
import PropertyListEditor from "../components/PropertyListEditor.vue";
import ReferenceList from "../components/ReferenceList.vue";
import TagEditor from "../components/TagEditor.vue";
import ThumbnailPicker from "../components/ThumbnailPicker.vue";
const route = useRoute(); const router = useRouter(); const entityId = typeof route.params.id === "string" ? route.params.id : "";
const editor = useEntityEditor(entityId); const { draft, dirty, loading, saving, activeTab, references, referencesLoading, isWorld, isEntityRoute, loadReferences, patch, addTrait, removeTrait, setTraitValue, traitPropertyDisplayValue, save, remove } = editor;
const intrinsic = computed(() => isWorld.value);
const templateId = computed(() => (draft.value && "template_id" in draft.value ? draft.value.template_id : null));
const parentTemplate = computed(() => {
  const tid = templateId.value;
  return tid ? studioState.entityTemplates.find((t) => t.id === tid) : undefined;
});
const pendingTraitId = ref("");
const availableTemplates = computed(() => studioState.traitTemplates.filter((t) => t.allow_multiple || !(draft.value?.traits ?? []).some((i) => i.template_id === t.id)));
const appliedTraits = computed(() => (draft.value?.traits ?? []).map((instance: TraitInstance) => ({ instance, template: studioState.traitTemplates.find((t) => t.id === instance.template_id) })).filter((e) => e.template));
const rootOptions = computed(() => studioState.gameplayTags.filter((tag) => !tag.name.includes(".") && tag.name.toLowerCase() !== "world").map((tag) => tag.name));
function selectRoot(value: string) { if (draft.value && value !== draft.value.root_tag) patch({ root_tag: value }); }
const characterRoleOptions = computed<string[]>(() => studioState.gameplayTags.filter((tag) => tag.name.startsWith("character.")).map((tag) => tag.name));
function roleTagOptions(templateKey: string | undefined, propertyKey: string): string[] | undefined {
  return templateKey === "character_settings" && propertyKey === "role" ? characterRoleOptions.value : undefined;
}
function updateProperties(rows: Array<EntityProperty | PropertyDefinition>) { patch({ properties: rows as EntityProperty[] }); }
function updateTags(value: string | string[]) { patch({ tags: Array.isArray(value) ? value : value ? [value] : [] }); }
function addSelectedTrait() { const template = studioState.traitTemplates.find((t) => t.id === pendingTraitId.value); if (!template) return; addTrait(template); pendingTraitId.value = ""; }
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

  const existingTraits = new Map((draft.value.traits ?? []).map((t) => [t.template_id, t]));
  const mergedTraits = template.traits.map((tt) => {
    const existing = existingTraits.get(tt.template_id);
    return existing
      ? { ...tt, id: existing.id || crypto.randomUUID(), values: { ...tt.values, ...existing.values } }
      : { ...JSON.parse(JSON.stringify(tt)), id: crypto.randomUUID() };
  });
  for (const [tid, tr] of existingTraits.entries()) {
    if (!template.traits.some((tt) => tt.template_id === tid)) {
      mergedTraits.push(tr);
    }
  }

  patch({
    template_id: template.id,
    root_tag: template.root_tag,
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
onMounted(() => { void loadReferences(); }); onBeforeRouteLeave(() => !dirty.value || window.confirm("Discard unsaved changes?"));
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
            <span class="entity-type-label" :style="{ color: rootTagColor(draft.root_tag) }">
              <span class="entity-status-dot" :style="{ backgroundColor: rootTagColor(draft.root_tag) }"></span>
              {{ rootTagLabel(draft.root_tag) }}
            </span>
            <span class="entity-bullet-sep">•</span>
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
          <code class="runtime-val">{{ draft.game_id || draft.object_id }}</code>
        </div>
        <div class="entity-hero-buttons">
          <span v-if="intrinsic" class="badge">Intrinsic · Read-Only</span>
          <button v-else class="btn btn-primary" :disabled="saving || !dirty" @click="save">Save</button>
          <button v-if="!intrinsic" class="btn btn-secondary btn-danger-hover" :disabled="saving" @click="onDelete">Delete</button>
        </div>
      </div>
    </header>

    <!-- Tab Navigation matching Image #2 -->
    <nav class="entity-tabs" aria-label="Entity sections">
      <button :class="['entity-tab', activeTab === 'properties' && 'is-active']" @click="activeTab = 'properties'">
        <svg class="tab-glyph" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
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
              {{ template.name }} ({{ template.root_tag }})
            </option>
          </select>
          <span class="archetype-hint">
            {{ parentTemplate ? `Subclassing archetype '${parentTemplate.name}'. Copies root tag, properties, and trait schemas.` : 'Choose an Entity Template to seed traits and properties.' }}
          </span>
        </div>
      </div>

      <!-- Root category if standalone or template -->
      <div v-if="!parentTemplate && !intrinsic" class="content-block" style="padding: 12px 16px;">
        <div style="display:flex; align-items:center; gap:12px;">
          <span class="block-title" style="margin:0;">Root entity type</span>
          <select class="form-select" style="max-width:220px; height:32px;" :value="draft.root_tag" :disabled="!rootOptions.length" @change="selectRoot(($event.target as HTMLSelectElement).value)">
            <option v-for="tag in rootOptions" :key="tag" :value="tag">{{ rootTagLabel(tag) }} ({{ tag }})</option>
          </select>
        </div>
      </div>

      <!-- 1. Description -->
      <div class="content-block lore-block">
        <div class="content-block-head">
          <span class="block-title">Description</span>
          <span class="char-count-tag">{{ (draft.description || '').length }} characters</span>
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
        <span v-else class="intrinsic-tags-text">{{ draft.tags.length ? draft.tags.join(', ') : 'No gameplay tags' }}</span>
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
        <select v-model="pendingTraitId" class="form-select">
          <option value="">Choose trait template…</option>
          <option v-for="template in availableTemplates" :key="template.id" :value="template.id">{{ template.name }}</option>
        </select>
        <button class="btn btn-secondary" :disabled="!pendingTraitId" @click="addSelectedTrait">Add</button>
      </div>
      <div v-else-if="!appliedTraits.length" class="form-hint" style="color:var(--text-muted); margin-bottom:12px;">
        No traits applied to intrinsic entity.
      </div>
      <details v-for="entry in appliedTraits" :key="entry.instance.id" class="trait-card" open>
        <summary>{{ entry.template?.name }}</summary>
        <div v-for="property in entry.template?.properties ?? []" :key="property.key" class="trait-row">
          <label>
            <span class="field-label">{{ property.name }}</span>
            <PropertyValueInput
              :model-value="traitPropertyDisplayValue(entry.instance, property)"
              :type="property.type"
              :is-array="property.is_array"
              :tag-options="roleTagOptions(entry.template?.key, property.key)"
              :selection-only="entry.template?.key === 'character_settings' && property.key === 'role'"
              :root-filter="entry.template?.key === 'character_settings' && (property.key === 'home_location' || property.key === 'current_location') ? 'location' : undefined"
              :disabled="intrinsic"
              @update:model-value="setTraitValue(entry.instance.id, property.key, $event)"
            />
          </label>
        </div>
        <button v-if="!intrinsic" class="btn btn-danger" @click="removeTrait(entry.instance.id)">Remove</button>
      </details>
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
.entity-page-surface {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px 24px;
  max-width: 960px;
  margin: 0 auto;
  width: 100%;
  height: 100%;
  overflow-y: auto;
}

/* Hero card matching Image #2 */
.entity-hero-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 18px;
  background: var(--bg-card);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg, 10px);
}

.entity-hero-left {
  display: flex;
  align-items: center;
  gap: 14px;
  flex: 1;
  min-width: 0;
}

.entity-hero-meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.entity-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.entity-hero-input {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-main);
  background: transparent;
  border: 1px solid transparent;
  border-radius: var(--radius-sm, 4px);
  padding: 2px 6px;
  outline: none;
  letter-spacing: -0.01em;
  max-width: 380px;
  width: 100%;
  transition: border-color 0.15s ease, background-color 0.15s ease;
}

.entity-hero-input:hover {
  border-color: var(--border-subtle);
  background: var(--bg-input);
}

.entity-hero-input:focus {
  border-color: var(--accent);
  background: var(--bg-input);
  box-shadow: 0 0 0 1px var(--accent-subtle);
}

.entity-hero-heading {
  font-size: 20px;
  font-weight: 700;
  margin: 0;
  color: var(--text-main);
}

.entity-id-tag {
  font-family: var(--font-mono);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  padding: 2px 6px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xs, 3px);
  color: var(--text-dim);
}

.entity-subtitle-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--text-muted);
  flex-wrap: wrap;
}

.entity-type-label {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-weight: 600;
  font-size: 12px;
}

.entity-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 9999px;
  flex-shrink: 0;
}

.entity-bullet-sep {
  color: var(--text-dim);
}

.entity-parent-link {
  color: var(--text-muted);
}

.entity-parent-link a {
  color: var(--accent);
  text-decoration: none;
  font-weight: 500;
}
.entity-parent-link a:hover {
  text-decoration: underline;
}

.entity-standalone-text {
  color: var(--text-dim);
}

.entity-unsaved-pill {
  font-size: 10px;
  font-weight: 600;
  padding: 1px 6px;
  border-radius: 9999px;
  background: rgba(245, 158, 11, 0.15);
  color: var(--status-warning);
  border: 1px solid rgba(245, 158, 11, 0.3);
}

.entity-hero-right {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 8px;
  flex-shrink: 0;
}

.entity-runtime-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
}

.runtime-key {
  color: var(--text-dim);
}

.runtime-val {
  font-family: var(--font-mono);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  color: var(--text-muted);
  background: rgba(255, 255, 255, 0.04);
  padding: 1px 5px;
  border-radius: var(--radius-xs, 3px);
  border: 1px solid var(--border-subtle);
}

.entity-hero-buttons {
  display: flex;
  align-items: center;
  gap: 8px;
}

.btn-danger-hover:hover {
  border-color: var(--status-danger) !important;
  color: var(--status-danger) !important;
  background: var(--status-danger-subtle) !important;
}

/* Tab bar matching Image #2 */
.entity-tabs {
  display: flex;
  gap: 4px;
  border-bottom: 1px solid var(--border-subtle);
  padding: 0 4px;
}

.entity-tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  padding: 8px 14px;
  color: var(--text-muted);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: color 0.15s ease, border-color 0.15s ease;
}

.entity-tab:hover {
  color: var(--text-main);
}

.entity-tab.is-active {
  color: var(--text-main);
  font-weight: 600;
  border-bottom-color: var(--accent);
}

.tab-glyph {
  width: 14px;
  height: 14px;
}

.tab-badge {
  font-size: 10px;
  font-family: var(--font-mono);
  font-variant-numeric: tabular-nums;
  padding: 1px 6px;
  border-radius: 9999px;
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-muted);
}

/* Content stack grouping with spacing */
.entity-content-stack {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.content-block {
  background: var(--bg-card);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg, 10px);
  padding: 14px 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.content-block-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.block-title {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.05em;
  color: var(--text-muted);
}

.block-sub {
  font-size: 10px;
  color: var(--text-dim);
  font-weight: 400;
}

.block-hint {
  font-size: 11px;
  color: var(--text-dim);
}

.char-count-tag, .defined-pill {
  font-family: var(--font-mono);
  font-size: 10px;
  font-variant-numeric: tabular-nums;
  color: var(--text-dim);
  background: rgba(255, 255, 255, 0.05);
  padding: 2px 6px;
  border-radius: var(--radius-xs, 3px);
  border: 1px solid var(--border-subtle);
}

.lore-textarea {
  font-family: var(--font-serif);
  font-size: 15px;
  line-height: 1.6;
  color: var(--text-main);
  background: var(--bg-input);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md, 8px);
  padding: 12px;
  width: 100%;
  max-width: 75ch;
  min-height: 84px;
  outline: none;
  resize: vertical;
  transition: border-color 0.15s ease;
}

.lore-textarea:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 1px var(--accent-subtle);
}

.btn-compact {
  font-size: 11px;
  padding: 2px 8px;
}

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
