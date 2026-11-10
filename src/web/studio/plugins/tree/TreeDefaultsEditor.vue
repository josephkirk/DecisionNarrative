<script setup lang="ts">
import { computed, ref } from "vue";
import ColorWheelPicker from "../../components/ColorWheelPicker.vue";
import { studioState } from "../../state.ts";
import type { AnyEntity } from "../runtime.ts";
import {
  CARD_HEIGHT,
  CARD_WIDTH,
  MAX_NODE_HEIGHT,
  MAX_NODE_WIDTH,
  MAX_OUTLINE_WIDTH,
  MAX_WIRE_WIDTH,
  MIN_NODE_HEIGHT,
  MIN_NODE_WIDTH,
  MIN_OUTLINE_WIDTH,
  MIN_WIRE_WIDTH,
  TREE_GRAPH_KEY,
  TREE_SHAPES,
  TREE_WIRE_PATTERNS,
  setTreeGraph,
  treeGraphOf,
  treeTrait,
  type TreeAppearance,
  type TreeEdgeAppearance,
  type TreeGraph,
  type TreeNodeAppearance,
  type TreeShape,
  type TreeWirePattern,
} from "./model.ts";

const props = defineProps<{
  entity: AnyEntity;
  readonly?: boolean;
}>();

const emit = defineEmits<{
  (event: "patch", patch: Partial<AnyEntity>): void;
}>();

const fileInputRef = ref<HTMLInputElement | null>(null);
const uploadedFileName = ref("");
const cssError = ref("");

const owner = computed(() => treeTrait(props.entity, studioState.traitTemplates, TREE_GRAPH_KEY));

const parsed = computed<{ graph: TreeGraph | null; error: string }>(() => {
  if (!owner.value) return { graph: null, error: "" };
  try {
    return { graph: treeGraphOf(owner.value), error: "" };
  } catch (error) {
    return { graph: null, error: error instanceof Error ? error.message : String(error) };
  }
});

const graph = computed(() => parsed.value.graph);
const appearance = computed(() => graph.value?.appearance);
const defaults = computed(() => appearance.value?.defaults);

const defaultShape = computed<TreeShape>(() => defaults.value?.node?.shape ?? "card");
const defaultNodeBackground = computed(() => defaults.value?.node?.background ?? "#1a1c20");
const defaultNodeOutline = computed(() => defaults.value?.node?.outline ?? "#06b6d4");
const defaultOutlineWidth = computed(() => defaults.value?.node?.outlineWidth ?? 1);
const defaultNodeWidth = computed(() => defaults.value?.node?.width ?? CARD_WIDTH);
const defaultNodeHeight = computed(() => defaults.value?.node?.height ?? CARD_HEIGHT);

const defaultWireColor = computed(() => defaults.value?.edge?.color ?? "#06b6d4");
const defaultWireWidth = computed(() => defaults.value?.edge?.width ?? 2);
const defaultWirePattern = computed<TreeWirePattern>(() => defaults.value?.edge?.pattern ?? "solid");

const canvasBackground = computed(() => appearance.value?.canvasBackground ?? "#1a1c20");
const hasCanvasBackground = computed(() => Boolean(appearance.value?.canvasBackground));

const customCssText = computed(() => defaults.value?.customCss ?? "");

function getBaseAppearance() {
  const current: TreeAppearance = graph.value?.appearance ?? { nodes: {}, edges: {} };
  return {
    ...current,
    nodes: current.nodes ?? {},
    edges: current.edges ?? {},
    defaults: {
      ...(current.defaults ?? {}),
    },
  };
}

function patchGraph(updatedAppearance: TreeAppearance) {
  if (!owner.value || !graph.value || props.readonly) return;
  const nextGraph: TreeGraph = {
    ...graph.value,
    appearance: updatedAppearance,
  };
  emit("patch", setTreeGraph(props.entity, owner.value, nextGraph));
}

function updateNodeDefault(update: Partial<TreeNodeAppearance & { shape?: TreeShape }>) {
  if (!owner.value || !graph.value || props.readonly) return;
  const app = getBaseAppearance();
  app.defaults = {
    ...app.defaults,
    node: {
      ...app.defaults.node,
      ...update,
    },
  };
  patchGraph(app);
}

