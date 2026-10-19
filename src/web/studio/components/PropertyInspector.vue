<script setup lang="ts">
import { ref, computed } from "vue";
import type { NarrativeNode, NarrativeGraph, DialogueSection, DialogueItem, LocalVariableDefinition, GraphLayer, AttributeOp, AttributeValueType, AttributeValue, GraphBinding, VertexPattern } from "../narrativeGraphTypes.ts";
import { applyJumpPreconditions, isGraphTransformNode, isPassionMutationNode } from "../narrativeGraphTypes.ts";
import type { ChronoQualityPredicate, MetCharacterPrecondition, QualityCondition, StoryletRecord, VisitedLocationPrecondition } from "../contracts.ts";
import TagEditor from "./TagEditor.vue";
import { normalizeTagName, uniqueTagNames, buildTagIndex } from "../tagData.ts";
import { studioState, notify } from "../state.ts";

const props = defineProps<{
  storylet: StoryletRecord;
  graph: NarrativeGraph;
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

// Available roles in storylet + default Narrator and Player
const availableRoles = computed(() => {
  const roles = props.storylet.parameters.map((p) => p.param_name);
  if (!roles.includes("Narrator")) roles.unshift("Narrator");
  return roles;
});

const tagIndex = computed(() => buildTagIndex({
  characters: studioState.characters,
  locations: studioState.locations,
  storylets: [...props.storylets.filter((storylet) => storylet.id !== props.storylet.id), { ...props.storylet, graph: props.graph }],
}));
const roleTagOptions = computed(() => uniqueTagNames([
  "Narrator",
  ...studioState.characters.map((character) => character.role),
  ...props.storylet.parameters.map((parameter) => parameter.param_name),
  ...tagIndex.value.filter((tag) => tag.kind === "role").map((tag) => tag.name),
]));
const targetRoles = computed(() => uniqueTagNames(["player", ...roleTagOptions.value]));
const emotionTagOptions = computed(() => tagIndex.value.filter((tag) => tag.kind === "emotion").map((tag) => tag.name));
const actionTagOptions = computed(() => tagIndex.value.filter((tag) => tag.kind === "action").map((tag) => tag.name));
const locationTagOptions = computed(() => uniqueTagNames([
  ...studioState.locations.flatMap((location) => location.tags ?? []),
  ...tagIndex.value.filter((tag) => tag.kind === "location").map((tag) => tag.name),
]));
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
const chronoUrgencyColorTag = computed({
  get: () => props.storylet.chrono?.urgency_color_tag ?? "Standard",
  set: (value: string) => {
    ensureChrono().urgency_color_tag = normalizeTagName(value);
    emit("update-storylet");
  },
});
function ensureChrono() {
  if (!props.storylet.chrono) props.storylet.chrono = {
    display_priority: 0,
    urgency_color_tag: "Standard",
    is_repeatable: props.storylet.repeatability !== "once_ever",
    cooldown_ms: 0,
    preconditions: [],
    required_tags: [],
    required_any_tags: [],
    forbidden_tags: [],
    required_arc_phases: [],
    ideal_state_vector: {},
    ideal_state_weights: {},
    salience_bonus_tags: [],
    effects_on_complete: [],
  };
  return props.storylet.chrono;
}
const chronoArcPhases = computed({
  get: () => uniqueTagNames(props.storylet.chrono?.required_arc_phases ?? []),
  set: (values: string[]) => { ensureChrono().required_arc_phases = uniqueTagNames(values); emit("update-storylet"); },
});
const chronoRequiredTags = computed({
  get: () => uniqueTagNames(props.storylet.chrono?.required_tags ?? []),
  set: (values: string[]) => { ensureChrono().required_tags = uniqueTagNames(values); emit("update-storylet"); },
});
const chronoRequiredAnyTags = computed({
  get: () => uniqueTagNames(props.storylet.chrono?.required_any_tags ?? []),
  set: (values: string[]) => { ensureChrono().required_any_tags = uniqueTagNames(values); emit("update-storylet"); },
});
const chronoForbiddenTags = computed({
  get: () => uniqueTagNames(props.storylet.chrono?.forbidden_tags ?? []),
  set: (values: string[]) => { ensureChrono().forbidden_tags = uniqueTagNames(values); emit("update-storylet"); },
});
type ChronoJsonField = "ideal_state_vector" | "ideal_state_weights" | "effects_on_complete";
function chronoJson(field: ChronoJsonField) {
  const emptyValue = field === "effects_on_complete" ? [] : {};
  return JSON.stringify(props.storylet.chrono?.[field] ?? emptyValue, null, 2);
}
function updateChronoJson(field: ChronoJsonField, event: Event) {
  const target = event.target as HTMLTextAreaElement;
  try {
    const value: unknown = JSON.parse(target.value);
    if (field === "effects_on_complete" ? !Array.isArray(value) : !value || Array.isArray(value) || typeof value !== "object") {
      throw new Error(field === "effects_on_complete" ? "Expected a JSON array" : "Expected a JSON object");
    }
    (ensureChrono() as unknown as Record<string, unknown>)[field] = value;
    target.setCustomValidity("");
    emit("update-storylet");
  } catch (error) {
    target.setCustomValidity(error instanceof Error ? error.message : "Invalid JSON");
    target.reportValidity();
  }
}

function eventValue(event: Event): string {
  return (event.currentTarget as HTMLInputElement).value;
}
function addChronoPredicate() {
  const target = globalQualities.value[0] ?? locationTagOptions.value[0] ?? "";
  if (!target) {
    notify("Add a quality or location tag before defining a ChronoFlow predicate", "info");
    return;
  }
  ensureChrono().preconditions.push({
    target_id: target,
    subject: globalQualities.value.includes(target) ? "metric" : "location",
    operator: "gte",
    comparison_value: 1,
    specificity_weight: 1,
  });
  emit("update-storylet");
}
function setChronoPredicate(
  index: number,
  field: keyof ChronoQualityPredicate,
  value: string | number,
) {
  if (typeof value === "number" && !Number.isFinite(value)) return;
  const predicates = [...ensureChrono().preconditions];
  const predicate = predicates[index];
  if (!predicate) return;
  const updated = { ...predicate, [field]: value } as ChronoQualityPredicate;
  if (field === "subject" && updated.subject !== "metric") {
    updated.operator = "eq";
    updated.comparison_value = 1;
  }
  predicates[index] = updated;
  ensureChrono().preconditions = predicates;
  emit("update-storylet");
}
function removeChronoPredicate(index: number) {
  ensureChrono().preconditions = ensureChrono().preconditions.filter((_, current) => current !== index);
  emit("update-storylet");
}
function updateChronoNumber(field: "display_priority" | "cooldown_ms", event: Event) {
  const input = event.target as HTMLInputElement;
  const value = Number(input.value);
  const validInteger = Number.isSafeInteger(value);
  const validRange = field === "display_priority"
    ? value >= -2147483648 && value <= 2147483647
    : value >= 0;
  if (!validInteger || !validRange) {
    input.setCustomValidity(field === "display_priority"
      ? "Priority must be a signed 32-bit integer"
      : "Cooldown must be a non-negative integer");
    input.reportValidity();
    return;
  }
  input.setCustomValidity("");
  ensureChrono()[field] = value;
  emit("update-storylet");
}
function updateChronoRepeatable(event: Event) {
  ensureChrono().is_repeatable = (event.target as HTMLInputElement).checked;
  emit("update-storylet");
}
function initializeChronoOnOpen(event: Event) {
  if ((event.currentTarget as HTMLDetailsElement).open && !props.storylet.chrono) {
    ensureChrono();
    emit("update-storylet");
  }
}
const chronoSalienceTags = computed({
  get: () => uniqueTagNames(props.storylet.chrono?.salience_bonus_tags ?? []),
  set: (values: string[]) => { ensureChrono().salience_bonus_tags = uniqueTagNames(values); emit("update-storylet"); },
});
function updatePreconditions(values: Partial<StoryletRecord["preconditions"]>) {
  props.storylet.preconditions = { ...props.storylet.preconditions, ...values };
  emit("update-storylet");
}
const visitedLocationTags = computed({
  get: () => uniqueTagNames((props.storylet.preconditions.visited_locations_any_of ?? []).flatMap((r) => r.kind === "location_tag" ? [r.tag] : [])),
  set: (values: string[]) => updatePreconditions({ visited_locations_any_of: [
    ...(props.storylet.preconditions.visited_locations_any_of ?? []).filter((r) => r.kind === "location_id"),
    ...uniqueTagNames(values).map((tag): VisitedLocationPrecondition => ({ kind: "location_tag", tag })),
  ] }),
});
const visitedLocationIds = computed({
  get: () => (props.storylet.preconditions.visited_locations_any_of ?? []).flatMap((r) => r.kind === "location_id" ? [r.location_id] : []),
  set: (values: string[]) => updatePreconditions({ visited_locations_any_of: [
    ...(props.storylet.preconditions.visited_locations_any_of ?? []).filter((r) => r.kind === "location_tag"),
    ...[...new Set(values)].map((location_id): VisitedLocationPrecondition => ({ kind: "location_id", location_id })),
  ] }),
});
const metCharacterRoleTags = computed({
  get: () => uniqueTagNames((props.storylet.preconditions.met_characters_any_of ?? []).flatMap((r) => r.kind === "role_tag" ? [r.role] : [])),
  set: (values: string[]) => updatePreconditions({ met_characters_any_of: [
    ...(props.storylet.preconditions.met_characters_any_of ?? []).filter((r) => r.kind === "character_id"),
    ...uniqueTagNames(values).map((role): MetCharacterPrecondition => ({ kind: "role_tag", role })),
  ] }),
});
const metCharacterIds = computed({
  get: () => (props.storylet.preconditions.met_characters_any_of ?? []).flatMap((r) => r.kind === "character_id" ? [r.character_id] : []),
  set: (values: string[]) => updatePreconditions({ met_characters_any_of: [
    ...(props.storylet.preconditions.met_characters_any_of ?? []).filter((r) => r.kind === "role_tag"),
    ...[...new Set(values)].map((character_id): MetCharacterPrecondition => ({ kind: "character_id", character_id })),
  ] }),
});
const locationOptions = computed(() => [...studioState.locations].sort((a, b) => a.name.localeCompare(b.name)));
const characterOptions = computed(() => [...studioState.characters].sort((a, b) => a.name.localeCompare(b.name)));
const qualityConditionGroups = [{ key: "all_of", label: "All of" }, { key: "any_of", label: "Any of" }] as const;
type QualityConditionGroup = typeof qualityConditionGroups[number]["key"];
function updateQualityConditions(group: QualityConditionGroup, conditions: QualityCondition[]) { updatePreconditions({ [group]: conditions }); }
function addQualityCondition(group: QualityConditionGroup) {
  const quality = globalQualities.value[0];
  if (!quality) { notify("Add a project quality before defining a quality precondition", "info"); return; }
  updateQualityConditions(group, [...(props.storylet.preconditions[group] ?? []), { variable: quality, op: "gte", value: 1 }]);
}
function removeQualityCondition(group: QualityConditionGroup, index: number) {
  updateQualityConditions(group, (props.storylet.preconditions[group] ?? []).filter((_, current) => current !== index));
}
function setQualityCondition(group: QualityConditionGroup, index: number, field: keyof QualityCondition, value: unknown) {
  const conditions = [...(props.storylet.preconditions[group] ?? [])]; const condition = conditions[index];
  if (!condition) return; conditions[index] = { ...condition, [field]: value }; updateQualityConditions(group, conditions);
}

// All project qualities + local variables
const globalQualities = computed(() => studioState.qualities.map((q) => q.name));
const localVariables = computed(() => props.graph.local_variables || []);


// Local variable creation
const newVarName = ref("");
const newVarType = ref<"boolean" | "number" | "string">("number");
const newVarDefault = ref<any>(0);

function addLocalVariable() {
  if (!newVarName.value.trim()) return;
  if (!props.graph.local_variables) props.graph.local_variables = [];

  let val: any = newVarDefault.value;
  if (newVarType.value === "number") val = Number(val) || 0;
  else if (newVarType.value === "boolean") val = Boolean(val);
  else val = String(val);

  props.graph.local_variables.push({
    name: newVarName.value.trim(),
    type: newVarType.value,
    default_value: val,
  });

  newVarName.value = "";
  emitGraphUpdate();
  notify("Added local variable", "success");
}

function removeLocalVariable(idx: number) {
  props.graph.local_variables?.splice(idx, 1);
  emitGraphUpdate();
}

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
    speaker_role: availableRoles.value[0] || "Narrator",
    target_role: "player",
    content: "",
    emotion_tag: "neutral",
  });
  emitGraphUpdate();
}

