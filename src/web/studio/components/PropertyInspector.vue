<script setup lang="ts">
import { sortChronicle } from "../chronicleModel.ts";
import { clockLabels, participantOptions, togglePlayedBy } from "../storyletModel.ts";
import { ref, computed, watch } from "vue";
import type { NarrativeNode, NarrativeGraph, DialogueSection, DialogueItem, GraphLayer, AttributeOp, AttributeValueType, AttributeValue, GraphBinding, VertexPattern } from "../narrativeGraphTypes.ts";
import { applyCommandOperationsForType, isGraphTransformNode, isPassionMutationNode } from "../narrativeGraphTypes.ts";
import type { EntityProperty, PropertyType, StoryletRecord } from "../contracts.ts";
import TagEditor from "./TagEditor.vue";
import ThumbnailPicker from "./ThumbnailPicker.vue";
import { normalizeTagName, uniqueTagNames, buildTagIndex } from "../tagData.ts";
import { studioState, notify } from "../state.ts";
import { hasTraitKey, traitLabels } from "../entityModel.ts";
import { nodeTypeContribution } from "../plugins/runtime.ts";
import StudioIcon from "./StudioIcon.vue";
const props = defineProps<{
  storylet: StoryletRecord;
  graph: NarrativeGraph;
  graphMode?: "story" | "preconditions";
  selectedNode: NarrativeNode | null;
  storylets: StoryletRecord[];
}>();

const emit = defineEmits<{
  (e: "update-storylet"): void;
  (e: "update-graph", graph: NarrativeGraph): void;
  (e: "delete-node", nodeId: string): void;
}>();

function emitGraphUpdate(node: NarrativeNode | null = props.selectedNode) {
  const graph = {
    ...props.graph,
    nodes: node
      ? props.graph.nodes.map((current) => current.id === node.id ? { ...node } : current)
      : props.graph.nodes,
  };
  emit("update-graph", graph);
}

function eventValue(event: Event): string {
  return (event.target as HTMLInputElement | HTMLSelectElement).value;
}

// Registered role choices; player remains the runtime actor sentinel.
const availableRoles = computed(() => uniqueTagNames([
  ...roleTagOptions.value,
  ...props.storylet.parameters.map((p) => p.param_name),
]));

const tagIndex = computed(() => buildTagIndex({
  gameplayTags: studioState.gameplayTags,
  entities: studioState.entities,
  entityTemplates: studioState.entityTemplates,
  traitTemplates: studioState.traitTemplates,
  storylets: [...props.storylets.filter((storylet) => storylet.id !== props.storylet.id), { ...props.storylet, graph: props.graph }],
}));

const runtimeEntityOptions = computed(() => [
  ...studioState.entities.map((entity) => ({ path: entity.id, label: `${entity.name}${traitLabels(entity, studioState.traitTemplates).length ? ` (${traitLabels(entity, studioState.traitTemplates).join(" · ")})` : ""}` })),
  ...studioState.entityTemplates.map((entity) => ({ path: entity.id, label: `${entity.name}${traitLabels(entity, studioState.traitTemplates).length ? ` (${traitLabels(entity, studioState.traitTemplates).join(" · ")})` : ""}` })),
]);
type GraphPropertyType = Exclude<PropertyType, "json" | "storylet" | "tag">;
type GraphPropertyOption = { key: string; name: string; type: GraphPropertyType };
function isGraphProperty(property: EntityProperty): property is EntityProperty & { type: GraphPropertyType } {
  return property.type !== "json" && property.type !== "storylet" && property.type !== "tag";
}

function entityForPath(path: string | undefined) {
  if (!path) return undefined;
  const normalized = path.trim();
  return studioState.entities.find((entity) => entity.id === normalized) ?? studioState.entityTemplates.find((entity) => entity.id === normalized);
}

function incomingEntityNode(node: NarrativeNode): NarrativeNode | undefined {
  const edge = props.graph.edges.find((candidate) =>
    candidate.kind === "data" && candidate.to_node === node.id &&
    (candidate.to_port === "entity" || candidate.to_port === "target" || candidate.to_port === "input"),
  );
  return edge ? props.graph.nodes.find((candidate) => candidate.id === edge.from_node) : undefined;
}

function propertiesForEntitySource(node: NarrativeNode): EntityProperty[] {
  const source = incomingEntityNode(node);
  if (!source) return [];
  if (source.type === "get_entity") return source.entity_path ? entityForPath(source.entity_path)?.properties ?? [] : [];
  if (source.type === "get_entity_property" && source.value_type === "entity") {
    const sourceEntityNode = incomingEntityNode(source);
    const sourceEntity = sourceEntityNode?.type === "get_entity"
      ? entityForPath(sourceEntityNode.entity_path)
      : undefined;
    const reference = sourceEntity?.properties.find((property) =>
      property.key === source.property_key && property.type === "entity" && !property.is_array,
    );
    if (!reference) return [];
    const entities = studioState.entityTemplates;
    if (!entities.length) return [];
    const common = new Map(
      (entities[0]?.properties ?? [])
        .filter((property) => !property.is_array)
        .map((property) => [property.key, property]),
    );
    for (const entity of entities.slice(1)) {
      for (const key of [...common.keys()]) {
        const property = entity.properties.find((candidate) =>
          key === candidate.key && !candidate.is_array && candidate.type === common.get(key)?.type,
        );
        if (!property) common.delete(key);
      }
    }
    return [...common.values()];
  }
  return [];
}

const entityPropertyOptions = computed<GraphPropertyOption[]>(() => {
  if (!props.selectedNode || !["get_entity_property", "apply_command"].includes(props.selectedNode.type)) return [];
  const seen = new Set<string>();
  return propertiesForEntitySource(props.selectedNode)
    .filter(isGraphProperty)
    .filter((property) => !property.is_array && !seen.has(property.key) && seen.add(property.key))
    .map((property) => ({ key: property.key, name: property.name || property.key, type: property.type }));
});

const entityPropertyUnavailableReason = computed(() => {
  const node = props.selectedNode;
  if (!node || !["get_entity_property", "apply_command"].includes(node.type) || entityPropertyOptions.value.length) return "";
  const source = incomingEntityNode(node);
  if (!source) return "Connect an Entity source to choose a property.";
  if (source.type === "get_entity") {
    const path = source.entity_path?.trim();
    if (!path) return "Select an Entity in the connected Get Entity node first.";
    const entity = entityForPath(path);
    if (!entity) return `Connected Entity "${path}" is not loaded in this workspace.`;
    if (!entity.properties.some((property) => !property.is_array)) {
      return `Connected Entity Template "${entity.name}" has no scalar custom properties. Add one under Entity Templates → ${entity.name} → Properties.`;
    }
  }
  if (source.type === "get_entity_property" && source.value_type === "entity") {
    return "The connected Entity reference has no scalar property shared by all possible targets.";
  }
  return "The connected source has no compatible scalar Entity property schema.";
});

function selectEntityProperty(node: NarrativeNode, key: string) {
  const property = entityPropertyOptions.value.find((option) => option.key === key);
  node.property_key = property?.key ?? "";
  node.value_type = property?.type;
  node.value = property?.type === "bool"
    ? false
    : property?.type === "int" || property?.type === "float"
      ? 0
      : "";
  if (node.type === "apply_command") {
    const operation = node.operation;
    const validOperation = property?.type === "bool"
      ? operation === "set" || operation === "toggle"
      : property?.type === "int" || property?.type === "float"
        ? operation === "set" || operation === "add" || operation === "subtract"
        : operation === "set";
    if (!validOperation) node.operation = "set";
  }
  emitGraphUpdate(node);
}
const applyCommandOperations = computed(() => {
  const type = props.selectedNode?.type === "apply_command"
    ? props.selectedNode.value_type
    : undefined;
  return applyCommandOperationsForType(type);
});

