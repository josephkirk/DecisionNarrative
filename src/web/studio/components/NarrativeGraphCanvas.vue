<script setup lang="ts">
import { ref, computed, watch } from "vue";
import { graphNodeTypes, graphConnectionIsCompatible, graphNodeValueType, graphTargetPinType, isSafeImageUrl, GRAPH_PIN_COLORS, type GraphValueType, type NarrativeGraph, type NarrativeNode, type NarrativeEdge, type NodeType } from "../narrativeGraphTypes.ts";
import { studioState, notify } from "../state.ts";

const props = withDefaults(defineProps<{
  initialGraph: NarrativeGraph;
  selectedNodeId: string | null;
  graphMode?: "story" | "preconditions";
}>(), { graphMode: "story" });

const emit = defineEmits<{
  (e: "update-graph", graph: NarrativeGraph): void;
  (e: "select-node", nodeId: string | null): void;
}>();

// Pan & Zoom
const zoom = ref(1.0);
const panX = ref(40);
const panY = ref(60);
const isPanning = ref(false);
const startPan = ref({ x: 0, y: 0 });

// Graph state
const nodes = ref<NarrativeNode[]>([]);
const edges = ref<NarrativeEdge[]>([]);
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
const selectedNodeId = computed({ get: () => props.selectedNodeId, set: (nodeId: string | null) => emit("select-node", nodeId) });
const selectedEdgeId = ref<string | null>(null);
const selectedNode = computed(() => nodes.value.find((node) => node.id === selectedNodeId.value) ?? null);
watch(() => props.initialGraph, (graph) => {
  nodes.value = graph.nodes;
  edges.value = graph.edges;

  if (selectedNodeId.value && !graph.nodes.some((node) => node.id === selectedNodeId.value)) {
    selectedNodeId.value = null;
    emit("select-node", null);
  }
  if (selectedEdgeId.value && !graph.edges.some((edge) => edge.id === selectedEdgeId.value)) {
    selectedEdgeId.value = null;
  }
}, { immediate: true });
const contextMenu = ref<{ kind: "canvas" | "node" | "edge"; x: number; y: number; targetId?: string } | null>(null);
const nodeSearch = ref("");
const scissorStart = ref<{ x: number; y: number } | null>(null);
const scissorEnd = ref<{ x: number; y: number } | null>(null);
const nodeTypes = computed<NodeType[]>(() => graphNodeTypes(props.graphMode));

const failedImageNodes = ref(new Set<string>());
watch(
  () => nodes.value.map((node) => [node.id, node.image_url] as const),
  (current, previous = []) => {
    const previousUrls = new Map(previous);
    for (const [nodeId, imageUrl] of current) {
      if (previousUrls.get(nodeId) !== imageUrl) failedImageNodes.value.delete(nodeId);
    }
  },
);

function markImageFailed(nodeId: string) {
  failedImageNodes.value.add(nodeId);
}
const matchingNodeTypes = computed(() => nodeTypes.value.filter((type) => nodeTypeLabel(type).toLowerCase().includes(nodeSearch.value.trim().toLowerCase())));
function nodeTypeLabel(type: NodeType): string {
  if (type === "condition") return "Condition Node";
  return type.split("_").map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join(" ");
}
const contextWorld = ref<{ x: number; y: number } | null>(null);

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
  if ((e.target as HTMLElement).closest(".graph-node") || (e.target as HTMLElement).closest(".socket") || (e.target as HTMLElement).closest(".graph-toolbar")) return;
  selectedNodeId.value = null;
  selectedEdgeId.value = null;
  emit("select-node", null);
  contextMenu.value = null;
  if (e.altKey) {
    e.preventDefault();
    scissorStart.value = eventToWorld(e);
    scissorEnd.value = scissorStart.value;
    return;
  }
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
  } else if (scissorStart.value) {
    scissorEnd.value = eventToWorld(e);
  }
}

function pointSegmentDistance(point: { x: number; y: number }, start: { x: number; y: number }, end: { x: number; y: number }) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const lengthSq = dx * dx + dy * dy;
  const t = lengthSq === 0 ? 0 : Math.max(0, Math.min(1, ((point.x - start.x) * dx + (point.y - start.y) * dy) / lengthSq));
  return Math.hypot(point.x - (start.x + t * dx), point.y - (start.y + t * dy));
}

