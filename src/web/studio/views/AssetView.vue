<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import type { AssetRecord, AssetVersionRecord, ContentReference, StorageLocationRecord } from "../contracts.ts";
import { notify } from "../state.ts";
import { currentFolderId, refreshWorkspace } from "../view-models/workspace.ts";
import ReferenceList from "../components/ReferenceList.vue";
import TagEditor from "../components/TagEditor.vue";
import ThumbnailPicker from "../components/ThumbnailPicker.vue";
import { thumbnailUrl } from "../thumbnails.ts";
const route = useRoute();
const router = useRouter();
const assetId = typeof route.params.id === "string" ? route.params.id : "";

const draft = ref<AssetRecord | null>(null);
const savedJson = ref("");
const saving = ref(false);
const references = ref<ContentReference[]>([]);
const referencesLoading = ref(false);
const showVersionForm = ref(false);
const customDataError = ref("");
const dirty = computed(() => draft.value !== null && JSON.stringify(draft.value) !== savedJson.value);
// A half-typed version is unsaved work too, even though it is not in the draft.
const hasPendingVersion = computed(() =>
  Boolean(
    versionForm.value.file_path.trim() ||
      versionForm.value.creator.trim() ||
      versionForm.value.tool_version.trim() ||
      versionForm.value.description.trim() ||
      versionForm.value.tags.length ||
      versionForm.value.thumbnail ||
      versionForm.value.custom_data.trim() !== "{}" ||
      versionForm.value.storage.some(
        (row) =>
          row.storage_id.trim() ||
          row.path.trim() ||
          row.storage_type !== "disk" ||
          row.reference_type !== "file",
      ),
  ),
);
const versionsNewestFirst = computed(() => [...(draft.value?.versions ?? [])].reverse());
const tagOptions = computed(() => [
  ...(draft.value?.tags ?? []),
  ...(draft.value?.versions ?? []).flatMap((version) => version.tags),
]);

interface VersionForm {
  file_path: string;
  creator: string;
  tool_version: string;
  description: string;
  tags: string[];
  custom_data: string;
  thumbnail: string | null;
  storage: StorageLocationRecord[];
}

function emptyStorage(): StorageLocationRecord {
  return {
    storage_type: "disk",
    storage_id: "",
    created_at: Date.now(),
    reference_type: "file",
    path: "",
    metadata: {},
  };
}

function newVersionForm(): VersionForm {
  return {
    file_path: "",
    creator: "",
    tool_version: "",
    description: "",
    tags: [],
    custom_data: "{}",
    thumbnail: null,
    storage: [emptyStorage()],
  };
}

const versionForm = ref<VersionForm>(newVersionForm());

function patch(changes: Partial<AssetRecord>) {
  if (!draft.value) return;
  draft.value = { ...draft.value, ...changes };
}

function toggleVersionForm() {
  if (!showVersionForm.value) {
    showVersionForm.value = true;
    return;
  }
  // Cancelling discards the half-typed version so no hidden draft lingers.
  showVersionForm.value = false;
  versionForm.value = newVersionForm();
  customDataError.value = "";
}

function patchVersionForm(changes: Partial<VersionForm>) {
  versionForm.value = { ...versionForm.value, ...changes };
}

function patchStorage(index: number, changes: Partial<StorageLocationRecord>) {
  patchVersionForm({
    storage: versionForm.value.storage.map((row, position) =>
      position === index ? { ...row, ...changes } : row,
    ),
  });
}

async function load() {
  try {
    const asset = await StudioApi.getContent<AssetRecord>("assets", assetId);
    draft.value = asset;
    savedJson.value = JSON.stringify(asset);
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  }
}

async function loadReferences() {
  referencesLoading.value = true;
  try {
    const result = await StudioApi.getReferences("assets", assetId);
    references.value = result.references;
  } catch {
    references.value = [];
  } finally {
    referencesLoading.value = false;
  }
}

async function save() {
  if (!draft.value) return false;
  saving.value = true;
  try {
    const saved = await StudioApi.saveContent<AssetRecord>("assets", syncFolder(draft.value));
    draft.value = saved;
    savedJson.value = JSON.stringify(saved);
    await refreshWorkspace(["assets"]);
    return true;
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
    return false;
  } finally {
    saving.value = false;
  }
}

function syncFolder(record: AssetRecord): AssetRecord {
  const storedFolder = currentFolderId("assets", assetId);
  return storedFolder === undefined || storedFolder === record.folder_id
    ? record
    : { ...record, folder_id: storedFolder };
}

async function persist(record: AssetRecord): Promise<AssetRecord | null> {
  try {
    const saved = await StudioApi.saveContent<AssetRecord>("assets", syncFolder(record));
    draft.value = saved;
    savedJson.value = JSON.stringify(saved);
    await refreshWorkspace(["assets"]);
    return saved;
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
    return null;
  }
}

