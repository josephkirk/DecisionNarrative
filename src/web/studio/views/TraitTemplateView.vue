<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import type { TraitPropertyDefinition, TraitTemplateRecord } from "../contracts.ts";
import { notify, studioState } from "../state.ts";
import { currentFolderId, refreshWorkspace } from "../view-models/workspace.ts";
import PropertyListEditor from "../components/PropertyListEditor.vue";

const route = useRoute();
const router = useRouter();
const templateId = typeof route.params.id === "string" ? route.params.id : "";

const draft = ref<TraitTemplateRecord | null>(null);
type InheritedProperty = TraitPropertyDefinition & { origin_template_id: string; origin_template_name: string };
const savedJson = ref("");
const saving = ref(false);
const pendingIncludeId = ref("");

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
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <h1 class="view-title">{{ draft?.name ?? "Trait" }}</h1>
      <div style="display:flex; gap:8px;">
        <button class="btn btn-secondary" :disabled="saving || !dirty || !!propertiesProblem" :title="propertiesProblem ?? 'Save'" @click="save">Save</button>
        <button class="btn btn-danger" :disabled="saving || !draft" @click="remove">Delete</button>
      </div>
    </header>

    <div v-if="!draft" class="view-content" style="padding:40px; color:var(--text-muted);">
      Loading trait…
    </div>

    <div v-else class="view-content editor-surface">
      <section class="editor-column">
        <div class="form-group">
          <label class="form-label">Name</label>
          <input class="form-input" :value="draft.name" @input="patch({ name: ($event.target as HTMLInputElement).value })" />
        </div>
        <div class="form-group">
          <label class="form-label" for="trait-key">Key</label>
          <input id="trait-key" class="form-input" aria-label="Key" :value="draft.key" placeholder="Generated from the name" @input="patch({ key: ($event.target as HTMLInputElement).value })" />
          <span class="form-hint">Runtime attribute prefix: trait.&lt;Key&gt;.&lt;property_key&gt;. A generated key follows the template name until you edit it.</span>
        </div>
        <div class="form-group">
          <label class="form-label">Description</label>
          <textarea
            class="form-textarea"
            style="min-height:70px;"
            :value="draft.description ?? ''"
            @input="patch({ description: ($event.target as HTMLTextAreaElement).value })"
          ></textarea>
        </div>
        <div class="form-group">
          <label class="form-label" for="trait-unique">Unique trait</label>
          <label style="display:flex; align-items:center; gap:8px;">
            <input id="trait-unique" type="checkbox" :checked="draft.unique ?? false" @change="patch({ unique: ($event.target as HTMLInputElement).checked })" />
            <span>Only one unique trait may be applied to an entity</span>
          </label>
        </div>

        <!-- INCLUDED TRAITS -->
        <div style="display:flex; justify-content:space-between; align-items:center; margin-top:24px; margin-bottom:8px;">
          <h3 class="editor-heading" style="margin:0;">Included Traits</h3>
        </div>
        <div class="form-hint" style="margin-bottom:8px;">
          Compose other traits into this trait. Inherits their typed properties automatically without cyclic dependencies.
        </div>

        <div style="display:flex; gap:8px; margin-bottom:12px;">
          <select v-model="pendingIncludeId" class="form-select" style="flex:1;" aria-label="Trait to include">
            <option value="">Choose a trait to include…</option>
            <option v-for="t in availableIncludes" :key="t.id" :value="t.id">
              {{ t.name }} ({{ t.properties.length }} properties)
            </option>
          </select>
          <button class="btn btn-secondary" type="button" :disabled="!pendingIncludeId" @click="addInclude">
            + Include Trait
          </button>
        </div>

        <div v-if="!includedTemplates.length" class="form-hint" style="margin-bottom:16px;">
          No other traits included yet.
        </div>
        <div v-else style="display:flex; flex-direction:column; gap:6px; margin-bottom:16px;">
          <div
            v-for="inc in includedTemplates"
            :key="inc.id"
            style="display:flex; justify-content:space-between; align-items:center; padding:8px 12px; background:var(--bg-input, rgba(0,0,0,0.2)); border:1px solid var(--border-subtle, #333); border-radius:6px;"
          >
            <div style="display:flex; align-items:center; gap:8px;">
              <span style="font-weight:600; font-size:13px;">{{ inc.name }}</span>
              <span class="badge" style="font-size:10px;">{{ inc.properties.length }} properties</span>
              <router-link :to="`/trait-templates/${inc.id}`" style="font-size:12px; color:var(--accent);">View</router-link>
            </div>
            <button class="btn btn-danger" type="button" style="padding:2px 6px; font-size:11px;" :aria-label="`Remove ${inc.name}`" @click="removeInclude(inc.id)">
              Remove
            </button>
          </div>
        </div>

        <div style="display:flex; justify-content:space-between; align-items:center; margin-top:24px; margin-bottom:8px;">
          <h3 class="editor-heading" style="margin:0;">Typed Properties</h3>
        </div>
        <div v-if="propertiesProblem" class="form-hint" style="color:var(--danger, #ff5555); margin-bottom:8px;">{{ propertiesProblem }}</div>
        <PropertyListEditor
          :model-value="draft.properties"
          mode="definition"
          @update:model-value="draft.properties = $event as TraitPropertyDefinition[]"
        />
        <template v-if="inheritedProperties.length">
          <h3 class="editor-heading" style="margin-top:24px;">Inherited Properties</h3>
          <div v-for="property in inheritedProperties" :key="property.key" class="form-hint">
            {{ property.name }} ({{ property.type }}) from {{ property.origin_template_name }}
          </div>
        </template>

        <h3 class="editor-heading" style="margin-top:24px;">Used by</h3>
        <div v-if="!usedBy.length" class="form-hint">No entity template uses this template yet.</div>
        <div v-else class="usage-list">
          <router-link
            v-for="entity in usedBy"
            :key="entity.id"
            :to="`/entity-templates/${entity.id}`"
            class="usage-list__row"
          >
            {{ entity.name }}
          </router-link>
        </div>
      </section>
    </div>
  </div>
</template>