function onCanvasMouseUp() {
  if (scissorStart.value && scissorEnd.value) {
    const start = scissorStart.value;
    const end = scissorEnd.value;
    const removed = computedEdges.value.filter((edge) => {
      const samples = 32;
      for (let i = 0; i <= samples; i++) {
        const t = i / samples;
        const x = (1 - t) ** 3 * edge!.startX + 3 * (1 - t) ** 2 * t * (edge!.startX + Math.max(50, Math.abs(edge!.endX - edge!.startX) * 0.5)) + 3 * (1 - t) * t ** 2 * (edge!.endX - Math.max(50, Math.abs(edge!.endX - edge!.startX) * 0.5)) + t ** 3 * edge!.endX;
        const y = (1 - t) ** 3 * edge!.startY + 3 * (1 - t) ** 2 * t * edge!.startY + 3 * (1 - t) * t ** 2 * edge!.endY + t ** 3 * edge!.endY;
        if (pointSegmentDistance({ x, y }, start, end) <= 12) return true;
      }
      return false;
    }).map((edge) => edge!.id);
    if (removed.length) {
      edges.value = edges.value.filter((edge) => !removed.includes(edge.id));
      emitUpdate();
    }
  }
  scissorStart.value = null;
  scissorEnd.value = null;
  isPanning.value = false;
  draggingNodeId.value = null;
  activeConnection.value = null;
}

function onCanvasContextMenu(e: MouseEvent) {
  if ((e.target as HTMLElement).closest(".graph-node") || (e.target as HTMLElement).closest(".edge-hit-area")) return;
  e.preventDefault();
  openMenu(e, "canvas");
}

function openMenu(e: MouseEvent, kind: "canvas" | "node" | "edge", targetId?: string) {
  const rect = containerRef.value?.getBoundingClientRect();
  if (!rect) return;
  contextMenu.value = { kind, targetId, x: e.clientX - rect.left, y: e.clientY - rect.top };
  contextWorld.value = eventToWorld(e);
  nodeSearch.value = "";
}

function selectNode(node: NarrativeNode) {
  selectedNodeId.value = node.id;
  selectedEdgeId.value = null;
  emit("select-node", node.id);
}

function onWheel(e: WheelEvent) {
  e.preventDefault();
  const delta = e.deltaY > 0 ? -0.1 : 0.1;
  zoom.value = Math.max(0.3, Math.min(2.0, zoom.value + delta));
}

