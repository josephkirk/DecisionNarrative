import type { StoryletRecord, StoryletChoiceRecord, LocalVariableDefinition } from "./contracts.ts";


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
  | "variable"
  | "conditional"
  | "image"
  | "teleport"
  | "go_to_location"
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

export interface VariableAssignment {
  scope: "global" | "local";
  quality: string;
  op: "set" | "add";
  value: string | number | boolean;
}

export interface ConditionRule {
  scope: "global" | "local";
  variable: string;
  op: "eq" | "neq" | "gt" | "gte" | "lt" | "lte";
  value: string | number | boolean;
}

export interface NarrativeNode {
  id: string;
  type: NodeType;
  title: string;
  x: number;
  y: number;
  width: number;
  height: number;
  // Node specific data
  speaker_role?: string;
  text?: string;
  quip?: string;
  sections?: DialogueSection[];
  choices?: ChoiceOption[];
  assignments?: VariableAssignment[];
  conditions?: ConditionRule[];
  image_url?: string;
  destination_storylet?: string;
  location_id?: string;
  is_terminal?: boolean;
  jump_to_storylet?: string;
}

export interface NarrativeEdge {
  id: string;
  from_node: string;
  from_port: string;
  to_node: string;
  to_port: string;
}

export interface NarrativeGraph {
  local_variables?: LocalVariableDefinition[];
  nodes: NarrativeNode[];
  edges: NarrativeEdge[];
}

type StoryletGraphSource = Pick<StoryletRecord, "graph" | "content" | "parameters" | "choices"> & {
  ending?: boolean;
  local_variables?: LocalVariableDefinition[];
};