function updateEdgeDefault(update: Partial<TreeEdgeAppearance>) {
  if (!owner.value || !graph.value || props.readonly) return;
  const app = getBaseAppearance();
  app.defaults = {
    ...app.defaults,
    edge: {
      ...app.defaults.edge,
      ...update,
    },
  };
  patchGraph(app);
}

function updateCanvasBackground(color?: string) {
  if (!owner.value || !graph.value || props.readonly) return;
  const app = getBaseAppearance();
  if (color === undefined) {
    delete app.canvasBackground;
  } else {
    app.canvasBackground = color;
  }
  patchGraph(app);
}

function resetCanvasBackground() {
  updateCanvasBackground(undefined);
}

function updateCustomCss(text: string) {
  if (!owner.value || !graph.value || props.readonly) return;
  const app = getBaseAppearance();
  app.defaults = {
    ...app.defaults,
    customCss: text,
  };
  patchGraph(app);
}

function onOutlineWidthChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const val = Math.max(MIN_OUTLINE_WIDTH, Math.min(MAX_OUTLINE_WIDTH, Number(input.value) || 1));
  input.value = String(val);
  updateNodeDefault({ outlineWidth: val });
}

function onNodeWidthChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const val = Math.max(MIN_NODE_WIDTH, Math.min(MAX_NODE_WIDTH, Number(input.value) || CARD_WIDTH));
  input.value = String(val);
  updateNodeDefault({ width: val });
}

function onNodeHeightChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const val = Math.max(MIN_NODE_HEIGHT, Math.min(MAX_NODE_HEIGHT, Number(input.value) || CARD_HEIGHT));
  input.value = String(val);
  updateNodeDefault({ height: val });
}

function onWireWidthChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const val = Math.max(MIN_WIRE_WIDTH, Math.min(MAX_WIRE_WIDTH, Number(input.value) || 1));
  input.value = String(val);
  updateEdgeDefault({ width: val });
}

function onCssFileUpload(event: Event) {
  if (props.readonly) return;
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;
  cssError.value = "";
  const reader = new FileReader();
  reader.onload = (e) => {
    const text = typeof e.target?.result === "string" ? e.target.result : "";
    uploadedFileName.value = file.name;
    updateCustomCss(text);
    target.value = "";
  };
  reader.onerror = () => { cssError.value = "Could not read the CSS file."; target.value = ""; };
  reader.readAsText(file);
}

function clearCustomCss() {
  if (props.readonly) return;
  uploadedFileName.value = "";
  cssError.value = "";
  updateCustomCss("");
  if (fileInputRef.value) {
    fileInputRef.value.value = "";
  }
}

function onCustomCssInput(event: Event) {
  if (props.readonly) return;
  const text = (event.target as HTMLTextAreaElement).value;
  updateCustomCss(text);
}
</script>