watch(
  () => [props.selectedNode?.type, props.selectedNode?.value_type] as const,
  () => {
    const node = props.selectedNode;
    if (!node || node.type !== "apply_command") return;
    const valid = applyCommandOperations.value.some((option) => option.value === node.operation);
    if (!valid) {
      node.operation = "set";
      emitGraphUpdate(node);
    }
  },
);
const slotLabels = computed(() => clockLabels(studioState.entities));
function setWindow(slot: number, checked: boolean) {
  const selected = new Set(props.storylet.window ?? []);
  if (checked) selected.add(slot); else selected.delete(slot);
  props.storylet.window = [...selected].sort((a, b) => a - b);
  emit("update-storylet");
}
const playerOptions = computed(() => participantOptions(studioState.entityTemplates));
function setPlayedBy(id: string, checked: boolean) {
  props.storylet.played_by = togglePlayedBy(props.storylet.played_by ?? [], id, checked);
  emit("update-storylet");
}
const chronicleEntries = computed(() => sortChronicle(studioState.events));
const factTagOptions = computed(() => uniqueTagNames(studioState.gameplayTags.map((tag) => tag.name)).filter((name) => name.startsWith("fact.")));
const catalogTagNames = computed(() => uniqueTagNames(studioState.gameplayTags.map((tag) => tag.name)));
const roleTagOptions = computed(() => catalogTagNames.value);
const targetRoles = computed(() => uniqueTagNames(["player", ...roleTagOptions.value]));
const emotionTagOptions = computed(() => catalogTagNames.value.filter((name) => name === "emotion" || name.startsWith("emotion.")));
const actionTagOptions = computed(() => catalogTagNames.value.filter((name) => name === "action" || name.startsWith("action.")));
const locationTagOptions = computed(() => catalogTagNames.value.filter((name) => name === "location" || name.startsWith("location.")));
const targetGameplayTagOptions = computed(() => catalogTagNames.value);
const requiredRoleNames = computed({
  get: () => uniqueTagNames(props.storylet.parameters.map((parameter) => parameter.param_name)),
  set: (values: string[]) => {
    const names = uniqueTagNames(values);
    const existing = new Map(props.storylet.parameters.map((parameter) => [normalizeTagName(parameter.param_name), parameter]));
    props.storylet.parameters.splice(0, props.storylet.parameters.length, ...names.map((name) => existing.get(normalizeTagName(name)) ?? { param_name: name, query: `role == "${name}"` }));
    emit("update-storylet");
  },
});
const locationTags = computed({
  get: () => uniqueTagNames(props.storylet.location_tags ?? []),
  set: (values: string[]) => { props.storylet.location_tags = uniqueTagNames(values); emit("update-storylet"); },
});
const locationOptions = computed(() => studioState.entityTemplates.filter((entity) => hasTraitKey(entity, studioState.traitTemplates, "location")).sort((a, b) => a.name.localeCompare(b.name)));


// Dialogue Sections & Items
function addSection(node: NarrativeNode) {
  if (!node.sections) node.sections = [];
  const secId = `sec_${Date.now().toString(36)}`;
  node.sections.push({
    id: secId,
    title: `Section ${node.sections.length + 1}`,
    items: [],
  });
  emitGraphUpdate();
}

function removeSection(node: NarrativeNode, idx: number) {
  node.sections?.splice(idx, 1);
  emitGraphUpdate();
}

function addDialogueLine(sec: DialogueSection) {
  const lineId = `line_${Date.now().toString(36)}`;
  sec.items.push({
    kind: "line",
    id: lineId,
    speaker_role: availableRoles.value[0] || "",
    target_role: "player",
    content: "",
    emotion_tag: "",
  });
  emitGraphUpdate();
}

function addIdleAction(sec: DialogueSection) {
  const actId = `act_${Date.now().toString(36)}`;
  sec.items.push({
    kind: "idle_action",
    id: actId,
    action_type: "lookat",
    actor_role: availableRoles.value[0] || "player",
    target: "player",
    duration: 1.5,
  });
  emitGraphUpdate();
}

function removeItem(sec: DialogueSection, idx: number) {
  sec.items.splice(idx, 1);
  emitGraphUpdate();
}

function addChoice(node: NarrativeNode) {
  if (!node.choices) node.choices = [];
  node.choices.push({ id: `choice_${Date.now().toString(36)}`, text: "New choice" });
  emitGraphUpdate();
}

function removeChoice(node: NarrativeNode, index: number) {
  node.choices?.splice(index, 1);
  emitGraphUpdate();
}

// Audio inspection & Duration Calculation
async function handleAudioBrowse(event: Event, item: any) {
  const input = event.target as HTMLInputElement;
  if (!input.files || input.files.length === 0) return;
  const file = input.files[0];
  item.audio_path = file.name;

  const audioUrl = URL.createObjectURL(file);
  const audio = new Audio();
  audio.preload = "metadata";
  audio.onloadedmetadata = () => {
    if (Number.isFinite(audio.duration)) {
      item.duration = Math.round(audio.duration * 10) / 10;
      emitGraphUpdate();
      notify("Loaded audio: " + item.duration + "s", "success");
    }
    URL.revokeObjectURL(audioUrl);
  };
  audio.onerror = () => {
    URL.revokeObjectURL(audioUrl);
    notify("The selected file audio duration could not be read", "error");
  };
  audio.src = audioUrl;
}

async function checkAudioBackend(item: any) {
  if (!item.audio_path) return;
  try {
    const res = await fetch(`/api/v3/audio/inspect?path=${encodeURIComponent(item.audio_path)}`);
    const data = await res.json();
    if (data.exists && Number.isFinite(data.duration)) {
      item.duration = data.duration;
      emitGraphUpdate();
      notify("Audio duration: " + data.duration + "s", "success");
    } else if (data.exists) {
      notify("File found; duration metadata is unavailable for this format", "info");
    } else {
      notify(`File '${item.audio_path}' not found on server`, "info");
    }
  } catch {
    // ignore
  }
}



// Graph Transform Node (ChronoFlow v5)
const graphLayers: GraphLayer[] = ["location", "character", "item", "fact"];
const attributeOps: AttributeOp[] = ["eq", "neq", "gt", "gte", "lt", "lte"];
const attributeValueTypes: AttributeValueType[] = ["int", "float", "bool", "string", "tag"];
const bindingTypes: Array<"literal" | "vertex_pattern"> = ["literal", "vertex_pattern"];
type GraphPatternList = "vertices" | "edges";
type GraphMutationList = "add_vertices" | "remove_vertices" | "add_edges" | "remove_edges" | "set_attributes";

function attributeValueFrom(type: string, raw: string): AttributeValue {
  const numeric = Number(raw);
  if (type === "bool") return { type: "bool", value: raw === "true" };
  if (type === "string") return { type: "string", value: raw };
  if (type === "tag") return { type: "tag", value: Number.isInteger(numeric) ? numeric : 0 };
  if (type === "float") return { type: "float", value: Number.isFinite(numeric) ? numeric : 0 };
  return { type: "int", value: Number.isInteger(numeric) ? numeric : 0 };
}

function bindingRaw(binding: GraphBinding | undefined): string {
  if (!binding) return "";
  return binding.type === "literal" ? binding.id : String(binding.index);
}

function setAttributeValue(node: NarrativeNode, assign: (value: AttributeValue) => void, type: string, raw: string) {
  assign(attributeValueFrom(type, raw));
  emitGraphUpdate(node);
}

function setBinding(node: NarrativeNode, assign: (binding: GraphBinding) => void, type: string, raw: string) {
  const numeric = Number(raw);
  assign(type === "vertex_pattern"
    ? { type: "vertex_pattern", index: Number.isInteger(numeric) ? numeric : 0 }
    : { type: "literal", id: raw });
  emitGraphUpdate(node);
}

function addGraphPatternEntry(node: NarrativeNode, list: GraphPatternList) {
  if (!isGraphTransformNode(node)) return;
  const entries: unknown[] = node.rule.lhs[list];
  entries.push(list === "vertices"
    ? { layer: "fact", attributes: [] }
    : { source: { type: "literal", id: "" }, target: { type: "vertex_pattern", index: 0 } });
  emitGraphUpdate(node);
}

function addGraphMutationEntry(node: NarrativeNode, list: GraphMutationList) {
  if (!isGraphTransformNode(node)) return;
  const binding: GraphBinding = { type: "literal", id: "" };
  const entries: unknown[] = node.rule.rhs[list];
  if (list === "add_vertices") entries.push({ id: binding, layer: "fact", attributes: {} });
  else if (list === "add_edges") entries.push({ id: binding, source: { type: "literal", id: "" }, target: { type: "vertex_pattern", index: 0 }, relation_type: "", weight: 1 });
  else if (list === "set_attributes") entries.push({ vertex: binding, key: "", value: { type: "int", value: 0 } });
  else entries.push(binding);
  emitGraphUpdate(node);
}

function addAttributePredicate(node: NarrativeNode, pattern: VertexPattern) {
  pattern.attributes.push({ key: "", op: "eq", value: { type: "bool", value: false } });
  emitGraphUpdate(node);
}

function addRecordAttribute(node: NarrativeNode, attributes: Record<string, AttributeValue>) {
  attributes[`attribute${Object.keys(attributes).length + 1}`] = { type: "bool", value: false };
  emitGraphUpdate(node);
}

function renameRecordAttribute(node: NarrativeNode, attributes: Record<string, AttributeValue>, key: string, nextKey: string) {
  const name = nextKey.trim();
  if (!name || name === key) return;
  const value = attributes[key];
  delete attributes[key];
  attributes[name] = value;
  emitGraphUpdate(node);
}

function removeRecordAttribute(node: NarrativeNode, attributes: Record<string, AttributeValue>, key: string) {
  delete attributes[key];
  emitGraphUpdate(node);
}