function addIdleAction(sec: DialogueSection) {
  const actId = `act_${Date.now().toString(36)}`;
  sec.items.push({
    kind: "idle_action",
    id: actId,
    action_type: "lookat",
    actor_role: availableRoles.value[0] || "Narrator",
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
    const res = await fetch(`/api/v2/audio/inspect?path=${encodeURIComponent(item.audio_path)}`);
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

// Variable Node Assignment
function addAssignment(node: NarrativeNode) {
  if (!node.assignments) node.assignments = [];
  node.assignments.push({
    scope: "global",
    quality: globalQualities.value[0] || "tension",
    op: "set",
    value: 1,
  });
  emitGraphUpdate();
}

function removeAssignment(node: NarrativeNode, idx: number) {
  node.assignments?.splice(idx, 1);
  emitGraphUpdate();
}

// Conditional Node Rule
function addConditionRule(node: NarrativeNode) {
  if (!node.conditions) node.conditions = [];
  node.conditions.push({
    scope: "global",
    variable: globalQualities.value[0] || "tension",
    op: "gte",
    value: 1,
  });
  emitGraphUpdate();
}

function removeConditionRule(node: NarrativeNode, idx: number) {
  node.conditions?.splice(idx, 1);
  emitGraphUpdate();
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

// End Node Jump Precondition Auto-Sync
function autoSetJumpPreconditions(node: NarrativeNode) {
  if (!node.jump_to_storylet) return;
  const target = props.storylets.find((s) => s.id === node.jump_to_storylet);
  if (!target) {
    notify(`Target storylet '${node.jump_to_storylet}' not found`, "error");
    return;
  }

  const updated = applyJumpPreconditions(target, props.storylet);
  emit("update-storylet");
  if (updated.length > 0) {
    notify(`Applied preconditions to effects: ${updated.join(", ")}`, "success");
  } else {
    notify("Target storylet has no preconditions to set", "info");
  }
}
function handleJumpTargetChange(node: NarrativeNode) {
  emitGraphUpdate(node);
  if (node.jump_to_storylet) {
    autoSetJumpPreconditions(node);
  } else {
    emit("update-storylet");
  }
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
        <div style="display:flex; align-items:center; gap:8px;">
          <span :class="['type-badge', selectedNode.type]">{{ selectedNode.type.toUpperCase() }}</span>
          <h3 class="panel-title">{{ selectedNode.title }}</h3>
        </div>
        <button class="btn btn-danger" style="font-size:11px; padding:3px 8px;" @click="$emit('delete-node', selectedNode.id)">
          Delete Node
        </button>
      </div>

      <div class="form-group">
        <label class="field-label">Node Title</label>
        <input v-model="selectedNode.title" class="form-input" @input="emitGraphUpdate(selectedNode)" />
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
            <input v-model="sec.title" class="section-title-input" @input="emitGraphUpdate(selectedNode)" />
            <div style="display:flex; gap:4px;">
              <button class="btn-micro" @click="addDialogueLine(sec)">+ Line</button>
              <button class="btn-micro" @click="addIdleAction(sec)">+ Idle Action</button>
              <button class="btn-micro danger" @click="removeSection(selectedNode, sIdx)">✕</button>
            </div>
          </div>

          <div class="section-items">
            <div v-for="(item, iIdx) in sec.items" :key="item.id" class="dialogue-item-card" :data-tag-item-id="item.id">
              <!-- LINE ITEM -->
              <div v-if="item.kind === 'line'">
                <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px;">
                  <span style="font-size:10px; font-weight:700; color:var(--accent-purple);">💬 DIALOGUE</span>
                  <button class="btn-micro danger" @click="removeItem(sec, iIdx)">✕</button>
                </div>

                <div style="display:grid; grid-template-columns:1fr 1fr; gap:6px; margin-bottom:6px;">
                  <div>
                    <label class="field-sublabel">Speaker Role</label>
                    <TagEditor v-model="item.speaker_role" :options="roleTagOptions" :multiple="false" label="Speaker role" placeholder="Choose or create role" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                  <div>
                    <label class="field-sublabel">Addressed To</label>
                    <TagEditor v-model="item.target_role" :options="targetRoles" :multiple="false" label="Addressed to role" placeholder="Choose role" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>

                <div style="margin-bottom:6px;">
                  <label class="field-sublabel">Dialogue Text</label>
                  <textarea v-model="item.content" class="form-textarea" style="min-height:48px; font-size:11px; padding:4px;" @input="emitGraphUpdate(selectedNode)"></textarea>
                </div>

                <div style="display:grid; grid-template-columns:1fr 1fr; gap:6px; margin-bottom:6px;">
                  <div>
                    <label class="field-sublabel">Emotion Tag</label>
                    <TagEditor v-model="item.emotion_tag" :options="emotionTagOptions" :multiple="false" label="Emotion tag" placeholder="Choose or create emotion" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                  <div>
                    <label class="field-sublabel">Action Tag (Tied)</label>
                    <TagEditor v-model="item.action_tag" :options="actionTagOptions" :multiple="false" label="Action tag" placeholder="Choose or create action" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>

                <div v-if="item.action_tag" style="margin-bottom:6px;">
                  <label class="field-sublabel">Action Target</label>
                  <TagEditor v-model="item.action_target" :options="targetRoles" :multiple="false" label="Action target" placeholder="Choose role" @update:model-value="emitGraphUpdate(selectedNode)" />
                </div>

                <!-- Audio Link & Duration -->
                <div style="background:rgba(0,0,0,0.25); padding:6px; border-radius:6px;">
                  <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px;">
                    <label class="field-sublabel" style="margin-bottom:0;">Audio Path & Duration</label>
                    <label class="audio-browse-label">
                      📁 Browse
                      <input type="file" accept="audio/*" style="display:none;" @change="handleAudioBrowse($event, item)" />
                    </label>
                  </div>
                  <div style="display:flex; gap:4px;">
                    <input v-model="item.audio_path" class="form-input micro" style="flex:1;" placeholder="audio/line_01.mp3" @input="emitGraphUpdate(selectedNode)" />
                    <button class="btn-micro" title="Check server file duration" @click="checkAudioBackend(item)">🔍</button>
                    <input v-model.number="item.duration" type="number" step="0.1" class="form-input micro" style="width:60px;" placeholder="Sec" @input="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>
              </div>

              <!-- IDLE ACTION ITEM -->
              <div v-else-if="item.kind === 'idle_action'">
                <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:4px;">
                  <span style="font-size:10px; font-weight:700; color:#ff9f0a;">⚙️ IDLE ACTION</span>
                  <button class="btn-micro danger" @click="removeItem(sec, iIdx)">✕</button>
                </div>

                <div style="display:grid; grid-template-columns:1fr 1fr; gap:6px; margin-bottom:4px;">
                  <div>
                    <label class="field-sublabel">Action Type</label>
                    <select v-model="item.action_type" class="form-select micro" @change="emitGraphUpdate(selectedNode)">
                      <option value="lookat">Look At</option>
                      <option value="emote">Emote</option>
                    </select>
                  </div>
                  <div>
                    <label class="field-sublabel">Actor Role</label>
                    <TagEditor v-model="item.actor_role" :options="roleTagOptions" :multiple="false" label="Actor role" placeholder="Choose or create role" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                </div>

                <div style="display:grid; grid-template-columns:1fr 1fr; gap:6px;">
                  <div>
                    <label class="field-sublabel">Target</label>
                    <TagEditor v-model="item.target" :options="targetRoles" :multiple="false" label="Target role" placeholder="Choose role" @update:model-value="emitGraphUpdate(selectedNode)" />
                  </div>
                  <div v-if="item.action_type === 'emote'">
                    <label class="field-sublabel">Emote Tag</label>
                    <TagEditor v-model="item.emote" :options="actionTagOptions" :multiple="false" label="Emote action tag" placeholder="Choose or create action" @update:model-value="emitGraphUpdate(selectedNode)" />
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
          <input v-model="choice.text" class="form-input micro" placeholder="Choice text" @input="emitGraphUpdate(selectedNode)" />
          <input v-model="choice.system_verb" class="form-input micro" placeholder="System verb (optional)" @input="emitGraphUpdate(selectedNode)" />
          <button class="btn-micro danger" @click="removeChoice(selectedNode, index)">✕</button>
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

      <!-- VARIABLE NODE -->
      <div v-else-if="selectedNode.type === 'variable'">
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
          <label class="field-label" style="margin-bottom:0;">Quality & Var Mutations</label>
          <button class="btn btn-secondary" style="font-size:10px; padding:2px 6px;" @click="addAssignment(selectedNode)">
            + Add Mutation
          </button>
        </div>

        <div v-for="(a, idx) in selectedNode.assignments" :key="idx" class="mutation-row">
          <select v-model="a.scope" class="form-select micro" style="width:75px;" @change="a.quality = a.scope === 'local' ? (localVariables[0]?.name || '') : (globalQualities[0] || 'tension'); emitGraphUpdate(selectedNode)">
            <option value="global">Global</option>
            <option value="local">Local</option>
          </select>

          <!-- Variable selection based on scope -->
          <select v-if="a.scope === 'global'" v-model="a.quality" class="form-select micro" style="flex:1;" @change="emitGraphUpdate(selectedNode)">
            <option v-for="q in globalQualities" :key="q" :value="q">{{ q }}</option>
          </select>
          <select v-else v-model="a.quality" class="form-select micro" style="flex:1;" @change="emitGraphUpdate(selectedNode)">
            <option v-for="v in localVariables" :key="v.name" :value="v.name">{{ v.name }}</option>
          </select>

          <select v-model="a.op" class="form-select micro" style="width:55px;" @change="emitGraphUpdate(selectedNode)">
            <option value="set">=</option>
            <option value="add">+</option>
          </select>

          <input v-model="a.value" class="form-input micro" style="width:65px;" @input="emitGraphUpdate(selectedNode)" />
          <button class="btn-micro danger" @click="removeAssignment(selectedNode, idx)">✕</button>
        </div>
      </div>

      <!-- CONDITIONAL NODE -->
      <div v-else-if="selectedNode.type === 'conditional'">
        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;">
          <label class="field-label" style="margin-bottom:0;">Condition Rules</label>
          <button class="btn btn-secondary" style="font-size:10px; padding:2px 6px;" @click="addConditionRule(selectedNode)">
            + Add Rule
          </button>
        </div>

        <div v-for="(c, idx) in selectedNode.conditions" :key="idx" class="mutation-row">
          <select v-model="c.scope" class="form-select micro" style="width:75px;" @change="c.variable = c.scope === 'local' ? (localVariables[0]?.name || '') : (globalQualities[0] || 'tension'); emitGraphUpdate(selectedNode)">
            <option value="global">Global</option>
            <option value="local">Local</option>
          </select>

          <select v-if="c.scope === 'global'" v-model="c.variable" class="form-select micro" style="flex:1;" @change="emitGraphUpdate(selectedNode)">
            <option v-for="q in globalQualities" :key="q" :value="q">{{ q }}</option>
          </select>
          <select v-else v-model="c.variable" class="form-select micro" style="flex:1;" @change="emitGraphUpdate(selectedNode)">
            <option v-for="v in localVariables" :key="v.name" :value="v.name">{{ v.name }}</option>
          </select>

          <select v-model="c.op" class="form-select micro" style="width:55px;" @change="emitGraphUpdate(selectedNode)">
            <option value="eq">==</option>
            <option value="neq">!=</option>
            <option value="gte">&gt;=</option>
            <option value="lte">&lt;=</option>
            <option value="gt">&gt;</option>
            <option value="lt">&lt;</option>
          </select>

          <input v-model="c.value" class="form-input micro" style="width:65px;" @input="emitGraphUpdate(selectedNode)" />
          <button class="btn-micro danger" @click="removeConditionRule(selectedNode, idx)">✕</button>
        </div>
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
          <button class="btn-micro danger" @click="removeEntry(selectedNode, selectedNode.rule.lhs.vertices, index)">✕</button>
          <div v-for="(predicate, predicateIndex) in vertex.attributes" :key="predicateIndex" class="attribute-row">
            <input v-model="predicate.key" class="form-input micro" style="width:110px;" placeholder="attribute key" @input="emitGraphUpdate(selectedNode)" />
            <select v-model="predicate.op" class="form-select micro" style="width:55px;" @change="emitGraphUpdate(selectedNode)">
              <option v-for="op in attributeOps" :key="op" :value="op">{{ op }}</option>
            </select>
            <select class="form-select micro" style="width:65px;" :value="predicate.value.type" @change="setAttributeValue(selectedNode, (value) => { predicate.value = value; }, eventValue($event), String(predicate.value.value))">
              <option v-for="valueType in attributeValueTypes" :key="valueType" :value="valueType">{{ valueType }}</option>
            </select>
            <input class="form-input micro" style="width:80px;" :value="String(predicate.value.value)" @input="setAttributeValue(selectedNode, (value) => { predicate.value = value; }, predicate.value.type, eventValue($event))" />
            <button class="btn-micro danger" @click="removeEntry(selectedNode, vertex.attributes, predicateIndex)">✕</button>
          </div>
        </div>

        <div class="field-row-heading">
          <label class="field-sublabel">Edge Patterns</label>
          <button class="btn-micro" @click="addGraphPatternEntry(selectedNode, 'edges')">+ Add Edge</button>
        </div>
        <div v-for="(pattern, index) in selectedNode.rule.lhs.edges" :key="index" class="graph-pattern-row">
          <input v-model="pattern.edge_id" class="form-input micro" style="width:110px;" placeholder="edge id (blank matches any)" @input="emitGraphUpdate(selectedNode)" />
          <input v-model="pattern.relation_type" class="form-input micro" style="width:110px;" placeholder="relation type" @input="emitGraphUpdate(selectedNode)" />
          <button class="btn-micro danger" @click="removeEntry(selectedNode, selectedNode.rule.lhs.edges, index)">✕</button>
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
            <button class="btn-micro danger" @click="removeEntry(selectedNode, selectedNode.rule.rhs.add_vertices, index)">✕</button>
          </div>
          <div v-for="(value, key) in template.attributes" :key="key" class="attribute-row">
            <input :value="key" class="form-input micro" style="width:110px;" placeholder="attribute key" @change="renameRecordAttribute(selectedNode, template.attributes, key, eventValue($event))" />
            <select class="form-select micro" style="width:65px;" :value="value.type" @change="setAttributeValue(selectedNode, (next) => { template.attributes[key] = next; }, eventValue($event), String(value.value))">
              <option v-for="valueType in attributeValueTypes" :key="valueType" :value="valueType">{{ valueType }}</option>
            </select>
            <input class="form-input micro" style="width:80px;" :value="String(value.value)" @input="setAttributeValue(selectedNode, (next) => { template.attributes[key] = next; }, value.type, eventValue($event))" />
            <button class="btn-micro danger" @click="removeRecordAttribute(selectedNode, template.attributes, key)">✕</button>
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
          <button class="btn-micro danger" @click="removeEntry(selectedNode, selectedNode.rule.rhs.remove_vertices, index)">✕</button>
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
          <button class="btn-micro danger" @click="removeEntry(selectedNode, selectedNode.rule.rhs.add_edges, index)">✕</button>
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
          <button class="btn-micro danger" @click="removeEntry(selectedNode, selectedNode.rule.rhs.set_attributes, index)">✕</button>
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
          <button class="btn-micro danger" @click="removeEntry(selectedNode, selectedNode.op.micro_deltas, index)">✕</button>
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
          <button class="btn-micro danger" @click="removeEntry(selectedNode, selectedNode.op.active_ops, index)">✕</button>
        </div>

        <p class="field-help">Passion ids are u16 metric indices; deltas and active-pool flags apply through the runtime passion model.</p>
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

      <div v-else-if="selectedNode.type === 'logic_all' || selectedNode.type === 'logic_any'">
        <p class="field-help">{{ selectedNode.type === 'logic_all' ? 'Continue after all incoming branches complete.' : 'Continue after the selected incoming branch completes.' }}</p>
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

      <!-- END NODE -->
      <div v-else-if="selectedNode.type === 'end'">
        <div class="form-group">
          <label class="field-label">Terminal Climax</label>
          <input v-model="selectedNode.is_terminal" type="checkbox" @change="emitGraphUpdate(selectedNode)" />
          <span style="font-size:12px; margin-left:8px;">Marks scene resolution / finale</span>
        </div>

        <div class="form-group" style="background:rgba(0,0,0,0.25); padding:10px; border-radius:8px;">
          <label class="field-label" style="color:var(--accent-purple);">Jump To Storylet</label>
          <select v-model="selectedNode.jump_to_storylet" class="form-select" @change="handleJumpTargetChange(selectedNode)">
            <option :value="undefined">-- None (End Scene) --</option>
            <option v-for="s in props.storylets" :key="s.id" :value="s.id">
              {{ s.title }} ({{ s.id }})
            </option>
          </select>

          <div v-if="selectedNode.jump_to_storylet" style="margin-top:8px;">
            <button class="btn btn-primary" style="font-size:11px; width:100%;" @click="autoSetJumpPreconditions(selectedNode)">
              ⚡ Auto-Set Precondition Qualities
            </button>
            <p style="font-size:10px; color:var(--text-muted); margin-top:4px;">
              Chooses source effects that satisfy each target precondition. Unsupported predicates are skipped.
            </p>
          </div>
        </div>
      </div>
    </div>

    <!-- 2. STORYLET PROPERTIES (when clicked on empty canvas) -->
    <div v-else class="inspector-content">
      <div class="panel-header">
        <h3 class="panel-title">Storylet Properties</h3>
        <span class="badge" style="font-size:10px;">{{ storylet.id }}</span>
      </div>

      <div class="form-group">
        <label class="field-label">Title</label>
        <input v-model="storylet.title" class="form-input" @input="$emit('update-storylet')" />
      </div>

      <div class="form-group">
        <label class="field-label">Location Tags</label>
        <TagEditor v-model="locationTags" :options="locationTagOptions" label="Storylet location tags" placeholder="Choose or create location tags" />
        <small class="field-sublabel">Eligible at any location sharing one of these tags; leave empty for any location.</small>
      </div>

      <div style="display:grid; grid-template-columns:1fr 1fr; gap:8px;">
        <div class="form-group">
          <label class="field-label">Cluster Type</label>
          <select v-model="storylet.cluster_type" class="form-select" @change="$emit('update-storylet')">
            <option value="player_passage">Player Passage</option>
            <option value="linear_progression">Linear Progression</option>
            <option value="wheel_deck">Wheel Deck</option>
            <option value="aftermath_accelerator">Aftermath Accelerator</option>
            <option value="player_controlled_passage">Passage (legacy)</option>
            <option value="linear">Linear (legacy)</option>
            <option value="wheel">Wheel (legacy)</option>
            <option value="aftermath">Aftermath (legacy)</option>
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

      <details class="form-group chrono-settings" @toggle="initializeChronoOnOpen">
        <summary class="field-label">ChronoFlow Settings</summary>
        <p class="precondition-help">Authored QBN surfacing, salience, and timeline settings used by ChronoFlow.</p>
        <div class="chrono-fields">
          <label>Priority<input :value="storylet.chrono?.display_priority ?? 0" type="number" min="-2147483648" max="2147483647" step="1" class="form-input micro" @input="updateChronoNumber('display_priority', $event)" /></label>
          <label class="field-sublabel">Urgency color tag</label>
          <TagEditor v-model="chronoUrgencyColorTag" :options="tagIndex.map((tag) => tag.name)" :multiple="false" label="Urgency color tag" placeholder="Choose or create urgency tag" />
          <label>Cooldown (ms)<input :value="storylet.chrono?.cooldown_ms ?? 0" type="number" min="0" step="1" class="form-input micro" @input="updateChronoNumber('cooldown_ms', $event)" /></label>
          <label class="chrono-checkbox"><input :checked="storylet.chrono?.is_repeatable ?? storylet.repeatability !== 'once_ever'" type="checkbox" @change="updateChronoRepeatable" /> Repeatable</label>
        </div>
        <label class="field-sublabel">Required tags (all)</label>
        <TagEditor v-model="chronoRequiredTags" :options="tagIndex.map((tag) => tag.name)" label="Required tags" placeholder="Choose or create required tags" />
        <label class="field-sublabel">Required tags (any)</label>
        <TagEditor v-model="chronoRequiredAnyTags" :options="tagIndex.map((tag) => tag.name)" label="Required-any tags" placeholder="Choose or create required tags" />
        <label class="field-sublabel">Forbidden tags</label>
        <TagEditor v-model="chronoForbiddenTags" :options="tagIndex.map((tag) => tag.name)" label="Forbidden tags" placeholder="Choose or create forbidden tags" />
        <label class="field-sublabel">Required arc phases</label>
        <TagEditor v-model="chronoArcPhases" label="Required arc phases" placeholder="Choose or create phases" />
        <label class="field-sublabel">Salience bonus tags</label>
        <TagEditor v-model="chronoSalienceTags" label="Salience bonus tags" placeholder="Choose or create tags" />
        <details class="precondition-options">
          <summary>Advanced predicates and effects</summary>
          <div class="chrono-predicate-heading">
            <label class="field-sublabel">Additional predicates</label>
            <button type="button" class="btn-micro" @click="addChronoPredicate">+</button>
          </div>
          <div v-for="(predicate, index) in (storylet.chrono?.preconditions ?? [])" :key="index" class="chrono-predicate-row">
            <select :value="predicate.subject" class="form-select micro" :aria-label="'Predicate ' + (index + 1) + ' subject'" @change="setChronoPredicate(index, 'subject', ($event.target as HTMLSelectElement).value)">
              <option value="metric">Quality / passion</option>
              <option value="location">Current location</option>
              <option value="visited">Visited location</option>
              <option value="met_character">Met character</option>
              <option value="entity">Entity tag</option>
            </select>
            <input :value="predicate.target_id" class="form-input micro" :list="'chrono-targets-' + index" :aria-label="'Predicate ' + (index + 1) + ' target'" @change="setChronoPredicate(index, 'target_id', ($event.target as HTMLInputElement).value)" />
            <datalist :id="'chrono-targets-' + index">
              <option v-for="quality in globalQualities" :key="'q:' + quality" :value="quality" />
              <option v-for="tag in tagIndex" :key="'t:' + tag.name" :value="tag.name" />
              <option v-for="location in studioState.locations" :key="'l:' + location.id" :value="'location.' + location.id" />
              <option v-for="character in studioState.characters" :key="'c:' + character.id" :value="'character.' + character.id" />
            </datalist>
            <select :value="predicate.operator" class="form-select micro" :aria-label="'Predicate ' + (index + 1) + ' operator'" @change="setChronoPredicate(index, 'operator', ($event.target as HTMLSelectElement).value)">
              <option value="eq">=</option><option value="neq">≠</option>
              <option v-if="predicate.subject === 'metric'" value="gt">&gt;</option>
              <option v-if="predicate.subject === 'metric'" value="gte">≥</option>
              <option v-if="predicate.subject === 'metric'" value="lt">&lt;</option>
              <option v-if="predicate.subject === 'metric'" value="lte">≤</option>
            </select>
            <input :value="predicate.comparison_value" type="number" step="any" class="form-input micro" :aria-label="'Predicate ' + (index + 1) + ' comparison value'" @change="setChronoPredicate(index, 'comparison_value', Number(($event.target as HTMLInputElement).value))" />
            <input v-if="predicate.subject === 'entity'" :value="predicate.entity_index ?? 0" type="number" min="0" max="255" class="form-input micro" aria-label="Entity row index" @change="setChronoPredicate(index, 'entity_index', Number(($event.target as HTMLInputElement).value))" />
            <button type="button" class="btn-micro danger" :aria-label="'Remove predicate ' + (index + 1)" @click="removeChronoPredicate(index)">×</button>
          </div>
          <label class="field-sublabel">Ideal state vector (JSON object)</label>
          <textarea :value="chronoJson('ideal_state_vector')" class="form-textarea chrono-json" spellcheck="false" @change="updateChronoJson('ideal_state_vector', $event)"></textarea>
          <label class="field-sublabel">Ideal state weights (JSON object, default 1.0)</label>
          <textarea :value="chronoJson('ideal_state_weights')" class="form-textarea chrono-json" spellcheck="false" @change="updateChronoJson('ideal_state_weights', $event)"></textarea>
          <label class="field-sublabel">Completion effects (JSON array)</label>
          <textarea :value="chronoJson('effects_on_complete')" class="form-textarea chrono-json" spellcheck="false" @change="updateChronoJson('effects_on_complete', $event)"></textarea>
        </details>
      </details>

      <!-- LOCAL VARIABLES -->
      <div class="form-group" style="background:rgba(0,0,0,0.2); padding:10px; border-radius:8px; border:1px solid var(--border-subtle);">
        <label class="field-label" style="color:var(--accent-purple);">Local Storylet Variables</label>
        <p style="font-size:10px; color:var(--text-muted); margin-bottom:8px;">
          Variables scoped solely to this storylet. Condition and Variable nodes can check/mutate them.
        </p>

        <div v-for="(lv, idx) in graph.local_variables" :key="idx" class="local-var-row">
          <span style="font-weight:700; font-size:11px;">{{ lv.name }}</span>
          <span class="badge" style="font-size:9px;">{{ lv.type }}</span>
          <span style="font-size:10px; color:var(--text-muted); flex:1;">= {{ lv.default_value }}</span>
          <button class="btn-micro danger" @click="removeLocalVariable(idx)">✕</button>
        </div>

        <div style="display:flex; gap:4px; margin-top:8px;">
          <input v-model="newVarName" class="form-input micro" placeholder="var_name" style="flex:1;" />
          <select v-model="newVarType" class="form-select micro" style="width:75px;">
            <option value="number">Number</option>
            <option value="boolean">Boolean</option>
            <option value="string">String</option>
          </select>
          <input v-model="newVarDefault" class="form-input micro" placeholder="Default" style="width:60px;" />
          <button class="btn btn-secondary" style="font-size:10px; padding:2px 6px;" @click="addLocalVariable">+</button>
        </div>
      </div>

      <!-- ACTOR ROLES -->
      <div class="form-group">
        <label class="field-label">Character Roles Required</label>
        <TagEditor v-model="requiredRoleNames" :options="roleTagOptions" label="Required character roles" placeholder="Choose or create required roles" />
      </div>
      <!-- STORYLET PRECONDITIONS -->
      <div class="form-group preconditions-panel">
        <label class="field-label">Preconditions</label>
        <p class="precondition-help">Every enabled group is required. Within visited-location and met-character groups, any selected option is sufficient.</p>
        <div class="precondition-group">
          <div class="precondition-heading"><span>Quality checks</span></div>
          <p class="precondition-help">All checks must pass; at least one “any” check must pass when that list is used.</p>
          <div v-for="group in qualityConditionGroups" :key="group.key" class="quality-condition-group">
            <div class="precondition-heading"><span>{{ group.label }}</span><button type="button" class="btn-micro" @click="addQualityCondition(group.key)">+</button></div>
            <div v-for="(condition, index) in (storylet.preconditions[group.key] ?? [])" :key="index" class="quality-condition-row">
              <select :value="condition.variable" class="form-select micro" :aria-label="group.label + ' quality'" @change="setQualityCondition(group.key, index, 'variable', ($event.target as HTMLSelectElement).value)">
                <option v-if="!globalQualities.includes(condition.variable)" :value="condition.variable">{{ condition.variable }}</option>
                <option v-for="quality in globalQualities" :key="quality" :value="quality">{{ quality }}</option>
              </select>
              <select :value="condition.op" class="form-select micro" :aria-label="group.label + ' operator'" @change="setQualityCondition(group.key, index, 'op', ($event.target as HTMLSelectElement).value)">
                <option value="eq">=</option><option value="neq">≠</option><option value="gt">&gt;</option><option value="gte">≥</option><option value="lt">&lt;</option><option value="lte">≤</option>
              </select>
              <input :value="condition.value" type="number" class="form-input micro" :aria-label="group.label + ' value'" @input="setQualityCondition(group.key, index, 'value', Number(($event.target as HTMLInputElement).value))" />
              <button type="button" class="btn-micro danger" :aria-label="'Remove ' + group.label.toLowerCase() + ' quality check'" @click="removeQualityCondition(group.key, index)">✕</button>
            </div>
          </div>
        </div>
        <div class="precondition-group">
          <div class="precondition-heading"><span>Locations visited</span></div>
          <p class="precondition-help">The player must have visited at least one selected location or a location with one selected tag.</p>
          <TagEditor v-model="visitedLocationTags" :options="locationTagOptions" label="Visited location tags" placeholder="Choose or create location tags" />
          <details class="precondition-options">
            <summary>Specific locations ({{ visitedLocationIds.length }} selected)</summary>
            <label v-for="location in locationOptions" :key="location.id" class="precondition-option">
              <input v-model="visitedLocationIds" type="checkbox" :value="location.id" />
              <span>{{ location.name }}</span>
            </label>
            <p v-if="!locationOptions.length" class="precondition-help">Add a location to select a specific visited place.</p>
          </details>
        </div>
        <div class="precondition-group">
          <div class="precondition-heading"><span>Characters met</span></div>
          <p class="precondition-help">The player must have met a character with one selected role or one selected character.</p>
          <TagEditor v-model="metCharacterRoleTags" :options="roleTagOptions" label="Met character roles" placeholder="Choose or create character roles" />
          <details class="precondition-options">
            <summary>Specific characters ({{ metCharacterIds.length }} selected)</summary>
            <label v-for="character in characterOptions" :key="character.id" class="precondition-option">
              <input v-model="metCharacterIds" type="checkbox" :value="character.id" />
              <span>{{ character.name }} <small>({{ character.role }})</small></span>
            </label>
            <p v-if="!characterOptions.length" class="precondition-help">Add a character to select a specific person.</p>
          </details>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.preconditions-panel { display: grid; gap: 8px; }
.precondition-group { display: grid; gap: 6px; padding: 8px; border: 1px solid var(--border-subtle); border-radius: 7px; background: rgba(0, 0, 0, 0.12); }
.precondition-heading { display: flex; align-items: center; justify-content: space-between; color: var(--text-main); font-size: 11px; font-weight: 650; }
.precondition-help { margin: 0; color: var(--text-muted); font-size: 10px; line-height: 1.4; }
.quality-condition-group { display: grid; gap: 5px; }
.quality-condition-row { display: grid; grid-template-columns: minmax(0, 1fr) 58px 58px 24px; gap: 4px; }
.precondition-options { display: grid; gap: 5px; max-height: 150px; overflow-y: auto; color: var(--text-main); font-size: 11px; }
.precondition-options summary { cursor: pointer; color: var(--text-muted); }
.precondition-option { display: flex; align-items: center; gap: 6px; font-size: 11px; }
.precondition-option input { accent-color: var(--accent-purple); }
.precondition-option small { color: var(--text-muted); }
.property-inspector-panel {
  width: 340px;
  background: #18181a;
  border-left: 1px solid #2c2c2e;
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  user-select: text;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 14px;
  padding-bottom: 10px;
  border-bottom: 1px solid #2c2c2e;
}

.panel-title {
  font-size: 14px;
  font-weight: 700;
  margin: 0;
  color: #ffffff;
}

.type-badge {
  font-size: 9px;
  font-weight: 800;
  padding: 2px 6px;
  border-radius: 4px;
  color: #ffffff;
}

.type-badge.start { background: #10b981; }
.type-badge.dialog { background: #8b5cf6; }
.type-badge.choice { background: #a855f7; }
.type-badge.variable { background: #3b82f6; }
.type-badge.conditional { background: #eab308; }
.type-badge.image { background: #ec4899; }
.type-badge.teleport { background: #06b6d4; }
.type-badge.end { background: #f43f5e; }
.type-badge.graph_transform { background: #2563eb; }
.type-badge.passion_mutation { background: #db2777; }

.field-label {
  font-size: 11px;
  font-weight: 700;
  color: #aeaeb2;
  text-transform: uppercase;
  margin-bottom: 4px;
  display: block;
}

.field-sublabel {
  font-size: 10px;
  font-weight: 600;
  color: #8e8e93;
  margin-bottom: 2px;
  display: block;
}

.dialogue-section-card {
  background: #242428;
  border: 1px solid #38383c;
  border-radius: 8px;
  margin-bottom: 10px;
  overflow: hidden;
}

.section-card-header {
  padding: 6px 8px;
  background: #2c2c30;
  display: flex;
  justify-content: space-between;
  align-items: center;
  border-bottom: 1px solid #38383c;
}

.section-title-input {
  background: transparent;
  border: none;
  color: #ffffff;
  font-size: 11px;
  font-weight: 700;
  outline: none;
  flex: 1;
}

.section-items {
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.chrono-settings {
  padding: 9px;
  border: 1px solid var(--border-subtle);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.16);
}
.chrono-settings > summary { cursor: pointer; }
.chrono-fields { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; margin: 8px 0; }
.chrono-fields label { color: var(--text-muted); font-size: 10px; }
.chrono-predicate-heading { display: flex; justify-content: space-between; align-items: center; margin: 8px 0 4px; }
.chrono-predicate-row { display: grid; grid-template-columns: 1fr 1.2fr 44px 56px 52px 22px; gap: 3px; align-items: center; }
.chrono-checkbox { display: flex; align-items: center; gap: 6px; }
.chrono-json { min-height: 72px; font-family: monospace; font-size: 10px; }

.field-help { color: var(--text-muted); font-size: 12px; }
.field-row-heading { display: flex; justify-content: space-between; align-items: center; margin: 8px 0; }
.choice-editor-row { display: grid; grid-template-columns: 1fr 1fr auto; gap: 5px; margin-bottom: 6px; }
.dialogue-item-card {
  background: #1c1c1e;
  border: 1px solid #323236;
  border-radius: 6px;
  padding: 8px;
}

.form-select.micro,
.form-input.micro {
  padding: 3px 6px;
  font-size: 11px;
  height: 26px;
}

.btn-micro {
  background: #38383c;
  border: 1px solid #48484e;
  color: #e5e5ea;
  font-size: 9px;
  font-weight: 700;
  padding: 2px 5px;
  border-radius: 4px;
  cursor: pointer;
}

.btn-micro:hover {
  background: #48484e;
}

.btn-micro.danger {
  color: #ff453a;
}

.btn-micro.danger:hover {
  background: rgba(255, 69, 58, 0.2);
}

.audio-browse-label {
  font-size: 10px;
  color: #0a84ff;
  cursor: pointer;
  font-weight: 600;
}

.mutation-row,
.local-var-row {
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
</style>
