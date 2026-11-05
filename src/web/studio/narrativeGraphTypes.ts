import type { StoryletRecord, StoryletChoiceRecord } from "./contracts.ts";


export function isSafeImageUrl(value: string | undefined): boolean {
  const source = value?.trim();
  if (!source || source.startsWith("//") || source.includes("\\") || /[\u0000-\u001f\u007f-\u009f]/.test(source)) return false;
  const firstSegment = source.split("/", 1)[0] ?? "";
  const separator = firstSegment.indexOf(":");
  if (separator < 0) return true;
  const scheme = firstSegment.slice(0, separator).toLowerCase();
  return (scheme === "http" || scheme === "https") && source.slice(separator + 1).startsWith("//");
}
export type NodeType =
  | "start"
  | "dialog"
  | "choice"
  | "condition"
  | "compare"
  | "get_entity"
  | "get_entity_property"
  | "apply_command"
  | "qbn_storylet_hub"
  | "salience_selector"
  | "waypoint_conversation"
  | "waiter"
  | "and"
  | "or"
  | "scene_section"
  | "mission_sub_graph"
  | "poi"
  | "image"
  | "teleport"
  | "go_to_location"
  | "graph_transform"
  | "passion_mutation"
  | "add_entity_tag"
  | "remove_entity_tag"
  | "has_entity_tag"
  | "end";

export interface DialogueLineItem {
  kind: "line";
  id: string;
  speaker_role: string;
  target_role: string; // default "player"
  content: string;
  emotion_tag?: string;
  action_tag?: string;
  action_target?: string;
  audio_path?: string;
  duration?: number;
}

export interface DialogueIdleActionItem {
  kind: "idle_action";
  id: string;
  action_type: "lookat" | "emote";
  actor_role: string;
  target: string; // "player" or role name
  emote?: string;
  duration?: number;
}

export type DialogueItem = DialogueLineItem | DialogueIdleActionItem;

export interface DialogueSection {
  id: string;
  title: string;
  items: DialogueItem[];
  collapsed?: boolean;
}

export interface ChoiceOption {
  id: string;
  text: string;
  system_verb?: string;
  effects?: Record<string, unknown>;
}

export type GraphLayer = "location" | "character" | "item" | "fact";
export type AttributeOp = "eq" | "neq" | "gt" | "gte" | "lt" | "lte";
export type AttributeValueType = "int" | "float" | "bool" | "string" | "tag";

export type GraphValueType = "entity_ref" | "bool" | "int" | "float" | "string" | "asset" | "any";
export const GRAPH_PIN_COLORS: Record<GraphValueType | "flow", string> = {
  flow: "#ffffff",
  entity_ref: "#ff9f0a",
  bool: "#30d158",
  int: "#0a84ff",
  float: "#64d2ff",
  string: "#bf5af2",
  asset: "#ff375f",
  any: "#8e8e93",
};
export type ApplyCommandOperation = "set" | "add" | "subtract" | "toggle";
export function applyCommandOperationsForType(valueType: string | undefined): Array<{ value: ApplyCommandOperation; label: string }> {
  if (valueType === "bool") return [{ value: "set", label: "Set" }, { value: "toggle", label: "Toggle" }];
  if (valueType === "int" || valueType === "float") {
    return [{ value: "set", label: "Set" }, { value: "add", label: "Add" }, { value: "subtract", label: "Subtract" }];
  }
  return [{ value: "set", label: "Set" }];
}

export type AttributeValue =
  | { type: "int"; value: number }
  | { type: "float"; value: number }
  | { type: "bool"; value: boolean }
  | { type: "string"; value: string }
  | { type: "tag"; value: number };


export type GraphBinding =
  | { type: "literal"; id: string }
  | { type: "vertex_pattern"; index: number };

export interface AttributePredicate {
  key: string;
  op: AttributeOp;
  value: AttributeValue;
}

export interface VertexPattern {
  layer: GraphLayer;
  vertex_id?: string;
  attributes: AttributePredicate[];
}

export interface EdgePattern {
  edge_id?: string;
  relation_type?: string;
  source?: GraphBinding;
  target?: GraphBinding;
}

export interface GraphPattern {
  vertices: VertexPattern[];
  edges: EdgePattern[];
}

export interface VertexTemplate {
  id: GraphBinding;
  layer: GraphLayer;
  attributes: Record<string, AttributeValue>;
}

export interface EdgeTemplate {
  id: GraphBinding;
  source: GraphBinding;
  target: GraphBinding;
  relation_type: string;
  weight: number;
}

