<script setup lang="ts">
import type { HierarchicalTagNode } from "../tagData.ts";

const props = defineProps<{
  node: HierarchicalTagNode;
  depth: number;
  expandedPaths: Set<string>;
  selectedPath: string;
}>();
const hasChildren = () => props.node.children.length > 0;
function activate() {
  emit("select", props.node.fullPath);
  if (hasChildren()) emit("toggle", props.node.fullPath);
}
function onKeydown(event: KeyboardEvent) {
  if (event.target instanceof HTMLElement && event.target !== event.currentTarget) return;
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    activate();
  }
}

const emit = defineEmits<{
  (e: "toggle", path: string): void;
  (e: "select", path: string): void;
  (e: "add", path: string): void;
  (e: "rename" | "delete", tag: { name: string; references: import("../tagData.ts").TagReference[] }): void;
  (e: "open", ref: { entityType: string; entityId: string; nodeId?: string; itemId?: string }): void;
}>();
</script>

<template>
  <div class="tag-tree-node-wrap">
    <div
      :class="['tag-tree-row', depth === 0 ? 'root-level' : '', selectedPath === node.fullPath ? 'selected' : '']"
      :style="{ paddingLeft: `${12 + depth * 20}px` }"
      role="treeitem"
      tabindex="0"
      :aria-expanded="node.children.length ? expandedPaths.has(node.fullPath) : undefined"
      :aria-selected="selectedPath === node.fullPath"
      @click="activate"
      @keydown="onKeydown"
    >
      <span class="tag-tree-twisty" :aria-hidden="!node.children.length">
        {{ node.children.length ? (expandedPaths.has(node.fullPath) ? '▾' : '▸') : '' }}
      </span>
      <span :class="['tag-tree-label', depth === 0 ? 'root-name' : 'leaf']">{{ node.segment }}</span>
      <span v-if="depth > 0" class="tag-tree-badge">{{ node.fullPath }}</span>
      <span class="tag-tree-count">{{ node.totalCount }}</span>
      <button type="button" class="btn btn-secondary tag-tree-action" :disabled="!node.entry?.registered" :aria-label="`Add child tag under ${node.fullPath}`" @click.stop="node.entry?.registered && emit('add', node.fullPath)">Add</button>
      <template v-if="node.entry?.registered && !node.entry.protected && !node.children.length">
        <button type="button" class="btn btn-secondary tag-tree-action" :disabled="node.entry.references.length > 0" :title="node.entry.references.length ? 'Remove references before renaming this tag.' : undefined" :aria-label="`Rename ${node.fullPath}`" @click.stop="emit('rename', node.entry!)">Rename</button>
        <button type="button" class="btn btn-secondary tag-tree-action" :disabled="node.entry.references.length > 0" :title="node.entry.references.length ? 'Remove references before deleting this tag.' : undefined" :aria-label="`Delete ${node.fullPath}`" @click.stop="emit('delete', node.entry!)">Delete</button>
      </template>
    </div>

    <!-- References for this node if expanded -->
    <div
      v-if="node.entry && (!node.children.length || expandedPaths.has(node.fullPath))"
      class="tag-tree-refs"
      :style="{ paddingLeft: `${32 + depth * 20}px` }"
    >
      <div
        v-for="ref in node.entry.references"
        :key="`${ref.entityType}:${ref.entityId}`"
        class="tag-tree-ref-item"
      >
        <span><strong>{{ ref.entityLabel }}</strong> ({{ ref.detail }})</span>
        <button
          type="button"
          class="btn btn-secondary"
          style="font-size:11px; padding:2px 6px;"
          @click.stop="emit('open', ref)"
        >
          Open
        </button>
      </div>
    </div>

    <!-- Recursive Children -->
    <template v-if="expandedPaths.has(node.fullPath)">
      <TagTreeNode
        v-for="child in node.children"
        :key="child.fullPath"
        :node="child"
        :depth="depth + 1"
        :expanded-paths="expandedPaths"
        :selected-path="selectedPath"
        @toggle="emit('toggle', $event)"
        @select="emit('select', $event)"
        @add="emit('add', $event)"
        @rename="emit('rename', $event)"
        @delete="emit('delete', $event)"
        @open="emit('open', $event)"
      />
    </template>
  </div>
</template>

<style scoped>
.tag-tree-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 6px;
  cursor: pointer;
  user-select: none;
}
.tag-tree-row:hover {
  background: var(--bg-card-hover, rgba(255, 255, 255, 0.05));
}
.tag-tree-row.root-level {
  font-weight: 700;
  font-size: 14px;
  background: var(--bg-surface, rgba(0, 0, 0, 0.2));
  margin-top: 4px;
}
.tag-tree-row.selected {
  background: var(--accent-subtle);
  outline: 1px solid var(--accent);
}
.tag-tree-row:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}
.tag-tree-twisty {
  width: 14px;
  text-align: center;
  color: var(--text-muted);
  font-size: 11px;
}
.tag-tree-label {
  font-size: 13px;
  font-weight: 600;
}
.tag-tree-label.root-name {
  color: var(--accent, #38bdf8);
}
.tag-tree-label.leaf {
  color: var(--text-main);
}
.tag-tree-badge {
  font-family: var(--font-mono, monospace);
  font-size: 11px;
  color: var(--text-muted);
  background: var(--bg-surface);
  padding: 2px 6px;
  border-radius: 4px;
}
.tag-tree-count {
  margin-left: auto;
  font-size: 11px;
  color: var(--text-muted);
}
.tag-tree-refs {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 4px 0 8px;
}
.tag-tree-ref-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 10px;
  background: var(--bg-surface);
  border-radius: 4px;
  font-size: 12px;
}
</style>