async function addVersion() {
  if (!draft.value) return;
  const form = versionForm.value;
  customDataError.value = "";
  let customData: Record<string, unknown> = {};
  try {
    const parsed = form.custom_data.trim() ? JSON.parse(form.custom_data) : {};
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      customDataError.value = "Custom data must be a JSON object";
      return;
    }
    customData = parsed as Record<string, unknown>;
  } catch {
    customDataError.value = "Custom data must be valid JSON";
    return;
  }
  const version: AssetVersionRecord = {
    id: crypto.randomUUID(),
    file_path: form.file_path.trim(),
    creator: form.creator.trim(),
    tool_version: form.tool_version.trim(),
    description: form.description.trim() || null,
    created_at: Date.now(),
    tags: form.tags,
    custom_data: customData,
    thumbnail: form.thumbnail,
    storage: form.storage.filter((row) => row.storage_id.trim() || row.path.trim()),
  };
  saving.value = true;
  const saved = await persist({
    ...draft.value,
    versions: [...(draft.value.versions ?? []), version],
  });
  saving.value = false;
  if (saved) {
    showVersionForm.value = false;
    versionForm.value = newVersionForm();
    notify("Version appended", "success");
    await loadReferences();
  }
}

async function remove() {
  if (!draft.value) return;
  if (!window.confirm(`Delete asset '${draft.value.name}'?`)) return;
  try {
    await StudioApi.deleteContent("assets", assetId);
    await refreshWorkspace(["assets"]);
    notify("Asset deleted", "success");
    const folderId = draft.value.folder_id ?? null;
    router.push(folderId ? `/browse/assets/${folderId}` : "/browse/assets");
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  }
}

onMounted(async () => {
  await load();
  await loadReferences();
});

function confirmDiscard(): boolean {
  if (!dirty.value && !hasPendingVersion.value) return true;
  return window.confirm("Discard unsaved changes?");
}

