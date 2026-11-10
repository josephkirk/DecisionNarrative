<script setup lang="ts">
import { computed, onMounted, ref, useId } from "vue";
import { onBeforeRouteLeave, onBeforeRouteUpdate, useRoute, useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import type { AssetRecord, AssetVersionRecord, ContentReference, StorageLocationRecord } from "../contracts.ts";
import { notify } from "../state.ts";
import { currentFolderId, refreshWorkspace } from "../view-models/workspace.ts";
import ReferenceList from "../components/ReferenceList.vue";
import TagEditor from "../components/TagEditor.vue";
import ThumbnailPicker from "../components/ThumbnailPicker.vue";
import { thumbnailUrl } from "../thumbnails.ts";
import { countLabel } from "../countLabel.ts";
const route = useRoute();
const router = useRouter();
const assetId = typeof route.params.id === "string" ? route.params.id : "";
const fieldId = useId();

const draft = ref<AssetRecord | null>(null);
const savedJson = ref("");
const saving = ref(false);
const references = ref<ContentReference[]>([]);
const referencesLoading = ref(false);
const showVersionForm = ref(false);
const customDataError = ref("");
const activeTab = ref<"details" | "versions" | "references">("details");
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
          @update:model-value="patch({ thumbnail: $event })"
        />

        <div class="entity-hero-meta">
          <div class="entity-title-row">
            <input
              class="entity-hero-input"
              :value="draft.name"
              placeholder="Asset name…"
              aria-label="Asset name"
              @input="patch({ name: ($event.target as HTMLInputElement).value })"
            />
            <span class="entity-id-tag">#{{ (draft.id || '').slice(-6) }}</span>
          </div>

          <div class="entity-subtitle-row">
            <span v-if="draft.tags.length" class="entity-type-label">{{ draft.tags.join(" · ") }}</span>
            <span v-else class="entity-standalone-text">No tags</span>
            <span class="entity-bullet-sep" aria-hidden="true">•</span>
            <span class="entity-parent-link">{{ countLabel(draft.versions.length, "version") }}</span>
            <span v-if="dirty || hasPendingVersion" class="entity-unsaved-pill">Unsaved</span>
          </div>
        </div>
      </div>

      <div class="entity-hero-right">
        <div v-if="draft.versions.length" class="entity-runtime-badge">
          <span class="runtime-key">File:</span>
          <code class="runtime-val" :title="draft.versions[draft.versions.length - 1].file_path">
            {{ draft.versions[draft.versions.length - 1].file_path }}
          </code>
        </div>
        <div class="entity-hero-buttons">
          <button class="btn btn-primary" :disabled="saving || !dirty" @click="save">Save</button>
          <button class="btn btn-secondary btn-danger-hover" :disabled="saving || !draft" @click="remove">Delete</button>
        </div>
      </div>
    </header>

    <!-- Tabs matching EntityView -->
    <nav class="entity-tabs" aria-label="Asset sections" @wheel="scrollTabs">
      <button :class="['entity-tab', activeTab === 'details' && 'is-active']" @click="activeTab = 'details'">
        <svg aria-hidden="true" class="tab-glyph" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="3" y1="6" x2="21" y2="6"/>
          <line x1="3" y1="12" x2="21" y2="12"/>
          <line x1="3" y1="18" x2="21" y2="18"/>
        </svg>
        Details
      </button>
      <button :class="['entity-tab', activeTab === 'versions' && 'is-active']" @click="activeTab = 'versions'">
        Versions
        <span class="tab-badge">{{ draft.versions.length }}</span>
      </button>
      <button :class="['entity-tab', activeTab === 'references' && 'is-active']" @click="activeTab = 'references'; loadReferences()">
        References
        <span class="tab-badge">{{ references.length }}</span>
      </button>
    </nav>

    <!-- Details Tab -->
    <section v-show="activeTab === 'details'" class="entity-content-stack">
      <!-- 1. Tags card -->
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">Asset tags</span>
          <span class="defined-pill">{{ countLabel(draft.tags.length, "tag") }}</span>
        </div>
        <TagEditor
          :model-value="draft.tags"
          :options="tagOptions"
          label="Asset tags"
          @update:model-value="(value) => patch({ tags: Array.isArray(value) ? value : [value] })"
        />
      </div>

      <!-- 2. Overview / File Info Card -->
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">Asset overview</span>
          <span class="block-hint">Version history & reference status</span>
        </div>
        <div style="display:grid; grid-template-columns:repeat(auto-fit, minmax(200px, 1fr)); gap:12px;">
          <div style="padding:12px; background:rgba(255, 255, 255, 0.03); border:1px solid var(--border-subtle); border-radius:var(--radius-md, 8px);">
            <div style="font-size:11px; color:var(--text-muted); margin-bottom:4px;">Total Versions</div>
            <div style="font-size:20px; font-weight:600; color:var(--text-main); font-family:var(--font-mono);">{{ draft.versions.length }}</div>
          </div>
          <div style="padding:12px; background:rgba(255, 255, 255, 0.03); border:1px solid var(--border-subtle); border-radius:var(--radius-md, 8px);">
            <div style="font-size:11px; color:var(--text-muted); margin-bottom:4px;">References</div>
            <div style="font-size:20px; font-weight:600; color:var(--text-main); font-family:var(--font-mono);">{{ references.length }}</div>
          </div>
          <div v-if="draft.versions.length" style="padding:12px; background:rgba(255, 255, 255, 0.03); border:1px solid var(--border-subtle); border-radius:var(--radius-md, 8px);">
            <div style="font-size:11px; color:var(--text-muted); margin-bottom:4px;">Latest Creator</div>
            <div :title="draft.versions[draft.versions.length - 1].creator" style="font-size:14px; font-weight:500; color:var(--text-main); overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">
              {{ draft.versions[draft.versions.length - 1].creator || "—" }}
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Versions Tab -->
    <!-- Preserved across tab switches with v-show as requested -->
    <section v-show="activeTab === 'versions'" class="entity-content-stack">
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">Asset versions</span>
          <div style="display:flex; align-items:center; gap:8px;">
            <span class="defined-pill">{{ countLabel(draft.versions.length, "version") }}</span>
            <button class="btn btn-secondary btn-compact" type="button" @click="toggleVersionForm">
              {{ showVersionForm ? "Cancel new version" : "+ Add version" }}
            </button>
          </div>
        </div>

        <div v-if="!draft.versions.length && !showVersionForm" class="form-hint" style="color:var(--text-muted);">
          No versions yet. Click "+ Add version" to register the first file version.
        </div>

        <!-- Version form card when toggled -->
        <div v-if="showVersionForm" class="version-form" style="background:rgba(255, 255, 255, 0.02); margin-top:0; margin-bottom:12px;">
          <div class="content-block-head" style="margin-bottom:12px;">
            <span style="font-size:13px; font-weight:600; color:var(--text-main);">New Asset Version</span>
            <span class="badge" style="font-size:10px;">Pending</span>
          </div>

          <div class="form-group">
            <label class="form-label" :for="`${fieldId}-path`">File path</label>
            <input :id="`${fieldId}-path`" class="form-input" aria-label="File path" :value="versionForm.file_path" @input="patchVersionForm({ file_path: ($event.target as HTMLInputElement).value })" />
          </div>
          <div class="form-grid-2col">
            <div class="form-group">
              <label class="form-label" :for="`${fieldId}-creator`">Creator</label>
              <input :id="`${fieldId}-creator`" class="form-input" aria-label="Creator" :value="versionForm.creator" @input="patchVersionForm({ creator: ($event.target as HTMLInputElement).value })" />
            </div>
            <div class="form-group">
              <label class="form-label" :for="`${fieldId}-tool`">Tool version</label>
              <input :id="`${fieldId}-tool`" class="form-input" aria-label="Tool version" :value="versionForm.tool_version" @input="patchVersionForm({ tool_version: ($event.target as HTMLInputElement).value })" />
            </div>
          </div>
          <div class="form-group">
            <label class="form-label" :for="`${fieldId}-description`">Description</label>
            <input :id="`${fieldId}-description`" class="form-input" aria-label="Version description" :value="versionForm.description" @input="patchVersionForm({ description: ($event.target as HTMLInputElement).value })" />
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
            <label class="form-label" :for="`${fieldId}-custom-data`">Custom data (JSON object)</label>
            <textarea
              :id="`${fieldId}-custom-data`"
              class="form-textarea"
              style="min-height:80px; font-family:var(--font-mono);"
              aria-label="Custom data (JSON object)"
              :value="versionForm.custom_data"
              @input="patchVersionForm({ custom_data: ($event.target as HTMLTextAreaElement).value })"
            ></textarea>
            <span v-if="customDataError" class="property-list__error">{{ customDataError }}</span>
          </div>

          <h4 class="form-label">Storage locations</h4>
          <div
            v-for="(row, index) in versionForm.storage"
            :key="index"
            class="storage-location-row"
          >
            <label class="form-group storage-field" style="margin:0;">
              <span class="form-label">Type</span>
              <select class="form-select" :aria-label="`Storage ${index + 1} type`" :value="row.storage_type" @change="patchStorage(Number(index), { storage_type: ($event.target as HTMLSelectElement).value })">
                <option value="disk">Disk</option>
                <option value="git">Git</option>
                <option value="perforce">Perforce</option>
              </select>
            </label>
            <label class="form-group storage-field" style="margin:0;">
              <span class="form-label">Storage ID</span>
              <input class="form-input" :aria-label="`Storage ${index + 1} ID`" :value="row.storage_id" @input="patchStorage(Number(index), { storage_id: ($event.target as HTMLInputElement).value })" />
            </label>
            <label class="form-group storage-field" style="margin:0;">
              <span class="form-label">Reference</span>
              <select class="form-select" :aria-label="`Storage ${index + 1} reference`" :value="row.reference_type" @change="patchStorage(Number(index), { reference_type: ($event.target as HTMLSelectElement).value })">
                <option value="file">File</option>
                <option value="commit">Commit</option>
                <option value="changelist">Changelist</option>
                <option value="snapshot">Snapshot</option>
              </select>
            </label>
            <label class="form-group storage-field storage-field--path" style="margin:0;">
              <span class="form-label">Path</span>
              <input class="form-input" :aria-label="`Storage ${index + 1} path`" :value="row.path" @input="patchStorage(Number(index), { path: ($event.target as HTMLInputElement).value })" />
            </label>
            <div class="storage-field storage-field--remove">
              <button
                class="btn btn-secondary btn-danger-hover btn-compact"
                type="button"
                :aria-label="`Remove storage location ${index + 1}`"
                @click="patchVersionForm({ storage: versionForm.storage.filter((_row, position) => position !== Number(index)) })"
              >Remove</button>
            </div>
          </div>
          <button class="btn btn-secondary btn-compact" type="button" @click="patchVersionForm({ storage: [...versionForm.storage, emptyStorage()] })">
            + Add storage
          </button>

          <div style="margin-top:14px; display:flex; gap:8px;">
            <button class="btn btn-primary" :disabled="saving" @click="addVersion">Save version</button>
            <button class="btn btn-secondary" type="button" @click="toggleVersionForm">Cancel</button>
          </div>
        </div>

        <!-- Versions list -->
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
                width="48"
                height="48"
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
      </div>
    </section>

    <!-- References Tab -->
    <section v-show="activeTab === 'references'" class="entity-content-stack">
      <div class="content-block">
        <div class="content-block-head">
          <span class="block-title">References to this asset</span>
          <span class="defined-pill">{{ countLabel(references.length, "reference") }}</span>
        </div>
        <ReferenceList :references="references" :loading="referencesLoading" :target-name="draft?.name" />
      </div>
    </section>
  </div>

  <!-- Loading Skeleton -->
  <div v-else class="entity-page-surface" aria-label="Loading asset">
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
