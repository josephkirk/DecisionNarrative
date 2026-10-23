<script setup lang="ts">
import { computed, inject, nextTick, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useStoryletEditorViewModel } from "../view-models/storyletEditor.ts";
import NarrativeGraphCanvas from "../components/NarrativeGraphCanvas.vue";
import PropertyInspector from "../components/PropertyInspector.vue";
import { buildGraphFromStorylet, compileGraphToStorylet, validateNarrativeGraph, type NarrativeGraph } from "../narrativeGraphTypes.ts";
import { notify, studioState } from "../state.ts";
import { interactionContextKey } from "../interactionContext.ts";

const route = useRoute();
const router = useRouter();
const storyletId = typeof route.params.id === "string" ? route.params.id : undefined;
const folderId = typeof route.query.folder === "string" ? route.query.folder : null;
const { loading, isNew, draft, isApproved, loadStorylet, saveDraft, approveStorylet } = useStoryletEditorViewModel(storyletId, folderId);
const currentGraph = ref<NarrativeGraph>({ nodes: [], edges: [], local_variables: [] });
const selectedNodeId = ref<string | null>(null);
const selectedNode = computed(() => currentGraph.value.nodes.find((node) => node.id === selectedNodeId.value) ?? null);
const availableStorylets = computed(() => studioState.storylets);
const graphReady = ref(false);

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
  compileGraphToStorylet(currentGraph.value, draft);
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
  currentGraph.value = graph;
  compileGraphToStorylet(graph, draft);
  if (selectedNodeId.value && !graph.nodes.some((node) => node.id === selectedNodeId.value)) {
    selectedNodeId.value = null;
  }
}

function handleStoryletUpdate() {
  draft.graph = currentGraph.value;
}

function handleSelectNode(nodeId: string | null) {
  selectedNodeId.value = nodeId;
}

function goBack() {
  if (isDirty.value && !window.confirm("You have unsaved changes. Leave anyway?")) return;
  router.push("/storylets");
}

async function handleSave() {
  const graphIssues = validateNarrativeGraph(currentGraph.value);
  if (graphIssues.length) { notify(graphIssues.join(" "), "error"); return; }
  const wasNew = isNew.value;
  compileGraphToStorylet(currentGraph.value, draft);
  if (await saveDraft()) {
    lastSavedSnapshot.value = JSON.stringify(draft);
    if (wasNew) router.replace("/storylets/" + draft.id + "/edit");
  }
}

async function handleApprove() {
  const graphIssues = validateNarrativeGraph(currentGraph.value);
  if (graphIssues.length) { notify(graphIssues.join(" "), "error"); return; }
  compileGraphToStorylet(currentGraph.value, draft);
  if (await approveStorylet()) lastSavedSnapshot.value = JSON.stringify(draft);
}

function deleteSelectedNode(nodeId: string) {
  currentGraph.value = {
    ...currentGraph.value,
    nodes: currentGraph.value.nodes.filter((node) => node.id !== nodeId),
    edges: currentGraph.value.edges.filter((edge) => edge.from_node !== nodeId && edge.to_node !== nodeId),
  };
  selectedNodeId.value = null;
  handleGraphUpdate(currentGraph.value);
}
</script>

<template>
  <div class="full-editor">
    <header class="editor-toolbar">
      <div class="toolbar-left">
        <button class="btn btn-secondary" @click="goBack">← Back to Storylets</button>
        <span class="storylet-title">{{ draft.title || "Untitled Storylet" }}</span>
        <span :class="['badge', isApproved ? 'approved' : '']">{{ isApproved ? "Approved" : "Draft" }} · v{{ draft.revision }}</span>
        <span v-if="isDirty" class="badge unsaved">Unsaved Edits</span>
      </div>
      <div class="toolbar-right">
        <button class="btn btn-secondary" :disabled="loading" @click="handleSave">Save Draft</button>
        <button class="btn btn-primary" :disabled="!canApprove" :title="isDirty ? 'Save draft before approving' : 'Approve current revision'" @click="handleApprove">Approve Revision</button>
      </div>
    </header>

    <div v-if="loading || !graphReady" class="loading-state">Loading storylet…</div>
    <main v-else class="graph-editor-layout">
      <NarrativeGraphCanvas
        class="graph-editor-canvas"
        :initial-graph="currentGraph"
        :selected-node-id="selectedNodeId"
        @update-graph="handleGraphUpdate"
        @select-node="handleSelectNode"
      />
      <PropertyInspector
        :storylet="draft"
        :storylets="availableStorylets"
        :graph="currentGraph"
        :selected-node="selectedNode"
        @update-storylet="handleStoryletUpdate"
        @update-graph="handleGraphUpdate"
        @delete-node="deleteSelectedNode"
      />
    </main>
  </div>
</template>

<style scoped>
.full-editor { display: flex; flex-direction: column; height: 100%; min-height: 0; overflow: hidden; background: var(--bg-app); }
.editor-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 56px; padding: 8px 16px; background: var(--bg-card); border-bottom: 1px solid var(--border-subtle); }
.toolbar-left, .toolbar-right { display: flex; align-items: center; gap: 10px; }
.storylet-title { font-size: 14px; font-weight: 600; }
.unsaved { background: rgba(255, 149, 0, 0.18); color: var(--status-warning); }
.graph-editor-layout { display: flex; flex: 1; min-height: 0; overflow: hidden; }
.graph-editor-canvas { flex: 1; min-width: 0; }
.loading-state { display: grid; flex: 1; place-items: center; color: var(--text-muted); }
@media (max-width: 760px) {
  .editor-toolbar { align-items: flex-start; }
  .toolbar-left { flex-wrap: wrap; }
  :deep(.property-inspector-panel) { width: min(340px, 44vw); }
}
</style>
