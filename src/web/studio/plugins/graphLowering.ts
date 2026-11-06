/**
 * Plugin nodes live only in the authoring graph. Before a storylet is saved
 * each one is lowered into the core nodes the engine executes; the authored
 * node rides along as metadata on the fragment's entry node so loading the
 * storylet raises it back for editing.
 */
import type { NarrativeEdge, NarrativeGraph, NarrativeNode } from "../narrativeGraphTypes.ts";
import { nodeTypeContribution, type AnyEntity, type LowerContext } from "./runtime.ts";

export interface PluginNodeMeta {
  group: string;
  /** Present only on the fragment's entry node. */
  source?: NarrativeNode;
  exit?: string;
}

export function isPluginNode(node: NarrativeNode): boolean { return node.type === "plugin"; }

function lowerContext(node: NarrativeNode, entities: AnyEntity[]): LowerContext {
  return {
    id: (suffix) => `${node.id}__${suffix}`,
    entity: (id) => entities.find((entity) => entity.id === id),
  };
}

export function validatePluginNodes(graph: NarrativeGraph, entities: AnyEntity[]): string[] {
  const issues: string[] = [];
  for (const node of graph.nodes.filter(isPluginNode)) {
    const contribution = nodeTypeContribution(node.plugin_type);
    if (!contribution) { issues.push(`Node ${node.title} needs plugin '${node.plugin_type}', which is not loaded.`); continue; }
    issues.push(...(contribution.validate?.(node.plugin_config ?? {}, lowerContext(node, entities)) ?? []));
  }
  return issues;
}

export function lowerPluginGraph(graph: NarrativeGraph, entities: AnyEntity[]): NarrativeGraph {
  const plugins = graph.nodes.filter(isPluginNode);
  if (!plugins.length) return graph;
  const nodes: NarrativeNode[] = graph.nodes.filter((node) => !isPluginNode(node));
  let edges: NarrativeEdge[] = [...graph.edges];
  for (const node of plugins) {
    const contribution = nodeTypeContribution(node.plugin_type);
    if (!contribution) throw new Error(`Plugin node type '${node.plugin_type}' is not loaded.`);
    const fragment = contribution.lower(node.plugin_config ?? {}, lowerContext(node, entities));
    fragment.nodes.forEach((member, index) => {
      const meta: PluginNodeMeta = member.id === fragment.entry
        ? { group: node.id, source: JSON.parse(JSON.stringify(node)), exit: fragment.exit }
        : { group: node.id };
      nodes.push({ ...member, x: node.x + index * 24, y: node.y + index * 24, plugin: meta });
    });
    edges = edges.map((edge) => {
      if (edge.to_node === node.id) return { ...edge, to_node: fragment.entry };
      if (edge.from_node === node.id) return { ...edge, from_node: fragment.exit, from_port: "output" };
      return edge;
    });
    edges.push(...fragment.edges);
  }
  return { ...graph, nodes, edges };
}

export function raisePluginGraph(graph: NarrativeGraph): NarrativeGraph {
  const groups = new Map<string, PluginNodeMeta>();
  for (const node of graph.nodes) {
    const meta = node.plugin as PluginNodeMeta | undefined;
    if (meta?.source) groups.set(meta.group, meta);
  }
  if (!groups.size) return graph;
  const groupOf = new Map<string, string>();
  for (const node of graph.nodes) {
    const meta = node.plugin as PluginNodeMeta | undefined;
    if (meta && groups.has(meta.group)) groupOf.set(node.id, meta.group);
  }
  const nodes = graph.nodes.filter((node) => !groupOf.has(node.id));
  for (const meta of groups.values()) nodes.push(meta.source!);
  const edges: NarrativeEdge[] = [];
  for (const edge of graph.edges) {
    const fromGroup = groupOf.get(edge.from_node);
    const toGroup = groupOf.get(edge.to_node);
    if (fromGroup && fromGroup === toGroup) continue;
    let next = edge;
    if (toGroup) next = { ...next, to_node: toGroup, to_port: "input" };
    if (fromGroup) {
      if (groups.get(fromGroup)!.exit !== edge.from_node) continue;
      next = { ...next, from_node: fromGroup, from_port: "output" };
    }
    edges.push(next);
  }
  return { ...graph, nodes, edges };
}
