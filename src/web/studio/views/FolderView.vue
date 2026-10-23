<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { studioState } from "../state.ts";
import {
  buildTreeItems,
  folderBreadcrumb,
  folderContents,
  itemRoute,
  rootDefinition,
  type TreeItem,
} from "../projectTree.ts";
import { entityTypeColor } from "../entityModel.ts";
import { thumbnailInitial, thumbnailUrl } from "../thumbnails.ts";
import { useProjectTree } from "../view-models/projectTree.ts";

const route = useRoute();
const router = useRouter();
const { createFolder, createItem, moveTo, dragPayload, activeDragRoot } = useProjectTree();

const root = computed(() => (typeof route.params.root === "string" ? route.params.root : ""));
const folderId = computed(() => (typeof route.params.folderId === "string" ? route.params.folderId : null));
const definition = computed(() => rootDefinition(root.value));
const breadcrumb = computed(() => folderBreadcrumb(studioState.folders, folderId.value));

const source = computed(() =>
  buildTreeItems({
    projectName: studioState.project?.name ?? "Workspace",
    folders: studioState.folders,
    qualities: studioState.qualities,
    entities: studioState.entities,
    traitTemplates: studioState.traitTemplates,
    assets: studioState.assets,
    storylets: studioState.storylets,
  }),
);

const contents = computed(() =>
  folderContents(source.value.folders, source.value.items, root.value, folderId.value),
);

const VIEW_MODE_KEY = "dn.studio.folderView.";

function viewModeKey(): string {
  return `${VIEW_MODE_KEY}${root.value}:${folderId.value ?? "root"}`;
}

function readViewMode(): "grid" | "list" {
  try {
    return localStorage.getItem(viewModeKey()) === "list" ? "list" : "grid";
  } catch {
    return "grid";
  }
}

const viewMode = ref<"grid" | "list">(readViewMode());

watch([root, folderId], () => {
  viewMode.value = readViewMode();
});

watch(viewMode, (value) => {
  try {
    localStorage.setItem(viewModeKey(), value);
  } catch {
    // Persisting the view mode is best effort.
  }
});

function openFolder(id: string) {
  router.push(`/browse/${root.value}/${id}`);
}

function openItem(item: TreeItem) {
  router.push(itemRoute(item.root, item.id));
}

function tileColor(item: TreeItem): string {
  return item.entityType ? entityTypeColor(item.entityType) : "var(--accent)";
}

function onDrop(event: DragEvent) {
  const payload = dragPayload(event);
  void moveTo(payload, root.value, folderId.value);
}

function onTileDrop(event: DragEvent, folder: string) {
  event.stopPropagation();
  const payload = dragPayload(event);
  void moveTo(payload, root.value, folder);
}

function onDragEnd() {
  activeDragRoot.value = null;
}

function itemDrag(event: DragEvent, item: TreeItem) {
  activeDragRoot.value = item.root;
  event.dataTransfer?.setData(
    "application/x-dn-tree",
    JSON.stringify({ kind: "item", root: item.root, id: item.id }),
  );
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}

function folderDrag(event: DragEvent, id: string) {
  activeDragRoot.value = root.value;
  event.dataTransfer?.setData(
    "application/x-dn-tree",
    JSON.stringify({ kind: "folder", root: root.value, id }),
  );
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
}
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <div class="folder-breadcrumb">
        <router-link to="/project" class="folder-breadcrumb__link">{{ studioState.project?.name ?? "Workspace" }}</router-link>
        <template v-if="definition">
          <span class="folder-breadcrumb__sep">›</span>
          <router-link :to="`/browse/${root}`" class="folder-breadcrumb__link">{{ definition.label }}</router-link>
        </template>
        <template v-for="folder in breadcrumb" :key="folder.id">
          <span class="folder-breadcrumb__sep">›</span>
          <router-link :to="`/browse/${root}/${folder.id}`" class="folder-breadcrumb__link">{{ folder.name }}</router-link>
        </template>
      </div>

      <div v-if="definition" style="display:flex; align-items:center; gap:8px;">
        <div class="view-mode-toggle">
          <button :class="['tab-btn', viewMode === 'grid' ? 'active' : '']" @click="viewMode = 'grid'">Grid</button>
          <button :class="['tab-btn', viewMode === 'list' ? 'active' : '']" @click="viewMode = 'list'">List</button>
        </div>
        <button class="btn btn-secondary" @click="createFolder(root, folderId)">+ New Folder</button>
        <button class="btn btn-primary" @click="createItem(definition, folderId)">+ {{ definition.newLabel }}</button>
      </div>
    </header>

    <div
      v-if="!definition"
      class="view-content"
      style="padding:60px; text-align:center; color:var(--text-muted);"
    >
      Unknown folder
    </div>

    <div
      v-else
      class="view-content"
      @dragover.prevent
      @drop.prevent="onDrop"
    >
      <div v-if="!contents.folders.length && !contents.items.length" class="folder-empty">
        This folder is empty.
      </div>

      <div v-else-if="viewMode === 'grid'" class="folder-grid">
        <div
          v-for="folder in contents.folders"
          :key="folder.id"
          class="folder-tile"
          draggable="true"
          @click="openFolder(folder.id)"
          @dragstart="folderDrag($event, folder.id)"
          @dragend="onDragEnd"
          @dragover.prevent
          @drop.prevent="onTileDrop($event, folder.id)"
        >
          <div class="folder-tile__thumb folder-tile__thumb--folder">▸</div>
          <div class="folder-tile__banner">{{ folder.name }}</div>
        </div>

        <div
          v-for="item in contents.items"
          :key="item.id"
          class="folder-tile"
          draggable="true"
          @click="openItem(item)"
          @dragstart="itemDrag($event, item)"
          @dragend="onDragEnd"
        >
          <img
            v-if="thumbnailUrl(item.thumbnail)"
            class="folder-tile__thumb"
            :src="thumbnailUrl(item.thumbnail) ?? ''"
            :alt="item.label"
          />
          <div v-else class="folder-tile__thumb folder-tile__thumb--empty">{{ thumbnailInitial(item.label) }}</div>
          <div class="folder-tile__banner" :style="{ background: tileColor(item) }">{{ item.label }}</div>
        </div>
      </div>

      <div v-else class="folder-list">
        <div
          v-for="folder in contents.folders"
          :key="folder.id"
          class="folder-list__row"
          draggable="true"
          @click="openFolder(folder.id)"
          @dragstart="folderDrag($event, folder.id)"
          @dragend="onDragEnd"
          @dragover.prevent
          @drop.prevent="onTileDrop($event, folder.id)"
        >
          <span class="folder-list__thumb folder-list__thumb--folder">▸</span>
          <span class="folder-list__name">{{ folder.name }}</span>
          <span class="folder-list__detail">Folder</span>
        </div>

        <div
          v-for="item in contents.items"
          :key="item.id"
          class="folder-list__row"
          draggable="true"
          @click="openItem(item)"
          @dragstart="itemDrag($event, item)"
          @dragend="onDragEnd"
        >
          <img
            v-if="thumbnailUrl(item.thumbnail)"
            class="folder-list__thumb"
            :src="thumbnailUrl(item.thumbnail) ?? ''"
            :alt="item.label"
          />
          <span v-else class="folder-list__thumb folder-list__thumb--empty">{{ thumbnailInitial(item.label) }}</span>
          <span class="folder-list__name">{{ item.label }}</span>
          <span class="folder-list__detail">{{ item.detail }}</span>
        </div>
      </div>
    </div>
  </div>
</template>
