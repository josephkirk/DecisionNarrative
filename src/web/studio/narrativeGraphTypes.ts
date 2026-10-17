import type { StoryletRecord, StoryletChoiceRecord } from "./contracts.ts";

export type NodeType =
  | "start"
  | "dialog"
  | "choice"
  | "variable"
  | "conditional"
  | "image"
  | "teleport"
  | "end";

export interface ChoiceOption {
  id: string;
  text: string;
  system_verb?: string;
}

export interface VariableAssignment {
  quality: string;
  op: "set" | "add";
  value: string | number | boolean;
}

export interface ConditionRule {
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
  choices?: ChoiceOption[];
  assignments?: VariableAssignment[];
  conditions?: ConditionRule[];
  prompt?: string;
  image_url?: string;
  destination_storylet?: string;
  is_terminal?: boolean;
}

export interface NarrativeEdge {
  id: string;
  from_node: string;
  from_port: string;
  to_node: string;
  to_port: string;
}

export interface NarrativeGraph {
  nodes: NarrativeNode[];
  edges: NarrativeEdge[];
}

export function buildGraphFromStorylet(st: StoryletRecord): NarrativeGraph {
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
  nodes.push({
    id: "node_dialog_1",
    type: "dialog",
    title: "Dialog",
    x: 290,
    y: 120,
    width: 280,
    height: 180,
    speaker_role: st.content.speaker_id || "Narrator",
    text: prose,
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
    }));

    nodes.push({
      id: "node_choice_1",
      type: "choice",
      title: "Choice",
      x: 630,
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
      if (c.divert_target) {
        const teleId = `node_teleport_${idx + 1}`;
        nodes.push({
          id: teleId,
          type: "teleport",
          title: "Teleport",
          x: 950,
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
  } else if (st.ending) {
    nodes.push({
      id: "node_end",
      type: "end",
      title: "End",
      x: 630,
      y: 120,
      width: 150,
      height: 80,
      is_terminal: true,
    });

    edges.push({
      id: "edge_dialog_end",
      from_node: "node_dialog_1",
      from_port: "output",
      to_node: "node_end",
      to_port: "input",
    });
  }

  return { nodes, edges };
}

export function compileGraphToStorylet(graph: NarrativeGraph, st: StoryletRecord) {
  const proseParts: string[] = [];
  const choices: StoryletChoiceRecord[] = [];
  let isEnding = false;

  for (const node of graph.nodes) {
    if (node.type === "dialog" && node.text?.trim()) {
      proseParts.push(node.text.trim());
      if (node.speaker_role) {
        st.content.speaker_id = node.speaker_role;
      }
    } else if (node.type === "choice" && node.choices?.length) {
      node.choices.forEach((opt, idx) => {
        const portId = `choice_${idx}`;
        const edge = graph.edges.find(
          (e) => e.from_node === node.id && (e.from_port === portId || e.from_port === "output")
        );

        let divertTarget: string | undefined;
        if (edge) {
          const targetNode = graph.nodes.find((n) => n.id === edge.to_node);
          if (targetNode?.type === "teleport" && targetNode.destination_storylet) {
            divertTarget = targetNode.destination_storylet;
          } else if (targetNode?.type === "end") {
            isEnding = true;
          }
        }

        choices.push({
          choice_id: opt.id || `choice_${idx + 1}`,
          label: opt.text,
          systemVerb: opt.system_verb || "ACT",
          effects: {},
          divert_target: divertTarget,
        });
      });
    } else if (node.type === "end" && node.is_terminal) {
      isEnding = true;
    }
  }

  if (proseParts.length > 0) {
    const fullProse = proseParts.join("\n\n");
    st.content.sceneOpening = fullProse;
    st.content.text_template = fullProse;
  }

  if (choices.length > 0) {
    st.choices = choices;
  }

  if (isEnding) {
    st.ending = true;
  }
}
