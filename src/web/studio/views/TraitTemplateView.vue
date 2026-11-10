<script setup lang="ts">
import { computed, onMounted, ref, useId } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import type { TraitPropertyDefinition, TraitTemplateRecord } from "../contracts.ts";
import { notify, studioState } from "../state.ts";
import { currentFolderId, refreshWorkspace } from "../view-models/workspace.ts";
import PropertyListEditor from "../components/PropertyListEditor.vue";
import ThumbnailPicker from "../components/ThumbnailPicker.vue";
import { countLabel } from "../countLabel.ts";
const route = useRoute();
const router = useRouter();
const templateId = typeof route.params.id === "string" ? route.params.id : "";
const fieldId = useId();

const draft = ref<TraitTemplateRecord | null>(null);
type InheritedProperty = TraitPropertyDefinition & { origin_template_id: string; origin_template_name: string };
const savedJson = ref("");
const saving = ref(false);
const pendingIncludeId = ref("");

const activeTab = ref<"properties" | "includes" | "references">("properties");
const dirty = computed(() => draft.value !== null && JSON.stringify(draft.value) !== savedJson.value);

const propertiesProblem = computed(() => {
  if (!draft.value) return null;
  const seen = new Set<string>();
  for (const property of draft.value.properties) {
    const key = property.key.trim();
    if (!key) return "Property key cannot be empty";
    if (seen.has(key)) return `Duplicate property '${key}' in trait`;
    seen.add(key);
  }
  return null;
});

const usedBy = computed(() =>
  studioState.entityTemplates.filter((entity) => entity.traits.some((trait) => trait.template_id === templateId)),
);
// Included traits
const includedTemplates = computed(() => {
  const ids = draft.value?.includes ?? [];
  return ids
    .map((id) => studioState.traitTemplates.find((t) => t.id === id))
    .filter((t): t is TraitTemplateRecord => t !== undefined);
});

// Helper to check if template `checkId` directly or indirectly includes `targetId`
function includesTransitively(checkId: string, targetId: string, visited = new Set<string>()): boolean {
  if (checkId === targetId) return true;
  if (visited.has(checkId)) return false;
  visited.add(checkId);
  const t = studioState.traitTemplates.find((item) => item.id === checkId);
  if (!t || !t.includes) return false;
  for (const inc of t.includes) {
    if (includesTransitively(inc, targetId, visited)) return true;
  }
  return false;
}

const availableIncludes = computed(() => {
  if (!draft.value) return [];
  const currentIncludes = new Set(draft.value.includes ?? []);
  return studioState.traitTemplates.filter((t) => {
    if (t.id === templateId) return false;
    if (currentIncludes.has(t.id)) return false;
    // Cycle prevention: if t already includes templateId transitively, adding it would create a cycle!
    if (includesTransitively(t.id, templateId)) return false;
    return true;
  });
});

const inheritedProperties = computed<InheritedProperty[]>(() => {
  if (!draft.value) return [];
  const direct = new Set(draft.value.properties.map((p) => p.key));
  const result: InheritedProperty[] = [];
  const visited = new Set<string>();
  function collect(id: string) {
    if (visited.has(id)) return;
    visited.add(id);
    const template = studioState.traitTemplates.find((item) => item.id === id);
    if (!template) return;
    for (const property of template.properties ?? []) {
      if (!direct.has(property.key) && !result.some((item) => item.key === property.key)) {
        result.push({ ...property, origin_template_id: template.id, origin_template_name: template.name });
      }
    }
    for (const included of template.includes ?? []) collect(included);
  }
  for (const included of draft.value.includes ?? []) collect(included);
  return result;
});

function patch(changes: Partial<TraitTemplateRecord>) {
  if (draft.value) draft.value = { ...draft.value, ...changes };
}

function addProperty() {
  if (!draft.value) return;
  draft.value.properties.push({ id: `prop_${Date.now().toString(36)}`, key: "", name: "", type: "string", is_array: false, default: "" });
}

function removeProperty(index: number) { draft.value?.properties.splice(index, 1); }

function updateProperty(index: number, changes: Partial<TraitPropertyDefinition>) {
  if (draft.value?.properties[index]) draft.value.properties[index] = { ...draft.value.properties[index], ...changes };
}