// Node Header Dragging
function startNodeDrag(e: MouseEvent, node: NarrativeNode) {
  e.stopPropagation();
  selectNode(node);
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

function sourcePinType(node: NarrativeNode, port: string): GraphValueType | "flow" {
  if (port.startsWith("choice_") || (node.type === "condition" && ["true", "false"].includes(port)) ||
    (node.type === "waypoint_conversation" && waypointExitPins(node).some((pin) => pin.id === port))) return "flow";
  if (port === "output") {
    if (node.type === "get_entity") return "entity_ref";
    if (node.type === "get_entity_property") return graphNodeValueType(node) ?? "any";
    if (["has_entity_tag", "knows", "compare", "and", "or"].includes(node.type)) return "bool";
    return "flow";
  }
  return "flow";
}


function pinColor(node: NarrativeNode, port: string) {
  return GRAPH_PIN_COLORS[sourcePinType(node, port)];
}

function targetPinColor(node: NarrativeNode, port: string) {
  const type = graphTargetPinType(node, port);
  return GRAPH_PIN_COLORS[type ?? "any"];
}

const activeConnectionColor = computed(() => {
  const connection = activeConnection.value;
  const source = connection && nodes.value.find((node) => node.id === connection.fromNode);
  return source && connection ? GRAPH_PIN_COLORS[sourcePinType(source, connection.fromPort)] : GRAPH_PIN_COLORS.flow;
});

function edgeColor(edge: NarrativeEdge) {
  if ((edge.kind ?? "flow") === "flow") return GRAPH_PIN_COLORS.flow;
  const source = nodes.value.find((node) => node.id === edge.from_node);
  return source ? GRAPH_PIN_COLORS[sourcePinType(source, edge.from_port)] : GRAPH_PIN_COLORS.any;
}


function completeConnection(e: MouseEvent, targetNode: NarrativeNode, port: string) {
  e.stopPropagation();
  if (!activeConnection.value) return;
  if (activeConnection.value.fromNode === targetNode.id) return;
  const fromNode = nodes.value.find((node) => node.id === activeConnection.value!.fromNode);
  if (!fromNode) return;
  const sourceType = sourcePinType(fromNode, activeConnection.value.fromPort);
  if (!graphConnectionIsCompatible(sourceType, targetNode, port)) {
    activeConnection.value = null;
    return;
  }
  if (sourceType === "flow") {
    edges.value = edges.value.filter((edge) => !(edge.from_node === activeConnection.value!.fromNode && edge.from_port === activeConnection.value!.fromPort));
  }
  const allowsMultipleInputs = (targetNode.type === "and" || targetNode.type === "or") && sourceType !== "flow";
  if (allowsMultipleInputs) {
    edges.value = edges.value.filter((edge) => !(edge.to_node === targetNode.id && edge.from_node === activeConnection.value!.fromNode && edge.from_port === activeConnection.value!.fromPort));
  } else {
    edges.value = edges.value.filter((edge) => !(edge.to_node === targetNode.id && edge.to_port === port && edge.kind === (sourceType === "flow" ? "flow" : "data")));
  }
  edges.value.push({
    id: `edge_${activeConnection.value.fromNode}_${activeConnection.value.fromPort}_${targetNode.id}`,
    kind: sourceType === "flow" ? "flow" : "data",
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
  if (!window.confirm("Delete this node and its connections?")) return;
  nodes.value = nodes.value.filter((n) => n.id !== nodeId);
  edges.value = edges.value.filter((e) => e.from_node !== nodeId && e.to_node !== nodeId);
  if (selectedNodeId.value === nodeId) {
    selectedNodeId.value = null;
    emit("select-node", null);
  }
  emitUpdate();
}

// Create new node of type
function addNode(type: NodeType) {
  const nextId = `node_${type}_${Date.now().toString(36)}`;
  const fromCanvasMenu = contextMenu.value?.kind === "canvas" && contextWorld.value !== null;
  let x = fromCanvasMenu
    ? contextWorld.value!.x
    : Math.round((-panX.value + 400) / zoom.value);
  let y = fromCanvasMenu
    ? contextWorld.value!.y
    : Math.round((-panY.value + 250) / zoom.value);

  // Toolbar inserts would otherwise stack exactly on top of each other; walk a
  // diagonal cascade until the slot is free.
  if (!fromCanvasMenu) {
    let attempts = 0;
    const maxAttempts = nodes.value.length + 2;
    while (
      attempts < maxAttempts &&
      nodes.value.some((node) => Math.abs(node.x - x) < 24 && Math.abs(node.y - y) < 24)
    ) {
      x += 48;
      y += 48;
      attempts += 1;
    }
  }

  const baseNode: NarrativeNode = {
    id: nextId,
    type,
    title: nodeTypeLabel(type),
    x,
    y,
    width: 260,
    height: 160,
  };
  if (type === "start") {
    baseNode.text = "Begin Discovery";
    baseNode.height = 80;
  }

  if (type === "get_entity") {
    baseNode.entity_path = "";
    baseNode.height = 100;
  } else if (type === "get_entity_property") {
    baseNode.property_key = "";
    baseNode.value_type = "string";
    baseNode.height = 120;
  } else if (type === "apply_command") {
    baseNode.property_key = "";
    baseNode.operation = "set";
    baseNode.value_type = "string";
    baseNode.value = "";
    baseNode.height = 120;
  } else if (type === "dialog") {
    baseNode.speaker_role = "";
    baseNode.text = "Spoken text or monologue...";
    baseNode.sections = [{ id: `section_${nextId}`, title: "Main Beat", items: [{ kind: "line", id: `line_${nextId}`, speaker_role: "", target_role: "player", content: "", emotion_tag: "" }] }];
  } else if (type === "choice") {
    baseNode.choices = [
      { id: "choice_0", text: "Proceed ahead" },
      { id: "choice_1", text: "Turn back" },
    ];
    baseNode.height = 160;
  } else if (type === "condition") {
    baseNode.height = 140;
  } else if (type === "compare") {
    baseNode.operator = "eq";
    baseNode.value_type = "string";
    baseNode.value = "";
    baseNode.height = 120;
  } else if (type === "image") {
    baseNode.image_url = "";
    baseNode.height = 140;
  } else if (type === "teleport") {
    baseNode.destination_storylet = "";
    baseNode.height = 100;
  } else if (type === "learn" || type === "knows") {
    baseNode.fact_tag = "";
    if (type === "learn") { baseNode.kind = "evidence"; baseNode.visibility = "private"; }
    else { baseNode.holder = "acting"; baseNode.confirmed_only = false; }
  } else if (type === "go_to_location") {
    baseNode.location_id = studioState.entityTemplates.find((entity) => entity.root_tag === "location")?.id ?? "";
    baseNode.height = 100;
  } else if (type === "qbn_storylet_hub") {
    baseNode.deck_storylet_ids = [];
    baseNode.hub_exit_storylet_id = "";
    baseNode.height = 120;
  } else if (type === "salience_selector") {
    baseNode.candidate_pool_ids = [];
    baseNode.height = 120;
  } else if (type === "waypoint_conversation") {
    baseNode.topic_nodes = [];
    baseNode.topic_edges = [];
    baseNode.initial_topic_id = "";
    baseNode.npc_goal_topic_id = "";
    baseNode.height = 120;
  } else if (type === "waiter") {
    baseNode.wait_event = "";
    baseNode.height = 100;
  } else if (type === "scene_section") {
    baseNode.duration_ms = 10_000;
    baseNode.timeline_events = [];
    baseNode.height = 100;
  } else if (type === "mission_sub_graph") {
    baseNode.sub_graph_asset_id = "";
    baseNode.height = 100;
  } else if (type === "poi") {
    baseNode.entity_path = "";
    baseNode.height = 100;
  } else if (type === "graph_transform") {
    baseNode.rule = {
      lhs: { vertices: [{ layer: "fact", vertex_id: "", attributes: [] }], edges: [] },
      rhs: { add_vertices: [], remove_vertices: [], add_edges: [], remove_edges: [], set_attributes: [] },
    };
    baseNode.height = 160;
  } else if (type === "passion_mutation") {
    baseNode.op = { micro_deltas: [], active_ops: [] };
    baseNode.height = 140;
  } else if (type === "add_entity_tag") {
    baseNode.target_entity_id = "world";
    baseNode.tag = "";
    baseNode.height = 110;
  } else if (type === "remove_entity_tag") {
    baseNode.target_entity_id = "world";
    baseNode.tag = "";
    baseNode.height = 110;
  } else if (type === "has_entity_tag") {
    baseNode.target_entity_id = "world";
    baseNode.tag = "";
    baseNode.match_mode = "all";
    baseNode.height = 140;
  } else if (type === "and" || type === "or") {
    baseNode.height = 100;
  } else if (type === "end") {
    baseNode.is_terminal = true;
    baseNode.height = 80;
  }

  nodes.value.push(baseNode);
  contextMenu.value = null;
  selectNode(baseNode);
  emitUpdate();
}

function waypointExitPins(node: NarrativeNode): Array<{ id: string; label: string }> {
  return (node.topic_nodes ?? []).flatMap((topic) => {
    if (topic.triggers_scene_exit !== true || typeof topic.exit_output_pin !== "string" || !topic.exit_output_pin) return [];
    return [{ id: topic.exit_output_pin, label: String(topic.topic_id ?? topic.exit_output_pin) }];
  });
}

const conditionalBranchPins = [
  { id: "true", label: "True" },
  { id: "false", label: "False" },
];

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
    } else if (fromNode.type === "waypoint_conversation") {
      const idx = waypointExitPins(fromNode).findIndex((pin) => pin.id === edge.from_port);
      if (idx >= 0) fromY = fromNode.y + 72 + idx * 30;
    } else if (fromNode.type === "condition") {
      const idx = conditionalBranchPins.findIndex((pin) => pin.id === edge.from_port);
      if (idx >= 0) fromY = fromNode.y + 58 + idx * 30;
    }

    const startX = fromNode.x + fromNode.width;
    const startY = fromY;
    const endX = toNode.x;
    const inputOffset = edge.kind === "data"
      ? ["apply_command", "condition", "get_entity_property", "add_entity_tag", "remove_entity_tag", "has_entity_tag"].includes(toNode.type) ? 52 : 40
      : toNode.type === "condition" || toNode.type === "apply_command" ? 18 : 40;
    const endY = toNode.y + inputOffset;
    return {
      id: edge.id,
      path: calculateBezier(startX, startY, endX, endY),
      startX,
      startY,
      endX,
      endY,
      color: edgeColor(edge),
    };
  }).filter(Boolean);
});

