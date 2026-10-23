<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import type { QualityRecord } from "../contracts.ts";
import { notify } from "../state.ts";
import { currentFolderId, refreshWorkspace } from "../view-models/workspace.ts";

const route = useRoute();
const router = useRouter();
const isNew = computed(() => route.name === "new-quality");
const draft = ref<QualityRecord | null>(null);
const savedJson = ref("");
const saving = ref(false);
const dirty = computed(() => draft.value !== null && JSON.stringify(draft.value) !== savedJson.value);

function newDraft(): QualityRecord {
  const folder = typeof route.query.folder === "string" ? route.query.folder : null;
  return {
    name: "",
    type: "metric",
    value: 0,
    min_value: 0,
    max_value: 100,
    is_hidden: false,
    description: "",
    folder_id: folder,
  };
}

function patch(changes: Partial<QualityRecord>) {
  if (!draft.value) return;
  draft.value = { ...draft.value, ...changes };
}

async function load() {
  if (isNew.value) {
    draft.value = newDraft();
    savedJson.value = "";
    return;
  }
  const name = typeof route.params.name === "string" ? route.params.name : "";
  try {
    const quality = await StudioApi.getContent<QualityRecord>("qualities", name);
    draft.value = quality;
    savedJson.value = JSON.stringify(quality);
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  }
}

async function save() {
  if (!draft.value) return;
  saving.value = true;
  try {
    const storedFolder = currentFolderId("qualities", draft.value.name);
    if (storedFolder !== undefined && storedFolder !== draft.value.folder_id) {
      draft.value = { ...draft.value, folder_id: storedFolder };
    }
    const saved = await StudioApi.saveContent<QualityRecord>("qualities", draft.value);
    draft.value = saved;
    savedJson.value = JSON.stringify(saved);
    await refreshWorkspace(["qualities"]);
    notify(`Quality '${saved.name}' saved`, "success");
    if (isNew.value) router.replace(`/qualities/${encodeURIComponent(saved.name)}`);
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  } finally {
    saving.value = false;
  }
}

async function remove() {
  if (!draft.value) return;
  if (!window.confirm(`Delete quality '${draft.value.name}'?`)) return;
  try {
    await StudioApi.deleteContent("qualities", draft.value.name);
    await refreshWorkspace(["qualities"]);
    notify("Quality deleted", "success");
    const folderId = draft.value.folder_id ?? null;
    router.push(folderId ? `/browse/qualities/${folderId}` : "/browse/qualities");
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
      <h1 class="view-title">{{ isNew ? "New Quality" : draft?.name ?? "Quality" }}</h1>
      <div style="display:flex; gap:8px;">
        <button class="btn btn-secondary" :disabled="saving || !dirty" @click="save">Save</button>
        <button v-if="!isNew" class="btn btn-danger" :disabled="saving" @click="remove">Delete</button>
      </div>
    </header>

    <div v-if="!draft" class="view-content" style="padding:40px; color:var(--text-muted);">Loading quality…</div>

    <div v-else class="view-content editor-surface">
      <section class="editor-column">
        <div class="form-group">
          <label class="form-label">Quality Name / Key</label>
          <input
            class="form-input"
            :value="draft.name"
            :disabled="!isNew"
            @input="patch({ name: ($event.target as HTMLInputElement).value })"
          />
          <span v-if="!isNew" class="form-hint">Quality names are keys referenced by storylets and cannot be renamed.</span>
        </div>
        <div class="form-group">
          <label class="form-label">Category</label>
          <select class="form-select" :value="draft.type" @change="patch({ type: ($event.target as HTMLSelectElement).value as QualityRecord['type'] })">
            <option value="currency">Currency</option>
            <option value="metric">Metric</option>
            <option value="tool">Tool</option>
            <option value="progress">Progress</option>
            <option value="menace">Menace</option>
            <option value="territory">Territory (legacy)</option>
          </select>
        </div>
        <div class="form-group">
          <label class="form-label">Default Value</label>
          <input type="number" class="form-input" :value="draft.value" @input="patch({ value: Number(($event.target as HTMLInputElement).value) })" />
        </div>
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px;">
          <div class="form-group">
            <label class="form-label">Minimum</label>
            <input type="number" class="form-input" :value="draft.min_value ?? 0" @input="patch({ min_value: Number(($event.target as HTMLInputElement).value) })" />
          </div>
          <div class="form-group">
            <label class="form-label">Maximum</label>
            <input type="number" class="form-input" :value="draft.max_value ?? 100" @input="patch({ max_value: Number(($event.target as HTMLInputElement).value) })" />
          </div>
        </div>
        <label class="form-group" style="display:flex; align-items:center; gap:8px;">
          <input type="checkbox" :checked="draft.is_hidden === true" @change="patch({ is_hidden: ($event.target as HTMLInputElement).checked })" />
          Hide this quality from the player-facing quality list
        </label>
        <div v-if="draft.type === 'menace'" class="form-group">
          <label class="form-label">Crisis Storylet ID (optional)</label>
          <input class="form-input" :value="draft.menace_crisis_storylet_id ?? ''" placeholder="storylet-id" @input="patch({ menace_crisis_storylet_id: ($event.target as HTMLInputElement).value })" />
        </div>
        <div class="form-group">
          <label class="form-label">Description</label>
          <textarea
            class="form-textarea"
            style="min-height:80px;"
            :value="draft.description ?? ''"
            @input="patch({ description: ($event.target as HTMLTextAreaElement).value })"
          ></textarea>
        </div>
      </section>
    </div>
  </div>
</template>
