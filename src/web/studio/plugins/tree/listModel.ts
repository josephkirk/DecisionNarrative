import type { TreeEdge, TreeGraph } from "./model.ts";

export interface TreeListRow {
  key: string;
  nodeId: string;
  depth: number;
  edge?: TreeEdge;
  reference: boolean;
  hasChildren: boolean;
}

interface StackFrame {
  nodeId: string;
  depth: number;
  edgeIndex: number;
}

export function treeListRows(
  graph: TreeGraph,
  collapsed: ReadonlySet<string>,
): TreeListRow[] {
  const nodes = graph.nodes ?? [];
  const edges = graph.edges ?? [];
  const nodeSet = new Set(nodes);

  // Index valid outgoing edges in original graph.edges order
  const outgoing = new Map<string, TreeEdge[]>();
  for (const node of nodes) {
    outgoing.set(node, []);
  }

  const targets = new Set<string>();
  for (const edge of edges) {
    if (nodeSet.has(edge.from) && nodeSet.has(edge.to)) {
      outgoing.get(edge.from)?.push(edge);
      targets.add(edge.to);
    }
  }

  // Root nodes preserve graph.nodes order
  const roots = nodes.filter((node) => !targets.has(node));

  const seen = new Set<string>();
  const edgeOccurrences = new Map<string, number>();

  function nodeKey(nodeId: string): string {
    return `node:${nodeId}`;
  }

  function nextEdgeKey(from: string, to: string): string {
    const pair = JSON.stringify([from, to]);
    const count = edgeOccurrences.get(pair) ?? 0;
    edgeOccurrences.set(pair, count + 1);
    return count === 0 ? `edge:${pair}` : `edge:${pair}#${count}`;
  }

  const fullForest: TreeListRow[] = [];

  function traverseCanonicalNode(rootId: string) {
    seen.add(rootId);
    const rootEdges = outgoing.get(rootId) ?? [];
    fullForest.push({
      key: nodeKey(rootId),
      nodeId: rootId,
      depth: 0,
      reference: false,
      hasChildren: rootEdges.length > 0,
    });

    const stack: StackFrame[] = [
      {
        nodeId: rootId,
        depth: 0,
        edgeIndex: 0,
      },
    ];

    while (stack.length > 0) {
      const top = stack[stack.length - 1]!;
      const currentEdges = outgoing.get(top.nodeId) ?? [];

      if (top.edgeIndex >= currentEdges.length) {
        stack.pop();
        continue;
      }

      const edge = currentEdges[top.edgeIndex]!;
      top.edgeIndex++;

      const targetId = edge.to;
      const childDepth = top.depth + 1;

      if (!seen.has(targetId)) {
        seen.add(targetId);
        const targetEdges = outgoing.get(targetId) ?? [];
        fullForest.push({
          key: nodeKey(targetId),
          nodeId: targetId,
          depth: childDepth,
          edge,
          reference: false,
          hasChildren: targetEdges.length > 0,
        });

        stack.push({
          nodeId: targetId,
          depth: childDepth,
          edgeIndex: 0,
        });
      } else {
        fullForest.push({
          key: nextEdgeKey(edge.from, edge.to),
          nodeId: targetId,
          depth: childDepth,
          edge,
          reference: true,
          hasChildren: false,
        });
      }
    }
  }

  // 1. Traverse all true roots in graph.nodes order
  for (const root of roots) {
    if (!seen.has(root)) {
      traverseCanonicalNode(root);
    }
  }

  // 2. Traverse any unseen nodes in rootless components in graph.nodes order
  for (const node of nodes) {
    if (!seen.has(node)) {
      traverseCanonicalNode(node);
    }
  }

  // 3. Filter collapsed canonical rows without incorrectly promoting descendants to roots
  const result: TreeListRow[] = [];
  let hideUntilDepth: number | null = null;

  for (const row of fullForest) {
    if (hideUntilDepth !== null) {
      if (row.depth > hideUntilDepth) {
        continue;
      }
      hideUntilDepth = null;
    }

    result.push(row);

    if (!row.reference && collapsed.has(row.nodeId)) {
      hideUntilDepth = row.depth;
    }
  }

  return result;
}
