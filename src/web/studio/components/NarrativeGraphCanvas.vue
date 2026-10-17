<script setup lang="ts">
import { ref, reactive, computed, onMounted } from "vue";
import type { NarrativeGraph, NarrativeNode, NarrativeEdge, NodeType } from "../narrativeGraphTypes.ts";
import { studioState, notify } from "../state.ts";

const props = defineProps<{
  initialGraph: NarrativeGraph;
}>();

const emit = defineEmits<{
  (e: "update-graph", graph: NarrativeGraph): void;
}>();

// Pan & Zoom
const zoom = ref(1.0);
const panX = ref(60);
const panY = ref(60);
const isPanning = ref(false);
const startPan = ref({ x: 0, y: 0 });

// Graph state
const nodes = ref<NarrativeNode[]>([]);
const edges = ref<NarrativeEdge[]>([]);

onMounted(() => {
  nodes.value = JSON.parse(JSON.stringify(props.initialGraph.nodes));
  edges.value = JSON.parse(JSON.stringify(props.initialGraph.edges));
});

// Dragging node
const draggingNodeId = ref<string | null>(null);
const dragStartPos = ref({ x: 0, y: 0 });

// Active connection drawing (from port to mouse)
const activeConnection = ref<{
  fromNode: string;
  fromPort: string;
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
} | null>(null);

// Selected elements
const selectedNodeId = ref<string | null>(null);
const selectedEdgeId = ref<string | null>(null);

// AI Generation loading states
const generatingNodeId = ref<string | null>(null);
const containerRef = ref<HTMLElement | null>(null);

function emitUpdate() {
  emit("update-graph", {
    nodes: nodes.value,
    edges: edges.value,
  });
}


function eventToWorld(e: MouseEvent): { x: number; y: number } {
  if (!containerRef.value) {
    return { x: 0, y: 0 };
  }
  const rect = containerRef.value.getBoundingClientRect();
  return {
    x: Math.round((e.clientX - rect.left - panX.value) / zoom.value),
    y: Math.round((e.clientY - rect.top - panY.value) / zoom.value),
  };
}

// Canvas Mouse Interactions
function onCanvasMouseDown(e: MouseEvent) {
  if ((e.target as HTMLElement).closest(".graph-node") || (e.target as HTMLElement).closest(".socket")) return;
  selectedNodeId.value = null;
  selectedEdgeId.value = null;
  isPanning.value = true;
  startPan.value = { x: e.clientX - panX.value, y: e.clientY - panY.value };
}

function onCanvasMouseMove(e: MouseEvent) {
  if (isPanning.value) {
    panX.value = e.clientX - startPan.value.x;
    panY.value = e.clientY - startPan.value.y;
  } else if (draggingNodeId.value) {
    const node = nodes.value.find((n) => n.id === draggingNodeId.value);
    if (node) {
      const pt = eventToWorld(e);
      node.x = pt.x - dragStartPos.value.x;
      node.y = pt.y - dragStartPos.value.y;
      emitUpdate();
    }
  } else if (activeConnection.value) {
    const pt = eventToWorld(e);
    activeConnection.value.currentX = pt.x;
    activeConnection.value.currentY = pt.y;
  }
}

function onCanvasMouseUp() {
  isPanning.value = false;
  draggingNodeId.value = null;
  activeConnection.value = null;
}

function onWheel(e: WheelEvent) {
  e.preventDefault();
  const delta = e.deltaY > 0 ? -0.1 : 0.1;
  zoom.value = Math.max(0.3, Math.min(2.0, zoom.value + delta));
}

// Node Header Dragging
function startNodeDrag(e: MouseEvent, node: NarrativeNode) {
  e.stopPropagation();
  selectedNodeId.value = node.id;
  selectedEdgeId.value = null;
  draggingNodeId.value = node.id;
  const pt = eventToWorld(e);
  dragStartPos.value = {
    x: pt.x - node.x,
    y: pt.y - node.y,
  };
}

// Socket Connection Dragging
function startConnection(e: MouseEvent, node: NarrativeNode, port: string, socketYOffset: number) {
  e.stopPropagation();
  let startX = node.x + node.width;
  let startY = node.y + socketYOffset;

  const targetEl = e.currentTarget as HTMLElement | null;
  if (targetEl && containerRef.value) {
    const sRect = targetEl.getBoundingClientRect();
    const cRect = containerRef.value.getBoundingClientRect();
    startX = Math.round((sRect.left + sRect.width / 2 - cRect.left - panX.value) / zoom.value);
    startY = Math.round((sRect.top + sRect.height / 2 - cRect.top - panY.value) / zoom.value);
  }

  const mousePt = eventToWorld(e);

  activeConnection.value = {
    fromNode: node.id,
    fromPort: port,
    startX,
    startY,
    currentX: mousePt.x,
    currentY: mousePt.y,
  };
}

