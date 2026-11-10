<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import type { TreeRow } from "../projectTree.ts";
import { itemRoute } from "../projectTree.ts";
import { thumbnailUrl } from "../thumbnails.ts";
import type { ContentRootDefinition } from "../projectTree.ts";
import { useProjectTree } from "../view-models/projectTree.ts";
import { refreshWorkspace } from "../view-models/workspace.ts";
import { canSplit, focusedRouter as router, openSplit, paneState } from "../view-models/panes.ts";
import { studioState } from "../state.ts";
import StudioIcon from "./StudioIcon.vue";
import SearchField from "./ui/SearchField.vue";

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
  createNewProject,
  deleteItem,
  activeDragRoot,
} = useProjectTree();

const menuKey = ref<string | null>(null);
const dropKey = ref<string | null>(null);

interface ContextMenuState {
  x: number;
  y: number;
  row: TreeRow | null;
}
const contextMenu = ref<ContextMenuState | null>(null);

function canAcceptDrop(row: TreeRow): boolean {
  if (row.kind !== "folder" && row.kind !== "root") return false;
  if (!activeDragRoot.value || !row.root) return false;
  return row.root === activeDragRoot.value;
}

function rowIcon(row: TreeRow): string {
  if (row.root === "entities" && row.id === "world") return "globe";
  if (row.kind === "project") return "project";
  if (row.kind === "folder") return "folder";
  return row.root ?? "file";
}

function rowPath(row: TreeRow): string | null {
  if (!row.root || !row.id) return null;
  if (row.kind === "folder") return `/browse/${row.root}/${encodeURIComponent(row.id)}`;
  return row.kind === "item" ? itemRoute(row.root, row.id) : null;
}

function onRowClick(row: TreeRow) {
  menuKey.value = null;
  contextMenu.value = null;
  openRow(row);
}

function openContextMenu(event: MouseEvent, row: TreeRow | null) {
  event.preventDefault();
  event.stopPropagation();
  menuKey.value = null;
  const menuWidth = 200;
  const menuHeight = 260;
  const x = Math.min(event.clientX, Math.max(10, window.innerWidth - menuWidth));
  const y = Math.min(event.clientY, Math.max(10, window.innerHeight - menuHeight));
  contextMenu.value = { x, y, row };
}