// Passion Mutation Node (ChronoFlow v5)
function addPassionRow(node: NarrativeNode, list: "micro_deltas" | "active_ops") {
  if (!isPassionMutationNode(node)) return;
  const entries: unknown[] = node.op[list];
  entries.push(list === "micro_deltas" ? [0, 0] : [0, false]);
  emitGraphUpdate(node);
}

function setPassionRow(node: NarrativeNode, row: [number, number | boolean], index: number, raw: string) {
  const numeric = Number(raw);
  if (index === 0) row[0] = Number.isInteger(numeric) ? numeric : 0;
  else if (typeof row[1] === "boolean") row[1] = raw === "true";
  else row[1] = Number.isFinite(numeric) ? numeric : 0;
  emitGraphUpdate(node);
}

function removeEntry(node: NarrativeNode, entries: unknown[], index: number) {
  entries.splice(index, 1);
  emitGraphUpdate(node);
}

function handleJumpTargetChange(node: NarrativeNode) {
  if (node.jump_to_storylet) {
    node.is_terminal = false;
  }
  emitGraphUpdate(node);
}
function setNodeStringArray(node: NarrativeNode, field: "deck_storylet_ids" | "candidate_pool_ids", event: Event) {
  const select = event.target as HTMLSelectElement;
  node[field] = Array.from(select.selectedOptions, (option) => option.value);
  emitGraphUpdate(node);
}
function setNodeJsonArray(node: NarrativeNode, field: "topic_nodes" | "topic_edges" | "timeline_events", event: Event) {
  const textarea = event.target as HTMLTextAreaElement;
  try {
    const value: unknown = JSON.parse(textarea.value);
    if (!Array.isArray(value)) throw new Error("Expected a JSON array.");
    node[field] = value as Array<Record<string, unknown>>;
    emitGraphUpdate(node);
  } catch (error) {
    notify(`Invalid JSON array: ${error instanceof Error ? error.message : String(error)}`, "error");
  }
}
</script>

