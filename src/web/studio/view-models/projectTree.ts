import { computed, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { StudioApi } from "../api.ts";
import type { AssetRecord, ContentRoot, EntityRecord, FolderRecord, TraitTemplateRecord } from "../contracts.ts";
import { newEntityDraft } from "../entityModel.ts";
import {
  buildTreeItems,
  CONTENT_ROOTS,
  defaultExpandedKeys,
  flattenTree,
  folderKey,
  isFolderDescendant,
  itemKey,
  itemRoute,
  projectKey,
  rootKey,
  type ContentRootDefinition,
  type TreeRow,
} from "../projectTree.ts";
import { notify, setEntities, studioState } from "../state.ts";
import { refreshWorkspace } from "./workspace.ts";

const EXPANDED_KEY = "dn.studio.tree.expanded";

function loadExpanded(): Set<string> {
  try {
    const raw = localStorage.getItem(EXPANDED_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as unknown;
      if (Array.isArray(parsed)) {
        return new Set(parsed.filter((value): value is string => typeof value === "string"));
      }
    }
  } catch {
    // Fall through to defaults when storage is unavailable or corrupt.
  }
  return new Set(defaultExpandedKeys());
}

function newId(): string {
  const cryptoApi = globalThis.crypto;
  if (cryptoApi && typeof cryptoApi.randomUUID === "function") return cryptoApi.randomUUID();
  return `id-${Math.random().toString(36).slice(2)}-${Date.now().toString(36)}`;
}

export const activeDragRoot = ref<string | null>(null);

export interface DragPayload {
  kind: "folder" | "item";
  root: string;
  id: string;
}

export function useProjectTree() {
  const router = useRouter();
  const route = useRoute();
  const expanded = ref<Set<string>>(loadExpanded());
  const query = ref("");

  watch(
    expanded,
    (value) => {
      try {
        localStorage.setItem(EXPANDED_KEY, JSON.stringify([...value]));
      } catch {
        // Persisting the collapsed state is best effort.
      }
    },
    { deep: true },
  );

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

  const rows = computed<TreeRow[]>(() =>
    flattenTree({
      projectName: source.value.projectName,
      folders: source.value.folders,
      items: source.value.items,
      expanded: expanded.value,
      query: query.value,
    }),
  );

  function toggle(key: string) {
    const next = new Set(expanded.value);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    expanded.value = next;
  }

  function expand(keys: string[]) {
    const next = new Set(expanded.value);
    for (const key of keys) next.add(key);
    expanded.value = next;
  }

  function ancestorsOf(folderId: string | null): string[] {
    const keys = [projectKey()];
    let cursor = folderId;
    while (cursor) {
      const folder = studioState.folders.find((candidate) => candidate.id === cursor);
      if (!folder) break;
      keys.push(rootKey(folder.root), folderKey(folder.id));
      cursor = folder.parent_id ?? null;
    }
    return keys;
  }

  const activeKey = computed(() => {
    const name = typeof route.name === "string" ? route.name : "";
    const params = route.params as Record<string, unknown>;
    if (name === "browse") {
      const root = typeof params.root === "string" ? params.root : "";
      const folderId = typeof params.folderId === "string" ? params.folderId : "";
      return folderId ? folderKey(folderId) : rootKey(root);
    }
    if (name === "project") return projectKey();
    if (name === "edit-storylet") return itemKey("storylets", String(params.id ?? ""));
    if (name === "new-storylet") return rootKey("storylets");
    if (name === "entity") return itemKey("entities", String(params.id ?? ""));
    if (name === "quality" || name === "new-quality") {
      return itemKey("qualities", String(params.name ?? ""));
    }
    if (name === "trait-template") return itemKey("trait_templates", String(params.id ?? ""));
    if (name === "asset") return itemKey("assets", String(params.id ?? ""));
    return "";
  });

  async function createFolder(root: string, parentId: string | null) {
    const name = window.prompt("Folder name");
    if (!name || !name.trim()) return;
    try {
      const saved = await StudioApi.saveContent<FolderRecord>("folders", {
        id: "",
        name: name.trim(),
        root,
        parent_id: parentId,
      });
      await refreshWorkspace(["folders"]);
      expand([projectKey(), rootKey(root), ...ancestorsOf(parentId), folderKey(saved.id)]);
      notify(`Folder '${saved.name}' created`, "success");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    }
  }

  async function renameFolder(folder: FolderRecord) {
    const name = window.prompt("Folder name", folder.name);
    if (!name || !name.trim() || name.trim() === folder.name) return;
    try {
      await StudioApi.saveContent<FolderRecord>("folders", { ...folder, name: name.trim() });
      await refreshWorkspace(["folders"]);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    }
  }

  async function deleteFolder(folder: FolderRecord) {
    if (!window.confirm(`Delete folder '${folder.name}'?`)) return;
    try {
      await StudioApi.deleteContent("folders", folder.id);
      await refreshWorkspace(["folders"]);
      notify(`Folder '${folder.name}' deleted`, "success");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    }
  }

  async function createItem(definition: ContentRootDefinition, folderId: string | null) {
    try {
      switch (definition.root) {
        case "storylets":
          router.push({ path: "/storylets/new", query: folderId ? { folder: folderId } : {} });
          return;
        case "qualities":
          router.push({ path: "/qualities/new", query: folderId ? { folder: folderId } : {} });
          return;
        case "entities": {
          const saved = await StudioApi.saveContent<EntityRecord>("entities", newEntityDraft(folderId));
          await refreshWorkspace(["entities"]);
          router.push(`/entities/${saved.id}`);
          return;
        }
        case "trait_templates": {
          const saved = await StudioApi.saveContent<TraitTemplateRecord>("trait_templates", {
            id: "",
            key: "",
            name: "New Trait Template",
            description: "",
            folder_id: folderId,
            properties: [],
          });
          await refreshWorkspace(["trait_templates"]);
          router.push(`/trait-templates/${saved.id}`);
          return;
        }
        case "assets": {
          const saved = await StudioApi.saveContent<AssetRecord>("assets", {
            id: "",
            name: "New Asset",
            folder_id: folderId,
            thumbnail: null,
            tags: [],
            versions: [],
          });
          await refreshWorkspace(["assets"]);
          router.push(`/assets/${saved.id}`);
          return;
        }
      }
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    }
  }

  function dragPayload(event: DragEvent): DragPayload | null {
    const raw = event.dataTransfer?.getData("application/x-dn-tree");
    if (!raw) return null;
    try {
      return JSON.parse(raw) as DragPayload;
    } catch {
      return null;
    }
  }

  async function moveTo(payload: DragPayload | null, targetRoot: string, targetFolderId: string | null) {
    if (!payload) return;
    if (payload.kind === "folder") {
      const folder = studioState.folders.find((candidate) => candidate.id === payload.id);
      if (!folder || folder.root !== targetRoot) return;
      if (targetFolderId === folder.id) return;
      if (targetFolderId && isFolderDescendant(studioState.folders, targetFolderId, folder.id)) {
        notify(`Cannot move '${folder.name}' into itself or one of its subfolders`, "error");
        return;
      }
      try {
        await StudioApi.moveContent("folders", folder.id, targetFolderId);
        await refreshWorkspace(["folders"]);
        expand(ancestorsOf(targetFolderId));
        notify(`Moved '${folder.name}'`, "success");
      } catch (error) {
        notify(error instanceof Error ? error.message : String(error), "error");
      }
      return;
    }
    if (payload.root !== targetRoot) return;
    const item = source.value.items.find((candidate) => candidate.root === payload.root && candidate.id === payload.id);
    if (!item || item.folderId === targetFolderId) return;
    try {
      await StudioApi.moveContent(payload.root, payload.id, targetFolderId);
      await refreshWorkspace([payload.root as ContentRoot]);
      expand(ancestorsOf(targetFolderId));
      notify(`Moved '${item.label}'`, "success");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    }
  }

  function openRow(row: TreeRow) {
    if (row.kind === "project") {
      router.push("/project");
      return;
    }
    if (row.kind === "root" && row.root) {
      router.push(`/browse/${row.root}`);
      return;
    }
    if (row.kind === "folder" && row.root && row.id) {
      router.push(`/browse/${row.root}/${row.id}`);
      return;
    }
    if (row.kind === "item" && row.root && row.id) {
      router.push(itemRoute(row.root, row.id));
    }
  }

  return {
    roots: CONTENT_ROOTS,
    rows,
    query,
    expanded,
    activeKey,
    activeDragRoot,
    toggle,
    expand,
    createFolder,
    renameFolder,
    deleteFolder,
    createItem,
    moveTo,
    dragPayload,
    openRow,
  };
}