export interface AttributeAssignment {
  vertex: GraphBinding;
  key: string;
  value: AttributeValue;
}

export interface GraphMutation {
  add_vertices: VertexTemplate[];
  remove_vertices: GraphBinding[];
  add_edges: EdgeTemplate[];
  remove_edges: GraphBinding[];
  set_attributes: AttributeAssignment[];
}

export interface GraphTransformRule {
  lhs: GraphPattern;
  rhs: GraphMutation;
}

export interface PassionMutation {
  micro_deltas: Array<[number, number]>;
  active_ops: Array<[number, boolean]>;
}

export function isGraphTransformNode(node: NarrativeNode): node is NarrativeNode & { rule: GraphTransformRule } {
  const rule = node.rule;
  if (node.type !== "graph_transform" || !rule?.lhs || !rule?.rhs) return false;
  const rhs = rule.rhs;
  return Array.isArray(rule.lhs.vertices) && Array.isArray(rule.lhs.edges) &&
    Array.isArray(rhs.add_vertices) && Array.isArray(rhs.remove_vertices) &&
    Array.isArray(rhs.add_edges) && Array.isArray(rhs.remove_edges) && Array.isArray(rhs.set_attributes);
}

export function isPassionMutationNode(node: NarrativeNode): node is NarrativeNode & { op: PassionMutation } {
  const op = node.op;
  return node.type === "passion_mutation" && !!op && Array.isArray(op.micro_deltas) && Array.isArray(op.active_ops);
}
export interface NarrativeNode {
  id: string;
  type: NodeType;
  title: string;
  x: number;
  y: number;
  width: number;
  height: number;
  speaker_role?: string;
  text?: string;
  quip?: string;
  sections?: DialogueSection[];
  choices?: ChoiceOption[];
  rule?: GraphTransformRule;
  op?: PassionMutation;
  image_url?: string;
  destination_storylet?: string;
  location_id?: string;
  is_terminal?: boolean;
  jump_to_storylet?: string;
  precondition_result?: boolean;
  deck_storylet_ids?: string[];
  hub_exit_storylet_id?: string;
  candidate_pool_ids?: string[];
  fallback_storylet_id?: string;
  topic_nodes?: Array<Record<string, unknown>>;
  topic_edges?: Array<Record<string, unknown>>;
  initial_topic_id?: string;
  npc_goal_topic_id?: string;
  wait_event?: string;
  duration_ms?: number;
  timeline_events?: Array<Record<string, unknown>>;
  sub_graph_asset_id?: string;
  entity_path?: string;
  target_entity_id?: "self" | "world" | string;
  tag?: string;
  match_mode?: "all" | "any";
  property_key?: string;
  value_type?: "bool" | "int" | "float" | "string" | "asset" | "entity";
  operation?: "set" | "add" | "subtract" | "toggle";
  operator?: AttributeOp;
  value?: unknown;
  // Entity membership predicates use Has Entity Tag dot-notation.
}

export function graphNodeValueType(node: NarrativeNode): GraphValueType | undefined {
  return node.value_type === "entity" ? "entity_ref" : node.value_type;
}

export function graphTargetPinType(node: NarrativeNode, port: string): GraphValueType | "flow" | undefined {
  if (port === "entity" || port === "target") return "entity_ref";
  if (port === "input" && (node.type === "condition" || node.type === "and" || node.type === "or")) return "bool";
  if (port === "input" && node.type === "compare") return graphNodeValueType(node) ?? "any";
  return port === "input" ? "flow" : undefined;
}

export function graphConnectionIsCompatible(source: GraphValueType | "flow", targetNode: NarrativeNode, port: string): boolean {
  if (targetNode.type === "condition" && port === "input") return source === "flow" || source === "bool";
  const target = graphTargetPinType(targetNode, port);
  return source === "flow" ? target === "flow" : target === "any" || source === target;
}


export interface NarrativeEdge {
  id: string;
  kind: "flow" | "data";
  from_node: string;
  from_port: string;
  to_node: string;
  to_port: string;
}

export interface NarrativeGraph {
  nodes: NarrativeNode[];
  edges: NarrativeEdge[];
}