function completeConnection(e: MouseEvent, targetNode: NarrativeNode, port: string) {
  e.stopPropagation();
  if (!activeConnection.value) return;
  if (activeConnection.value.fromNode === targetNode.id) return; // Disallow self-loops

  // Remove existing edge from this same source port if exclusive
  edges.value = edges.value.filter(
    (edge) => !(edge.from_node === activeConnection.value!.fromNode && edge.from_port === activeConnection.value!.fromPort)
  );

  edges.value.push({
    id: `edge_${activeConnection.value.fromNode}_${activeConnection.value.fromPort}_${targetNode.id}`,
    from_node: activeConnection.value.fromNode,
    from_port: activeConnection.value.fromPort,
    to_node: targetNode.id,
    to_port: port,
  });

  activeConnection.value = null;
  emitUpdate();
}

function removeEdge(edgeId: string) {
  edges.value = edges.value.filter((e) => e.id !== edgeId);
  selectedEdgeId.value = null;
  emitUpdate();
}

function removeNode(nodeId: string) {
  nodes.value = nodes.value.filter((n) => n.id !== nodeId);
  edges.value = edges.value.filter((e) => e.from_node !== nodeId && e.to_node !== nodeId);
  if (selectedNodeId.value === nodeId) selectedNodeId.value = null;
  emitUpdate();
}

// Create new node of type
function addNode(type: NodeType) {
  const nextId = `node_${type}_${Date.now().toString(36)}`;
  const x = Math.round((-panX.value + 400) / zoom.value);
  const y = Math.round((-panY.value + 250) / zoom.value);

  const baseNode: NarrativeNode = {
    id: nextId,
    type,
    title: type.charAt(0).toUpperCase() + type.slice(1),
    x,
    y,
    width: 260,
    height: 160,
  };

  if (type === "dialog") {
    baseNode.speaker_role = "Narrator";
    baseNode.text = "Spoken text or monologue...";
  } else if (type === "choice") {
    baseNode.choices = [
      { id: "choice_0", text: "Proceed ahead" },
      { id: "choice_1", text: "Turn back" },
    ];
    baseNode.height = 160;
  } else if (type === "variable") {
    baseNode.assignments = [{ quality: studioState.qualities[0]?.name || "tension", op: "add", value: 1 }];
    baseNode.height = 140;
  } else if (type === "conditional") {
    baseNode.conditions = [{ variable: studioState.qualities[0]?.name || "tension", op: "gte", value: 1 }];
    baseNode.height = 140;
  } else if (type === "image") {
    baseNode.prompt = "Dark mystery cellar, cinematic lighting, moody";
    baseNode.height = 220;
  } else if (type === "teleport") {
    baseNode.destination_storylet = "";
    baseNode.height = 100;
  } else if (type === "end") {
    baseNode.is_terminal = true;
    baseNode.height = 80;
  }

  nodes.value.push(baseNode);
  selectedNodeId.value = nextId;
  emitUpdate();
}

// Calculate SVG Bezier Curve between ports
function calculateBezier(x1: number, y1: number, x2: number, y2: number): string {
  const dx = Math.abs(x2 - x1) * 0.5;
  const cx1 = x1 + Math.max(50, dx);
  const cy1 = y1;
  const cx2 = x2 - Math.max(50, dx);
  const cy2 = y2;
  return `M ${x1} ${y1} C ${cx1} ${cy1}, ${cx2} ${cy2}, ${x2} ${y2}`;
}

const computedEdges = computed(() => {
  return edges.value.map((edge) => {
    const fromNode = nodes.value.find((n) => n.id === edge.from_node);
    const toNode = nodes.value.find((n) => n.id === edge.to_node);
    if (!fromNode || !toNode) return null;

    let fromY = fromNode.y + 40;
    if (fromNode.type === "choice" && edge.from_port.startsWith("choice_")) {
      const idx = parseInt(edge.from_port.replace("choice_", ""), 10);
      fromY = fromNode.y + 64 + idx * 35;
    }

    const startX = fromNode.x + fromNode.width;
    const startY = fromY;
    const endX = toNode.x;
    const endY = toNode.y + 40;

    return {
      id: edge.id,
      path: calculateBezier(startX, startY, endX, endY),
      startX,
      startY,
      endX,
      endY,
    };
  }).filter(Boolean);
});

