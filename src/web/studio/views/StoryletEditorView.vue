<script setup lang="ts">
import { computed, inject, nextTick, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useStoryletEditorViewModel } from "../view-models/storyletEditor.ts";
import NarrativeGraphCanvas from "../components/NarrativeGraphCanvas.vue";
import PropertyInspector from "../components/PropertyInspector.vue";
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
  currentGraph.value = buildGraphFromStorylet(draft);
  preconditionGraph.value = draft.precondition_graph?.nodes.length ? draft.precondition_graph : defaultPreconditionGraph();
  draft.precondition_graph = preconditionGraph.value;
  lastSavedSnapshot.value = JSON.stringify(draft);
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

async function handleSave() {
  const graphIssues = [...validateNarrativeGraph(currentGraph.value), ...validatePreconditionGraph(preconditionGraph.value)];
  if (graphIssues.length) { notify(graphIssues.join(" "), "error"); return; }
  const wasNew = isNew.value;
  compileGraphToStorylet(currentGraph.value, draft);
  draft.precondition_graph = preconditionGraph.value;
  if (await saveDraft()) {
    lastSavedSnapshot.value = JSON.stringify(draft);
    if (wasNew) router.replace("/storylets/" + draft.id + "/edit");
  }
}

async function handleApprove() {
  const graphIssues = [...validateNarrativeGraph(currentGraph.value), ...validatePreconditionGraph(preconditionGraph.value)];
  if (graphIssues.length) { notify(graphIssues.join(" "), "error"); return; }
  compileGraphToStorylet(currentGraph.value, draft);
  draft.precondition_graph = preconditionGraph.value;
  if (await approveStorylet()) lastSavedSnapshot.value = JSON.stringify(draft);
}

function deleteSelectedNode(nodeId: string) {
  if (!window.confirm("Delete this node and its connections?")) return;
  const graph = activeGraph.value;
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
        <button class="btn btn-secondary" @click="goBack">← Back to storylets</button>
        <input v-model="draft.title" class="form-input storylet-title-input" placeholder="Storylet title…" aria-label="Storylet title" />
        <span :class="['badge', isApproved ? 'approved' : '']">{{ isApproved ? "Approved" : "Draft" }} · v{{ draft.revision }}</span>
        <span v-if="originBadge(draft)" class="badge">{{ originBadge(draft) }}</span>
        <span v-if="isDirty" class="badge unsaved">Unsaved edits</span>
      </div>
      <div class="toolbar-right">
        <button
          class="btn btn-secondary"
          :disabled="loading || isNew || isDirty"
          :title="isNew || isDirty ? 'Save the draft before playing it' : 'Play starting at this storylet'"
          @click="playFromHere"
        >▶ Play from here</button>
        <button class="btn btn-secondary" :disabled="loading" @click="handleSave">Save draft</button>
        <button class="btn btn-primary" :disabled="!canApprove" :title="isDirty ? 'Save draft before approving' : 'Approve current revision'" @click="handleApprove">Approve revision</button>
      </div>
    </header>

    <div v-if="loading || !graphReady" class="loading-state">Loading storylet…</div>
    <main v-else class="graph-editor-layout">
      <nav class="graph-tabs" aria-label="Storylet graph">
        <button :class="{ active: activeGraphTab === 'preconditions' }" @click="activeGraphTab = 'preconditions'; selectedNodeId = null">Preconditions</button>
        <button :class="{ active: activeGraphTab === 'story' }" @click="activeGraphTab = 'story'; selectedNodeId = null">Story graph</button>
      </nav>
      <div class="graph-editor-workspace">
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
.full-editor { display: flex; flex-direction: column; height: 100%; min-height: 0; overflow: hidden; background: var(--bg-app); }
.editor-toolbar { height: auto; flex-shrink: 0; display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 56px; padding: 8px 16px; background: var(--bg-card); border-bottom: 1px solid var(--border-subtle); flex-wrap: wrap; }
.toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 10px; min-width: 0; }
.toolbar-right { margin-left: auto; flex-shrink: 0; }
.storylet-title-input { font-size: 13px; font-weight: 600; width: min(220px, 40vw); min-width: 0; }
.unsaved { background: rgba(255, 149, 0, 0.18); color: var(--status-warning); }
.graph-editor-layout { display: flex; flex-direction: column; flex: 1; min-height: 0; overflow: hidden; }
.graph-editor-workspace { display: flex; flex: 1; min-height: 0; overflow: hidden; }

.graph-tabs { display: flex; gap: 4px; padding: 7px 12px; background: var(--bg-card); border-bottom: 1px solid var(--border-subtle); overflow-x: auto; }
.graph-tabs button { border: 1px solid var(--border-subtle); border-radius: 6px; background: transparent; color: var(--text-muted); padding: 6px 12px; font-size: 12px; cursor: pointer; white-space: nowrap; flex-shrink: 0; }
.graph-tabs button.active { color: var(--text-main); background: var(--bg-app); border-color: var(--accent); }
.graph-editor-canvas { flex: 1; min-width: 0; }
.loading-state { display: grid; flex: 1; place-items: center; color: var(--text-muted); }
@media (max-width: 760px) {
  .editor-toolbar { align-items: flex-start; row-gap: 8px; }
  .toolbar-left { flex-wrap: wrap; flex: 1 1 auto; min-width: 0; }
  .toolbar-right { flex-wrap: nowrap; }
  .toolbar-right .btn { white-space: nowrap; }
  :deep(.property-inspector-panel) { width: min(340px, 44vw); }
}
@media (max-width: 560px) {
  .editor-toolbar { flex-wrap: nowrap; overflow-x: auto; }
  .toolbar-left { flex-wrap: nowrap; flex: 0 0 auto; }
  .toolbar-right { flex: 0 0 auto; }
  .graph-editor-workspace { flex-direction: column; overflow-y: auto; }
  .graph-editor-canvas { flex: 0 0 auto; height: 46vh; min-height: 280px; }
  :deep(.property-inspector-panel) { width: 100%; border-left: 0; border-top: 1px solid var(--border-subtle); overflow-y: visible; }
}
</style>
