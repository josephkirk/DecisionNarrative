<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { StudioApi } from "../../api.ts";
import { notify, studioState } from "../../state.ts";
import { refreshWorkspace } from "../../view-models/workspace.ts";
import type { NarrativeNode } from "../../narrativeGraphTypes.ts";
import type { AnyEntity } from "../runtime.ts";
import {
  CANVAS_PROPERTY_KEY, emptyCanvas, newId, nodeVersion, snapshotOf, syncCanvasProperties, upsertNodeVersion, versionChanges,
  type CanvasDoc, type ModifyCanvasConfig,
} from "./model.ts";
import { canvasOf } from "./modifyCanvasNode.ts";
import { allEntities, refName } from "./refs.ts";
import { hasCanvasTrait } from "./trait.ts";
import CanvasShapeView from "./CanvasShapeView.vue";
import VectorCanvas from "./VectorCanvas.vue";

/**
 * Modify Canvas inspector. Selecting the node opens the full canvas editor on
 * a working copy (undo only, no history). Saving writes the result as the
 * node's single version in the entity's canvas history.
 */
const props = defineProps<{ node: NarrativeNode; config: ModifyCanvasConfig }>();
const emit = defineEmits<{ (e: "update:config", value: ModifyCanvasConfig): void }>();
const route = useRoute();

const entities = computed(() => allEntities());
const withCanvas = computed(() => entities.value.filter((entity) =>
  hasCanvasTrait(entity, studioState.traitTemplates) || entity.properties.some((property) => property.key === CANVAS_PROPERTY_KEY)));
const pickedEntity = ref(props.config.canvas_entity);
const target = computed(() => entities.value.find((entity) => entity.id === pickedEntity.value));
const baseDoc = computed<CanvasDoc | null>(() => target.value ? canvasOf(target.value) ?? emptyCanvas() : null);
const version = computed(() => baseDoc.value && pickedEntity.value === props.config.canvas_entity ? nodeVersion(baseDoc.value, props.config.version_id) : undefined);
const changes = computed(() => baseDoc.value && version.value ? versionChanges(baseDoc.value, version.value.snapshot, refName) : []);

const storyletId = computed(() => typeof route.params.id === "string" ? route.params.id : "");
const storyletTitle = computed(() => studioState.storylets.find((storylet) => storylet.id === storyletId.value)?.title ?? (storyletId.value || "storylet"));

const open = ref(false);
const working = ref<CanvasDoc | null>(null);
const editorKey = ref(0);
const saving = ref(false);
const uid = newId("mc");

/** Working copy: the node's saved version, or the entity's canvas when there is none. */
function startEditing() {
  const base = baseDoc.value;
  if (!base) { working.value = null; return; }
  const snapshot = version.value?.snapshot ?? snapshotOf(base);
  working.value = { ...base, ...snapshotOf(snapshot), revisions: [] };
  editorKey.value++;
}
function openEditor() { open.value = true; startEditing(); }
function closeEditor() { open.value = false; working.value = null; }
function pickEntity(id: string) { pickedEntity.value = id; startEditing(); }

onMounted(openEditor);
watch(() => props.node.id, () => { pickedEntity.value = props.config.canvas_entity; openEditor(); });

async function saveVersion() {
  const entity = target.value;
  if (!entity || !working.value) return;
  saving.value = true;
  try {
    const kind = studioState.entities.some((row) => row.id === entity.id) ? "entities" : "entity_templates";
    const fresh = await StudioApi.getContent<AnyEntity>(kind, entity.id);
    const doc = canvasOf(fresh) ?? emptyCanvas();
    const reuse = pickedEntity.value === props.config.canvas_entity && props.config.version_id;
    const id = reuse ? props.config.version_id : newId("cv_");
    const next = upsertNodeVersion(doc, {
      id, label: `Modify Canvas · ${storyletTitle.value}`,
      source: { storylet_id: storyletId.value, storylet_title: storyletTitle.value, node_id: props.node.id },
      snapshot: working.value,
    });
    await StudioApi.saveContent(kind, { ...fresh, properties: syncCanvasProperties(fresh.properties, next) });
    await refreshWorkspace([kind]);
    emit("update:config", { canvas_entity: entity.id, version_id: id });
    notify(`Canvas version saved to ${entity.name}. Save the storylet draft to apply it.`, "success");
    closeEditor();
  } catch (error) {
    notify(`Could not save the canvas version: ${error instanceof Error ? error.message : String(error)}`, "error");
  } finally {
    saving.value = false;
  }
}