// LiteLLM Dialogue Generation
async function generateDialogueForNode(node: NarrativeNode) {
  generatingNodeId.value = node.id;
  try {
    const prompt = `Write an engaging line of dialogue for the role '${node.speaker_role || 'Character'}'. Context: ${node.text || 'In a tense mystery situation.'}`;
    const resp = await fetch("/api/v2/generation/litellm", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ prompt, model: "gpt-4o-mini" }),
    });
    const res = await resp.json();
    if (!resp.ok) throw new Error(res.error || "LiteLLM unavailable");
    node.text = res.text.trim();
    notify("Generated dialogue via LiteLLM", "success");
    emitUpdate();
  } catch (err) {
    notify(err instanceof Error ? err.message : String(err), "error");
  } finally {
    generatingNodeId.value = null;
  }
}

// ComfyUI Image Generation
async function generateImageForNode(node: NarrativeNode) {
  generatingNodeId.value = node.id;
  node.image_url = undefined;
  try {
    const resp = await fetch("/api/v2/generation/comfyui/prompt", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ prompt: node.prompt || "cinematic narrative illustration" }),
    });
    const res = await resp.json();
    if (!resp.ok) throw new Error(res.error || "ComfyUI unavailable");

    notify(`Queued image on ComfyUI (Prompt #${res.prompt_id.slice(0, 8)})`, "info");
    node.image_url = `/api/v2/generation/comfyui/view?filename=DecisionNarrative_00001_.png`;
    emitUpdate();
  } catch (err) {
    notify(err instanceof Error ? err.message : String(err), "error");
  } finally {
    generatingNodeId.value = null;
  }
}
</script>