export function buildGraphFromStorylet(st: StoryletGraphSource): NarrativeGraph {
  // If storylet already has a saved graph, reuse it!
  if (st.graph && typeof st.graph === "object" && Array.isArray((st.graph as NarrativeGraph).nodes)) {
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
  const speaker = st.content.speaker_id || (st.parameters[0]?.param_name ?? "Narrator");

  const initialLine: DialogueLineItem = {
    kind: "line",
    id: "line_0",
    speaker_role: speaker,
    target_role: "player",
    content: prose,
    emotion_tag: "neutral",
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
        edges.push({ id: `edge_choice_${idx}_${locationNodeId}`, from_node: "node_choice_1", from_port: `choice_${idx}`, to_node: locationNodeId, to_port: "input" });
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
    edges.push({ id: "edge_dialog_end", from_node: "node_dialog_1", from_port: "output", to_node: "node_end", to_port: "input" });
  } else {
    st.choices.forEach((choice, idx) => {
      if (typeof choice.effects?.["$move_player"] === "string") {
        edges.push({ id: `edge_location_${idx + 1}_end`, from_node: `node_location_${idx + 1}`, from_port: "output", to_node: "node_end", to_port: "input" });
      } else if (choice.divert_target) {
        edges.push({ id: `edge_teleport_${idx + 1}_end`, from_node: `node_teleport_${idx + 1}`, from_port: "output", to_node: "node_end", to_port: "input" });
      } else {
        edges.push({ id: `edge_choice_${idx}_end`, from_node: "node_choice_1", from_port: `choice_${idx}`, to_node: "node_end", to_port: "input" });
      }
    });
  }

  return {
    local_variables: st.local_variables ? [...st.local_variables] : [],
    nodes,
    edges,
  };
}

export function validateNarrativeGraph(graph: NarrativeGraph): string[] {
  const issues: string[] = [];
  if (!graph || !Array.isArray(graph.nodes) || !Array.isArray(graph.edges)) return ["Graph must contain node and edge arrays."];
  const allowedTypes = new Set<NodeType>(["start", "dialog", "choice", "variable", "conditional", "image", "teleport", "go_to_location", "end"]);
  if (graph.nodes.some((node) => !node || typeof node !== "object" || typeof node.id !== "string" || !node.id.trim() || !allowedTypes.has(node.type))) {
    return ["Graph nodes must have nonempty IDs and recognized types."];
  }
  for (const node of graph.nodes) {
    if (node.type === "go_to_location" && !node.location_id?.trim()) {
      issues.push("Go To Location nodes must select a location.");
    }
    if (node.type === "image" && !isSafeImageUrl(node.image_url)) {
      issues.push(`Image node ${node.title} must have a safe HTTP(S) URL or project-relative path.`);
    }
  }
  if (graph.edges.some((edge) => !edge || typeof edge !== "object" || typeof edge.id !== "string" || !edge.id.trim() || typeof edge.from_node !== "string" || typeof edge.from_port !== "string" || typeof edge.to_node !== "string" || typeof edge.to_port !== "string")) {
    return ["Graph edges must have nonempty IDs and valid endpoint fields."];
  }
  const starts = graph.nodes.filter((node) => node.type === "start");
  const ends = graph.nodes.filter((node) => node.type === "end");
  const ids = new Set(graph.nodes.map((node) => node.id));
  if (starts.length !== 1) issues.push("Graph must have exactly one Start node.");
  if (ends.length !== 1) issues.push("Graph must have exactly one End node.");
  if (ids.size !== graph.nodes.length) issues.push("Node IDs must be unique.");
  if (starts.length !== 1 || ends.length !== 1 || ids.size !== graph.nodes.length) return issues;
  const outgoing = new Map<string, string[]>(); const incoming = new Map<string, string[]>();
  const edgeIds = new Set<string>();
  for (const edge of graph.edges) {
    if (edgeIds.has(edge.id)) issues.push("Edge IDs must be unique.");
    edgeIds.add(edge.id);
    if (!ids.has(edge.from_node) || !ids.has(edge.to_node)) { issues.push(`Edge ${edge.id} references a missing node.`); continue; }
    if (edge.to_port !== "input") issues.push(`Edge ${edge.id} must target an input port.`);
    outgoing.set(edge.from_node, [...(outgoing.get(edge.from_node) ?? []), edge.to_node]);
    incoming.set(edge.to_node, [...(incoming.get(edge.to_node) ?? []), edge.from_node]);
  }
  const start = starts[0]!.id; const end = ends[0]!.id;
  if ((incoming.get(start)?.length ?? 0) || (outgoing.get(start)?.length ?? 0) !== 1) issues.push("Start must have no incoming edges and exactly one outgoing edge.");
  const walk = (seed: string, links: Map<string, string[]>): Set<string> => { const seen = new Set<string>(); const stack = [seed]; while (stack.length) { const id = stack.pop()!; if (seen.has(id)) continue; seen.add(id); stack.push(...(links.get(id) ?? [])); } return seen; };
  const reachable = walk(start, outgoing); const reachesEnd = walk(end, incoming);
  if (graph.nodes.some((node) => !reachable.has(node.id))) issues.push("Every node must be reachable from Start.");
  if (graph.nodes.some((node) => !reachesEnd.has(node.id))) issues.push("Every node must have a path to End.");
  for (const node of graph.nodes) {
    const edges = graph.edges.filter((edge) => edge.from_node === node.id);
    if (node.type === "choice") {
      const expectedPorts = new Set((node.choices ?? []).map((_, index) => `choice_${index}`));
      for (let index = 0; index < (node.choices?.length ?? 0); index++) {
        if (edges.filter((edge) => edge.from_port === `choice_${index}`).length !== 1) issues.push(`Choice ${index + 1} on ${node.title} must have exactly one outgoing branch.`);
      }
      if (edges.some((edge) => !expectedPorts.has(edge.from_port))) issues.push(`Choice node ${node.title} has an edge from an unknown output port.`);
    } else if (node.type === "go_to_location") {
      const incomingEdges = graph.edges.filter((edge) => edge.to_node === node.id);
      if (incomingEdges.some((edge) => graph.nodes.find((source) => source.id === edge.from_node)?.type !== "choice")) {
        issues.push(`Go To Location node ${node.title} may only be targeted directly by a Choice branch.`);
      }
      if (edges.length !== 1 || edges[0]?.from_port !== "output") {
        issues.push(`Go To Location node ${node.title} must have exactly one outgoing edge from its output port.`);
      }
    } else if (node.type === "end") {
      if (edges.length) issues.push("End cannot have outgoing edges.");
    } else if (edges.length !== 1 || edges[0]?.from_port !== "output") {
      issues.push(`${node.type} node ${node.title} must have exactly one outgoing edge from its output port.`);
    }
  }
  return issues;
}

export function compileGraphToStorylet(graph: NarrativeGraph, st: StoryletRecord) {
  const proseParts: string[] = [];
  const choices: StoryletChoiceRecord[] = [];
  let isEnding = false;
  const effects = { ...st.effects };

  // Sync local variables back to storylet
  if (graph.local_variables) {
    st.local_variables = [...graph.local_variables];
  }

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
    } else if (node.type === "variable" && node.assignments) {
      for (const a of node.assignments) {
        if (a.scope === "global") {
          effects[a.quality] = a.value;
        }
      }
    } else if (node.type === "end") {
      if (node.is_terminal || node.jump_to_storylet) {
        isEnding = true;
      }
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

/**
 * Jump to a storylet: copy all preconditions of target storylet into source storylet's effects!
 */
export function applyJumpPreconditions(targetStorylet: StoryletRecord, sourceStorylet: StoryletRecord): string[] {
  const updatedKeys: string[] = [];
  const preconditions = targetStorylet.preconditions?.all_of || [];

  for (const cond of preconditions) {
    if (!cond.variable) continue;
    const expected = cond.value ?? true;
    let effectValue: unknown;

    switch (cond.op) {
      case "eq":
      case "gte":
      case "lte":
        effectValue = expected;
        break;
      case "gt":
      case "lt":
        if (typeof expected !== "number" || !Number.isFinite(expected)) continue;
        effectValue = expected + (cond.op === "gt" ? 1 : -1);
        break;
      case "neq":
        if (typeof expected === "number" && Number.isFinite(expected)) effectValue = expected + 1;
        else if (typeof expected === "boolean") effectValue = !expected;
        else if (typeof expected === "string") effectValue = expected + "_other";
        else continue;
        break;
      default:
        continue;
    }

    sourceStorylet.effects[cond.variable] = effectValue;
    updatedKeys.push(cond.variable + " = " + JSON.stringify(effectValue));
  }

  return updatedKeys;
}

export type { LocalVariableDefinition } from "./contracts.ts";
