<script setup lang="ts">
import { computed, inject, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useStoryletEditorViewModel } from "../view-models/storyletEditor.ts";
import NarrativeGraphCanvas from "../components/NarrativeGraphCanvas.vue";
import PropertyInspector from "../components/PropertyInspector.vue";
import ReferenceList from "../components/ReferenceList.vue";
import StudioIcon from "../components/StudioIcon.vue";
import type { ContentReference } from "../contracts.ts";
import { lowerPluginGraph, raisePluginGraph, validatePluginNodes } from "../plugins/graphLowering.ts";
import { emit as emitPluginEvent, pluginNodeDeleted, type AnyEntity } from "../plugins/runtime.ts";
import { buildGraphFromStorylet, compileGraphToStorylet, validateNarrativeGraph, validatePreconditionGraph, type NarrativeGraph } from "../narrativeGraphTypes.ts";
import { notify, studioState } from "../state.ts";
import { originBadge } from "../storyletModel.ts";
import { StudioApi } from "../api.ts";
import { interactionContextKey } from "../interactionContext.ts";

const route = useRoute();
const router = useRouter();
const storyletId = typeof route.params.id === "string" ? route.params.id : undefined;
const folderId = typeof route.query.folder === "string" ? route.query.folder : null;
const { loading, isNew, draft, isApproved, loadStorylet, saveDraft, approveStorylet } = useStoryletEditorViewModel(storyletId, folderId);
const currentGraph = ref<NarrativeGraph>({ nodes: [], edges: [] });
const defaultPreconditionGraph = (): NarrativeGraph => ({
  nodes: [
    { id: "precondition_start", type: "start", title: "Start", text: "Check Candidate", x: 80, y: 120, width: 180, height: 80 },
    { id: "precondition_end", type: "end", title: "Valid Candidate", x: 360, y: 120, width: 180, height: 80, precondition_result: true },
  ],
  edges: [{ id: "precondition_start_end", kind: "flow", from_node: "precondition_start", from_port: "output", to_node: "precondition_end", to_port: "input" }],
});
const graphReady = ref(false);
const showReferences = ref(false);
const references = ref<ContentReference[]>([]);
const referencesLoading = ref(false);
async function loadReferences() {
  if (!storyletId) { references.value = []; return; }
  referencesLoading.value = true;
  try { references.value = (await StudioApi.getReferences("storylets", storyletId)).references; }
  catch { references.value = []; }
  finally { referencesLoading.value = false; }
}
onMounted(() => { void loadReferences(); });
const availableStorylets = computed(() => studioState.storylets);
const activeGraphTab = ref<"preconditions" | "story">("story");
const preconditionGraph = ref<NarrativeGraph>(defaultPreconditionGraph());
const activeGraph = computed(() => activeGraphTab.value === "story" ? currentGraph.value : preconditionGraph.value);
const selectedNodeId = ref<string | null>(null);
const selectedNode = computed(() => activeGraph.value.nodes.find((node) => node.id === selectedNodeId.value) ?? null);

const interactionContext = inject(interactionContextKey);
watch(() => [
  draft.id,
  draft.title,
  JSON.stringify(draft),
  selectedNode.value?.id,
  selectedNode.value?.type,
  route.query.item,
  route.query.node,
], () => {
  const node = selectedNode.value;
  const itemId = typeof route.query.item === "string" ? route.query.item : undefined;
  const nodeId = typeof route.query.node === "string" ? route.query.node : undefined;
  interactionContext?.setFocus({
    entity: {
      type: "storylet",
      id: String(draft.id ?? "new-storylet"),
      label: String(draft.title ?? "Untitled Storylet"),
      data: JSON.parse(JSON.stringify(draft)),
    },
    selection: itemId
      ? { type: "graph_item", id: itemId, data: { nodeId } }
      : node
        ? { type: "graph_node", id: node.id, label: node.type, data: node }
        : null,
  });
}, { deep: true });
const lastSavedSnapshot = ref("");

onMounted(async () => {
  const loads: Promise<unknown>[] = [];
  if (storyletId) loads.push(loadStorylet(storyletId));
  await Promise.all(loads);
  currentGraph.value = raisePluginGraph(buildGraphFromStorylet(draft));
  draft.graph = currentGraph.value;
  preconditionGraph.value = draft.precondition_graph?.nodes.length ? raisePluginGraph(draft.precondition_graph) : defaultPreconditionGraph();
  draft.precondition_graph = preconditionGraph.value;
  lastSavedSnapshot.value = JSON.stringify(draft);
  // The editor now shows the stored storylet: plugins drop temp changes left by an unsaved session.
  emitPluginEvent("storylet/opened", isNew.value ? "" : String(draft.id));
  const requestedNode = route.query.node;
  if (typeof requestedNode === "string" && currentGraph.value.nodes.some((node) => node.id === requestedNode)) selectedNodeId.value = requestedNode;
  graphReady.value = true;
  const requestedItem = route.query.item;
  if (typeof requestedItem === "string") {
    await nextTick();
    const element = [...document.querySelectorAll<HTMLElement>("[data-tag-item-id]")].find((item) => item.dataset.tagItemId === requestedItem);
    element?.scrollIntoView({ block: "center", behavior: "smooth" });
    element?.querySelector<HTMLElement>("input, textarea, select")?.focus({ preventScroll: true });
  }
});