<template>
  <div
    ref="containerRef"
    class="graph-canvas-container"
    @mousedown="onCanvasMouseDown"
    @mousemove="onCanvasMouseMove"
    @mouseup="onCanvasMouseUp"
    @wheel="onWheel"
  >
    <!-- Top Floating Toolbar -->
    <div class="graph-toolbar">
      <div style="display:flex; align-items:center; gap:8px;">
        <span style="font-size:12px; font-weight:700; color:var(--text-muted); text-transform:uppercase;">+ Add Node:</span>
        <button class="node-tool-btn start" @click="addNode('start')">Start</button>
        <button class="node-tool-btn dialog" @click="addNode('dialog')">Dialog</button>
        <button class="node-tool-btn choice" @click="addNode('choice')">Choice</button>
        <button class="node-tool-btn variable" @click="addNode('variable')">Variable</button>
        <button class="node-tool-btn conditional" @click="addNode('conditional')">Conditional</button>
        <button class="node-tool-btn image" @click="addNode('image')">Image (AI)</button>
        <button class="node-tool-btn teleport" @click="addNode('teleport')">Teleport</button>
        <button class="node-tool-btn end" @click="addNode('end')">End</button>
      </div>

      <div style="display:flex; align-items:center; gap:8px;">
        <span style="font-size:11px; font-weight:600; width:45px; text-align:center;">
          {{ Math.round(zoom * 100) }}%
        </span>
        <button class="btn btn-secondary" style="font-size:11px; padding:3px 8px;" @click="zoom = 1.0; panX = 60; panY = 60;">Reset</button>
        <button class="btn btn-primary" style="font-size:11px; padding:4px 10px;" @click="emitUpdate">Sync to Storylet</button>
      </div>
    </div>

    <!-- Canvas Transform Layer -->
    <div
      class="graph-surface"
      :style="{
        transform: `translate(${panX}px, ${panY}px) scale(${zoom})`,
        transformOrigin: '0 0',
      }"
    >
      <!-- SVG Connection Lines Layer -->
      <svg class="graph-svg-layer">
        <defs>
          <marker id="socket-arrow" viewBox="0 0 10 10" refX="6" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M 0 1 L 8 5 L 0 9 z" fill="#ffd60a" />
          </marker>
        </defs>

        <!-- Existing Edges -->
        <g v-for="edge in computedEdges" :key="edge!.id">
          <!-- Clickable thick hit area -->
          <path
            :d="edge!.path"
            class="edge-hit-area"
            @click.stop="selectedEdgeId = edge!.id"
          />
          <!-- Visible Bezier line -->
          <path
            :d="edge!.path"
            :class="['narrative-edge-line', selectedEdgeId === edge!.id ? 'selected' : '']"
            marker-end="url(#socket-arrow)"
          />
        </g>

        <!-- Active Dragging Connection Line -->
        <path
          v-if="activeConnection"
          :d="calculateBezier(activeConnection.startX, activeConnection.startY, activeConnection.currentX, activeConnection.currentY)"
          class="active-connecting-line"
        />
      </svg>

      <!-- Node Cards Layer -->
      <div
        v-for="node in nodes"
        :key="node.id"
        :class="['graph-node', node.type, selectedNodeId === node.id ? 'selected' : '']"
        :style="{
          left: `${node.x}px`,
          top: `${node.y}px`,
          width: `${node.width}px`,
        }"
        @click.stop="selectedNodeId = node.id"
      >
        <!-- Node Header -->
        <div class="node-header" @mousedown="startNodeDrag($event, node)">
          <div class="node-header-title">
            <span class="node-icon">
              {{ node.type === 'start' ? '▶' : node.type === 'dialog' ? '💬' : node.type === 'choice' ? '🔀' : node.type === 'image' ? '🎨' : '⚙️' }}
            </span>
            <input v-model="node.title" class="node-title-input" @input="emitUpdate" />
          </div>
          <button class="node-close-btn" @click.stop="removeNode(node.id)">✕</button>
        </div>

        <!-- Node Body Content based on Type -->
        <div class="node-body">
          <!-- START -->
          <div v-if="node.type === 'start'">
            <div class="field-label">Entrypoint</div>
            <div style="font-size:12px; font-weight:600;">{{ node.text || 'Begin Scene' }}</div>
          </div>

          <!-- DIALOG -->
          <div v-else-if="node.type === 'dialog'">
            <div style="margin-bottom:8px;">
              <label class="field-label">Dialog Source (Role):</label>
              <input v-model="node.speaker_role" class="form-input" style="padding:4px 8px; font-size:11px;" @input="emitUpdate" />
            </div>

            <div style="margin-bottom:8px;">
              <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px;">
                <label class="field-label" style="margin-bottom:0;">Dialog Text:</label>
                <button
                  class="ai-action-btn"
                  :disabled="generatingNodeId === node.id"
                  @click.stop="generateDialogueForNode(node)"
                >
                  {{ generatingNodeId === node.id ? 'Drafting...' : '✨ AI Draft' }}
                </button>
              </div>
              <textarea v-model="node.text" class="form-textarea" style="min-height:75px; font-size:11px; padding:6px;" @input="emitUpdate"></textarea>
            </div>
          </div>

          <!-- CHOICE -->
          <div v-else-if="node.type === 'choice'">
            <div class="field-label" style="margin-bottom:8px;">Choices & Branching:</div>
            <div v-for="(opt, idx) in node.choices" :key="opt.id" class="choice-option-row">
              <input v-model="opt.text" class="form-input" style="font-size:11px; padding:4px 6px; flex:1;" @input="emitUpdate" />
              <!-- Sockets on each choice item -->
              <div
                class="socket output-socket choice-socket"
                title="Connect this choice branch"
                @mousedown="startConnection($event, node, `choice_${idx}`, 72 + idx * 36)"
              ></div>
            </div>
          </div>

          <!-- IMAGE (ComfyUI) -->
          <div v-else-if="node.type === 'image'">
            <div style="margin-bottom:8px;">
              <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px;">
                <label class="field-label" style="margin-bottom:0;">Prompt:</label>
                <button
                  class="ai-action-btn"
                  :disabled="generatingNodeId === node.id"
                  @click.stop="generateImageForNode(node)"
                >
                  {{ generatingNodeId === node.id ? 'Rendering...' : '🎨 Generate' }}
                </button>
              </div>
              <textarea v-model="node.prompt" class="form-textarea" style="min-height:50px; font-size:11px; padding:6px;" @input="emitUpdate"></textarea>
            </div>

            <div v-if="node.image_url" style="text-align:center; margin-top:6px;">
              <img :src="node.image_url" style="width:100%; height:110px; object-fit:cover; border-radius:6px; border:1px solid var(--border-subtle);" />
            </div>
          </div>

          <!-- TELEPORT / DIVERT -->
          <div v-else-if="node.type === 'teleport'">
            <label class="field-label">Target Storylet ID:</label>
            <input v-model="node.destination_storylet" class="form-input" placeholder="stl_next_scene" style="font-size:11px; padding:4px 8px;" @input="emitUpdate" />
          </div>

          <!-- END -->
          <div v-else-if="node.type === 'end'">
            <div style="font-size:12px; font-weight:700; color:var(--status-danger);">Terminal Climax / Resolution</div>
          </div>
        </div>

        <!-- Input Socket (Left edge) -->
        <div
          v-if="node.type !== 'start'"
          class="socket input-socket"
          title="Input connection port"
          @mouseup="completeConnection($event, node, 'input')"
        ></div>

        <!-- Default Output Socket (Right edge) -->
        <div
          v-if="node.type !== 'choice' && node.type !== 'end'"
          class="socket output-socket"
          title="Output connection port"
          @mousedown="startConnection($event, node, 'output', 40)"
        ></div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.graph-canvas-container {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background-color: #121214;
  background-image: radial-gradient(#2c2c2e 1.2px, transparent 1.2px);
  background-size: 24px 24px;
  cursor: grab;
  user-select: none;
}

.graph-canvas-container:active {
  cursor: grabbing;
}

.graph-toolbar {
  position: absolute;
  top: 14px;
  left: 20px;
  right: 20px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 14px;
  background: rgba(30, 30, 34, 0.92);
  backdrop-filter: blur(8px);
  border: 1px solid #38383a;
  border-radius: 10px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  z-index: 100;
}

.node-tool-btn {
  background: #2c2c2e;
  border: 1px solid #48484a;
  color: #f2f2f7;
  font-size: 11px;
  font-weight: 600;
  padding: 4px 9px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.12s ease;
}

.node-tool-btn:hover {
  background: #3a3a3c;
  border-color: #0a84ff;
}

.graph-surface {
  position: absolute;
  inset: 0;
  width: 10000px;
  height: 10000px;
  pointer-events: none;
}

.graph-svg-layer {
  position: absolute;
  inset: 0;
  width: 10000px;
  height: 10000px;
  pointer-events: none;
}

.narrative-edge-line {
  fill: none;
  stroke: #ffd60a;
  stroke-width: 2.8;
  pointer-events: none;
}

.narrative-edge-line.selected {
  stroke: #ff9f0a;
  stroke-width: 3.5;
}

.edge-hit-area {
  fill: none;
  stroke: transparent;
  stroke-width: 16;
  cursor: pointer;
  pointer-events: auto;
}

.active-connecting-line {
  fill: none;
  stroke: #ffd60a;
  stroke-width: 2.5;
  stroke-dasharray: 5 4;
}

/* Node Styling matching NarrativeFlow & Image #1 */
.graph-node {
  position: absolute;
  background: #1c1c1e;
  border: 1.5px solid #38383a;
  border-radius: 10px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
  pointer-events: auto;
  user-select: none;
  transition: border-color 0.12s ease, box-shadow 0.12s ease;
}

.graph-node.selected {
  border-color: #0a84ff;
  box-shadow: 0 0 16px rgba(10, 132, 255, 0.4);
}

.node-header {
  padding: 6px 10px;
  border-top-left-radius: 8px;
  border-top-right-radius: 8px;
  background: #5e17eb; /* Purple header */
  color: #ffffff;
  display: flex;
  justify-content: space-between;
  align-items: center;
  cursor: grab;
}

.graph-node.start .node-header { background: #10b981; }
.graph-node.dialog .node-header { background: #8b5cf6; }
.graph-node.choice .node-header { background: #a855f7; }
.graph-node.image .node-header { background: #ec4899; }
.graph-node.end .node-header { background: #f43f5e; }

.node-header-title {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
}

.node-title-input {
  background: transparent;
  border: none;
  color: #ffffff;
  font-weight: 700;
  font-size: 12px;
  outline: none;
  width: 100%;
}

.node-close-btn {
  background: transparent;
  border: none;
  color: rgba(255, 255, 255, 0.7);
  font-size: 12px;
  cursor: pointer;
}

.node-close-btn:hover {
  color: #ffffff;
}

.node-body {
  padding: 10px 12px;
  color: #f2f2f7;
  font-size: 11px;
}

.field-label {
  font-size: 10px;
  font-weight: 700;
  color: #aeaeb2;
  text-transform: uppercase;
  margin-bottom: 3px;
  display: block;
}

.choice-option-row {
  position: relative;
  display: flex;
  align-items: center;
  margin-bottom: 6px;
}

/* Sockets */
.socket {
  position: absolute;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #ffd60a;
  border: 2px solid #1c1c1e;
  box-shadow: 0 0 6px rgba(255, 214, 10, 0.6);
  cursor: crosshair;
  z-index: 20;
}

.input-socket {
  left: -8px;
  top: 35px;
}

.output-socket {
  right: -8px;
  top: 35px;
}

.choice-socket {
  position: static;
  margin-left: 6px;
}

.ai-action-btn {
  background: rgba(10, 132, 255, 0.2);
  border: 1px solid #0a84ff;
  color: #409cff;
  font-size: 10px;
  font-weight: 600;
  padding: 2px 6px;
  border-radius: 4px;
  cursor: pointer;
}

.ai-action-btn:hover {
  background: #0a84ff;
  color: #ffffff;
}
</style>