type StoryletGraphSource = Pick<StoryletRecord, "graph" | "content" | "parameters" | "choices"> & {
  ending?: boolean;
};
export function buildGraphFromStorylet(st: StoryletGraphSource): NarrativeGraph {
  // A new draft begins with an empty placeholder; initialize the usable graph.
  if (st.graph && typeof st.graph === "object" && Array.isArray((st.graph as NarrativeGraph).nodes) && (st.graph as NarrativeGraph).nodes.length > 0) {
    return JSON.parse(JSON.stringify(st.graph));
  }

  const nodes: NarrativeNode[] = [];
  const edges: NarrativeEdge[] = [];

  // Start node
  nodes.push({
    id: "node_start",
    type: "start",
    title: "Start",
    x: 60,
    y: 120,
    width: 170,
    height: 80,
    text: "Begin Discovery",
  });

  // Dialog node
  const prose = st.content.sceneOpening || st.content.text_template || "Scene opening text...";
  const speaker = st.content.speaker_id || (st.parameters[0]?.param_name ?? "player");

  const initialLine: DialogueLineItem = {
    kind: "line",
    id: "line_0",
    speaker_role: speaker,
    target_role: "player",
    content: prose,
  };

  const initialSection: DialogueSection = {
    id: "sec_main",
    title: "Main Beat",
    items: [initialLine],
  };

  nodes.push({
    id: "node_dialog_1",
    type: "dialog",
    title: "Dialog",
    x: 250,
    y: 120,
    width: 280,
    height: 240,
    speaker_role: speaker,
    text: prose,
    sections: [initialSection],
  });

    edges.push({
      id: "edge_start_dialog",
      kind: "flow",
      from_node: "node_start",
      from_port: "output",
      to_node: "node_dialog_1",
      to_port: "input",
  });

  // Choice node
  if (st.choices.length > 0) {
    const opts: ChoiceOption[] = st.choices.map((c, idx) => ({
      id: c.choice_id || `choice_${idx + 1}`,
      text: c.label || `Option ${idx + 1}`,
      system_verb: c.systemVerb,
      effects: c.effects,
    }));

    nodes.push({
      id: "node_choice_1",
      type: "choice",
      title: "Choice",
      x: 670,
      y: 120,
      width: 260,
      height: Math.max(140, opts.length * 36 + 80),
      choices: opts,
    });

    edges.push({
      id: "edge_dialog_choice",
      kind: "flow",
      from_node: "node_dialog_1",
      from_port: "output",
      to_node: "node_choice_1",
      to_port: "input",
    });

    // Destination nodes for choices
    st.choices.forEach((c, idx) => {
      if (typeof c.effects?.["$move_player"] === "string") {
        const locationNodeId = `node_location_${idx + 1}`;
        nodes.push({ id: locationNodeId, type: "go_to_location", title: "Go To Location", x: 990, y: idx * 130 + 80, width: 220, height: 90, location_id: c.effects["$move_player"] as string });
        edges.push({ id: `edge_choice_${idx}_${locationNodeId}`, kind: "flow", from_node: "node_choice_1", from_port: `choice_${idx}`, to_node: locationNodeId, to_port: "input" });
      } else if (c.divert_target) {
        const teleId = `node_teleport_${idx + 1}`;
        nodes.push({
          id: teleId,
          type: "teleport",
          title: "Teleport",
          x: 990,
          y: idx * 130 + 80,
          width: 220,
          height: 90,
          destination_storylet: c.divert_target,
        });

        edges.push({
          id: `edge_choice_${idx}_${teleId}`,
          kind: "flow",
          from_node: "node_choice_1",
          from_port: `choice_${idx}`,
          to_node: teleId,
          to_port: "input",
        });
      }
    });
  }

  nodes.push({ id: "node_end", type: "end", title: "End", x: 560, y: 120, width: 170, height: 90, is_terminal: st.ending === true });
  if (st.choices.length === 0) {
    edges.push({ id: "edge_dialog_end", kind: "flow", from_node: "node_dialog_1", from_port: "output", to_node: "node_end", to_port: "input" });
  } else {
    st.choices.forEach((choice, idx) => {
      if (typeof choice.effects?.["$move_player"] === "string") {
        edges.push({ id: `edge_location_${idx + 1}_end`, kind: "flow", from_node: `node_location_${idx + 1}`, from_port: "output", to_node: "node_end", to_port: "input" });
      } else if (choice.divert_target) {
        edges.push({ id: `edge_teleport_${idx + 1}_end`, kind: "flow", from_node: `node_teleport_${idx + 1}`, from_port: "output", to_node: "node_end", to_port: "input" });
      } else {
        edges.push({ id: `edge_choice_${idx}_end`, kind: "flow", from_node: "node_choice_1", from_port: `choice_${idx}`, to_node: "node_end", to_port: "input" });
      }
    });
  }

  return {
    nodes,
    edges,
  };
}