</script>

<template>
  <div
    ref="containerRef"
    class="graph-canvas-container"
    @mousedown="onCanvasMouseDown"
    @mousemove="onCanvasMouseMove"
    @mouseup="onCanvasMouseUp"
    @wheel="onWheel"
    @contextmenu.prevent="onCanvasContextMenu"
  >
    <!-- Top Floating Toolbar -->
    <div class="graph-toolbar">
      <div style="display:flex; align-items:center; gap:8px; min-width:0; max-width:calc(100% - 150px); overflow-x:auto;">
        <button class="node-tool-btn" @click="addNode('start')">Start</button>
        <button class="node-tool-btn" @click="addNode('get_entity')">Get entity</button>
        <button class="node-tool-btn" @click="addNode('get_entity_property')">Get property</button>
        <button class="node-tool-btn" @click="addNode('compare')">Compare</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('apply_command')">Apply command</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('dialog')">Dialog</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('choice')">Choice</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('qbn_storylet_hub')">QBN hub</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('salience_selector')">Salience</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('waypoint_conversation')">Waypoint</button>
        <button class="node-tool-btn" @click="addNode('and')">And</button>
        <button class="node-tool-btn" @click="addNode('or')">Or</button>
        <button class="node-tool-btn" @click="addNode('condition')">Condition</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('image')">Image</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('teleport')">Teleport</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('go_to_location')">Go to location</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('add_entity_tag')">+ Tag</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('remove_entity_tag')">- Tag</button>
        <button class="node-tool-btn" @click="addNode('has_entity_tag')">Has tag</button>
        <button v-if="props.graphMode === 'story'" class="node-tool-btn" @click="addNode('learn')">Learn</button>
        <button class="node-tool-btn" @click="addNode('knows')">Knows</button>
        <button class="node-tool-btn" @click="addNode('end')">End</button>
      </div>

      <div style="display:flex; align-items:center; gap:8px;">
        <span style="font-size:11px; font-weight:600; width:45px; text-align:center;">
          {{ Math.round(zoom * 100) }}%
        </span>
        <button class="btn btn-secondary" style="font-size:11px; padding:3px 8px;" @click="zoom = 1.0; panX = 40; panY = 60;">Reset</button>
        <button v-if="props.graphMode === 'story'" class="btn btn-primary" style="font-size:11px; padding:4px 10px;" @click="emitUpdate">Sync to Storylet</button>
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
            <path d="M 0 1 L 8 5 L 0 9 z" fill="context-stroke" />
          </marker>
        </defs>

        <!-- Existing Edges -->
        <g v-for="edge in computedEdges" :key="edge!.id">
          <!-- Clickable thick hit area -->
          <path
            :d="edge!.path"
            class="edge-hit-area"
            @click.stop="selectedEdgeId = edge!.id; selectedNodeId = null; emit('select-node', null)"
            @contextmenu.stop.prevent="openMenu($event, 'edge', edge!.id)"
          />
          <!-- Visible Bezier line -->
          <path
            :d="edge!.path"
            :class="['narrative-edge-line', selectedEdgeId === edge!.id ? 'selected' : '']"
            :style="{ stroke: edge!.color }"
            marker-end="url(#socket-arrow)"
          />
        </g>

        <path v-if="scissorStart && scissorEnd" :d="`M ${scissorStart.x} ${scissorStart.y} L ${scissorEnd.x} ${scissorEnd.y}`" class="scissor-line" />

        <!-- Active Dragging Connection Line -->
        <path
          v-if="activeConnection"
          :d="calculateBezier(activeConnection.startX, activeConnection.startY, activeConnection.currentX, activeConnection.currentY)"
          class="active-connecting-line"
          :style="{ stroke: activeConnectionColor }"
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
        @click.stop="selectNode(node)"
        @contextmenu.stop.prevent="openMenu($event, 'node', node.id)"
      >
        <!-- Node Header -->
        <div class="node-header" @mousedown="startNodeDrag($event, node)">
          <div class="node-header-title">
            <span class="node-icon">
              {{ node.type === 'start' ? '▶' : node.type === 'dialog' ? '💬' : node.type === 'choice' ? '🔀' : node.type === 'image' ? '🎨' : node.type === 'go_to_location' ? '📍' : node.type === 'graph_transform' ? '🕸️' : node.type === 'passion_mutation' ? '💗' : node.type === 'add_entity_tag' ? '🏷️+' : node.type === 'remove_entity_tag' ? '🏷️-' : node.type === 'has_entity_tag' ? '🏷️?' : '⚙️' }}
            </span>
            <input v-model="node.title" class="node-title-input" @input="emitUpdate" />
          </div>
          <button class="node-close-btn" title="Delete node" aria-label="Delete node" @click.stop="removeNode(node.id)">✕</button>
        </div>

        <!-- Node Body Content based on Type -->
        <div class="node-body">
          <!-- START -->
          <div v-if="node.type === 'start'">
            <div class="field-label">Entrypoint</div>
            <div style="font-size:12px; font-weight:600;">{{ node.text || 'Begin Scene' }}</div>
          </div>
          <div v-else-if="node.type === 'get_entity'">
            <div class="field-label">Entity reference</div>
            <div style="font-size:11px;">{{ node.entity_path || 'Choose an entity' }} · EntityRef output</div>
          </div>
          <div v-else-if="node.type === 'compare'">
            <div class="field-label">Compare</div>
            <div style="font-size:11px;">{{ node.operator || 'eq' }} {{ node.value ?? '' }} · Bool output</div>
          </div>
          <div v-else-if="node.type === 'get_entity_property'">
            <div class="field-label">Get Entity Property</div>
            <div style="font-size:11px;">{{ node.property_key || 'property key' }} · {{ node.value_type || 'typed value' }}</div>
          </div>
          <div v-else-if="node.type === 'apply_command'">
            <div class="field-label">Apply Command</div>
            <div style="font-size:11px;">{{ node.operation || 'set' }} {{ node.property_key || 'property key' }} = {{ node.value ?? '' }}</div>
          </div>

          <!-- DIALOG -->
          <div v-else-if="node.type === 'dialog'">
            <div class="field-label">Dialogue sequence</div>
            <div style="font-size:11px;">{{ node.sections?.length ?? 0 }} sections · {{ node.sections?.reduce((total, section) => total + section.items.length, 0) ?? 0 }} steps</div>
            <div v-if="node.sections?.[0]?.items?.[0]?.kind === 'line'" class="node-dialog-preview">
              {{ node.sections[0].items[0].content || 'Select to edit dialogue…' }}
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
                :style="{ backgroundColor: GRAPH_PIN_COLORS.flow }"
                title="Connect this choice branch"
                @mousedown="startConnection($event, node, `choice_${idx}`, 72 + idx * 36)"
              ></div>
            </div>
          </div>

          <!-- IMAGE -->
          <div v-else-if="node.type === 'image'">
            <div v-if="!node.image_url" style="font-size:11px; color:var(--text-muted); padding:8px 0;">
              No image URL configured.
            </div>
            <div v-else-if="!isSafeImageUrl(node.image_url) || failedImageNodes.has(node.id)" style="font-size:11px; color:var(--text-muted); padding:8px 0;">
              Image URL is unsafe or unavailable.
            </div>
            <div v-else style="text-align:center; margin-top:6px;">
              <img :src="node.image_url" @error="markImageFailed(node.id)" style="width:100%; height:110px; object-fit:cover; border-radius:6px; border:1px solid var(--border-subtle);" />
            </div>
          </div>

          <!-- TELEPORT / DIVERT -->
          <div v-else-if="node.type === 'teleport'">
            <div class="field-label">Target Storylet</div>
            <div style="font-size:11px;">{{ node.destination_storylet || 'Select to configure…' }}</div>
          </div>

          <div v-else-if="node.type === 'go_to_location'">
            <div class="field-label">Destination Location</div>
            <div style="font-size:11px;">{{ studioState.entityTemplates.find((location) => location.id === node.location_id)?.name || 'Choose a location…' }}</div>
          </div>

          <div v-else-if="node.type === 'qbn_storylet_hub'">
            <div class="field-label">Eligible storylet deck</div>
            <div style="font-size:11px;">{{ node.deck_storylet_ids?.length ?? 0 }} candidates · exit {{ node.hub_exit_storylet_id || 'unset' }}</div>
          </div>
          <div v-else-if="node.type === 'salience_selector'">
            <div class="field-label">Automatic context match</div>
            <div style="font-size:11px;">{{ node.candidate_pool_ids?.length ?? 0 }} candidates</div>
          </div>
          <div v-else-if="node.type === 'waypoint_conversation'">
            <div class="field-label">Topic graph</div>
            <div style="font-size:11px;">{{ node.topic_nodes?.length ?? 0 }} topics · {{ node.topic_edges?.length ?? 0 }} transitions</div>
            <div v-for="(pin, index) in waypointExitPins(node)" :key="pin.id" class="choice-option-row">
              <span style="font-size:11px; flex:1;">{{ pin.label }}</span>
              <div class="socket output-socket choice-socket" :style="{ backgroundColor: GRAPH_PIN_COLORS.flow }" :title="`Connect ${pin.label} exit`" @mousedown="startConnection($event, node, pin.id, 72 + index * 30)"></div>
            </div>
          </div>
          <div v-else-if="node.type === 'waiter'">
            <div class="field-label">Wait for gameplay event</div>
            <div style="font-size:11px;">{{ node.wait_event || 'Configure event…' }}</div>
          </div>
          <div v-else-if="node.type === 'and' || node.type === 'or'">
            <div class="field-label">{{ node.type === 'and' ? 'AND boolean' : 'OR boolean' }}</div>
            <div style="font-size:11px;">Combines boolean inputs</div>
          </div>
          <div v-else-if="node.type === 'condition'">
            <div class="field-label">Condition Node · Boolean switch input</div>
            <div v-for="(pin, index) in conditionalBranchPins" :key="pin.id" class="choice-option-row">
              <span style="font-size:11px; flex:1;">{{ pin.label }}</span>
              <div class="socket output-socket choice-socket" :style="{ backgroundColor: GRAPH_PIN_COLORS.flow }" :title="`Connect ${pin.label} branch`" @mousedown="startConnection($event, node, pin.id, 58 + index * 30)"></div>
            </div>
          </div>
          <div v-else-if="node.type === 'scene_section'">
            <div class="field-label">Timeline section</div>
            <div style="font-size:11px;">{{ node.duration_ms ?? 0 }} ms · {{ node.timeline_events?.length ?? 0 }} events</div>
          </div>
          <div v-else-if="node.type === 'mission_sub_graph'">
            <div class="field-label">Scoped mission</div>
            <div style="font-size:11px;">{{ node.sub_graph_asset_id || 'Choose subgraph…' }}</div>
          </div>
          <div v-else-if="node.type === 'poi'">
            <div class="field-label">Point of interest</div>
            <div style="font-size:11px;">{{ node.entity_path || 'Configure entity path…' }}</div>
          </div>
          <div v-else-if="node.type === 'graph_transform'">
            <div class="field-label">Graph rewrite rule</div>
            <div style="font-size:11px;">{{ node.rule?.lhs.vertices?.length ?? 0 }} vertex patterns · {{ node.rule?.lhs.edges?.length ?? 0 }} edge patterns</div>
            <div style="font-size:11px;">{{ (node.rule?.rhs?.add_vertices?.length ?? 0) + (node.rule?.rhs?.remove_vertices?.length ?? 0) + (node.rule?.rhs?.add_edges?.length ?? 0) + (node.rule?.rhs?.remove_edges?.length ?? 0) + (node.rule?.rhs?.set_attributes?.length ?? 0) }} mutations</div>
          </div>
          <div v-else-if="node.type === 'passion_mutation'">
            <div class="field-label">Passion state change</div>
            <div style="font-size:11px;">{{ node.op?.micro_deltas?.length ?? 0 }} typed deltas · {{ node.op?.active_ops?.length ?? 0 }} active-pool ops</div>
          </div>
          <!-- ADD / REMOVE ENTITY TAG -->
          <div v-else-if="node.type === 'add_entity_tag' || node.type === 'remove_entity_tag'">
            <div class="field-label">{{ node.type === 'add_entity_tag' ? 'Add Entity Tag' : 'Remove Entity Tag' }}</div>
            <div style="font-size:11px; font-weight:600; color:var(--text-main); margin-top:2px;">
              {{ node.tag || 'No tag specified' }}
            </div>
            <div style="font-size:10px; color:var(--text-muted); margin-top:2px;">
              Target: {{ node.target_entity_id === 'world' || !node.target_entity_id ? 'World (Global)' : (studioState.entityTemplates.find(e => e.id === node.target_entity_id)?.name || node.target_entity_id) }}
            </div>
          </div>

          <!-- HAS ENTITY TAG -->
          <div v-else-if="node.type === 'has_entity_tag'">
            <div class="field-label">Has Tag Query</div>
            <div style="font-size:11px; font-weight:600; margin-bottom:4px; color:var(--text-main);">
              {{ node.tag || 'No tag specified' }}
            </div>
            <div style="font-size:10px; color:var(--text-muted);">Boolean output</div>
          </div>
          <div v-else-if="node.type === 'learn' || node.type === 'knows'">{{ node.fact_tag || 'Choose fact' }}</div>
          <div v-else-if="node.type === 'end'">
            <div v-if="props.graphMode === 'preconditions'" style="font-size:12px; font-weight:700; color:var(--status-danger);">{{ node.precondition_result === false ? 'Reject candidate' : 'Valid candidate' }}</div>
            <div v-else style="font-size:12px; font-weight:700; color:var(--status-danger);">{{ node.jump_to_storylet || 'Terminal ending' }}</div>
          </div>
        </div>
        <!-- Data/flow input sockets -->
        <template v-if="node.type !== 'start'">
          <div v-if="node.type === 'apply_command' || node.type === 'condition'" class="socket input-socket" style="top:18px" :style="{ backgroundColor: GRAPH_PIN_COLORS.flow }" title="Flow input" @mouseup="completeConnection($event, node, 'input')"></div>
          <div v-if="node.type === 'get_entity_property' || node.type === 'apply_command'" class="socket input-socket" style="top:52px" :style="{ backgroundColor: targetPinColor(node, 'entity') }" title="Entity data input" @mouseup="completeConnection($event, node, 'entity')"></div>
          <div v-if="['add_entity_tag','remove_entity_tag','has_entity_tag'].includes(node.type)" class="socket input-socket" style="top:52px" :style="{ backgroundColor: targetPinColor(node, 'target') }" title="Optional target EntityRef input" @mouseup="completeConnection($event, node, 'target')"></div>
          <div v-if="node.type === 'condition'" class="socket input-socket" style="top:52px" :style="{ backgroundColor: targetPinColor(node, 'input') }" title="Boolean data input" @mouseup="completeConnection($event, node, 'input')"></div>
          <div v-if="node.type === 'and' || node.type === 'or'" class="socket input-socket" :style="{ backgroundColor: targetPinColor(node, 'input') }" title="Boolean data input" @mouseup="completeConnection($event, node, 'input')"></div>
          <div v-if="node.type === 'compare'" class="socket input-socket" :style="{ backgroundColor: targetPinColor(node, 'input') }" title="Value data input" @mouseup="completeConnection($event, node, 'input')"></div>
          <div v-if="!['get_entity', 'get_entity_property', 'apply_command', 'condition', 'has_entity_tag', 'knows', 'compare', 'and', 'or'].includes(node.type)" class="socket input-socket" :style="{ backgroundColor: targetPinColor(node, 'input') }" title="Flow input" @mouseup="completeConnection($event, node, 'input')"></div>
        </template>

        <!-- Default Output Socket (Right edge) -->
        <div
          v-if="node.type !== 'choice' && node.type !== 'end' && node.type !== 'condition' && (node.type !== 'waypoint_conversation' || waypointExitPins(node).length === 0)"
          class="socket output-socket"
          title="Output connection port"
          :style="{ backgroundColor: pinColor(node, 'output') }"
          @mousedown="startConnection($event, node, 'output', 40)"
        ></div>
      </div>
    </div>
    <div v-if="contextMenu" class="graph-context-menu" :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }" @mousedown.stop @click.stop>
      <template v-if="contextMenu.kind === 'canvas'">
        <input v-model="nodeSearch" class="form-input" placeholder="Search node types" autofocus @keydown.esc="contextMenu = null" />
        <button v-for="type in matchingNodeTypes" :key="type" class="context-menu-item" @click="addNode(type)">+ {{ nodeTypeLabel(type) }}</button>
        <div v-if="!matchingNodeTypes.length" class="context-menu-empty">No matching node types</div>
      </template>
      <button v-else class="context-menu-item danger" @click="contextMenu.kind === 'node' ? removeNode(contextMenu.targetId!) : removeEdge(contextMenu.targetId!); contextMenu = null">Delete {{ contextMenu.kind }}</button>
    </div>
  </div>