<template>
  <div class="tree-defaults-editor">
    <!-- Conditional parse error alert -->
    <div v-if="parsed.error" class="tree-defaults-error" role="alert">
      <strong>TreeGraph Trait Error:</strong> {{ parsed.error }}
    </div>

    <div v-else-if="!owner" class="tree-defaults-empty">
      No TreeGraph trait found on this entity.
    </div>

    <div v-else class="tree-defaults-sections">
      <!-- Section: Node Defaults -->
      <fieldset class="tree-defaults-group"><legend class="tree-defaults-legend">Node Defaults</legend>
        <div class="tree-defaults-grid">
          <div class="tree-defaults-field">
            <label for="tree-default-shape" class="field-label">Default shape</label>
            <select
              id="tree-default-shape"
              class="form-select"
              aria-label="Default node shape"
              :value="defaultShape"
              :disabled="readonly"
              @change="updateNodeDefault({ shape: ($event.target as HTMLSelectElement).value as TreeShape })"
            >
              <option v-for="shape in TREE_SHAPES" :key="shape" :value="shape">
                {{ shape.charAt(0).toUpperCase() + shape.slice(1) }}
              </option>
            </select>
          </div>

          <div class="tree-defaults-field">
            <span class="field-label">Default background</span>
            <div class="picker-row">
              <ColorWheelPicker
                label="Default node background color"
                :model-value="defaultNodeBackground"
                :disabled="readonly"
                @update:model-value="updateNodeDefault({ background: $event })"
              />
              <span class="color-value-text">{{ defaultNodeBackground }}</span>
            </div>
          </div>

          <div class="tree-defaults-field">
            <span class="field-label">Default outline</span>
            <div class="picker-row">
              <ColorWheelPicker
                label="Default node outline color"
                :model-value="defaultNodeOutline"
                :disabled="readonly"
                @update:model-value="updateNodeDefault({ outline: $event })"
              />
              <span class="color-value-text">{{ defaultNodeOutline }}</span>
            </div>
          </div>

          <div class="tree-defaults-field">
            <label for="tree-default-outline-width" class="field-label">Outline width (1–12 px)</label>
            <input
              id="tree-default-outline-width"
              type="number"
              class="form-input"
              min="1"
              max="12"
              :value="defaultOutlineWidth"
              :disabled="readonly"
              @change="onOutlineWidthChange"
            />
          </div>

          <div class="tree-defaults-field">
            <label for="tree-default-width" class="field-label">Node width (100–600 px)</label>
            <input
              id="tree-default-width"
              type="number"
              class="form-input"
              min="100"
              max="600"
              :value="defaultNodeWidth"
              :disabled="readonly"
              @change="onNodeWidthChange"
            />
          </div>

          <div class="tree-defaults-field">
            <label for="tree-default-height" class="field-label">Node height (60–400 px)</label>
            <input
              id="tree-default-height"
              type="number"
              class="form-input"
              min="60"
              max="400"
              :value="defaultNodeHeight"
              :disabled="readonly"
              @change="onNodeHeightChange"
            />
          </div>
        </div>
      </fieldset>

      <!-- Section: Wire Defaults -->
      <fieldset class="tree-defaults-group"><legend class="tree-defaults-legend">Wire Defaults</legend>
        <div class="tree-defaults-grid">
          <div class="tree-defaults-field">
            <span class="field-label">Default wire color</span>
            <div class="picker-row">
              <ColorWheelPicker
                label="Default wire color"
                :model-value="defaultWireColor"
                :disabled="readonly"
                @update:model-value="updateEdgeDefault({ color: $event })"
              />
              <span class="color-value-text">{{ defaultWireColor }}</span>
            </div>
          </div>

          <div class="tree-defaults-field">
            <label for="tree-default-wire-width" class="field-label">Wire thickness (1–12 px)</label>
            <input
              id="tree-default-wire-width"
              type="number"
              class="form-input"
              min="1"
              max="12"
              :value="defaultWireWidth"
              :disabled="readonly"
              @change="onWireWidthChange"
            />
          </div>

          <div class="tree-defaults-field">
            <label for="tree-default-wire-pattern" class="field-label">Default wire style</label>
            <select
              id="tree-default-wire-pattern"
              class="form-select"
              aria-label="Default wire pattern"
              :value="defaultWirePattern"
              :disabled="readonly"
              @change="updateEdgeDefault({ pattern: ($event.target as HTMLSelectElement).value as TreeWirePattern })"
            >
              <option v-for="pattern in TREE_WIRE_PATTERNS" :key="pattern" :value="pattern">
                {{ pattern.charAt(0).toUpperCase() + pattern.slice(1) }}
              </option>
            </select>
          </div>
        </div>
      </fieldset>

      <!-- Section: Canvas Background -->
      <fieldset class="tree-defaults-group"><legend class="tree-defaults-legend">Canvas Background</legend>
        <div class="tree-defaults-grid">
          <div class="tree-defaults-field">
            <span class="field-label">Background color</span>
            <div class="picker-row">
              <ColorWheelPicker
                label="Canvas background color"
                :model-value="canvasBackground"
                :disabled="readonly"
                @update:model-value="updateCanvasBackground($event)"
              />
              <span class="color-value-text">{{ canvasBackground }}</span>
              <button
                v-if="hasCanvasBackground && !readonly"
                type="button"
                class="btn btn-secondary btn-compact"
                title="Reset canvas background to default"
                aria-label="Reset canvas background to default"
                @click="resetCanvasBackground"
              >
                Reset
              </button>
            </div>
          </div>
        </div>
      </fieldset>

      <!-- Section: Custom CSS -->
      <fieldset class="tree-defaults-group"><legend class="tree-defaults-legend">Custom Tree CSS</legend>
        <p class="form-hint css-help">CSS applies only inside this tree canvas; @import is not supported.</p>

        <div class="css-toolbar">
          <label
            for="tree-css-upload-input"
            :class="['btn', 'btn-secondary', 'btn-compact', 'css-upload-label', { disabled: readonly }]"
          >
            Import .css file
          </label>
          <input
            id="tree-css-upload-input"
            ref="fileInputRef"
            type="file"
            accept=".css,text/css"
            class="sr-only"
            :disabled="readonly"
            aria-label="Upload CSS file"
            @change="onCssFileUpload"
          />
          <span class="css-status-text" aria-live="polite">
            {{ uploadedFileName ? uploadedFileName : (customCssText ? 'Custom CSS loaded' : 'No file chosen') }}
          </span>
          <span v-if="cssError" class="css-error" role="alert">{{ cssError }}</span>
          <button
            v-if="customCssText && !readonly"
            type="button"
            class="btn btn-secondary btn-compact"
            @click="clearCustomCss"
          >
            Clear CSS
          </button>
        </div>

        <div class="tree-defaults-field css-field">
          <label for="tree-custom-css-textarea" class="field-label">CSS rules</label>
          <textarea
            id="tree-custom-css-textarea"
            class="form-textarea tree-css-textarea"
            rows="5"
            placeholder="/* e.g. .tree-node { font-family: sans-serif; } */"
            :value="customCssText"
            :disabled="readonly"
            @input="onCustomCssInput"
          ></textarea>
        </div>
      </fieldset>
    </div>
  </div>