<template>
  <aside class="property-inspector-panel">
    <!-- 1. NODE PROPERTIES (when a node is selected) -->
    <div v-if="selectedNode" class="inspector-content">
      <div class="panel-header">
        <div style="display:flex; align-items:center; gap:8px; min-width:0;">
          <span :class="['type-badge', selectedNode.type]">{{ selectedNode.type === 'condition' ? 'condition node' : selectedNode.type.replaceAll('_', ' ') }}</span>
          <h3 class="panel-title">{{ selectedNode.title }}</h3>
        </div>
        <button class="btn btn-danger" style="font-size:11px; padding:3px 8px;" @click="$emit('delete-node', selectedNode.id)">
          Delete Node
        </button>
      </div>

      <div class="form-group">
        <label class="field-label" for="node-title-input">Node Title</label>
        <input id="node-title-input" v-model="selectedNode.title" aria-label="Node title" class="form-input" @input="emitGraphUpdate(selectedNode)" />
      </div>

      <!-- DIALOG NODE -->
      <div v-if="selectedNode.type === 'dialog'">
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
          <label class="field-label" style="margin-bottom:0;">Dialogue Sections</label>
          <button class="btn btn-secondary" style="font-size:10px; padding:2px 6px;" @click="addSection(selectedNode)">
            + Add Section
          </button>
        </div>

        <div v-for="(sec, sIdx) in selectedNode.sections" :key="sec.id" class="dialogue-section-card">
          <div class="section-card-header">
            <input v-model="sec.title" class="section-title-input" aria-label="Dialogue section title" @input="emitGraphUpdate(selectedNode)" />
            <div style="display:flex; gap:4px; flex:none; white-space:nowrap;">
              <button class="btn-micro" aria-label="Add dialogue line" @click="addDialogueLine(sec)">+ Line</button>
              <button class="btn-micro" aria-label="Add idle action" @click="addIdleAction(sec)">+ Idle Action</button>
              <button class="btn-micro danger" aria-label="Remove dialogue section" title="Remove dialogue section" @click="removeSection(selectedNode, sIdx)"><StudioIcon name="close" /></button>
            </div>
          </div>

          <div class="section-items">
            <div v-for="(item, iIdx) in sec.items" :key="item.id" class="dialogue-item-card" :data-tag-item-id="item.id">
              <!-- LINE ITEM -->
              <div v-if="item.kind === 'line'">
                <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px;">
                  <span class="item-kind-label item-kind-label--line"><StudioIcon name="chat" /> Dialogue</span>
                  <button class="btn-micro danger" aria-label="Remove dialogue item" title="Remove dialogue item" @click="removeItem(sec, iIdx)"><StudioIcon name="close" /></button>
                </div>

                <div style="display:grid; grid-template-columns:repeat(2, minmax(0, 1fr)); gap:6px; margin-bottom:6px;">
                  <div>
                    <label class="field-sublabel">Speaker Role</label>
                    <TagEditor v-model="item.speaker_role" :options="roleTagOptions" :allow-create="false" :multiple="false" label="Speaker role" placeholder="Choose role" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                  <div>
                    <label class="field-sublabel">Addressed To</label>
                    <TagEditor v-model="item.target_role" :options="targetRoles" :allow-create="false" :multiple="false" label="Addressed to role" placeholder="Choose role" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>

                <div style="margin-bottom:6px;">
                  <label class="field-sublabel" :for="'dialogue-text-' + item.id">Dialogue Text</label>
                  <textarea :id="'dialogue-text-' + item.id" v-model="item.content" aria-label="Dialogue text" class="form-textarea" style="min-height:48px; font-size:11px; padding:4px;" @input="emitGraphUpdate(selectedNode)"></textarea>
                </div>

                <div style="display:grid; grid-template-columns:repeat(2, minmax(0, 1fr)); gap:6px; margin-bottom:6px;">
                  <div>
                    <label class="field-sublabel">Emotion Tag</label>
                    <TagEditor v-model="item.emotion_tag" :options="emotionTagOptions" :allow-create="false" :multiple="false" label="Emotion tag" placeholder="Choose emotion" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                  <div>
                    <label class="field-sublabel">Action Tag (Tied)</label>
                    <TagEditor v-model="item.action_tag" :options="actionTagOptions" :allow-create="false" :multiple="false" label="Action tag" placeholder="Choose action" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>

                <div v-if="item.action_tag" style="margin-bottom:6px;">
                  <label class="field-sublabel">Action Target</label>
                  <TagEditor v-model="item.action_target" :options="targetRoles" :allow-create="false" :multiple="false" label="Action target" placeholder="Choose role" @update:model-value="emitGraphUpdate(selectedNode)" />
                </div>

                <!-- Audio Link & Duration -->
                <div style="background:rgba(0,0,0,0.25); padding:6px; border-radius:6px;">
                  <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px;">
                    <label class="field-sublabel" style="margin-bottom:0;">Audio Path & Duration</label>
                    <label class="audio-browse-label">
                      <StudioIcon name="folder" /> Browse
                      <input type="file" accept="audio/*" style="display:none;" @change="handleAudioBrowse($event, item)" />
                    </label>
                  </div>
                  <div style="display:flex; gap:4px;">
                    <input v-model="item.audio_path" aria-label="Audio file path" class="form-input micro" style="flex:1;" placeholder="audio/line_01.mp3" @input="emitGraphUpdate(selectedNode)" />
                    <button class="btn-micro" title="Check server file duration" aria-label="Check server file duration" @click="checkAudioBackend(item)"><StudioIcon name="search" /></button>
                    <input v-model.number="item.duration" aria-label="Audio duration in seconds" type="number" step="0.1" class="form-input micro" style="width:60px;" placeholder="Sec" @input="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>
              </div>

              <!-- IDLE ACTION ITEM -->
              <div v-else-if="item.kind === 'idle_action'">
                <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px;">
                  <span class="item-kind-label item-kind-label--idle"><StudioIcon name="settings" /> Idle action</span>
                  <button class="btn-micro danger" aria-label="Remove idle action" title="Remove idle action" @click="removeItem(sec, iIdx)"><StudioIcon name="close" /></button>
                </div>

                <div style="display:grid; grid-template-columns:repeat(2, minmax(0, 1fr)); gap:6px; margin-bottom:4px;">
                  <div>
                    <label class="field-sublabel">Action Type</label>
                    <select v-model="item.action_type" class="form-select micro" @change="emitGraphUpdate(selectedNode)">
                      <option value="lookat">Look At</option>
                      <option value="emote">Emote</option>
                    </select>
                  </div>
                  <div>
                    <label class="field-sublabel">Actor Role</label>
                    <TagEditor v-model="item.actor_role" :options="roleTagOptions" :allow-create="false" :multiple="false" label="Actor role" placeholder="Choose role" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>

                <div style="display:grid; grid-template-columns:repeat(2, minmax(0, 1fr)); gap:6px;">
                  <div>
                    <label class="field-sublabel">Target</label>
                    <TagEditor v-model="item.target" :options="targetRoles" :allow-create="false" :multiple="false" label="Target role" placeholder="Choose role" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                  <div v-if="item.action_type === 'emote'">
                    <label class="field-sublabel">Emote Tag</label>
                    <TagEditor v-model="item.emote" :options="actionTagOptions" :allow-create="false" :multiple="false" label="Emote action tag" placeholder="Choose action" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                  <div v-else>
                    <label class="field-sublabel">Duration (s)</label>
                    <input v-model.number="item.duration" type="number" step="0.1" class="form-input micro" @input="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'start'">
        <p class="field-help">The start node marks the entry point of this narrative graph.</p>
      </div>

      <div v-else-if="selectedNode.type === 'choice'">
        <div class="field-row-heading"><label class="field-label">Choice Options</label><button class="btn-micro" @click="addChoice(selectedNode)">+ Add Choice</button></div>
        <div v-for="(choice, index) in selectedNode.choices || []" :key="choice.id" class="choice-editor-row">
          <input v-model="choice.text" aria-label="Choice text" class="form-input micro" placeholder="Choice text" @input="emitGraphUpdate(selectedNode)" />
          <input v-model="choice.system_verb" aria-label="Choice system verb, optional" class="form-input micro" placeholder="System verb (optional)" @input="emitGraphUpdate(selectedNode)" />
          <button class="btn-micro danger" title="Remove choice" aria-label="Remove choice" @click="removeChoice(selectedNode, index)"><StudioIcon name="close" /></button>
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'image'">
        <div class="form-group">
          <label class="field-label">Image URL</label>
          <input v-model="selectedNode.image_url" class="form-input" placeholder="Image URL or project-relative path" @input="emitGraphUpdate(selectedNode)" />
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'teleport'">
        <div class="form-group"><label class="field-label">Destination Storylet</label><select v-model="selectedNode.destination_storylet" class="form-select" @change="emitGraphUpdate(selectedNode)"><option value="">Choose destination…</option><option v-for="story in props.storylets" :key="story.id" :value="story.id">{{ story.title }} ({{ story.id }})</option></select></div>
      </div>

      <div v-else-if="selectedNode.type === 'reveal'">
        <div class="form-group">
          <label class="field-label" for="reveal-event">Canon event</label>
          <select id="reveal-event" v-model="selectedNode.event_id" class="form-select" @change="emitGraphUpdate(selectedNode)">
            <option value="">Choose a canon event…</option>
            <option v-for="entry in chronicleEntries" :key="entry.id" :value="entry.id">{{ entry.story_time_label || entry.title }} ({{ entry.id }})</option>
          </select>
          <p v-if="!chronicleEntries.length" class="field-hint">No canon events with a story time yet. Add one in Rules &amp; events.</p>
        </div>
        <div class="form-group"><label class="field-label" for="reveal-kind">Knowledge kind</label><select id="reveal-kind" v-model="selectedNode.kind" class="form-select" @change="emitGraphUpdate(selectedNode)"><option v-for="kind in ['evidence','witnessed','told','rumor','recollection','testimony','inference']" :key="kind" :value="kind">{{ kind }}</option></select></div>
        <div class="form-group"><label class="field-label" for="reveal-visibility">Visibility</label><select id="reveal-visibility" v-model="selectedNode.visibility" class="form-select" @change="emitGraphUpdate(selectedNode)"><option value="private">Private</option><option value="co_located">Co-located</option></select></div>
      </div>

      <div v-else-if="selectedNode.type === 'learn' || selectedNode.type === 'knows'">
        <div class="form-group">
          <TagEditor v-model="selectedNode.fact_tag" :options="factTagOptions" :allow-create="false" :multiple="false" label="Fact" placeholder="Choose a fact" @update:model-value="emitGraphUpdate(selectedNode)" />
        </div>
        <template v-if="selectedNode.type === 'learn'">
          <div class="form-group"><label class="field-label" for="learn-kind">Knowledge kind</label><select id="learn-kind" v-model="selectedNode.kind" class="form-select" @change="emitGraphUpdate(selectedNode)"><option v-for="kind in ['evidence','witnessed','told','rumor','recollection','testimony','inference']" :key="kind" :value="kind">{{ kind }}</option></select></div>
          <div class="form-group"><label class="field-label" for="learn-visibility">Visibility</label><select id="learn-visibility" v-model="selectedNode.visibility" class="form-select" @change="emitGraphUpdate(selectedNode)"><option value="private">Private</option><option value="co_located">Co-located</option></select></div>
        </template>
        <template v-else>
          <div class="form-group"><label class="field-label" for="knows-holder">Holder</label><select id="knows-holder" v-model="selectedNode.holder" class="form-select" @change="emitGraphUpdate(selectedNode)"><option value="acting">Acting participant</option><option v-for="entity in studioState.entityTemplates.filter((e) => e.traits?.some((t) => t.template_id === 'system.participant'))" :key="entity.id" :value="entity.id">{{ entity.name }}</option></select></div>
          <div class="form-group"><label><input v-model="selectedNode.confirmed_only" type="checkbox" @change="emitGraphUpdate(selectedNode)" /> Confirmed only</label></div>
        </template>
      </div>

      <div v-else-if="selectedNode.type === 'go_to_location'">
        <div class="form-group">
          <label class="field-label">Destination Location</label>
          <select v-model="selectedNode.location_id" class="form-select" @change="emitGraphUpdate(selectedNode)">
            <option value="">Choose a location…</option>
            <option v-for="location in locationOptions" :key="location.id" :value="location.id">{{ location.name }} ({{ location.id }})</option>
          </select>
          <p v-if="locationOptions.length === 0" class="field-help">Add a location to the project before configuring this node.</p>
        </div>
      </div>


      <!-- BOOLEAN CONDITION SWITCH -->
      <div v-else-if="selectedNode.type === 'condition'">
        <div class="field-label">Condition Node</div>
        <p class="field-help">Routes the boolean received from a connected predicate node through True or False.</p>
      </div>

      <!-- GRAPH TRANSFORM NODE -->
      <div v-else-if="isGraphTransformNode(selectedNode)">
        <div class="field-row-heading"><label class="field-label">Left-Hand Side Pattern</label></div>

        <div class="field-row-heading">
          <label class="field-sublabel">Vertex Patterns</label>
          <button class="btn-micro" @click="addGraphPatternEntry(selectedNode, 'vertices')">+ Add Vertex</button>
        </div>
        <div v-for="(vertex, index) in selectedNode.rule.lhs.vertices" :key="index" class="graph-pattern-row">
          <select v-model="vertex.layer" class="form-select micro" style="width:85px;" @change="emitGraphUpdate(selectedNode)">
            <option v-for="layer in graphLayers" :key="layer" :value="layer">{{ layer }}</option>
          </select>
          <input v-model="vertex.vertex_id" class="form-input micro" style="flex:1;" placeholder="vertex id (blank matches any)" @input="emitGraphUpdate(selectedNode)" />
          <button class="btn-micro" @click="addAttributePredicate(selectedNode, vertex)">+ Predicate</button>
          <button class="btn-micro danger" aria-label="Remove vertex pattern" title="Remove vertex pattern" @click="removeEntry(selectedNode, selectedNode.rule.lhs.vertices, index)"><StudioIcon name="close" /></button>
          <div v-for="(predicate, predicateIndex) in vertex.attributes" :key="predicateIndex" class="attribute-row">
            <input v-model="predicate.key" class="form-input micro" style="width:110px;" placeholder="attribute key" @input="emitGraphUpdate(selectedNode)" />
            <select v-model="predicate.op" class="form-select micro" style="width:55px;" @change="emitGraphUpdate(selectedNode)">
              <option v-for="op in attributeOps" :key="op" :value="op">{{ op }}</option>
            </select>
            <select class="form-select micro" style="width:65px;" :value="predicate.value.type" @change="setAttributeValue(selectedNode, (value) => { predicate.value = value; }, eventValue($event), String(predicate.value.value))">
              <option v-for="valueType in attributeValueTypes" :key="valueType" :value="valueType">{{ valueType }}</option>
            </select>
            <input class="form-input micro" style="width:80px;" :value="String(predicate.value.value)" @input="setAttributeValue(selectedNode, (value) => { predicate.value = value; }, predicate.value.type, eventValue($event))" />
            <button class="btn-micro danger" aria-label="Remove predicate" title="Remove predicate" @click="removeEntry(selectedNode, vertex.attributes, predicateIndex)"><StudioIcon name="close" /></button>
          </div>
        </div>

        <div class="field-row-heading">
          <label class="field-sublabel">Edge Patterns</label>
          <button class="btn-micro" @click="addGraphPatternEntry(selectedNode, 'edges')">+ Add Edge</button>
        </div>
        <div v-for="(pattern, index) in selectedNode.rule.lhs.edges" :key="index" class="graph-pattern-row">
          <input v-model="pattern.edge_id" class="form-input micro" style="width:110px;" placeholder="edge id (blank matches any)" @input="emitGraphUpdate(selectedNode)" />
          <input v-model="pattern.relation_type" class="form-input micro" style="width:110px;" placeholder="relation type" @input="emitGraphUpdate(selectedNode)" />
          <button class="btn-micro danger" aria-label="Remove edge pattern" title="Remove edge pattern" @click="removeEntry(selectedNode, selectedNode.rule.lhs.edges, index)"><StudioIcon name="close" /></button>
          <div class="binding-row">
            <label class="field-sublabel">source</label>
            <select class="form-select micro" style="width:105px;" :value="pattern.source?.type ?? 'literal'" @change="setBinding(selectedNode, (binding) => { pattern.source = binding; }, eventValue($event), bindingRaw(pattern.source))">
              <option v-for="bindingType in bindingTypes" :key="bindingType" :value="bindingType">{{ bindingType }}</option>
            </select>
            <input class="form-input micro" style="flex:1;" :value="bindingRaw(pattern.source)" @input="setBinding(selectedNode, (binding) => { pattern.source = binding; }, pattern.source?.type ?? 'literal', eventValue($event))" />
          </div>
          <div class="binding-row">
            <label class="field-sublabel">target</label>
            <select class="form-select micro" style="width:105px;" :value="pattern.target?.type ?? 'vertex_pattern'" @change="setBinding(selectedNode, (binding) => { pattern.target = binding; }, eventValue($event), bindingRaw(pattern.target))">
              <option v-for="bindingType in bindingTypes" :key="bindingType" :value="bindingType">{{ bindingType }}</option>
            </select>
            <input class="form-input micro" style="flex:1;" :value="bindingRaw(pattern.target)" @input="setBinding(selectedNode, (binding) => { pattern.target = binding; }, pattern.target?.type ?? 'vertex_pattern', eventValue($event))" />
          </div>
        </div>

        <div class="field-row-heading"><label class="field-label">Right-Hand Side Mutations</label></div>

        <div class="field-row-heading">
          <label class="field-sublabel">Add Vertices</label>
          <button class="btn-micro" @click="addGraphMutationEntry(selectedNode, 'add_vertices')">+ Add Vertex</button>
        </div>
        <div v-for="(template, index) in selectedNode.rule.rhs.add_vertices" :key="index" class="graph-pattern-row">
          <div class="binding-row">
            <label class="field-sublabel">id</label>
            <select class="form-select micro" style="width:105px;" :value="template.id.type" @change="setBinding(selectedNode, (binding) => { template.id = binding; }, eventValue($event), bindingRaw(template.id))">
              <option v-for="bindingType in bindingTypes" :key="bindingType" :value="bindingType">{{ bindingType }}</option>
            </select>
            <input class="form-input micro" style="flex:1;" :value="bindingRaw(template.id)" @input="setBinding(selectedNode, (binding) => { template.id = binding; }, template.id.type, eventValue($event))" />
            <select v-model="template.layer" class="form-select micro" style="width:85px;" @change="emitGraphUpdate(selectedNode)">
              <option v-for="layer in graphLayers" :key="layer" :value="layer">{{ layer }}</option>
            </select>
            <button class="btn-micro" @click="addRecordAttribute(selectedNode, template.attributes)">+ Attribute</button>
            <button class="btn-micro danger" aria-label="Remove vertex" title="Remove vertex" @click="removeEntry(selectedNode, selectedNode.rule.rhs.add_vertices, index)"><StudioIcon name="close" /></button>
          </div>
          <div v-for="(value, key) in template.attributes" :key="key" class="attribute-row">
            <input :value="key" class="form-input micro" style="width:110px;" placeholder="attribute key" @change="renameRecordAttribute(selectedNode, template.attributes, key, eventValue($event))" />
            <select class="form-select micro" style="width:65px;" :value="value.type" @change="setAttributeValue(selectedNode, (next) => { template.attributes[key] = next; }, eventValue($event), String(value.value))">
              <option v-for="valueType in attributeValueTypes" :key="valueType" :value="valueType">{{ valueType }}</option>
            </select>
            <input class="form-input micro" style="width:80px;" :value="String(value.value)" @input="setAttributeValue(selectedNode, (next) => { template.attributes[key] = next; }, value.type, eventValue($event))" />
            <button class="btn-micro danger" aria-label="Remove attribute" title="Remove attribute" @click="removeRecordAttribute(selectedNode, template.attributes, key)"><StudioIcon name="close" /></button>
          </div>
        </div>

        <div class="field-row-heading">
          <label class="field-sublabel">Remove Vertices</label>
          <button class="btn-micro" @click="addGraphMutationEntry(selectedNode, 'remove_vertices')">+ Add Binding</button>
        </div>
        <div v-for="(binding, index) in selectedNode.rule.rhs.remove_vertices" :key="index" class="mutation-row">
          <select class="form-select micro" style="width:105px;" :value="binding.type" @change="setBinding(selectedNode!, (next) => { selectedNode!.rule!.rhs.remove_vertices[index] = next; }, eventValue($event), bindingRaw(binding))">
            <option v-for="bindingType in bindingTypes" :key="bindingType" :value="bindingType">{{ bindingType }}</option>
          </select>
          <input class="form-input micro" style="flex:1;" :value="bindingRaw(binding)" @input="setBinding(selectedNode!, (next) => { selectedNode!.rule!.rhs.remove_vertices[index] = next; }, binding.type, eventValue($event))" />
          <button class="btn-micro danger" aria-label="Remove binding" title="Remove binding" @click="removeEntry(selectedNode, selectedNode.rule.rhs.remove_vertices, index)"><StudioIcon name="close" /></button>
        </div>

        <div class="field-row-heading">
          <label class="field-sublabel">Add Edges</label>
          <button class="btn-micro" @click="addGraphMutationEntry(selectedNode, 'add_edges')">+ Add Edge</button>
        </div>
        <div v-for="(edge, index) in selectedNode.rule.rhs.add_edges" :key="index" class="graph-pattern-row">
          <div class="binding-row">
            <label class="field-sublabel">id</label>
            <select class="form-select micro" style="width:105px;" :value="edge.id.type" @change="setBinding(selectedNode, (binding) => { edge.id = binding; }, eventValue($event), bindingRaw(edge.id))">
              <option v-for="bindingType in bindingTypes" :key="bindingType" :value="bindingType">{{ bindingType }}</option>
            </select>
            <input class="form-input micro" style="flex:1;" :value="bindingRaw(edge.id)" @input="setBinding(selectedNode, (binding) => { edge.id = binding; }, edge.id.type, eventValue($event))" />
          </div>
          <div class="binding-row">
            <label class="field-sublabel">source</label>
            <select class="form-select micro" style="width:105px;" :value="edge.source.type" @change="setBinding(selectedNode, (binding) => { edge.source = binding; }, eventValue($event), bindingRaw(edge.source))">
              <option v-for="bindingType in bindingTypes" :key="bindingType" :value="bindingType">{{ bindingType }}</option>
            </select>
            <input class="form-input micro" style="flex:1;" :value="bindingRaw(edge.source)" @input="setBinding(selectedNode, (binding) => { edge.source = binding; }, edge.source.type, eventValue($event))" />
          </div>
          <div class="binding-row">
            <label class="field-sublabel">target</label>
            <select class="form-select micro" style="width:105px;" :value="edge.target.type" @change="setBinding(selectedNode, (binding) => { edge.target = binding; }, eventValue($event), bindingRaw(edge.target))">
              <option v-for="bindingType in bindingTypes" :key="bindingType" :value="bindingType">{{ bindingType }}</option>
            </select>
            <input class="form-input micro" style="flex:1;" :value="bindingRaw(edge.target)" @input="setBinding(selectedNode, (binding) => { edge.target = binding; }, edge.target.type, eventValue($event))" />
          </div>
          <input v-model="edge.relation_type" class="form-input micro" style="width:110px;" placeholder="relation type" @input="emitGraphUpdate(selectedNode)" />
          <input type="number" class="form-input micro" style="width:70px;" :value="edge.weight" @input="edge.weight = Number.isFinite(Number(eventValue($event))) ? Number(eventValue($event)) : 0; emitGraphUpdate(selectedNode)" />
          <button class="btn-micro danger" aria-label="Remove edge" title="Remove edge" @click="removeEntry(selectedNode, selectedNode.rule.rhs.add_edges, index)"><StudioIcon name="close" /></button>
        </div>

        <div class="field-row-heading">
          <label class="field-sublabel">Set Attributes</label>
          <button class="btn-micro" @click="addGraphMutationEntry(selectedNode, 'set_attributes')">+ Add Assignment</button>
        </div>
        <div v-for="(assignment, index) in selectedNode.rule.rhs.set_attributes" :key="index" class="mutation-row">
          <select class="form-select micro" style="width:105px;" :value="assignment.vertex.type" @change="setBinding(selectedNode, (binding) => { assignment.vertex = binding; }, eventValue($event), bindingRaw(assignment.vertex))">
            <option v-for="bindingType in bindingTypes" :key="bindingType" :value="bindingType">{{ bindingType }}</option>
          </select>
          <input class="form-input micro" style="width:110px;" :value="bindingRaw(assignment.vertex)" @input="setBinding(selectedNode, (binding) => { assignment.vertex = binding; }, assignment.vertex.type, eventValue($event))" />
          <input v-model="assignment.key" class="form-input micro" style="width:110px;" placeholder="attribute key" @input="emitGraphUpdate(selectedNode)" />
          <select class="form-select micro" style="width:65px;" :value="assignment.value.type" @change="setAttributeValue(selectedNode, (value) => { assignment.value = value; }, eventValue($event), String(assignment.value.value))">
            <option v-for="valueType in attributeValueTypes" :key="valueType" :value="valueType">{{ valueType }}</option>
          </select>
          <input class="form-input micro" style="width:80px;" :value="String(assignment.value.value)" @input="setAttributeValue(selectedNode, (value) => { assignment.value = value; }, assignment.value.type, eventValue($event))" />
          <button class="btn-micro danger" aria-label="Remove assignment" title="Remove assignment" @click="removeEntry(selectedNode, selectedNode.rule.rhs.set_attributes, index)"><StudioIcon name="close" /></button>
        </div>

        <p class="field-help">The left-hand side matches graph vertices and edges; the right-hand side applies the rewrite. Pattern indexes refer to matched left-hand side vertices.</p>
      </div>

      <!-- PASSION MUTATION NODE -->
      <div v-else-if="isPassionMutationNode(selectedNode)">
        <div class="field-row-heading">
          <label class="field-label">Micro Passion Deltas</label>
          <button class="btn-micro" @click="addPassionRow(selectedNode, 'micro_deltas')">+ Add Delta</button>
        </div>
        <div v-for="(delta, index) in selectedNode.op.micro_deltas" :key="index" class="mutation-row">
          <input type="number" class="form-input micro" style="width:70px;" :value="delta[0]" @input="setPassionRow(selectedNode, delta, 0, eventValue($event))" />
          <input type="number" class="form-input micro" style="flex:1;" :value="delta[1]" @input="setPassionRow(selectedNode, delta, 1, eventValue($event))" />
          <button class="btn-micro danger" aria-label="Remove delta" title="Remove delta" @click="removeEntry(selectedNode, selectedNode.op.micro_deltas, index)"><StudioIcon name="close" /></button>
        </div>

        <div class="field-row-heading">
          <label class="field-label">Active Pool Operations</label>
          <button class="btn-micro" @click="addPassionRow(selectedNode, 'active_ops')">+ Add Operation</button>
        </div>
        <div v-for="(operation, index) in selectedNode.op.active_ops" :key="index" class="mutation-row">
          <input type="number" class="form-input micro" style="width:70px;" :value="operation[0]" @input="setPassionRow(selectedNode, operation, 0, eventValue($event))" />
          <select class="form-select micro" style="width:105px;" :value="operation[1] ? 'true' : 'false'" @change="setPassionRow(selectedNode, operation, 1, eventValue($event))">
            <option value="true">activate</option>
            <option value="false">deactivate</option>
          </select>
          <button class="btn-micro danger" aria-label="Remove operation" title="Remove operation" @click="removeEntry(selectedNode, selectedNode.op.active_ops, index)"><StudioIcon name="close" /></button>
        </div>

        <p class="field-help">Passion ids are u16 passion indices; deltas and active-pool flags apply through the runtime passion model.</p>
      </div>

      <div v-else-if="selectedNode.type === 'qbn_storylet_hub'">
        <div class="form-group">
          <label class="field-label">QBN Storylet Deck</label>
          <select multiple class="form-select" :value="selectedNode.deck_storylet_ids ?? []" @change="setNodeStringArray(selectedNode, 'deck_storylet_ids', $event)">
            <option v-for="item in props.storylets" :key="item.id" :value="item.id">{{ item.title }} ({{ item.id }})</option>
          </select>
          <p class="field-help">Eligible storylets appear in authored priority order.</p>
        </div>
        <div class="form-group">
          <label class="field-label">Hub Exit Storylet</label>
          <select v-model="selectedNode.hub_exit_storylet_id" class="form-select" @change="emitGraphUpdate(selectedNode)">
            <option value="">Choose the gate storylet…</option>
            <option v-for="item in props.storylets" :key="item.id" :value="item.id">{{ item.title }} ({{ item.id }})</option>
          </select>
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'salience_selector'">
        <div class="form-group">
          <label class="field-label">Salience Candidate Pool</label>
          <select multiple class="form-select" :value="selectedNode.candidate_pool_ids ?? []" @change="setNodeStringArray(selectedNode, 'candidate_pool_ids', $event)">
            <option v-for="item in props.storylets" :key="item.id" :value="item.id">{{ item.title }} ({{ item.id }})</option>
          </select>
          <p class="field-help">The runtime selects the highest-scoring eligible storylet.</p>
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'waypoint_conversation'">
        <div class="form-group">
          <label class="field-label">Initial Topic</label>
          <input v-model="selectedNode.initial_topic_id" class="form-input" @input="emitGraphUpdate(selectedNode)" />
        </div>
        <div class="form-group">
          <label class="field-label">NPC Goal Topic</label>
          <input v-model="selectedNode.npc_goal_topic_id" class="form-input" @input="emitGraphUpdate(selectedNode)" />
        </div>
        <div class="form-group">
          <label class="field-label">Topic Nodes (JSON array)</label>
          <textarea class="form-input" rows="6" :value="JSON.stringify(selectedNode.topic_nodes ?? [], null, 2)" @change="setNodeJsonArray(selectedNode, 'topic_nodes', $event)"></textarea>
        </div>
        <div class="form-group">
          <label class="field-label">Topic Edges (JSON array)</label>
          <textarea class="form-input" rows="6" :value="JSON.stringify(selectedNode.topic_edges ?? [], null, 2)" @change="setNodeJsonArray(selectedNode, 'topic_edges', $event)"></textarea>
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'waiter'">
        <div class="form-group">
          <label class="field-label">Gameplay Event</label>
          <input v-model="selectedNode.wait_event" class="form-input" placeholder="e.g. player_interacted" @input="emitGraphUpdate(selectedNode)" />
          <p class="field-help">The signal pauses here until the host submits this event.</p>
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'and' || selectedNode.type === 'or'">
        <p class="field-help">{{ selectedNode.type === 'and' ? 'AND combines all incoming boolean values.' : 'OR combines incoming boolean values.' }}</p>
      </div>

      <div v-else-if="selectedNode.type === 'scene_section'">
        <div class="form-group">
          <label class="field-label">Duration (milliseconds)</label>
          <input v-model.number="selectedNode.duration_ms" type="number" min="0" class="form-input" @input="emitGraphUpdate(selectedNode)" />
        </div>
        <div class="form-group">
          <label class="field-label">Timeline Events (JSON array)</label>
          <textarea class="form-input" rows="8" :value="JSON.stringify(selectedNode.timeline_events ?? [], null, 2)" @change="setNodeJsonArray(selectedNode, 'timeline_events', $event)"></textarea>
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'mission_sub_graph'">
        <div class="form-group">
          <label class="field-label">Mission Storylet</label>
          <select v-model="selectedNode.sub_graph_asset_id" class="form-select" @change="emitGraphUpdate(selectedNode)">
            <option value="">Choose a storylet graph…</option>
            <option v-for="story in props.storylets.filter((item) => item.id !== props.storylet.id)" :key="story.id" :value="story.id">{{ story.title }} ({{ story.id }})</option>
          </select>
        </div>
      </div>

      <div v-else-if="selectedNode.type === 'poi'">
        <div class="form-group">
          <label class="field-label">Entity Path</label>
          <input v-model="selectedNode.entity_path" class="form-input" placeholder="Location/actor binding" @input="emitGraphUpdate(selectedNode)" />
        </div>
      </div>

      <!-- ADD / REMOVE ENTITY TAG -->
      <div v-else-if="selectedNode.type === 'add_entity_tag' || selectedNode.type === 'remove_entity_tag'">
        <div class="panel-header">
          <h3 class="panel-title">{{ selectedNode.type === 'add_entity_tag' ? 'Add Entity Tag' : 'Remove Entity Tag' }}</h3>
        </div>
        <div class="form-group">
          <label class="field-label">Target Entity</label>
          <select v-model="selectedNode.target_entity_id" class="form-select" @change="emitGraphUpdate()">
            <option value="world">World (Global)</option>
            <option v-for="e in studioState.entityTemplates.filter(item => item.id !== 'world')" :key="e.id" :value="e.id">
              {{ e.name }} ({{ e.id }}) {{ traitLabels(e, studioState.traitTemplates).join(" · ") }}
            </option>
          </select>
          <small class="field-sublabel">Tags are applied to this entity's gameplay tags container.</small>
        </div>
        <div class="form-group">
          <label class="field-label">Tag Name</label>
          <TagEditor v-model="selectedNode.tag" :options="targetGameplayTagOptions" :allow-create="false" :multiple="false" label="Tag name" placeholder="Choose a registered gameplay tag" @update:model-value="emitGraphUpdate()" />
          <small class="field-sublabel" style="display:block; margin-top:4px;">Canonical tag: <strong>{{ selectedNode.tag || '(none)' }}</strong></small>
        </div>
      </div>

      <!-- HAS ENTITY TAG -->
      <div v-else-if="selectedNode.type === 'plugin'" data-testid="plugin-node-inspector">
        <component
          :is="nodeTypeContribution(selectedNode.plugin_type)!.inspector"
          v-if="nodeTypeContribution(selectedNode.plugin_type)?.inspector"
          :node="selectedNode"
          :graph="graph"
          :config="selectedNode.plugin_config ?? {}"
          @update:config="selectedNode.plugin_config = $event; emitGraphUpdate(selectedNode)"
        />
        <p v-else class="field-sublabel">Plugin '{{ selectedNode.plugin_type }}' is not loaded or has no inspector.</p>
      </div>
      <div v-else-if="selectedNode.type === 'get_entity'">
        <div class="form-group">
          <label class="field-label">Entity</label>
          <select
            :value="selectedNode.entity_path || ''"
            class="form-select"
            @change="selectedNode.entity_path = eventValue($event); emitGraphUpdate(selectedNode)"
          >
            <option value="">Choose an entity</option>
            <option v-for="entity in runtimeEntityOptions" :key="entity.path" :value="entity.path">{{ entity.label }}</option>
          </select>
        </div>
      </div>
      <div v-else-if="selectedNode.type === 'get_entity_property'">
        <div class="form-group">
          <label class="field-label">Property</label>
          <select :value="selectedNode.property_key || ''" class="form-select" :disabled="!entityPropertyOptions.length" @change="selectEntityProperty(selectedNode, eventValue($event))">
            <option value="">Choose a property</option>
            <option v-for="property in entityPropertyOptions" :key="property.key" :value="property.key">{{ property.name }} ({{ property.type }})</option>
          </select>
          <small v-if="entityPropertyUnavailableReason" class="field-sublabel">{{ entityPropertyUnavailableReason }}</small>
        </div>
      </div>
      <div v-else-if="selectedNode.type === 'apply_command'">
        <div class="form-group">
          <label class="field-label">Property</label>
          <select :value="selectedNode.property_key || ''" class="form-select" :disabled="!entityPropertyOptions.length" @change="selectEntityProperty(selectedNode, eventValue($event))">
            <option value="">Choose a property</option>
            <option v-for="property in entityPropertyOptions" :key="property.key" :value="property.key">{{ property.name }} ({{ property.type }})</option>
          </select>
          <small v-if="entityPropertyUnavailableReason" class="field-sublabel">{{ entityPropertyUnavailableReason }}</small>
        </div>
        <div class="form-group"><label class="field-label">Operation</label><select v-model="selectedNode.operation" class="form-select" @change="emitGraphUpdate(selectedNode)"><option v-for="operation in applyCommandOperations" :key="operation.value" :value="operation.value">{{ operation.label }}</option></select></div>
        <div class="form-group"><label class="field-label">Command Value</label>
          <select v-if="selectedNode.value_type === 'bool'" v-model="selectedNode.value" class="form-select" @change="emitGraphUpdate(selectedNode)"><option :value="true">True</option><option :value="false">False</option></select>
          <input v-else v-model="selectedNode.value" :type="selectedNode.value_type === 'int' || selectedNode.value_type === 'float' ? 'number' : 'text'" class="form-input" @input="selectedNode.value = (selectedNode.value_type === 'int' ? Number.parseInt(String(selectedNode.value), 10) : selectedNode.value_type === 'float' ? Number.parseFloat(String(selectedNode.value)) : selectedNode.value); emitGraphUpdate(selectedNode)" />
        </div>
      </div>
      <div v-else-if="selectedNode.type === 'compare'">
        <div class="field-label">Compare</div>
        <label class="field-label">Value Type<select v-model="selectedNode.value_type" class="form-select" @change="emitGraphUpdate(selectedNode)"><option value="bool">Bool</option><option value="int">Int</option><option value="float">Float</option><option value="string">String</option><option value="asset">Asset</option><option value="entity">Entity Ref</option></select></label>
        <label class="field-label">Typed value
          <select v-if="selectedNode.value_type === 'bool'" v-model="selectedNode.value" class="form-select" @change="emitGraphUpdate(selectedNode)"><option :value="true">True</option><option :value="false">False</option></select>
          <input v-else v-model="selectedNode.value" :type="selectedNode.value_type === 'int' || selectedNode.value_type === 'float' ? 'number' : 'text'" class="form-input" @input="selectedNode.value = (selectedNode.value_type === 'int' ? Number.parseInt(String(selectedNode.value), 10) : selectedNode.value_type === 'float' ? Number.parseFloat(String(selectedNode.value)) : selectedNode.value); emitGraphUpdate(selectedNode)" />
        </label>
      </div>
      <div v-else-if="selectedNode.type === 'has_entity_tag'">
        <div class="panel-header">
          <h3 class="panel-title">Has Entity Tag (Condition)</h3>
        </div>
        <div class="form-group">
          <label class="field-label">Target Entity</label>
          <select v-model="selectedNode.target_entity_id" class="form-select" @change="emitGraphUpdate()">
            <option value="world">World (Global)</option>
            <option v-for="e in studioState.entityTemplates.filter(item => item.id !== 'world')" :key="e.id" :value="e.id">
              {{ e.name }} ({{ e.id }}) {{ traitLabels(e, studioState.traitTemplates).join(" · ") }}
            </option>
          </select>
        </div>
        <div class="form-group">
          <label class="field-label">Tag Query</label>
          <TagEditor v-model="selectedNode.tag" :options="targetGameplayTagOptions" :allow-create="false" :multiple="false" label="Tag query" placeholder="Choose a registered gameplay tag" @update:model-value="emitGraphUpdate()" />
          <small class="field-sublabel" style="display:block; margin-top:4px;">Canonical tag: <strong>{{ selectedNode.tag || '(none)' }}</strong></small>
        </div>
        <div class="form-group">
          <label class="field-label">Match Mode</label>
          <select v-model="selectedNode.match_mode" class="form-select" @change="emitGraphUpdate()">
            <option value="all">All Tags (AND)</option>
            <option value="any">Any Tag (OR)</option>
          </select>
        </div>
        <div class="form-hint" style="padding:10px; background:rgba(0,0,0,0.2); border-radius:6px; margin-top:10px;">
          Emits one boolean value for a connected Condition Node or boolean combinator.
        </div>
      </div>


      <!-- END NODE -->
      <div v-else-if="selectedNode.type === 'end'">
        <div v-if="props.graphMode === 'preconditions'" class="form-group">
          <label class="field-label">Candidate result</label>
          <select
            :value="selectedNode.precondition_result === false ? 'false' : 'true'"
            class="form-select"
            @change="selectedNode.precondition_result = ($event.target as HTMLSelectElement).value === 'true'; emitGraphUpdate(selectedNode)"
          >
            <option value="true">Valid candidate</option>
            <option value="false">Reject candidate</option>
          </select>
        </div>
        <template v-else>
          <div class="form-group">
            <label class="field-label">Terminal Climax</label>
            <input v-model="selectedNode.is_terminal" type="checkbox" @change="emitGraphUpdate(selectedNode)" />
            <span style="font-size:12px; margin-left:8px;">Marks scene resolution / finale</span>
          </div>
          <div class="form-group" style="background:rgba(0,0,0,0.25); padding:10px; border-radius:8px;">
            <label class="field-label" style="color:var(--accent-purple);">Jump To Storylet</label>
            <select v-model="selectedNode.jump_to_storylet" class="form-select" @change="handleJumpTargetChange(selectedNode)">
              <option :value="undefined">-- None (End Scene) --</option>
              <option v-for="s in props.storylets" :key="s.id" :value="s.id">{{ s.title }} ({{ s.id }})</option>
            </select>
          </div>
        </template>
      </div>
    </div>

    <!-- Storylet settings (when clicked on empty canvas) -->
    <div v-else-if="props.graphMode === 'preconditions'" class="inspector-content">
      <div class="panel-header"><h3 class="panel-title">Precondition Graph</h3></div>
      <p class="field-help">Has Entity Tag, Get Entity Property, And, and Or emit typed boolean values through data pins. Connect the result to a Condition Node to choose the True or False flow. Entity tags use dotted notation.</p>
    </div>

    <!-- Storylet settings (when clicked on empty canvas) -->
    <div v-else class="inspector-content">
      <div class="panel-header">
        <h3 class="panel-title">Storylet Settings</h3>
        <span class="badge" style="font-size:10px;">{{ storylet.id }}</span>
      </div>

      <div class="form-group">
        <label class="field-label">Title</label>
        <input v-model="storylet.title" class="form-input" @input="$emit('update-storylet')" />
      </div>

      <div class="form-group">
        <label class="field-label">Thumbnail</label>
        <ThumbnailPicker
          :model-value="storylet.thumbnail ?? null"
          :label="storylet.title"
          @update:model-value="storylet.thumbnail = $event; $emit('update-storylet')"
        />
      </div>
      <div class="form-group">
        <label class="field-label">Location Tags</label>
        <TagEditor v-model="locationTags" :options="locationTagOptions" :allow-create="false" label="Storylet location tags" placeholder="Choose location tags" />
        <small class="field-sublabel">Eligible at any location sharing one of these tags; leave empty for any location.</small>
      </div>

      <h3 class="field-label">Who &amp; when</h3>
      <fieldset class="form-group played-by">
        <legend class="field-label">Played by</legend>
        <p v-if="!playerOptions.length" class="field-sublabel">No participants yet. Add the Participant trait to a character.</p>
        <label v-for="option in playerOptions" :key="option.id" class="played-by__option">
          <input type="checkbox" :checked="(storylet.played_by ?? []).includes(option.id)" @change="setPlayedBy(option.id, ($event.target as HTMLInputElement).checked)" />
          {{ option.name }}
        </label>
        <small class="field-sublabel">Leave empty for any human. A rule participant plays its storylets on its own at the start of each slot.</small>
      </fieldset>

      <div style="display:grid; grid-template-columns:repeat(2, minmax(0, 1fr)); gap:8px;">
        <div class="form-group">
          <label class="field-label">Cluster Type</label>
          <select v-model="storylet.cluster_type" class="form-select" @change="$emit('update-storylet')">
            <option value="player_passage">Player Passage</option>
            <option value="linear_progression">Linear Progression</option>
            <option value="wheel_deck">Wheel Deck</option>
            <option value="aftermath_accelerator">Aftermath Accelerator</option>
          </select>
        </div>

        <div class="form-group">
          <label class="field-label">Repeatability</label>
          <select v-model="storylet.repeatability" class="form-select" @change="$emit('update-storylet')">
            <option value="always">Always</option>
            <option value="once_ever">Once Ever</option>
            <option value="cooldown">Cooldown</option>
            <option value="persistent">Persistent</option>
          </select>
        </div>
      </div>


      <div class="form-group">
        <label class="played-by__option"><input v-model="storylet.is_fallback" type="checkbox" @change="$emit('update-storylet')" /> Fallback storylet</label>
        <small class="field-sublabel">Mark filler so coverage can measure places with no progress storylets.</small>
      </div>
      <fieldset class="form-group played-by">
        <legend class="field-label">Time window</legend>
        <p v-if="!slotLabels.length" class="field-sublabel">Add a Clock to World to choose time slots.</p>
        <label v-for="(label, slot) in slotLabels" :key="slot" class="played-by__option">
          <input type="checkbox" :checked="(storylet.window ?? []).includes(slot)" @change="setWindow(slot, ($event.target as HTMLInputElement).checked)" /> {{ label }}
        </label>
        <small class="field-sublabel">Leave all slots unchecked to allow any time.</small>
      </fieldset>
      <div class="form-group">
        <label class="field-label" for="default-learn-visibility">Default Learn visibility</label>
        <select id="default-learn-visibility" v-model="storylet.default_learn_visibility" class="form-select" @change="$emit('update-storylet')">
          <option value="private">Private</option><option value="co_located">Co-located witnesses</option>
        </select>
      </div>
      <!-- ACTOR ROLES -->
      <div class="form-group">
        <label class="field-label">Character Roles Required</label>
        <TagEditor v-model="requiredRoleNames" :options="roleTagOptions" :allow-create="false" label="Required character roles" placeholder="Choose roles" />
      </div>
    </div>
  </aside>