onBeforeRouteLeave(confirmDiscard);
onBeforeRouteUpdate(confirmDiscard);
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <h1 class="view-title">{{ draft?.name ?? "Asset" }}</h1>
      <div style="display:flex; gap:8px;">
        <button class="btn btn-secondary" :disabled="saving || !dirty" @click="save">Save</button>
        <button class="btn btn-danger" :disabled="saving || !draft" @click="remove">Delete</button>
      </div>
    </header>

    <div v-if="!draft" class="view-content" style="padding:40px; color:var(--text-muted);">Loading asset…</div>

    <div v-else class="view-content editor-surface">
      <section class="editor-column">
        <div class="form-group">
          <label class="form-label">Name</label>
          <input class="form-input" :value="draft.name" @input="patch({ name: ($event.target as HTMLInputElement).value })" />
        </div>
        <div class="form-group">
          <label class="form-label">Thumbnail</label>
          <ThumbnailPicker :model-value="draft.thumbnail ?? null" :label="draft.name" @update:model-value="patch({ thumbnail: $event })" />
        </div>
        <div class="form-group">
          <label class="form-label">Tags</label>
          <TagEditor
            :model-value="draft.tags"
            :options="tagOptions"
            label="Asset tags"
            @update:model-value="(value) => patch({ tags: Array.isArray(value) ? value : [value] })"
          />
        </div>

        <h3 class="editor-heading">Versions</h3>
        <div v-if="!draft.versions.length" class="form-hint">No versions yet.</div>
        <div v-for="(version, index) in versionsNewestFirst" :key="version.id" class="version-card">
          <div class="version-card__head">
            <strong>{{ version.file_path }}</strong>
            <span v-if="index === 0" class="badge approved">Latest</span>
          </div>
          <dl class="version-card__meta">
            <dt>Creator</dt><dd>{{ version.creator }}</dd>
            <dt>Tool</dt><dd>{{ version.tool_version }}</dd>
            <dt>Created</dt><dd>{{ new Date(version.created_at).toLocaleString() }}</dd>
            <dt>Description</dt><dd>{{ version.description || "—" }}</dd>
            <dt>Tags</dt><dd>{{ version.tags.length ? version.tags.join(", ") : "—" }}</dd>
            <dt v-if="version.thumbnail">Thumbnail</dt>
            <dd v-if="version.thumbnail">
              <img
                :src="thumbnailUrl(version.thumbnail) ?? ''"
                alt="Version thumbnail"
                style="width:48px; height:48px; object-fit:cover; border-radius:4px;"
              />
            </dd>
            <dt v-if="version.custom_data && Object.keys(version.custom_data).length > 0">Custom data</dt>
            <dd v-if="version.custom_data && Object.keys(version.custom_data).length > 0">
              <pre style="font-family:var(--font-mono); font-size:11px; white-space:pre-wrap;">{{ JSON.stringify(version.custom_data) }}</pre>
            </dd>
          </dl>
          <table v-if="version.storage.length" class="version-card__storage">
            <thead>
              <tr><th>Storage</th><th>ID</th><th>Reference</th><th>Path</th></tr>
            </thead>
            <tbody>
              <tr v-for="(row, rowIndex) in version.storage" :key="rowIndex">
                <td>{{ row.storage_type }}</td>
                <td>{{ row.storage_id }}</td>
                <td>{{ row.reference_type }}</td>
                <td>{{ row.path }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <button class="btn btn-secondary" @click="toggleVersionForm">
          {{ showVersionForm ? "Cancel new version" : "+ Add Version" }}
        </button>

        <div v-if="showVersionForm" class="version-form">
          <div class="form-group">
            <label class="form-label">File path</label>
            <input class="form-input" :value="versionForm.file_path" @input="patchVersionForm({ file_path: ($event.target as HTMLInputElement).value })" />
          </div>
          <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px;">
            <div class="form-group">
              <label class="form-label">Creator</label>
              <input class="form-input" :value="versionForm.creator" @input="patchVersionForm({ creator: ($event.target as HTMLInputElement).value })" />
            </div>
            <div class="form-group">
              <label class="form-label">Tool version</label>
              <input class="form-input" :value="versionForm.tool_version" @input="patchVersionForm({ tool_version: ($event.target as HTMLInputElement).value })" />
            </div>
          </div>
          <div class="form-group">
            <label class="form-label">Description</label>
            <input class="form-input" :value="versionForm.description" @input="patchVersionForm({ description: ($event.target as HTMLInputElement).value })" />
          </div>
          <div class="form-group">
            <label class="form-label">Tags</label>
            <TagEditor
              :model-value="versionForm.tags"
              :options="tagOptions"
              label="Version tags"
              @update:model-value="(value) => patchVersionForm({ tags: Array.isArray(value) ? value : [value] })"
            />
          </div>
          <div class="form-group">
            <label class="form-label">Thumbnail</label>
            <ThumbnailPicker :model-value="versionForm.thumbnail" :label="draft.name" @update:model-value="patchVersionForm({ thumbnail: $event })" />
          </div>
          <div class="form-group">
            <label class="form-label">Custom data (JSON object)</label>
            <textarea
              class="form-textarea"
              style="min-height:80px; font-family:var(--font-mono);"
              :value="versionForm.custom_data"
              @input="patchVersionForm({ custom_data: ($event.target as HTMLTextAreaElement).value })"
            ></textarea>
            <span v-if="customDataError" class="property-list__error">{{ customDataError }}</span>
          </div>

          <h4 class="form-label">Storage locations</h4>
          <div
            v-for="(row, index) in versionForm.storage"
            :key="index"
            style="display:grid; grid-template-columns:1fr 1fr 1fr 1.5fr auto; gap:8px; align-items:end; margin-bottom:8px;"
          >
            <label class="form-group">
              <span class="form-label">Type</span>
              <select class="form-select" :value="row.storage_type" @change="patchStorage(Number(index), { storage_type: ($event.target as HTMLSelectElement).value })">
                <option value="disk">Disk</option>
                <option value="git">Git</option>
                <option value="perforce">Perforce</option>
              </select>
            </label>
            <label class="form-group">
              <span class="form-label">Storage ID</span>
              <input class="form-input" :value="row.storage_id" @input="patchStorage(Number(index), { storage_id: ($event.target as HTMLInputElement).value })" />
            </label>
            <label class="form-group">
              <span class="form-label">Reference</span>
              <select class="form-select" :value="row.reference_type" @change="patchStorage(Number(index), { reference_type: ($event.target as HTMLSelectElement).value })">
                <option value="file">File</option>
                <option value="commit">Commit</option>
                <option value="changelist">Changelist</option>
                <option value="snapshot">Snapshot</option>
              </select>
            </label>
            <label class="form-group">
              <span class="form-label">Path</span>
              <input class="form-input" :value="row.path" @input="patchStorage(Number(index), { path: ($event.target as HTMLInputElement).value })" />
            </label>
            <button
              class="btn btn-danger"
              type="button"
              @click="patchVersionForm({ storage: versionForm.storage.filter((_row, position) => position !== Number(index)) })"
            >Remove</button>
          </div>
          <button class="btn btn-secondary" type="button" @click="patchVersionForm({ storage: [...versionForm.storage, emptyStorage()] })">
            + Add storage
          </button>

          <div style="margin-top:14px;">
            <button class="btn btn-primary" :disabled="saving" @click="addVersion">Save version</button>
          </div>
        </div>

        <h3 class="editor-heading">Used by</h3>
        <ReferenceList :references="references" :loading="referencesLoading" />
      </section>
    </div>
  </div>
</template>