</template>

<style scoped>
.graph-canvas-container {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background-color: var(--bg-canvas);
  background-image: radial-gradient(#1d2332 1.2px, transparent 1.2px);
  background-size: 24px 24px;
  cursor: grab;
  user-select: none;
}

.graph-canvas-container:active {
  cursor: grabbing;
}

.graph-toolbar {
  position: absolute;
  top: 12px;
  left: 16px;
  right: 16px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--bg-card);
  border: 1px solid var(--border-subtle);
  border-radius: 0;
  z-index: 100;
}

.node-tool-btn {
  background: var(--surface-container-high, #212735);
  border: 1px solid var(--border-subtle);
  color: var(--text-main);
  font-size: 12px;
  font-weight: 500;
  min-height: 44px;
  padding: 0 12px;
  border-radius: 0;
  cursor: pointer;
  transition: background-color 0.12s ease-out, border-color 0.12s ease-out, color 0.12s ease-out;
  flex: 0 0 auto;
  white-space: nowrap;
}

.node-tool-btn:hover {
  background: var(--bg-card-hover);
  border-color: var(--accent);
  color: var(--accent);
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

.scissor-line {
  fill: none;
  stroke: #ff453a;
  stroke-width: 3;
  stroke-dasharray: 7 5;
  pointer-events: none;
}

.node-dialog-preview {
  margin-top: 6px;
  color: #c7c7cc;
  max-height: 40px;
  overflow: hidden;
}

.graph-context-menu {
  position: absolute;
  z-index: 200;
  display: flex;
  flex-direction: column;
  gap: 4px;
  width: 220px;
  max-height: min(360px, 70vh);
  padding: 8px;
  overflow: auto;
  background: var(--surface-container-high, #212735);
  border: 1px solid var(--border-muted);
  border-radius: 0;
}

.context-menu-item { min-height: 44px; padding: 0 10px; text-align: left; color: var(--text-main); background: transparent; border: 0; border-radius: 0; cursor: pointer; transition: background-color 0.12s ease-out; }
.context-menu-item:hover { background: var(--bg-card-hover); }
.context-menu-item.danger { color: var(--status-danger); }
.context-menu-empty { padding: 6px 8px; color: var(--text-muted); font-size: 12px; }

.active-connecting-line {
  fill: none;
  stroke: #ffd60a;
  stroke-width: 2.5;
  stroke-dasharray: 5 4;
}

/* Node cards (DESIGN.md): solid fill, 1px structural border, zero radius, 2px type strip */
.graph-node {
  position: absolute;
  background: var(--bg-card);
  border: 1px solid var(--border-subtle);
  border-top: 2px solid var(--text-dim);
  border-radius: 0;
  pointer-events: auto;
  user-select: none;
  transition: border-color 0.12s ease-out;
}

.graph-node.selected {
  border-color: var(--accent);
  border-top-color: var(--accent);
  outline: 1px solid var(--accent);
  outline-offset: 0;
}

.graph-node.start { border-top-color: var(--status-success); }
.graph-node.dialog { border-top-color: var(--secondary); }
.graph-node.choice { border-top-color: var(--tertiary); }
.graph-node.condition, .graph-node.and, .graph-node.or, .graph-node.compare, .graph-node.has_entity_tag { border-top-color: var(--status-success); }
.graph-node.end { border-top-color: var(--status-danger); }

.node-header {
  padding: 0 10px;
  min-height: 28px;
  background: var(--surface-container-high, #212735);
  color: var(--text-main);
  display: flex;
  justify-content: space-between;
  align-items: center;
  cursor: grab;
}

.node-header-title {
  display: flex;
  align-items: center;
  gap: 6px;
  flex: 1;
}

.node-title-input {
  background: transparent;
  border: none;
  color: var(--text-main);
  font-weight: 700;
  font-size: 12px;
  outline: none;
  width: 100%;
}

.node-close-btn {
  position: relative;
  background: transparent;
  border: none;
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
  padding: 6px 8px;
  min-width: 44px;
  min-height: 44px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
.node-close-btn::after { content: ""; position: absolute; inset: 0; }
.node-close-btn:hover { color: var(--status-danger); }

.node-body {
  padding: 10px 12px;
  color: var(--text-main);
  font-size: 11px;
}

.field-label {
  font-size: 10px;
  font-weight: 700;
  color: var(--text-muted);
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
  width: 12px;
  height: 12px;
  border-radius: 0;
  background: var(--text-dim);
  border: 1px solid var(--bg-card);
  cursor: crosshair;
  z-index: 20;
}
.socket::after { content: ""; position: absolute; inset: -16px; }
.socket:hover { background: var(--accent); }

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

</style>