function closeContextMenu() {
  contextMenu.value = null;
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
  // Copy lets panes take the row as "open here" or a split; move keeps folder drops working.
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "copyMove";
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
  contextMenu.value = null;
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

function contextAction(action: string) {
  const row = contextMenu.value?.row;
  closeContextMenu();

  if (action === "new-project") {
    void createNewProject();
    return;
  }
  if (action === "refresh") {
    void refreshWorkspace();
    return;
  }
  if (action === "project-settings") {
    router.push("/project");
    return;
  }
  if (action === "tags") {
    router.push("/tags");
    return;
  }
  if (action === "storylets") {
    router.push("/storylets");
    return;
  }
  if (action === "codex") {
    router.push("/codex");
    return;
  }

  if (!row) return;

  if (action === "open") {
    openRow(row);
    return;
  }
  if (action === "split") {
    const path = rowPath(row);
    if (path) openSplit(paneState.activeId, "right", path);
    return;
  }
  if (action === "toggle-expand") {
    toggle(row.key);
    return;
  }
  if (action === "delete-item") {
    void deleteItem(row);
    return;
  }

  const definition = menuDefinition(row);
  if (!definition) return;
  const folderId = row.kind === "folder" ? row.id ?? null : null;

  if (action === "folder") {
    void createFolder(definition.root, folderId);
  } else if (action === "item") {
    void createItem(definition, folderId);
  } else if (action === "rename") {
    const folder = folderRecord(row);
    if (folder) void renameFolder(folder);
  } else if (action === "delete-folder") {
    const folder = folderRecord(row);
    if (folder) void deleteFolder(folder);
  }
}

function onDocumentClick(event: MouseEvent) {
  const target = event.target as HTMLElement | null;
  if (!target?.closest(".project-tree__context-menu")) {
    contextMenu.value = null;
  }
  if (!target?.closest(".project-tree__menu") && !target?.closest(".project-tree__menu-toggle")) {
    menuKey.value = null;
  }
}

function onDocumentKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    menuKey.value = null;
    contextMenu.value = null;
  }
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
      <SearchField v-model="query" input-id="tree-search-input" label="Search project" placeholder="Search project…" />
    </div>

    <div class="project-tree__rows" @contextmenu.prevent="openContextMenu($event, null)">
      <div
        v-for="row in rows"
        :key="row.key"
        :class="[
          'project-tree__row',
          `project-tree__row--${row.kind}`,
          activeKey === row.key ? 'is-active' : '',
          dropKey === row.key ? 'is-drop-target' : '',
        ]"
        :style="{ paddingLeft: `${6 + row.depth * 14}px` }"
        :draggable="row.kind === 'folder' || row.kind === 'item'"
        :title="row.kind === 'item' ? 'Drag onto a pane edge to open it in a split view' : undefined"
        tabindex="0"
        :aria-current="activeKey === row.key ? 'page' : undefined"
        @click="onRowClick(row)"
        @keydown.enter.self.prevent="onRowClick(row)"
        @contextmenu.prevent.stop="openContextMenu($event, row)"
        @dragstart="rowDragPayload($event, row)"
        @dragend="onDragEnd"
        @dragover.prevent="dropKey = canAcceptDrop(row) ? row.key : null"
        @dragleave="dropKey = dropKey === row.key ? null : dropKey"
        @drop.prevent="onDrop($event, row)"
      >
        <button
          v-if="row.hasChildren"
          type="button"
          class="project-tree__twisty"
          :class="{ 'is-open': row.expanded }"
          :aria-label="row.expanded ? `Collapse ${row.label}` : `Expand ${row.label}`"
          :aria-expanded="row.expanded"
          @click.stop="toggle(row.key)"
        >
          <StudioIcon name="chevron" />
        </button>
        <span v-else class="project-tree__twisty project-tree__twisty--empty"></span>

        <img
          v-if="row.thumbnail && thumbnailUrl(row.thumbnail)"
          class="project-tree__thumb"
          :src="thumbnailUrl(row.thumbnail) ?? ''"
          alt=""
        />
        <span v-else class="project-tree__glyph" :class="`is-${row.kind}`"><StudioIcon :name="rowIcon(row)" /></span>

        <span class="project-tree__label" :title="row.detail ? `${row.label} — ${row.detail}` : row.label">{{ row.label }}</span>

        <button
          v-if="row.kind === 'folder' || (row.kind === 'root' && menuDefinition(row)?.newLabel)"
          type="button"
          class="project-tree__menu-toggle"
          aria-haspopup="menu"
          :aria-expanded="menuKey === row.key"
          :aria-label="`Actions for ${row.label}`"
          @click.stop="toggleMenu(row)"
          @keydown.enter.stop="toggleMenu(row)"
          @keydown.space.stop.prevent="toggleMenu(row)"
        ><StudioIcon name="plus" /></button>

        <div v-if="menuKey === row.key" class="project-tree__menu" role="menu" @click.stop>
          <template v-if="menuDefinition(row)?.newLabel">
            <button type="button" class="project-tree__menu-item" role="menuitem" @click="runMenu(row, 'item')">
              <StudioIcon :name="row.root ?? 'file'" /> {{ menuDefinition(row)?.newLabel ?? "New item" }}
            </button>
            <button type="button" class="project-tree__menu-item" role="menuitem" @click="runMenu(row, 'folder')"><StudioIcon name="folder" /> New folder</button>
          </template>
          <template v-if="row.kind === 'folder'">
            <button type="button" class="project-tree__menu-item" role="menuitem" @click="runMenu(row, 'rename')"><StudioIcon name="pencil" /> Rename</button>
            <button type="button" class="project-tree__menu-item project-tree__menu-item--danger" role="menuitem" @click="runMenu(row, 'delete')"><StudioIcon name="trash" /> Delete</button>
          </template>
        </div>
      </div>
    </div>

    <!-- Floating Context Menu -->
    <div
      v-if="contextMenu"
      class="project-tree__context-menu"
      role="menu"
      :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
      @click.stop
    >
      <!-- Row: Project or empty background -->
      <template v-if="!contextMenu.row || contextMenu.row.kind === 'project'">
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('new-project')"><StudioIcon name="file" /> New project…</button>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('refresh')"><StudioIcon name="refresh" /> Refresh workspace</button>
        <div class="project-tree__context-menu-divider" role="separator"></div>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('project-settings')"><StudioIcon name="settings" /> Project & export</button>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('tags')"><StudioIcon name="tag" /> Tag manager</button>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('storylets')"><StudioIcon name="storylets" /> Storylet library</button>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('codex')"><StudioIcon name="scroll" /> Rules & events</button>
      </template>

      <!-- Row: Root -->
      <template v-else-if="contextMenu.row.kind === 'root'">
        <template v-if="menuDefinition(contextMenu.row)?.newLabel">
          <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('item')"><StudioIcon name="plus" /> {{ menuDefinition(contextMenu.row)?.newLabel ?? "New item" }}</button>
          <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('folder')"><StudioIcon name="folder" /> New folder…</button>
          <div class="project-tree__context-menu-divider" role="separator"></div>
        </template>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('open')"><StudioIcon name="search" /> Browse {{ contextMenu.row.label }}</button>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('toggle-expand')"><StudioIcon name="chevron" /> {{ contextMenu.row.expanded ? 'Collapse' : 'Expand' }}</button>
      </template>

      <!-- Row: Folder -->
      <template v-else-if="contextMenu.row.kind === 'folder'">
        <template v-if="menuDefinition(contextMenu.row)?.newLabel">
          <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('item')"><StudioIcon name="plus" /> {{ menuDefinition(contextMenu.row)?.newLabel ?? "New item" }}</button>
          <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('folder')"><StudioIcon name="folder" /> New subfolder…</button>
        </template>
        <div class="project-tree__context-menu-divider" role="separator"></div>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('open')"><StudioIcon name="search" /> Open folder view</button>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" :disabled="!canSplit()" @click="contextAction('split')"><StudioIcon name="split-right" /> Open in split view</button>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('rename')"><StudioIcon name="pencil" /> Rename…</button>
        <div class="project-tree__context-menu-divider" role="separator"></div>
        <button type="button" role="menuitem" class="project-tree__context-menu-item project-tree__context-menu-item--danger" @click="contextAction('delete-folder')"><StudioIcon name="trash" /> Delete folder</button>
      </template>

      <!-- Row: Item -->
      <template v-else-if="contextMenu.row.kind === 'item'">
        <button type="button" role="menuitem" class="project-tree__context-menu-item" @click="contextAction('open')"><StudioIcon name="open" /> Open</button>
        <button type="button" role="menuitem" class="project-tree__context-menu-item" :disabled="!canSplit()" @click="contextAction('split')"><StudioIcon name="split-right" /> Open in split view</button>
        <template v-if="contextMenu.row.root !== 'entities' || contextMenu.row.id !== 'world'">
          <div class="project-tree__context-menu-divider" role="separator"></div>
          <button type="button" role="menuitem" class="project-tree__context-menu-item project-tree__context-menu-item--danger" @click="contextAction('delete-item')"><StudioIcon name="trash" /> Delete</button>
        </template>
      </template>
    </div>
  </div>
</template>