const isDirty = computed(() => JSON.stringify(draft) !== lastSavedSnapshot.value);
const canApprove = computed(() => !isDirty.value && !isApproved.value && !isNew.value);

function handleGraphUpdate(graph: NarrativeGraph) {
  if (activeGraphTab.value === "story") {
    currentGraph.value = graph;
    compileGraphToStorylet(graph, draft);
  } else {
    preconditionGraph.value = graph;
    draft.precondition_graph = graph;
  }
  if (selectedNodeId.value && !graph.nodes.some((node) => node.id === selectedNodeId.value)) {
    selectedNodeId.value = null;
  }
}

function handleStoryletUpdate() {
  if (activeGraphTab.value === "story") draft.graph = currentGraph.value;
  else draft.precondition_graph = preconditionGraph.value;
}

function handleSelectNode(nodeId: string | null) {
  selectedNodeId.value = nodeId;
}

function goBack() {
  if (isDirty.value && !window.confirm("You have unsaved changes. Leave anyway?")) return;
  router.push("/storylets");
}

async function playFromHere() {
  try {
    const run = await StudioApi.runs.start({ start: { kind: "beginning" }, storyletId: String(draft.id) });
    await router.push({ name: "play-run", params: { runId: run.id } });
  } catch (e) {
    notify(e instanceof Error ? e.message : String(e), "error");
  }
}

/** Plugin nodes are lowered to core nodes for validation and storage. */
function lowerForEngine(): { graph: NarrativeGraph; preconditionGraph: NarrativeGraph } | null {
  const entities = [...studioState.entities, ...studioState.entityTemplates] as AnyEntity[];
  const pluginIssues = [
    ...validatePluginNodes(currentGraph.value, entities),
    ...validatePluginNodes(preconditionGraph.value, entities),
  ];
  if (pluginIssues.length) { notify(pluginIssues.join(" "), "error"); return null; }
  const lowered = lowerPluginGraph(currentGraph.value, entities);
  const loweredPreconditions = lowerPluginGraph(preconditionGraph.value, entities);
  const graphIssues = [...validateNarrativeGraph(lowered), ...validatePreconditionGraph(loweredPreconditions)];
  if (graphIssues.length) { notify(graphIssues.join(" "), "error"); return null; }
  return { graph: lowered, preconditionGraph: loweredPreconditions };
}

async function handleSave() {
  const lowered = lowerForEngine();
  if (!lowered) return;
  const wasNew = isNew.value;
  compileGraphToStorylet(lowered.graph, draft);
  draft.precondition_graph = lowered.preconditionGraph;
  const saved = await saveDraft();
  draft.graph = currentGraph.value;
  draft.precondition_graph = preconditionGraph.value;
  if (saved) {
    lastSavedSnapshot.value = JSON.stringify(draft);
    emitPluginEvent("storylet/saved", String(draft.id), currentGraph.value);
    if (wasNew) router.replace("/storylets/" + draft.id + "/edit");
  }
}

async function handleApprove() {
  const lowered = lowerForEngine();
  if (!lowered) return;
  compileGraphToStorylet(lowered.graph, draft);
  draft.precondition_graph = lowered.preconditionGraph;
  const approved = await approveStorylet();
  draft.graph = currentGraph.value;
  draft.precondition_graph = preconditionGraph.value;
  if (approved) {
    lastSavedSnapshot.value = JSON.stringify(draft);
    emitPluginEvent("storylet/saved", String(draft.id), currentGraph.value);
  }
}

onBeforeUnmount(() => emitPluginEvent("storylet/closed", isNew.value ? "" : String(draft.id)));

function deleteSelectedNode(nodeId: string) {
  if (!window.confirm("Delete this node and its connections?")) return;
  const graph = activeGraph.value;
  void pluginNodeDeleted(graph.nodes.find((node) => node.id === nodeId));
  const updated = {
    ...graph,
    nodes: graph.nodes.filter((node) => node.id !== nodeId),
    edges: graph.edges.filter((edge) => edge.from_node !== nodeId && edge.to_node !== nodeId),
  };
  selectedNodeId.value = null;
  handleGraphUpdate(updated);
}
</script>

