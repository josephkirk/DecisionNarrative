<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import type { PropertyDefinition, TraitTemplateRecord } from "../contracts.ts";
import { notify, studioState } from "../state.ts";
import { propertyListProblem } from "../propertyModel.ts";
import { currentFolderId, refreshWorkspace } from "../view-models/workspace.ts";
import PropertyListEditor from "../components/PropertyListEditor.vue";

const route = useRoute();
const router = useRouter();
const templateId = typeof route.params.id === "string" ? route.params.id : "";

const draft = ref<TraitTemplateRecord | null>(null);
const savedJson = ref("");
const saving = ref(false);
const dirty = computed(() => draft.value !== null && JSON.stringify(draft.value) !== savedJson.value);
const propertyProblem = computed(() =>
  draft.value ? propertyListProblem(draft.value.properties) : null,
);
const usedBy = computed(() =>
  studioState.entities.filter((entity) =>
    entity.traits.some((trait) => trait.template_id === templateId),
  ),
);

function patch(changes: Partial<TraitTemplateRecord>) {
  if (!draft.value) return;
  draft.value = { ...draft.value, ...changes };
}

async function load() {
  try {
    const template = await StudioApi.getContent<TraitTemplateRecord>("trait_templates", templateId);
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
    await refreshWorkspace(["trait_templates", "entities"]);
    notify(`Trait template '${saved.name}' saved`, "success");
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!draft.value) return;
  if (!window.confirm(`Delete trait template '${draft.value.name}'?`)) return;
  try {
    await StudioApi.deleteContent("trait_templates", templateId);
    await refreshWorkspace(["trait_templates"]);
    notify("Trait template deleted", "success");
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
      <h1 class="view-title">{{ draft?.name ?? "Trait Template" }}</h1>
      <div style="display:flex; gap:8px;">
        <button class="btn btn-secondary" :disabled="saving || !dirty || !!propertyProblem" :title="propertyProblem ?? 'Save'" @click="save">Save</button>
        <button class="btn btn-danger" :disabled="saving || !draft" @click="remove">Delete</button>
      </div>
    </header>

    <div v-if="!draft" class="view-content" style="padding:40px; color:var(--text-muted);">
      Loading trait template…
    </div>

    <div v-else class="view-content editor-surface">
      <section class="editor-column">
        <div class="form-group">
          <label class="form-label">Name</label>
          <input class="form-input" :value="draft.name" @input="patch({ name: ($event.target as HTMLInputElement).value })" />
        </div>
        <div class="form-group">
          <label class="form-label">Key</label>
          <input class="form-input" :value="draft.key" placeholder="Generated from the name" @input="patch({ key: ($event.target as HTMLInputElement).value })" />
          <span class="form-hint">Runtime attribute prefix: trait.&lt;Key&gt;.&lt;property key&gt;. A generated key follows the template name until you edit it.</span>
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

        <h3 class="editor-heading">Properties</h3>
        <PropertyListEditor
          :model-value="draft.properties as PropertyDefinition[]"
          mode="definition"
          @update:model-value="patch({ properties: $event as PropertyDefinition[] })"
        />

        <h3 class="editor-heading">Used by</h3>
        <div v-if="!usedBy.length" class="form-hint">No entity uses this template yet.</div>
        <div v-else class="usage-list">
          <router-link
            v-for="entity in usedBy"
            :key="entity.id"
            :to="`/entities/${entity.id}`"
            class="usage-list__row"
          >
            {{ entity.name }}
          </router-link>
        </div>
      </section>
    </div>
  </div>
</template>
