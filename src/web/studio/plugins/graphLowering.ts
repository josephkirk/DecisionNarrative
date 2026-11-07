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
  inputs?: NarrativeEdge[];
}

export function isPluginNode(node: NarrativeNode): boolean { return node.type === "plugin"; }

function lowerContext(node: NarrativeNode, graph: NarrativeGraph, entities: AnyEntity[]): LowerContext {
  return {
    id: (suffix) => `${node.id}__${suffix}`,
    entity: (id) => entities.find((entity) => entity.id === id),
    graph,
    node,
  };
}

export function validatePluginNodes(graph: NarrativeGraph, entities: AnyEntity[]): string[] {
  const issues: string[] = [];
  for (const node of graph.nodes.filter(isPluginNode)) {
    const contribution = nodeTypeContribution(node.plugin_type);
    if (!contribution) { issues.push(`Node ${node.title} needs plugin '${node.plugin_type}', which is not loaded.`); continue; }
    issues.push(...(contribution.validate?.(node.plugin_config ?? {}, lowerContext(node, graph, entities)) ?? []));
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
    const fragment = contribution.lower(node.plugin_config ?? {}, lowerContext(node, graph, entities));
    const inputs = edges.filter((edge) => edge.to_node === node.id && edge.kind === "data");
    fragment.nodes.forEach((member, index) => {
      const meta: PluginNodeMeta = member.id === fragment.entry
        ? { group: node.id, source: JSON.parse(JSON.stringify(node)), exit: fragment.exit, inputs }
        : { group: node.id };
      nodes.push({ ...member, x: node.x + index * 24, y: node.y + index * 24, plugin: meta });
    });
    edges = edges.flatMap((edge) => {
      if (edge.to_node === node.id && edge.kind === "data") {
        const targets = fragment.inputs?.[edge.to_port];
        if (!targets?.length) throw new Error(`Plugin node ${node.title} has no data input '${edge.to_port}'.`);
        return targets.map((target, index) => ({
          ...edge, id: index === 0 ? edge.id : `${edge.id}__${node.id}_${index}`, to_node: target.node, to_port: target.port,
        }));
      }
      if (edge.to_node === node.id) return [{ ...edge, to_node: fragment.entry }];
      if (edge.from_node === node.id) return [{ ...edge, from_node: fragment.exit, from_port: "output" }];
      return [edge];
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
    if (toGroup && !fromGroup && edge.kind === "data" && groups.get(toGroup)!.inputs) continue;
    let next = edge;
    if (toGroup) next = { ...next, to_node: toGroup, to_port: "input" };
    if (fromGroup) {
      if (groups.get(fromGroup)!.exit !== edge.from_node) continue;
      next = { ...next, from_node: fromGroup, from_port: "output" };
    }
    edges.push(next);
  }
  for (const meta of groups.values()) edges.push(...(meta.inputs ?? []));
  // Older canvas fragments owned an implicit Get Entity. Raise it as an
  // explicit graph input so existing authored versions remain editable.
  for (const [group, meta] of groups) {
    if (meta.inputs || meta.source?.plugin_type !== "canvas.modify") continue;
    const source = graph.nodes.find((node) => groupOf.get(node.id) === group && node.type === "get_entity" &&
      node.entity_path === meta.source?.plugin_config?.canvas_entity);
    const edge = source && graph.edges.find((item) => item.kind === "data" && item.from_node === source.id && groupOf.get(item.to_node) === group);
    if (!source || !edge) continue;
    const { plugin: _plugin, ...entityNode } = source;
    nodes.push(entityNode);
    edges.push({ ...edge, to_node: group, to_port: "entity" });
  }
  return { ...graph, nodes, edges };
}