export function validateNarrativeGraph(graph: NarrativeGraph): string[] {
  const issues: string[] = [];
  if (!graph || !Array.isArray(graph.nodes) || !Array.isArray(graph.edges)) return ["Graph must contain node and edge arrays."];
  const allowedTypes = new Set<NodeType>([
    "start", "dialog", "choice", "condition", "compare", "get_entity", "get_entity_property", "apply_command", "qbn_storylet_hub",
    "salience_selector", "waypoint_conversation", "waiter", "and", "or",
    "scene_section", "mission_sub_graph", "poi", "image", "teleport", "go_to_location", "graph_transform", "passion_mutation",
    "add_entity_tag", "remove_entity_tag", "has_entity_tag", "end",
  ]);
  if (graph.nodes.some((node) => !node || typeof node !== "object" || typeof node.id !== "string" || !node.id.trim() || !allowedTypes.has(node.type))) {
    return ["Graph nodes must have nonempty IDs and recognized types."];
  }
  for (const node of graph.nodes) {
    if (node.type === "go_to_location" && !node.location_id?.trim()) issues.push("Go To Location nodes must select a location.");
    if (node.type === "get_entity" && !node.entity_path?.trim()) issues.push(`Get Entity node ${node.title} must select an Entity.`);
    if (node.type === "get_entity_property") {
      if (!node.property_key?.trim()) issues.push(`Get Entity Property node ${node.title} must specify a property key.`);
      if (!["bool", "int", "float", "string", "asset", "entity"].includes(node.value_type ?? "")) {
        issues.push(`Get Entity Property node ${node.title} must specify a supported value type.`);
      }
    }
    if (node.type === "apply_command") {
      if (!node.property_key?.trim()) issues.push(`Apply Command node ${node.title} must specify a property key.`);
      if (!["set", "add", "subtract", "toggle"].includes(node.operation ?? "")) {
        issues.push(`Apply Command node ${node.title} must specify a supported operation.`);
      }
      const valueMatches = node.value_type === "bool" ? typeof node.value === "boolean" :
        node.value_type === "int" ? typeof node.value === "number" && Number.isInteger(node.value) :
          node.value_type === "float" ? typeof node.value === "number" && Number.isFinite(node.value) :
            ["string", "asset", "entity"].includes(node.value_type ?? "") && typeof node.value === "string";
      if (!valueMatches) issues.push(`Apply Command node ${node.title} must specify a literal matching its value type.`);
      if (node.operation === "toggle" && node.value_type !== "bool") {
        issues.push(`Apply Command node ${node.title} can only toggle boolean properties.`);
      }
      if (["add", "subtract"].includes(node.operation ?? "") && !["int", "float"].includes(node.value_type ?? "")) {
        issues.push(`Apply Command node ${node.title} can only add or subtract numeric properties.`);
      }
    }
    if (node.type === "image" && !isSafeImageUrl(node.image_url)) issues.push(`Image node ${node.title} must have a safe HTTP(S) URL or project-relative path.`);
    if ((node.type === "add_entity_tag" || node.type === "remove_entity_tag" || node.type === "has_entity_tag") && !node.tag?.trim()) {
      issues.push(`Entity tag node ${node.title} must specify a tag.`);
    }
    if (node.type === "qbn_storylet_hub") {
      if (!(node.deck_storylet_ids?.length)) issues.push(`QBN hub ${node.title} must have a nonempty storylet deck.`);
      if (!node.hub_exit_storylet_id?.trim() || !node.deck_storylet_ids?.includes(node.hub_exit_storylet_id)) {
        issues.push(`QBN hub ${node.title} must select its exit storylet from its deck.`);
      }
    }
    if (node.type === "salience_selector" && !(node.candidate_pool_ids?.length)) {
      issues.push(`Salience selector ${node.title} must have a nonempty candidate pool.`);
    }
    if (node.type === "waiter" && !node.wait_event?.trim()) issues.push(`Waiter ${node.title} must name a gameplay event.`);
    if (node.type === "mission_sub_graph" && !node.sub_graph_asset_id?.trim()) issues.push(`Mission ${node.title} must select a subgraph asset.`);
    if (node.type === "poi" && !node.entity_path?.trim()) issues.push(`POI ${node.title} must bind an entity path.`);
    if (node.type === "scene_section") {
      if (!Number.isSafeInteger(node.duration_ms) || (node.duration_ms ?? -1) < 0) {
        issues.push(`Scene Section ${node.title} must have a nonnegative integer duration.`);
      }
      for (const event of node.timeline_events ?? []) {
        const start = event.start_time_ms;
        const duration = event.duration_ms;
        if (!Number.isSafeInteger(start) || !Number.isSafeInteger(duration) || (start as number) < 0 || (duration as number) < 0 ||
          (start as number) + (duration as number) > (node.duration_ms ?? 0)) {
          issues.push(`Scene Section ${node.title} has a timeline event outside its duration.`);
        }
      }
    }
    if (node.type === "waypoint_conversation") {
      const topics = node.topic_nodes ?? [];
      const topicIds = new Set(topics.map((topic) => topic.topic_id).filter((id): id is string => typeof id === "string"));
      const exitPins = topics.filter((topic) => topic.triggers_scene_exit === true).map((topic) => topic.exit_output_pin);
      if (!node.initial_topic_id || !topicIds.has(node.initial_topic_id)) issues.push(`Waypoint ${node.title} must have a valid initial topic.`);
      if (!node.npc_goal_topic_id || !topicIds.has(node.npc_goal_topic_id)) issues.push(`Waypoint ${node.title} must have a valid NPC goal topic.`);
      if (!exitPins.length || exitPins.some((pin) => typeof pin !== "string" || !pin.trim()) || new Set(exitPins).size !== exitPins.length) {
        issues.push(`Waypoint ${node.title} must define unique output pins for its scene-exit topics.`);
      }
      for (const edge of node.topic_edges ?? []) {
        if (typeof edge.from_topic_id !== "string" || typeof edge.to_topic_id !== "string" ||
          !topicIds.has(edge.from_topic_id) || !topicIds.has(edge.to_topic_id) ||
          typeof edge.base_cost !== "number" || !Number.isFinite(edge.base_cost) || edge.base_cost < 0) {
          issues.push(`Waypoint ${node.title} has an invalid topic transition.`);
          break;
        }
      }
    }
    if (node.type === "graph_transform") {
      if (!isGraphTransformNode(node)) {
        issues.push(`Graph Transform ${node.title} must define a left-hand side pattern and a right-hand side mutation.`);
      } else {
        const rule = node.rule;
        const rhs = rule.rhs;
        if (!(rule.lhs.vertices.length || rule.lhs.edges.length)) issues.push(`Graph Transform ${node.title} must match at least one graph pattern.`);
        if (!(rhs.add_vertices.length || rhs.remove_vertices.length || rhs.add_edges.length || rhs.remove_edges.length || rhs.set_attributes.length)) {
          issues.push(`Graph Transform ${node.title} must apply at least one mutation.`);
        }
        const values = [
          ...rule.lhs.vertices.flatMap((vertex) => vertex.attributes.map((predicate) => predicate.value)),
          ...rhs.add_vertices.flatMap((vertex) => Object.values(vertex.attributes)),
          ...rhs.set_attributes.map((assignment) => assignment.value),
        ];
        if (values.some((value) => value.type === "float" && !Number.isFinite(value.value))) {
          issues.push(`Graph Transform ${node.title} has a non-finite attribute value.`);
        }
        if (rule.lhs.vertices.some((vertex) => vertex.attributes.some((predicate) =>
          (predicate.op === "gt" || predicate.op === "gte" || predicate.op === "lt" || predicate.op === "lte") &&
          predicate.value.type !== "int" && predicate.value.type !== "float"))) {
          issues.push(`Graph Transform ${node.title} has an ordered attribute predicate without a numeric value.`);
        }
        const bindings = [
          ...rule.lhs.edges.flatMap((edge) => [edge.source, edge.target].filter((binding): binding is GraphBinding => !!binding)),
          ...rhs.add_vertices.map((vertex) => vertex.id),
          ...rhs.add_edges.flatMap((edge) => [edge.id, edge.source, edge.target]),
          ...rhs.remove_vertices,
          ...rhs.remove_edges,
          ...rhs.set_attributes.map((assignment) => assignment.vertex),
        ];
        if (bindings.some((binding) => binding.type === "vertex_pattern" && binding.index >= rule.lhs.vertices.length)) {
          issues.push(`Graph Transform ${node.title} binds a vertex pattern outside its left-hand side.`);
        }
      }
    }
    if (node.type === "passion_mutation") {
      if (!isPassionMutationNode(node)) {
        issues.push(`Passion Mutation ${node.title} must define typed passion deltas and active-pool operations.`);
      } else {
        const op = node.op;
        if (!(op.micro_deltas.length || op.active_ops.length)) {
          issues.push(`Passion Mutation ${node.title} must apply at least one typed delta or active-pool operation.`);
        }
        if (op.micro_deltas.some(([id]) => !Number.isInteger(id) || id < 0 || id > 65535) ||
          op.active_ops.some(([id]) => !Number.isInteger(id) || id < 0 || id > 65535)) {
          issues.push(`Passion Mutation ${node.title} has a passion index outside the u16 range.`);
        }
        if (op.micro_deltas.some(([, delta]) => !Number.isFinite(delta))) issues.push(`Passion Mutation ${node.title} has a non-finite passion delta.`);
      }
    }
  }
  if (graph.edges.some((edge) => !edge || typeof edge !== "object" || typeof edge.id !== "string" || !edge.id.trim() ||
    typeof edge.from_node !== "string" || typeof edge.from_port !== "string" || typeof edge.to_node !== "string" || typeof edge.to_port !== "string" ||
    (edge.kind !== undefined && edge.kind !== "flow" && edge.kind !== "data"))) {
    return ["Graph edges must have nonempty IDs, valid endpoint fields, and a valid kind."];
  }
  const starts = graph.nodes.filter((node) => node.type === "start");
  const ends = graph.nodes.filter((node) => node.type === "end");
  const ids = new Set(graph.nodes.map((node) => node.id));
  if (starts.length !== 1) issues.push("Graph must have exactly one Start node.");
  if (!ends.length) issues.push("Graph must have at least one End node.");
  if (ids.size !== graph.nodes.length) issues.push("Node IDs must be unique.");
  if (starts.length !== 1 || !ends.length || ids.size !== graph.nodes.length) return issues;
  const outgoing = new Map<string, string[]>();
  const incoming = new Map<string, string[]>();
  const dataIncoming = new Map<string, NarrativeEdge[]>();
  const flowEdges = graph.edges.filter((edge) => (edge.kind ?? "flow") === "flow");
  const dataEdges = graph.edges.filter((edge) => edge.kind === "data");
  const edgeIds = new Set<string>();
  for (const edge of graph.edges) {
    if (edgeIds.has(edge.id)) issues.push("Edge IDs must be unique.");
    edgeIds.add(edge.id);
    if (!ids.has(edge.from_node) || !ids.has(edge.to_node)) { issues.push(`Edge ${edge.id} references a missing node.`); continue; }
    const source = graph.nodes.find((node) => node.id === edge.from_node)!;
    const target = graph.nodes.find((node) => node.id === edge.to_node)!;
    if ((edge.kind ?? "flow") === "flow") {
      const pureDataSource = ["get_entity", "get_entity_property", "compare", "has_entity_tag", "and", "or"].includes(source.type);
      if (pureDataSource || edge.to_port !== "input") issues.push(`Flow edge ${edge.id} uses an invalid source or target port.`);
      outgoing.set(edge.from_node, [...(outgoing.get(edge.from_node) ?? []), edge.to_node]);
      incoming.set(edge.to_node, [...(incoming.get(edge.to_node) ?? []), edge.from_node]);
    } else {
      dataIncoming.set(edge.to_node, [...(dataIncoming.get(edge.to_node) ?? []), edge]);
      const sourceType = source.type === "get_entity" ? "entity_ref" :
        source.type === "get_entity_property" ? graphNodeValueType(source) :
          ["compare", "has_entity_tag", "and", "or"].includes(source.type) ? "bool" : undefined;
      const targetType = graphTargetPinType(target, edge.to_port);
      const sourcePort = ["get_entity", "get_entity_property", "compare", "has_entity_tag", "and", "or"].includes(source.type)
        ? edge.from_port === "output" : false;
      if (!sourcePort || !sourceType || !targetType || (targetType !== "any" && targetType !== sourceType)) {
        issues.push(`Data edge ${edge.id} uses an invalid port or incompatible type.`);
      }
    }
  }
  const start = starts[0]!.id;
  if ((incoming.get(start)?.length ?? 0) || (outgoing.get(start)?.length ?? 0) !== 1) issues.push("Start must have no incoming edges and exactly one outgoing edge.");
  const walk = (seed: string, links: Map<string, string[]>): Set<string> => {
    const seen: Set<string> = new Set(); const stack = [seed];
    while (stack.length) { const id = stack.pop()!; if (seen.has(id)) continue; seen.add(id); stack.push(...(links.get(id) ?? [])); }
    return seen;
  };
  const addDataDependencies = (seen: Set<string>): Set<string> => {
    const stack = [...seen];
    while (stack.length) {
      const target = stack.pop()!;
      for (const edge of dataIncoming.get(target) ?? []) {
        if (!seen.has(edge.from_node)) { seen.add(edge.from_node); stack.push(edge.from_node); }
      }
    }
    return seen;
  };
  const reachable = addDataDependencies(walk(start, outgoing));
  const reachesEnd = new Set<string>();
  for (const end of ends) for (const id of addDataDependencies(walk(end.id, incoming))) reachesEnd.add(id);
  if (graph.nodes.some((node) => !reachable.has(node.id))) issues.push("Every node must be reachable from Start.");
  if (graph.nodes.some((node) => !reachesEnd.has(node.id))) issues.push("Every node must have a path to an End node.");
  for (const node of graph.nodes) {
    const edges = flowEdges.filter((edge) => edge.from_node === node.id);
    const valueEdges = dataEdges.filter((edge) => edge.from_node === node.id);
    if (node.type === "choice") {
      const expectedPorts = new Set((node.choices ?? []).map((_, index) => `choice_${index}`));
      for (let index = 0; index < (node.choices?.length ?? 0); index++) {
        if (edges.filter((edge) => edge.from_port === `choice_${index}`).length !== 1) issues.push(`Choice ${index + 1} on ${node.title} must have exactly one outgoing branch.`);
      }
      if (edges.some((edge) => !expectedPorts.has(edge.from_port))) issues.push(`Choice node ${node.title} has an edge from an unknown output port.`);
    } else if (node.type === "condition") {
      const expectedPorts = ["true", "false"];
      for (const port of expectedPorts) {
        if (edges.filter((edge) => edge.from_port === port).length !== 1) {
          issues.push(`Condition Node ${node.title} must have exactly one '${port}' branch.`);
        }
      }
      if (edges.some((edge) => !expectedPorts.includes(edge.from_port))) {
        issues.push(`Condition Node ${node.title} has an edge from an unknown branch.`);
      }
      if ((dataIncoming.get(node.id)?.length ?? 0) !== 1) {
        issues.push(`Condition Node ${node.title} must have exactly one boolean data input.`);
      }
    } else if (node.type === "waypoint_conversation") {
      const expectedPorts = new Set((node.topic_nodes ?? []).filter((topic) => topic.triggers_scene_exit === true).map((topic) => topic.exit_output_pin));
      for (const port of expectedPorts) {
        if (typeof port === "string" && edges.filter((edge) => edge.from_port === port).length !== 1) {
          issues.push(`Waypoint exit ${port} on ${node.title} must have exactly one outgoing edge.`);
        }
      }
      if (edges.some((edge) => !expectedPorts.has(edge.from_port))) issues.push(`Waypoint ${node.title} has an edge from an unknown exit pin.`);
    } else if (node.type === "has_entity_tag") {
      if (valueEdges.length !== 1 || valueEdges[0]?.from_port !== "output") {
        issues.push(`Has Entity Tag node ${node.title} must have exactly one boolean output connection.`);
      }
    } else if (node.type === "go_to_location") {
      const incomingEdges = flowEdges.filter((edge) => edge.to_node === node.id);
      if (incomingEdges.some((edge) => graph.nodes.find((source) => source.id === edge.from_node)?.type !== "choice")) {
        issues.push(`Go To Location node ${node.title} may only be targeted directly by a Choice branch.`);
      }
      if (edges.length !== 1 || edges[0]?.from_port !== "output") issues.push(`Go To Location node ${node.title} must have exactly one outgoing edge from its output port.`);
    } else if (node.type === "end") {
      if (edges.length) issues.push("End cannot have outgoing edges.");
    } else if (!["get_entity", "get_entity_property", "compare", "and", "or"].includes(node.type) &&
      (edges.length !== 1 || edges[0]?.from_port !== "output")) {
      issues.push(`${node.type} node ${node.title} must have exactly one outgoing edge from its output port.`);
    }
    if (node.type === "get_entity_property" && dataEdges.filter((edge) => edge.to_node === node.id && edge.to_port === "entity").length !== 1) {
      issues.push(`Get Entity Property node ${node.title} must have exactly one Entity data input.`);
    }
    if (node.type === "apply_command" && dataEdges.filter((edge) => edge.to_node === node.id && edge.to_port === "entity").length !== 1) {
      issues.push(`Apply Command node ${node.title} must have exactly one Entity data input.`);
    }
    if ((node.type === "and" || node.type === "or") && (dataIncoming.get(node.id)?.length ?? 0) < 2) {
      issues.push(`${node.type.toUpperCase()} node ${node.title} must have at least two boolean inputs.`);
    }
    if (node.type === "compare" && (dataIncoming.get(node.id)?.length ?? 0) !== 1) {
      issues.push(`Compare node ${node.title} must have exactly one value input.`);
    }
  }
  return issues;
}

