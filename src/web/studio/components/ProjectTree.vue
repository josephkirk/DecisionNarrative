<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import type { TreeRow } from "../projectTree.ts";
import { rootTagColor } from "../entityModel.ts";
import { thumbnailUrl } from "../thumbnails.ts";
import type { ContentRootDefinition } from "../projectTree.ts";
import { useProjectTree } from "../view-models/projectTree.ts";
import { refreshWorkspace } from "../view-models/workspace.ts";
import { studioState } from "../state.ts";

const router = useRouter();

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

function rowGlyph(row: TreeRow): string {
  if (row.root === "entities" && row.id === "world") return "🌐";
  if (row.kind === "project") return "▣";
  if (row.kind === "root") return "▤";
  if (row.kind === "folder") return "▸";
  return "◆";
}

function rowColor(row: TreeRow): string {
  if (row.rootTag) return rootTagColor(row.rootTag);
  return "var(--text-muted)";
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
      <label for="tree-search-input" class="sr-only">Search project</label>
      <input
        id="tree-search-input"
        v-model="query"
        class="form-input"
        placeholder="Search project…"
        aria-label="Search project"
      />
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
        :style="{ paddingLeft: `${8 + row.depth * 14}px` }"
        :draggable="row.kind === 'folder' || row.kind === 'item'"
        @click="onRowClick(row)"
        @contextmenu.prevent.stop="openContextMenu($event, row)"
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
        >+</button>

        <div v-if="menuKey === row.key" class="project-tree__menu" role="menu" @click.stop>
          <template v-if="menuDefinition(row)?.newLabel">
            <button class="project-tree__menu-item" @click="runMenu(row, 'folder')">New folder</button>
            <button class="project-tree__menu-item" @click="runMenu(row, 'item')">
              {{ menuDefinition(row)?.newLabel ?? "New item" }}
            </button>
          </template>
          <template v-if="row.kind === 'folder'">
            <button class="project-tree__menu-item" @click="runMenu(row, 'rename')">Rename</button>
            <button class="project-tree__menu-item project-tree__menu-item--danger" @click="runMenu(row, 'delete')">Delete</button>
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
        <button class="project-tree__context-menu-item" @click="contextAction('new-project')">
          <span>📄</span> New project…
        </button>
        <button class="project-tree__context-menu-item" @click="contextAction('refresh')">
          <span>🔄</span> Refresh workspace
        </button>
        <div class="project-tree__context-menu-divider"></div>
        <button class="project-tree__context-menu-item" @click="contextAction('project-settings')">
          <span>⚙️</span> Project & export
        </button>
        <button class="project-tree__context-menu-item" @click="contextAction('tags')">
          <span>🏷️</span> Tag manager
        </button>
        <button class="project-tree__context-menu-item" @click="contextAction('storylets')">
          <span>📚</span> Storylet library
        </button>
        <button class="project-tree__context-menu-item" @click="contextAction('codex')">
          <span>📜</span> Rules & events
        </button>
      </template>

      <!-- Row: Root -->
      <template v-else-if="contextMenu.row.kind === 'root'">
        <template v-if="menuDefinition(contextMenu.row)?.newLabel">
          <button class="project-tree__context-menu-item" @click="contextAction('item')">
            <span>➕</span> {{ menuDefinition(contextMenu.row)?.newLabel ?? "New item" }}
          </button>
          <button class="project-tree__context-menu-item" @click="contextAction('folder')">
            <span>📁</span> New folder…
          </button>
          <div class="project-tree__context-menu-divider"></div>
        </template>
        <button class="project-tree__context-menu-item" @click="contextAction('open')">
          <span>🔍</span> Browse {{ contextMenu.row.label }}
        </button>
        <button class="project-tree__context-menu-item" @click="contextAction('toggle-expand')">
          <span>{{ contextMenu.row.expanded ? '▾' : '▸' }}</span> {{ contextMenu.row.expanded ? 'Collapse' : 'Expand' }}
        </button>
      </template>

      <!-- Row: Folder -->
      <template v-else-if="contextMenu.row.kind === 'folder'">
        <template v-if="menuDefinition(contextMenu.row)?.newLabel">
          <button class="project-tree__context-menu-item" @click="contextAction('item')">
            <span>➕</span> {{ menuDefinition(contextMenu.row)?.newLabel ?? "New item" }}
          </button>
          <button class="project-tree__context-menu-item" @click="contextAction('folder')">
            <span>📁</span> New subfolder…
          </button>
        </template>
        <div class="project-tree__context-menu-divider"></div>
        <button class="project-tree__context-menu-item" @click="contextAction('open')">
          <span>🔍</span> Open folder view
        </button>
        <button class="project-tree__context-menu-item" @click="contextAction('rename')">
          <span>✏️</span> Rename…
        </button>
        <div class="project-tree__context-menu-divider"></div>
        <button class="project-tree__context-menu-item project-tree__context-menu-item--danger" @click="contextAction('delete-folder')">
          <span>🗑️</span> Delete folder
        </button>
      </template>

      <!-- Row: Item -->
      <template v-else-if="contextMenu.row.kind === 'item'">
        <button class="project-tree__context-menu-item" @click="contextAction('open')">
          <span>👁️</span> Open
        </button>
        <template v-if="contextMenu.row.root !== 'entities' || contextMenu.row.id !== 'world'">
          <div class="project-tree__context-menu-divider"></div>
          <button class="project-tree__context-menu-item project-tree__context-menu-item--danger" @click="contextAction('delete-item')">
            <span>🗑️</span> Delete
          </button>
        </template>
      </template>
    </div>
  </div>
</template>
