<script setup lang="ts">
import { ref } from "vue";
import { notify } from "../state.ts";

const emit = defineEmits<{
  (e: "close"): void;
  (e: "imported"): void;
}>();

const activeTab = ref<"export" | "import">("export");
const importText = ref("");
const isUploading = ref(false);

async function onFileSelected(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  try {
    importText.value = await file.text();
    notify(`Loaded ${file.name} (${importText.value.length} chars)`, "info");
  } catch (err) {
    notify("Could not read file", "error");
  }
}

async function submitImport() {
  if (!importText.value.trim()) {
    notify("Paste or select a Twee or Twine HTML file to import", "error");
    return;
  }

  isUploading.value = true;
  try {
    const resp = await fetch("/api/v2/twine/import", {
      method: "POST",
      headers: { "Content-Type": "text/plain; charset=utf-8" },
      body: importText.value,
    });
    const res = await resp.json();
    if (!resp.ok) throw new Error(res.error || "Import failed");
    notify(`Successfully imported ${res.imported_count} storylet passages!`, "success");
    emit("imported");
    emit("close");
  } catch (err) {
    notify(err instanceof Error ? err.message : String(err), "error");
  } finally {
    isUploading.value = false;
  }
}
</script>

<template>
  <div class="modal-backdrop" @click.self="emit('close')">
    <div class="modal-card" style="width:540px; background:var(--bg-card); border-radius:14px; border:1px solid var(--border-subtle); box-shadow:var(--shadow-md); padding:20px;">
      <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:16px;">
        <h3 style="font-size:16px; font-weight:700;">Twine 2 & Twee 3 Interchange</h3>
        <button class="btn" style="padding:2px 8px; font-size:14px;" @click="emit('close')">✕</button>
      </div>

      <div style="display:flex; gap:8px; margin-bottom:16px; border-bottom:1px solid var(--border-subtle); padding-bottom:8px;">
        <button
          :class="['btn', activeTab === 'export' ? 'btn-primary' : 'btn-secondary']"
          @click="activeTab = 'export'"
        >
          Export Twine
        </button>
        <button
          :class="['btn', activeTab === 'import' ? 'btn-primary' : 'btn-secondary']"
          @click="activeTab = 'import'"
        >
          Import Twine / Twee
        </button>
      </div>

      <!-- EXPORT TAB -->
      <div v-if="activeTab === 'export'">
        <p style="font-size:12px; color:var(--text-muted); line-height:1.5; margin-bottom:16px;">
          Export all storylets and their choices as spec-compliant Twine 2 formats. Can be imported directly into Twine (twinery.org) or compiled with Tweego.
        </p>

        <div style="display:flex; flex-direction:column; gap:12px;">
          <a
            href="/api/v2/twine/export?format=html"
            download
            class="btn btn-secondary"
            style="justify-content:center; padding:12px; font-weight:600;"
          >
            Download Twine 2 HTML (.html)
          </a>

          <a
            href="/api/v2/twine/export?format=twee"
            download
            class="btn btn-secondary"
            style="justify-content:center; padding:12px; font-weight:600;"
          >
            Download Twee 3 Source (.twee)
          </a>
        </div>
      </div>

      <!-- IMPORT TAB -->
      <div v-else>
        <p style="font-size:12px; color:var(--text-muted); line-height:1.5; margin-bottom:12px;">
          Select or paste a Twee 3 source file (<code>.twee</code> / <code>.tw</code>) or a Twine 2 HTML output file.
        </p>

        <div style="margin-bottom:12px;">
          <input type="file" accept=".twee,.tw,.html" class="form-input" @change="onFileSelected" />
        </div>

        <div style="margin-bottom:16px;">
          <textarea
            v-model="importText"
            class="form-textarea"
            style="min-height:140px; font-family:var(--font-mono); font-size:11px;"
            placeholder="Or paste Twee 3 text (:: PassageName ...) here..."
          ></textarea>
        </div>

        <button
          class="btn btn-primary"
          style="width:100%; justify-content:center;"
          :disabled="isUploading || !importText.trim()"
          @click="submitImport"
        >
          {{ isUploading ? 'Importing passages...' : 'Import into Storylet Library' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.45);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
</style>