<template>
  <div class="full-editor">
    <header class="editor-toolbar">
      <div class="toolbar-left">
        <button type="button" class="btn btn-secondary editor-back" aria-label="Back to storylets" title="Back to storylets" @click="goBack"><StudioIcon name="back" /><span class="editor-back__label">Back to storylets</span></button>
        <input v-model="draft.title" class="form-input storylet-title-input" placeholder="Storylet title…" aria-label="Storylet title" />
        <span :class="['badge', isApproved ? 'approved' : '']">{{ isApproved ? "Approved" : "Draft" }} · v{{ draft.revision }}</span>
        <span v-if="originBadge(draft)" class="badge">{{ originBadge(draft) }}</span>
        <span v-if="isDirty" class="badge unsaved">Unsaved edits</span>
      </div>
      <div class="toolbar-right">
        <button
          type="button"
          class="btn btn-secondary"
          :disabled="loading || isNew || isDirty"
          :title="isNew || isDirty ? 'Save the draft before playing it' : 'Play starting at this storylet'"
          @click="playFromHere"
        ><StudioIcon name="play" /> Play from here</button>
        <button class="btn btn-secondary" :disabled="loading" @click="handleSave">Save draft</button>
        <button class="btn btn-primary" :disabled="!canApprove" :title="isDirty ? 'Save draft before approving' : 'Approve current revision'" @click="handleApprove">Approve revision</button>
      </div>
    </header>

    <div v-if="loading || !graphReady" class="loading-state">Loading storylet…</div>
    <main v-else class="graph-editor-layout">
      <nav class="graph-tabs tab-nav" aria-label="Storylet graph">
        <button type="button" class="tab-btn" :class="{ active: !showReferences && activeGraphTab === 'preconditions' }" @click="showReferences = false; activeGraphTab = 'preconditions'; selectedNodeId = null">Preconditions</button>
        <button type="button" class="tab-btn" :class="{ active: !showReferences && activeGraphTab === 'story' }" @click="showReferences = false; activeGraphTab = 'story'; selectedNodeId = null">Story graph</button>
        <button v-if="!isNew" type="button" class="tab-btn" :class="{ active: showReferences }" data-testid="storylet-references-tab" @click="showReferences = true; loadReferences()">References <span class="tab-count">{{ references.length }}</span></button>
      </nav>
      <div v-if="showReferences" class="storylet-references">
        <ReferenceList :references="references" :loading="referencesLoading" :target-name="draft.title" />
      </div>
      <div v-else class="graph-editor-workspace">
        <NarrativeGraphCanvas
          :key="activeGraphTab"
          class="graph-editor-canvas"
          :initial-graph="activeGraph"
          :graph-mode="activeGraphTab"
          :default-learn-visibility="draft.default_learn_visibility"
          :selected-node-id="selectedNodeId"
          @update-graph="handleGraphUpdate"
          @select-node="handleSelectNode"
        />
        <PropertyInspector
          :key="activeGraphTab"
          :storylet="draft"
          :storylets="availableStorylets"
          :graph="activeGraph"
          :graph-mode="activeGraphTab"
          :default-learn-visibility="draft.default_learn_visibility"
          :selected-node="selectedNode"
          @update-storylet="handleStoryletUpdate"
          @update-graph="handleGraphUpdate"
          @delete-node="deleteSelectedNode"
        />
      </div>
    </main>
  </div>
</template>

<style scoped>
.full-editor { display: flex; flex-direction: column; container-type: inline-size; height: 100%; min-height: 0; overflow: hidden; background: transparent; }
.editor-toolbar { height: auto; flex-shrink: 0; display: flex; align-items: center; justify-content: space-between; gap: 8px 12px; min-height: 56px; padding: 10px 16px; background: transparent; border-bottom: 1px solid var(--border-faint); flex-wrap: wrap; }
.toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 8px; min-width: 0; }
.toolbar-left { flex: 1 1 320px; flex-wrap: wrap; }
.toolbar-right { margin-left: auto; flex-wrap: wrap; justify-content: flex-end; }
.editor-back { padding: 0 10px; }
.storylet-title-input { font-size: 14px; font-weight: 600; letter-spacing: -0.01em; flex: 1 1 160px; max-width: 320px; min-width: 140px; }
.unsaved { background: var(--tertiary-subtle); color: var(--status-warning); }
.graph-editor-layout { display: flex; flex-direction: column; flex: 1; min-height: 0; overflow: hidden; }
.graph-editor-workspace { display: flex; flex: 1; min-height: 0; overflow: hidden; border-top: 1px solid var(--border-faint); }

.graph-tabs { align-self: flex-start; margin: 10px 16px; overflow-x: auto; max-width: calc(100% - 32px); }
.graph-tabs .tab-btn { white-space: nowrap; flex-shrink: 0; display: inline-flex; align-items: center; }
.graph-editor-canvas { flex: 1; min-width: 0; }
.storylet-references { flex: 1; min-height: 0; overflow: auto; padding: 16px; }
.tab-count { margin-left: 6px; padding: 0 6px; border-radius: 999px; background: rgba(255, 255, 255, 0.08); font-size: 11px; line-height: 16px; font-variant-numeric: tabular-nums; }
.loading-state { display: grid; flex: 1; place-items: center; color: var(--text-muted); }
@container (max-width: 900px) {
  :deep(.property-inspector-panel) { width: min(340px, 44cqw); }
}
@container (max-width: 600px) {
  .editor-back__label { display: none; }
  .graph-editor-workspace { flex-direction: column; overflow-y: auto; }
  .graph-editor-canvas { flex: 0 0 auto; height: 46vh; min-height: 280px; }
  :deep(.property-inspector-panel) { width: 100%; border-left: 0; border-top: 1px solid var(--border-subtle); overflow-y: visible; }
}
</style>