const preview = computed(() => version.value?.snapshot ?? null);
const viewBox = computed(() => {
  const shapes = preview.value?.shapes ?? [];
  if (!shapes.length) return "0 0 400 240";
  const minX = Math.min(...shapes.map((s) => s.x)) - 40; const minY = Math.min(...shapes.map((s) => s.y)) - 40;
  const maxX = Math.max(...shapes.map((s) => s.x + s.w)) + 40; const maxY = Math.max(...shapes.map((s) => s.y + s.h)) + 40;
  return `${minX} ${minY} ${maxX - minX} ${maxY - minY}`;
});
</script>

<template>
  <div class="mc-root" data-testid="modify-canvas-inspector">
    <p class="field-sublabel">
      <template v-if="target">Canvas: <strong>{{ target.name }}</strong></template>
      <template v-else>No canvas chosen.</template>
    </p>
    <button class="btn btn-primary" data-testid="open-modify-canvas" @click="openEditor">Open canvas</button>

    <template v-if="version">
      <div class="form-group">
        <label class="field-label">Version</label>
        <p class="field-sublabel">{{ version.label }} · {{ new Date(version.at).toLocaleString() }}</p>
      </div>
      <div class="form-group">
        <label class="field-label">Gameplay changes when it runs</label>
        <ul class="mc-ops">
          <li v-for="change in changes" :key="change.key">{{ change.text }}</li>
          <li v-if="!changes.length" class="field-sublabel">Drawing changes only (no markers or references change).</li>
        </ul>
      </div>
      <svg class="mc-preview" :viewBox="viewBox" preserveAspectRatio="xMidYMid meet" aria-label="Canvas preview">
        <template v-for="shape in preview?.shapes ?? []" :key="shape.id">
          <g v-if="preview?.layers.find((layer) => layer.id === shape.layer)?.visible !== false">
            <CanvasShapeView :shape="shape" :runtime="true" :uid="uid" />
          </g>
        </template>
      </svg>
    </template>
    <p v-else-if="target" class="field-sublabel">No version yet. Open the canvas, edit it, and save.</p>

    <Teleport to="body">
      <div v-if="open" class="mc-overlay" role="dialog" aria-modal="true" aria-label="Modify Canvas editor" data-testid="modify-canvas-editor">
        <header class="mc-overlay-head">
          <div class="mc-overlay-title">
            <strong>Modify Canvas</strong>
            <select class="form-select" :value="pickedEntity" aria-label="Canvas entity" @change="pickEntity(($event.target as HTMLSelectElement).value)">
              <option value="">Choose an entity with a canvas…</option>
              <option v-for="entity in withCanvas" :key="entity.id" :value="entity.id">{{ entity.name }}</option>
            </select>
            <small class="field-sublabel">Edits here have no history. Saving adds one version to the entity's canvas history; this node applies it when it runs.</small>
          </div>
          <div class="mc-overlay-actions">
            <button class="btn btn-secondary" @click="closeEditor">Cancel</button>
            <button class="btn btn-primary" :disabled="!working || saving" @click="saveVersion">{{ saving ? "Saving…" : "Save version" }}</button>
          </div>
        </header>
        <div class="mc-overlay-body">
          <VectorCanvas v-if="working && target" :key="editorKey" mode="node" :entity="target" :initial-doc="working" @change="working = $event" />
          <p v-else-if="!withCanvas.length" class="field-sublabel">Give an entity the <strong>Canvas</strong> trait and draw on its Canvas tab first.</p>
          <p v-else class="field-sublabel">Choose the entity whose canvas this node changes.</p>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.mc-root { display: flex; flex-direction: column; gap: 10px; }
.mc-ops { margin: 4px 0; padding-left: 18px; display: flex; flex-direction: column; gap: 4px; font-size: 13px; }
.mc-preview { width: 100%; height: 200px; border: 1px solid var(--border, #333); border-radius: 8px; background: var(--surface-1, #111); }
.mc-overlay { position: fixed; inset: 0; z-index: 1000; display: flex; flex-direction: column; gap: 10px; padding: 14px 18px; background: var(--bg-app, #0b0b0d); overflow: auto; }
.mc-overlay-head { display: flex; justify-content: space-between; align-items: flex-start; gap: 12px; flex-wrap: wrap; }
.mc-overlay-title { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; min-width: 0; }
.mc-overlay-title .form-select { width: auto; min-width: 220px; }
.mc-overlay-title small { flex-basis: 100%; }
.mc-overlay-actions { display: flex; gap: 8px; }
.mc-overlay-body { flex: 1; min-height: 0; }
</style>
