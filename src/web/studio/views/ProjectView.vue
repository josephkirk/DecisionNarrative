<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useProjectViewModel } from "../view-models/project.ts";
import { StudioApi } from "../api.ts";
import { notify } from "../state.ts";

const {
  metadata,
  loading,
  loadProject,
  saveName,
} = useProjectViewModel();
const nameInput = ref("");
const fileInput = ref<HTMLInputElement | null>(null);
const pendingBundle = ref<any>(null);
const validationIssues = ref<string[]>([]);
const isValid = ref(false);

onMounted(async () => {
  await loadProject();
  nameInput.value = metadata.name;
});

function save() {
  saveName(nameInput.value);
}

async function onFileSelected(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  try {
    const text = await file.text();
    pendingBundle.value = JSON.parse(text);
    const res = await StudioApi.validateBundle(pendingBundle.value);
    isValid.value = res.valid;
    validationIssues.value = res.issues;
    if (res.valid) {
      notify("Project bundle validated successfully. Ready to import.", "success");
    } else {
      notify("Project bundle has validation issues.", "error");
    }
  } catch (err) {
    notify("Invalid JSON file selected.", "error");
  }
}

async function handleImport() {
  if (!pendingBundle.value || !isValid.value) return;
  if (!window.confirm("Replace entire workspace with the reviewed project bundle? Current uncommitted data will be replaced.")) return;
  try {
    await StudioApi.importBundle(pendingBundle.value);
    notify("Imported project bundle successfully.", "success");
    pendingBundle.value = null;
    isValid.value = false;
    await loadProject();
  } catch (err) {
    notify(err instanceof Error ? err.message : String(err), "error");
  }
}
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <h1 class="view-title">Project Settings & Bundle Interchange</h1>
    </header>

    <div class="view-content" style="max-width:680px; margin:0 auto; width:100%;">
      <!-- Project Name -->
      <div class="form-group" style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:20px;">
        <label class="form-label">Project Name</label>
        <div style="display:flex; gap:10px;">
          <input v-model="nameInput" class="form-input" />
          <button class="btn btn-primary" @click="save">Save Name</button>
        </div>
      </div>

      <!-- Export Bundle -->
      <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:20px;">
        <h3 style="font-size:14px; font-weight:700; margin-bottom:8px;">EXPORT PROJECT</h3>
        <p style="font-size:12px; color:var(--text-muted); margin-bottom:12px;">
          Download complete portable project bundle v2 (Story Codex, storylets, and scenarios).
        </p>
        <a href="/api/v2/project-bundle" class="btn btn-secondary" download>Export .storylet.json</a>
      </div>

      <!-- Import Bundle -->
      <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px;">
        <h3 style="font-size:14px; font-weight:700; margin-bottom:8px;">IMPORT & REPLACE WORKSPACE</h3>
        <p style="font-size:12px; color:var(--text-muted); margin-bottom:12px;">
          Inspect and validate an external project bundle before replacing your active workspace.
        </p>

        <div style="display:flex; gap:10px; align-items:center; margin-bottom:12px;">
          <input ref="fileInput" type="file" accept=".json,application/json" class="form-input" style="flex:1;" @change="onFileSelected" />
          <button
            class="btn btn-primary"
            :disabled="!isValid"
            @click="handleImport"
          >
            Replace Project Content
          </button>
        </div>

        <div v-if="validationIssues.length > 0" style="padding:10px 14px; background:rgba(255,59,48,0.1); border-radius:8px; color:var(--status-danger); font-size:12px;">
          <strong>Validation Issues:</strong>
          <ul style="margin-left:16px; margin-top:4px;">
            <li v-for="(iss, idx) in validationIssues" :key="idx">{{ iss }}</li>
          </ul>
        </div>
      </div>
    </div>
  </div>
</template>