</template>

<style scoped>
.tree-defaults-editor {
  display: flex;
  flex-direction: column;
  gap: 16px;
  width: 100%;
}

.tree-defaults-error {
  padding: 10px 14px;
  border-radius: var(--radius-md, 8px);
  background: var(--status-danger-subtle, rgba(239, 68, 68, 0.15));
  border: 1px solid var(--status-danger, #ef4444);
  color: var(--status-danger, #ef4444);
  font-size: 13px;
}

.tree-defaults-empty {
  padding: 12px;
  color: var(--text-muted, #94a3b8);
  font-size: 13px;
  font-style: italic;
}

.tree-defaults-sections {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.tree-defaults-group {
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
  border-radius: var(--radius-md, 8px);
  padding: 14px;
  margin: 0;
  background: rgba(0, 0, 0, 0.15);
}

.tree-defaults-legend {
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--text-muted, #94a3b8);
  padding: 0 6px;
}

.tree-defaults-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(200px, 100%), 1fr));
  gap: 12px 16px;
  margin-top: 8px;
}

.tree-defaults-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.field-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-muted, #94a3b8);
}

.picker-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.color-value-text {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  color: var(--text-dim, #64748b);
}

.css-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 8px;
}
.css-help { margin-top: 0; }
.css-field { margin-top: 10px; }
.css-upload-label { cursor: pointer; }
.css-toolbar:focus-within .css-upload-label { outline: 2px solid var(--accent); outline-offset: 2px; }

.css-status-text {
  font-size: 12px;
  color: var(--text-muted, #94a3b8);
  font-style: italic;
  flex: 1;
  min-width: 140px;
}
.css-error {
  font-size: 12px;
  color: var(--status-danger, #ef4444);
}

.tree-css-textarea {
  font-family: var(--font-mono, monospace);
  font-size: 12px;
  line-height: 1.5;
  width: 100%;
  resize: vertical;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border-width: 0;
}

.btn-compact {
  font-size: 11px;
  padding: 3px 8px;
}

.btn.disabled {
  opacity: 0.5;
  pointer-events: none;
}
</style>