export function compileGraphToStorylet(graph: NarrativeGraph, st: StoryletRecord) {
  const proseParts: string[] = [];
  const choices: StoryletChoiceRecord[] = [];
  const endNodes = graph.nodes.filter((node) => node.type === "end");
  let isEnding = endNodes.length > 0 && endNodes.every((node) => node.is_terminal === true && !node.jump_to_storylet);
  const effects = { ...st.effects };


  for (const node of graph.nodes) {
    if (node.type === "dialog") {
      if (node.sections && node.sections.length > 0) {
        for (const sec of node.sections) {
          for (const item of sec.items) {
            if (item.kind === "line") {
              if (item.content?.trim()) {
                const emoPrefix = item.emotion_tag ? `[${item.emotion_tag}] ` : "";
                proseParts.push(`${item.speaker_role}: ${emoPrefix}${item.content.trim()}`);
                st.content.speaker_id = item.speaker_role;
              }
            } else if (item.kind === "idle_action") {
              if (item.action_type === "emote" && item.emote) {
                proseParts.push(`(${item.actor_role} ${item.emote} at ${item.target})`);
              } else {
                proseParts.push(`(${item.actor_role} looks at ${item.target})`);
              }
            }
          }
        }
      } else if (node.text?.trim()) {
        proseParts.push(node.text.trim());
        if (node.speaker_role) {
          st.content.speaker_id = node.speaker_role;
        }
      }
    } else if (node.type === "choice" && node.choices?.length) {
      node.choices.forEach((opt, idx) => {
        const portId = `choice_${idx}`;
        const edge = graph.edges.find(
          (e) => e.from_node === node.id && (e.from_port === portId || e.from_port === "output")
        );

        let divertTarget: string | undefined;
        const choiceEffects: Record<string, unknown> = { ...(opt.effects ?? {}) };
        delete choiceEffects["$move_player"];
        if (edge) {
          const targetNode = graph.nodes.find((n) => n.id === edge.to_node);
          if (targetNode?.type === "go_to_location" && targetNode.location_id) {
            choiceEffects["$move_player"] = targetNode.location_id;
          } else if (targetNode?.type === "teleport" && targetNode.destination_storylet) {
            divertTarget = targetNode.destination_storylet;
          } else if (targetNode?.type === "end") {
            if (targetNode.jump_to_storylet) {
              divertTarget = targetNode.jump_to_storylet;
            }
          }
        }

        choices.push({
          choice_id: opt.id || `choice_${idx + 1}`,
          label: opt.text,
          systemVerb: opt.system_verb || "ACT",
          effects: choiceEffects,
          divert_target: divertTarget,
        });
      });
  }
    }

  if (proseParts.length > 0) {
    const fullProse = proseParts.join("\n\n");
    st.content.sceneOpening = fullProse;
    st.content.text_template = fullProse;
  }

  st.choices = choices;

  st.ending = isEnding;

  st.effects = effects;
  st.graph = graph;
}

export function validatePreconditionGraph(graph: NarrativeGraph): string[] {
  const issues = validateNarrativeGraph(graph);
  if (issues.length) return issues;

  const pureTypes: Partial<Record<NodeType, true>> = {
    start: true,
    end: true,
    condition: true,
    compare: true,
    get_entity: true,
    get_entity_property: true,
    has_entity_tag: true,
    and: true,
    or: true,
  };
  for (const node of graph.nodes) {
    if (!pureTypes[node.type]) {
      issues.push(`Precondition graph cannot contain ${node.type.replaceAll("_", " ")} nodes.`);
    }
  }
  if (!graph.nodes.some((node) => node.type === "end" && node.precondition_result !== false)) {
    issues.push("Precondition graph needs at least one valid-candidate End node.");
  }
  for (const node of graph.nodes) {
    if (node.type === "end" && node.precondition_result !== undefined && typeof node.precondition_result !== "boolean") {
      issues.push(`Precondition End ${node.title} must have a boolean candidate result.`);
    }
  }
  return issues;
}


