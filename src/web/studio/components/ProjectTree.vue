<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import type { TreeRow } from "../projectTree.ts";
import { entityTypeColor } from "../entityModel.ts";
import { thumbnailUrl } from "../thumbnails.ts";
import type { ContentRootDefinition } from "../projectTree.ts";
import { useProjectTree } from "../view-models/projectTree.ts";
import { studioState } from "../state.ts";

const {
  roots,
  rows,
  query,
  activeKey,
  toggle,
  createFolder,
  renameFolder,
  deleteFolder,
  createItem,
  moveTo,
  dragPayload,
  openRow,
  activeDragRoot,
} = useProjectTree();

const menuKey = ref<string | null>(null);
const dropKey = ref<string | null>(null);
function canAcceptDrop(row: TreeRow): boolean {
  if (row.kind !== "folder" && row.kind !== "root") return false;
  if (!activeDragRoot.value || !row.root) return false;
  return row.root === activeDragRoot.value;
}
function rowGlyph(row: TreeRow): string {
  if (row.kind === "project") return "▣";
  if (row.kind === "root") return "▤";
  if (row.kind === "folder") return "▸";
  return "◆";
}

function rowColor(row: TreeRow): string {
  if (row.entityType) return entityTypeColor(row.entityType);
  return "var(--text-muted)";
}

function onRowClick(row: TreeRow) {
  menuKey.value = null;
  openRow(row);
}

function onDrop(event: DragEvent, row: TreeRow) {
  dropKey.value = null;
  if (row.kind !== "folder" && row.kind !== "root") return;
  const payload = dragPayload(event);
  if (!payload || !row.root) return;
  const targetFolder = row.kind === "folder" ? row.id ?? null : null;
  void moveTo(payload, row.root, targetFolder);
}

function rowDragPayload(event: DragEvent, row: TreeRow) {
  if (!row.root) return;
  if (row.kind !== "folder" && row.kind !== "item") return;
  activeDragRoot.value = row.root;
  event.dataTransfer?.setData(
    "application/x-dn-tree",
    JSON.stringify({
      kind: row.kind === "folder" ? "folder" : "item",
      root: row.root,
      id: row.id,
    }),
  );
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

function onDragEnd() {
  activeDragRoot.value = null;
  dropKey.value = null;
}

function folderRecord(row: TreeRow) {
  return studioState.folders.find((folder) => folder.id === row.id);
}

function menuDefinition(row: TreeRow): ContentRootDefinition | undefined {
  return roots.find((definition) => definition.root === row.root);
}

function toggleMenu(row: TreeRow) {
  menuKey.value = menuKey.value === row.key ? null : row.key;
}

function runMenu(row: TreeRow, action: "folder" | "item" | "rename" | "delete") {
  menuKey.value = null;
  const definition = menuDefinition(row);
  if (!definition) return;
  const folderId = row.kind === "folder" ? row.id ?? null : null;
  if (action === "folder") void createFolder(definition.root, folderId);
  else if (action === "item") void createItem(definition, folderId);
  else if (action === "rename") {
    const folder = folderRecord(row);
    if (folder) void renameFolder(folder);
  } else {
    const folder = folderRecord(row);
    if (folder) void deleteFolder(folder);
  }
}

function onDocumentClick(event: MouseEvent) {
  if (!menuKey.value) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest(".project-tree__menu") || target?.closest(".project-tree__menu-toggle")) return;
  menuKey.value = null;
}

function onDocumentKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") menuKey.value = null;
}

onMounted(() => {
  document.addEventListener("click", onDocumentClick);
  document.addEventListener("keydown", onDocumentKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("click", onDocumentClick);
  document.removeEventListener("keydown", onDocumentKeydown);
});
</script>

<template>
  <div class="project-tree">
    <div class="project-tree__search">
      <input v-model="query" class="form-input" placeholder="Search project..." />
    </div>

    <div class="project-tree__rows">
      <div
        v-for="row in rows"
        :key="row.key"
        :class="[
          'project-tree__row',
          `project-tree__row--${row.kind}`,
          activeKey === row.key ? 'is-active' : '',
          dropKey === row.key ? 'is-drop-target' : '',
        ]"
        :style="{ paddingLeft: `${8 + row.depth * 14}px` }"
        :draggable="row.kind === 'folder' || row.kind === 'item'"
        @click="onRowClick(row)"
        @dragstart="rowDragPayload($event, row)"
        @dragend="onDragEnd"
        @dragover.prevent="dropKey = canAcceptDrop(row) ? row.key : null"
        @dragleave="dropKey = dropKey === row.key ? null : dropKey"
        @drop.prevent="onDrop($event, row)"
      >
        <button
          v-if="row.hasChildren"
          class="project-tree__twisty"
          :aria-label="row.expanded ? `Collapse ${row.label}` : `Expand ${row.label}`"
          @click.stop="toggle(row.key)"
        >
          {{ row.expanded ? "▾" : "▸" }}
        </button>
        <span v-else class="project-tree__twisty project-tree__twisty--empty"></span>

        <img
          v-if="row.thumbnail && thumbnailUrl(row.thumbnail)"
          class="project-tree__thumb"
          :src="thumbnailUrl(row.thumbnail) ?? ''"
          :alt="row.label"
        />
        <span v-else class="project-tree__glyph" :style="{ color: rowColor(row) }">{{ rowGlyph(row) }}</span>

        <span class="project-tree__label" :title="row.detail ? `${row.label} — ${row.detail}` : row.label">{{ row.label }}</span>

        <span
          v-if="row.kind === 'folder' || row.kind === 'root'"
          class="project-tree__menu-toggle"
          role="button"
          tabindex="0"
          :aria-label="`Actions for ${row.label}`"
          @click.stop="toggleMenu(row)"
          @keydown.enter.stop="toggleMenu(row)"
        >+</span>

        <div v-if="menuKey === row.key" class="project-tree__menu" @click.stop>
          <button class="project-tree__menu-item" @click="runMenu(row, 'folder')">New Folder</button>
          <button class="project-tree__menu-item" @click="runMenu(row, 'item')">
            {{ menuDefinition(row)?.newLabel ?? "New Item" }}
          </button>
          <template v-if="row.kind === 'folder'">
            <button class="project-tree__menu-item" @click="runMenu(row, 'rename')">Rename</button>
            <button class="project-tree__menu-item project-tree__menu-item--danger" @click="runMenu(row, 'delete')">Delete</button>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>