</template>

<style scoped>
.precondition-help { margin: 0; color: var(--text-muted); font-size: 10px; line-height: 1.4; }
.property-inspector-panel {
  width: 340px;
  flex: none;
  background: rgba(255, 255, 255, 0.015);
  border-left: 1px solid var(--border-faint);
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  user-select: text;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  margin-bottom: 14px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border-subtle);
}

.panel-title {
  font-size: 14px;
  font-weight: 600;
  letter-spacing: -0.01em;
  margin: 0;
  color: var(--text-main);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* Untyped (plugin and logic) nodes get a neutral chip; typed ones keep their pin colour with dark ink. */
.type-badge {
  flex: none;
  max-width: 14rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--bg-badge);
  color: var(--text-main);
  box-shadow: inset 0 0 0 1px var(--border-subtle);
}

.type-badge.start { background: var(--status-success); color: #04160f; }
.type-badge.dialog { background: var(--secondary); color: #0b0e2a; }
.type-badge.choice { background: var(--tertiary); color: #1f1300; }
.type-badge.condition { background: var(--status-warning); color: #1f1300; }
.type-badge.image { background: #f472b6; color: #2a0716; }
.type-badge.teleport { background: var(--accent); color: var(--accent-ink); }
.type-badge.end { background: var(--status-danger); color: #1a0308; }
.type-badge.graph_transform { background: #60a5fa; color: #04122b; }
.type-badge.passion_mutation { background: #f472b6; color: #2a0716; }

.field-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  margin-bottom: 6px;
  display: block;
}

.field-sublabel {
  font-size: 11px;
  font-weight: 500;
  color: var(--text-dim);
  margin-bottom: 4px;
  display: block;
}

.item-kind-label { display: inline-flex; align-items: center; gap: 5px; font-size: 10px; font-weight: 700; letter-spacing: 0.06em; text-transform: uppercase; }
.item-kind-label--line { color: var(--secondary); }
.item-kind-label--idle { color: var(--tertiary); }

.dialogue-section-card {
  background: rgba(255, 255, 255, 0.025);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  margin-bottom: 10px;
  overflow: hidden;
}

.section-card-header {
  padding: 6px 8px;
  background: var(--bg-card-hover, #1e2024);
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid var(--border-subtle);
}

.section-title-input {
  background: transparent;
  border: none;
  color: var(--text-main);
  font-size: 11px;
  font-weight: 700;
  outline: none;
  flex: 1;
  min-width: 0;
  text-overflow: ellipsis;
}

.section-items {
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}


.field-help { color: var(--text-muted); font-size: 12px; }
.field-row-heading { display: flex; justify-content: space-between; align-items: center; margin: 8px 0; }
.choice-editor-row { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) auto; gap: 5px; margin-bottom: 6px; }
.dialogue-item-card {
  background: var(--bg-input, #141720);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  padding: 8px;
}

.form-select.micro,
.form-input.micro {
  min-height: 28px;
  height: 28px;
  padding: 3px 8px;
  font-size: 11px;
  border-radius: var(--radius-sm);
}
.form-select.micro { padding-right: 24px; background-position: right 7px center; }

.btn-micro {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 4px;
  min-height: 24px;
  min-width: 24px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid var(--border-subtle);
  color: var(--text-main);
  font-size: 11px;
  font-weight: 600;
  padding: 2px 7px;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease, border-color 0.12s ease;
}

.btn-micro:hover {
  background: rgba(255, 255, 255, 0.09);
  border-color: var(--border-muted);
}

.btn-micro.danger {
  color: var(--text-dim);
  background: transparent;
  border-color: transparent;
  padding: 2px;
}

.btn-micro.danger:hover {
  color: var(--status-danger);
  background: var(--status-danger-subtle);
}

.audio-browse-label {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--accent-hover);
  cursor: pointer;
  font-weight: 600;
}

.mutation-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 6px;
  background: rgba(0, 0, 0, 0.2);
  border-radius: 4px;
  margin-bottom: 4px;
}

.graph-pattern-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
  padding: 6px;
  background: rgba(0, 0, 0, 0.2);
  border-radius: 4px;
  margin-bottom: 6px;
}

.attribute-row,
.binding-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 4px;
}
.played-by { border: 1px solid var(--border-subtle); border-radius: var(--radius-md); padding: 8px 12px; margin: 0 0 12px; }
.played-by__option { display: flex; align-items: center; gap: 8px; min-height: 32px; font-size: 13px; }
</style>