function addInclude() {
  if (!draft.value || !pendingIncludeId.value) return;
  if (!draft.value.includes) draft.value.includes = [];
  if (!draft.value.includes.includes(pendingIncludeId.value)) {
    draft.value.includes.push(pendingIncludeId.value);
  }
  pendingIncludeId.value = "";
}

function removeInclude(id: string) {
  if (!draft.value || !draft.value.includes) return;
  draft.value.includes = draft.value.includes.filter((inc) => inc !== id);
}

async function load() {
  try {
    const template = await StudioApi.getContent<TraitTemplateRecord>("trait_templates", templateId);
    if (!template.properties) template.properties = [];
    if (template.unique === undefined) template.unique = false;
    if (!template.includes) {
      template.includes = [];
    }
    draft.value = template;
    savedJson.value = JSON.stringify(template);
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  }
}

async function save() {
  if (!draft.value) return;
  saving.value = true;
  try {
    const storedFolder = currentFolderId("trait_templates", templateId);
    if (storedFolder !== undefined && storedFolder !== draft.value.folder_id) {
      draft.value = { ...draft.value, folder_id: storedFolder };
    }
    const saved = await StudioApi.saveContent<TraitTemplateRecord>("trait_templates", draft.value);
    draft.value = saved;
    savedJson.value = JSON.stringify(saved);
    await refreshWorkspace(["trait_templates", "entity_templates"]);
    notify(`Trait '${saved.name}' saved`, "success");
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!draft.value) return;
  if (!window.confirm(`Delete trait '${draft.value.name}'?`)) return;
  try {
    await StudioApi.deleteContent("trait_templates", templateId);
    await refreshWorkspace(["trait_templates"]);
    notify("Trait deleted", "success");
    const folderId = draft.value.folder_id ?? null;
    router.push(folderId ? `/browse/trait_templates/${folderId}` : "/browse/trait_templates");
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  }
}

onMounted(load);

function confirmDiscard(): boolean {
  if (!dirty.value) return true;
  return window.confirm("Discard unsaved changes?");
}

onBeforeRouteLeave(confirmDiscard);
onBeforeRouteUpdate(confirmDiscard);
/** Mouse wheels scroll the tab strip sideways when tabs overflow a narrow pane. */
function scrollTabs(event: WheelEvent) {
  const strip = event.currentTarget as HTMLElement;
  if (strip.scrollWidth <= strip.clientWidth || Math.abs(event.deltaX) > Math.abs(event.deltaY)) return;
  event.preventDefault();
  strip.scrollLeft += event.deltaY;
}
</script>

<template>
  <div v-if="draft" class="entity-page-surface scroll-fade-mask">
    <!-- Hero Card matching EntityView -->
    <header class="entity-hero-card">
      <div class="entity-hero-left">
        <ThumbnailPicker
          :model-value="draft.thumbnail ?? null"
          :label="draft.name"
          :size="56"
          :disabled="draft.system"
          @update:model-value="patch({ thumbnail: $event })"
        />
        <div class="entity-hero-meta">
          <div class="entity-title-row">
            <input
              class="entity-hero-input"
              :value="draft.name"
              placeholder="Trait name…"
              aria-label="Trait name"
              @input="patch({ name: ($event.target as HTMLInputElement).value })"
            />
            <span class="entity-id-tag">#{{ (draft.id || '').slice(-6) }}</span>
          </div>

          <div class="entity-subtitle-row">
            <span class="entity-type-label">trait.{{ draft.key || 'key' }}</span>
            <span class="entity-bullet-sep" aria-hidden="true">•</span>
            <span v-if="draft.unique" class="entity-standalone-text" style="color: var(--accent);">Unique Trait</span>
            <span v-else class="entity-standalone-text">Standard Trait</span>
            <span v-if="dirty" class="entity-unsaved-pill">Unsaved</span>
            <span v-if="propertiesProblem" class="editor-validation-pill" :title="propertiesProblem">{{ propertiesProblem }}</span>
          </div>
        </div>
      </div>

      <div class="entity-hero-right">
        <div class="entity-runtime-badge">
          <span class="runtime-key">Prefix:</span>
          <code class="runtime-val" :title="`trait.${draft.key || ''}.*`">trait.{{ draft.key || '&lt;key&gt;' }}.*</code>
        </div>
        <div class="entity-hero-buttons">
          <button
            class="btn btn-primary"
            :disabled="saving || !dirty || !!propertiesProblem"
            :title="propertiesProblem ?? 'Save'"
            @click="save"
          >
            Save
          </button>
          <button
            class="btn btn-secondary btn-danger-hover"
            :disabled="saving"
            @click="remove"
          >
            Delete
          </button>
        </div>
      </div>
    </header>

    <!-- Tabs matching EntityView -->
    <nav class="entity-tabs" aria-label="Trait sections" @wheel="scrollTabs">
      <button :class="['entity-tab', activeTab === 'properties' && 'is-active']" @click="activeTab = 'properties'">
        <svg aria-hidden="true" class="tab-glyph" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="3" y1="6" x2="21" y2="6"/>
          <line x1="3" y1="12" x2="21" y2="12"/>
          <line x1="3" y1="18" x2="21" y2="18"/>
        </svg>
        Properties
        <span class="tab-badge">{{ draft.properties.length }}</span>
      </button>
      <button :class="['entity-tab', activeTab === 'includes' && 'is-active']" @click="activeTab = 'includes'">
        Includes
        <span class="tab-badge">{{ includedTemplates.length }}</span>
      </button>
      <button :class="['entity-tab', activeTab === 'references' && 'is-active']" @click="activeTab = 'references'">
        References
        <span class="tab-badge">{{ usedBy.length }}</span>
      </button>
    </nav>

    <!-- Properties Tab -->
    <section v-show="activeTab === 'properties'" class="entity-content-stack">
      <!-- 1. Identity & Rules -->
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">Trait identity & rules</span>
          <span class="block-hint">Attribute prefix & entity constraints</span>
        </div>
        <div class="form-group" style="margin-bottom:0;">
          <label class="form-label" :for="`${fieldId}-key`">Key</label>
          <input
            :id="`${fieldId}-key`"
            class="form-input"
            aria-label="Trait key"
            :value="draft.key"
            placeholder="Generated from the name"
            @input="patch({ key: ($event.target as HTMLInputElement).value })"
          />
          <span class="form-hint">Runtime attribute prefix: trait.&lt;Key&gt;.&lt;property_key&gt;. A generated key follows the template name until you edit it.</span>
        </div>
        <div class="form-group" style="margin-bottom:0;">
          <label class="form-label" :for="`${fieldId}-unique`">Unique trait</label>
          <label style="display:flex; align-items:center; gap:8px; cursor:pointer;">
            <input
              :id="`${fieldId}-unique`"
              type="checkbox"
              :checked="draft.unique ?? false"
              @change="patch({ unique: ($event.target as HTMLInputElement).checked })"
            />
            <span>Only one unique trait may be applied to an entity</span>
          </label>
        </div>
      </div>

      <!-- 2. Description -->
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">Description</span>
          <span class="char-count-tag">{{ countLabel((draft.description || '').length, "character") }}</span>
        </div>
        <textarea
          class="lore-textarea"
          :value="draft.description ?? ''"
          rows="3"
          aria-label="Trait description"
          placeholder="Describe this trait's purpose and game mechanics…"
          @input="patch({ description: ($event.target as HTMLTextAreaElement).value })"
        ></textarea>
      </div>

      <!-- 3. Typed Properties -->
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">Typed properties</span>
          <span class="defined-pill">{{ draft.properties.length }} defined</span>
        </div>
        <div
          v-if="propertiesProblem"
          class="form-hint"
          style="color:var(--status-danger, #ef4444); background:var(--status-danger-subtle, rgba(239, 68, 68, 0.1)); padding:8px 12px; border-radius:6px; border:1px solid rgba(239, 68, 68, 0.25);"
        >
          {{ propertiesProblem }}
        </div>
        <PropertyListEditor
          :model-value="draft.properties"
          mode="definition"
          @update:model-value="draft.properties = $event as TraitPropertyDefinition[]"
        />
      </div>

      <!-- 4. Inherited Properties (if any) -->
      <div v-if="inheritedProperties.length" class="content-block">
        <div class="content-block-head">
          <span class="block-title">Inherited properties</span>
          <span class="defined-pill">{{ countLabel(inheritedProperties.length, "property", "properties") }}</span>
        </div>
        <div class="form-hint" style="margin-bottom:4px;">
          Automatically inherited from included traits:
        </div>
        <div style="display:flex; flex-direction:column; gap:6px;">
          <div
            v-for="property in inheritedProperties"
            :key="property.key"
            style="display:flex; justify-content:space-between; align-items:center; padding:6px 12px; background:rgba(255, 255, 255, 0.03); border:1px solid var(--border-subtle); border-radius:6px; font-size:12px;"
          >
            <div>
              <span style="font-weight:600; color:var(--text-main);">{{ property.name }}</span>
              <code style="font-size:11px; color:var(--text-muted); margin-left:8px;">{{ property.key }}</code>
              <span class="badge" style="font-size:10px; margin-left:8px;">{{ property.type }}</span>
            </div>
            <span style="color:var(--text-dim); font-size:11px;">from {{ property.origin_template_name }}</span>
          </div>
        </div>
      </div>
    </section>

    <!-- Includes Tab -->
    <section v-show="activeTab === 'includes'" class="entity-content-stack">
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">Compose other traits</span>
          <span class="defined-pill">{{ countLabel(includedTemplates.length, "trait") }}</span>
        </div>
        <div class="form-hint">
          Compose other traits into this trait. Inherits their typed properties automatically without cyclic dependencies.
        </div>

        <div class="include-controls-row">
          <select v-model="pendingIncludeId" class="form-select include-select" aria-label="Trait to include">
            <option value="">Choose a trait to include…</option>
            <option v-for="t in availableIncludes" :key="t.id" :value="t.id">
              {{ t.name }} ({{ countLabel(t.properties.length, "property", "properties") }})
            </option>
          </select>
          <button class="btn btn-secondary include-btn" type="button" :disabled="!pendingIncludeId" @click="addInclude">
            + Include Trait
          </button>
        </div>

        <div v-if="!includedTemplates.length" class="form-hint" style="color:var(--text-muted);">
          No other traits included yet.
        </div>
        <div v-else style="display:flex; flex-direction:column; gap:8px;">
          <div
            v-for="inc in includedTemplates"
            :key="inc.id"
            class="included-trait-row"
          >
            <div class="included-trait-meta">
              <span style="font-weight:600; font-size:13px; color:var(--text-main);">{{ inc.name }}</span>
              <span class="badge" style="font-size:10px;">{{ countLabel(inc.properties.length, "property", "properties") }}</span>
              <router-link :to="`/trait-templates/${inc.id}`" style="font-size:12px; color:var(--accent);">View</router-link>
            </div>
            <button class="btn btn-secondary btn-danger-hover btn-compact" type="button" :aria-label="`Remove ${inc.name}`" @click="removeInclude(inc.id)">
              Remove
            </button>
          </div>
        </div>
      </div>
    </section>

    <!-- References / Used By Tab -->
    <section v-show="activeTab === 'references'" class="entity-content-stack">
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">Entity templates using this trait</span>
          <span class="defined-pill">{{ countLabel(usedBy.length, "entity template") }}</span>
        </div>
        <div v-if="!usedBy.length" class="form-hint" style="color:var(--text-muted);">
          No entity template uses this template yet.
        </div>
        <div v-else class="usage-list" style="display:flex; flex-direction:column; gap:6px;">
          <router-link
            v-for="entity in usedBy"
            :key="entity.id"
            :to="`/entity-templates/${entity.id}`"
            class="usage-list__row"
            style="display:flex; align-items:center; justify-content:space-between; padding:8px 12px; background:rgba(255, 255, 255, 0.02); border:1px solid var(--border-subtle); border-radius:6px; text-decoration:none;"
          >
            <span style="color:var(--accent); font-weight:500;">{{ entity.name }}</span>
            <span class="entity-id-tag">#{{ entity.id.slice(-6) }}</span>
          </router-link>
        </div>
      </div>
    </section>
  </div>

  <!-- Loading Skeleton -->
  <div v-else class="entity-page-surface" aria-label="Loading trait">
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
