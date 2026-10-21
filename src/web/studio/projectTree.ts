import type {
  AssetRecord,
  ContentRoot,
  EntityRecord,
  EntityType,
  FolderRecord,
  QualityRecord,
  StoryletRecord,
  TraitTemplateRecord,
} from "./contracts.ts";
import { entityTypeLabel } from "./entityModel.ts";

export interface ContentRootDefinition {
  root: ContentRoot;
  label: string;
  kind: string;
  newLabel: string;
}

export const CONTENT_ROOTS: ContentRootDefinition[] = [
  { root: "storylets", label: "Storylets", kind: "storylets", newLabel: "New Storylet" },
  { root: "entities", label: "Entities", kind: "entities", newLabel: "New Entity" },
  { root: "qualities", label: "Qualities", kind: "qualities", newLabel: "New Quality" },
  { root: "trait_templates", label: "Trait Templates", kind: "trait_templates", newLabel: "New Trait Template" },
  { root: "assets", label: "Assets", kind: "assets", newLabel: "New Asset" },
];

export function rootDefinition(root: string): ContentRootDefinition | undefined {
  return CONTENT_ROOTS.find((entry) => entry.root === root);
}

export function itemRoute(root: ContentRoot | string, id: string): string {
  switch (root) {
    case "storylets":
      return `/storylets/${encodeURIComponent(id)}/edit`;
    case "entities":
      return `/entities/${encodeURIComponent(id)}`;
    case "qualities":
      return `/qualities/${encodeURIComponent(id)}`;
    case "trait_templates":
      return `/trait-templates/${encodeURIComponent(id)}`;
    case "assets":
      return `/assets/${encodeURIComponent(id)}`;
    default:
      return "/";
  }
}

export interface TreeItem {
  root: ContentRoot;
  id: string;
  label: string;
  folderId: string | null;
  thumbnail: string | null;
  detail: string;
  entityType?: EntityType;
}

export interface TreeSource {
  projectName: string;
  folders: FolderRecord[];
  items: TreeItem[];
}

function qualityDetail(quality: QualityRecord): string {
  return quality.type;
}

function storyletDetail(storylet: StoryletRecord): string {
  const approved = storylet.approved_revision === storylet.revision;
  return `${approved ? "Approved" : "Draft"} r${storylet.revision}`;
}

function templateDetail(template: TraitTemplateRecord): string {
  const count = template.properties.length;
  return `${count} propert${count === 1 ? "y" : "ies"}`;
}

function assetDetail(asset: AssetRecord): string {
  const count = asset.versions.length;
  return `${count} version${count === 1 ? "" : "s"}`;
}

export interface BuildTreeInput {
  projectName: string;
  folders: FolderRecord[];
  qualities: QualityRecord[];
  entities: EntityRecord[];
  traitTemplates: TraitTemplateRecord[];
  assets: AssetRecord[];
  storylets: StoryletRecord[];
}

export function buildTreeItems(input: BuildTreeInput): TreeSource {
  const items: TreeItem[] = [];
  for (const storylet of input.storylets) {
    items.push({
      root: "storylets",
      id: storylet.id,
      label: storylet.title,
      folderId: storylet.folder_id ?? null,
      thumbnail: null,
      detail: storyletDetail(storylet),
    });
  }
  for (const entity of input.entities) {
    items.push({
      root: "entities",
      id: entity.id,
      label: entity.name,
      folderId: entity.folder_id ?? null,
      thumbnail: entity.thumbnail ?? null,
      detail: entityTypeLabel(entity.type),
      entityType: entity.type,
    });
  }
  for (const quality of input.qualities) {
    items.push({
      root: "qualities",
      id: quality.name,
      label: quality.name,
      folderId: quality.folder_id ?? null,
      thumbnail: null,
      detail: qualityDetail(quality),
    });
  }
  for (const template of input.traitTemplates) {
    items.push({
      root: "trait_templates",
      id: template.id,
      label: template.name,
      folderId: template.folder_id ?? null,
      thumbnail: null,
      detail: templateDetail(template),
    });
  }
  for (const asset of input.assets) {
    items.push({
      root: "assets",
      id: asset.id,
      label: asset.name,
      folderId: asset.folder_id ?? null,
      thumbnail: asset.thumbnail ?? null,
      detail: assetDetail(asset),
    });
  }
  return { projectName: input.projectName, folders: input.folders, items };
}

export type TreeRowKind = "project" | "root" | "folder" | "item";

export interface TreeRow {
  key: string;
  kind: TreeRowKind;
  depth: number;
  label: string;
  root?: ContentRoot;
  id?: string;
  folderId?: string | null;
  thumbnail?: string | null;
  detail?: string;
  entityType?: EntityType;
  hasChildren: boolean;
  expanded: boolean;
}

export function projectKey(): string {
  return "project";
}

export function rootKey(root: string): string {
  return `root:${root}`;
}

export function folderKey(id: string): string {
  return `folder:${id}`;
}

export function itemKey(root: string, id: string): string {
  return `item:${root}:${id}`;
}

export function defaultExpandedKeys(): string[] {
  return [projectKey(), ...CONTENT_ROOTS.map((entry) => rootKey(entry.root))];
}

function compareLabels(left: string, right: string): number {
  return left.localeCompare(right, undefined, { sensitivity: "base" });
}

function compareNames(left: { name: string }, right: { name: string }): number {
  return compareLabels(left.name, right.name);
}

function compareItemLabels(left: { label: string }, right: { label: string }): number {
  return compareLabels(left.label, right.label);
}

export function flattenTree(input: {
  projectName: string;
  folders: FolderRecord[];
  items: TreeItem[];
  expanded: Iterable<string>;
  query?: string;
}): TreeRow[] {
  const expanded = new Set(input.expanded);
  const query = (input.query ?? "").trim().toLowerCase();
  const rows: TreeRow[] = [];
  const matchedItems = new Set<string>();
  const forcedFolders = new Set<string>();
  const forcedRoots = new Set<string>();

  if (query) {
    const keepAncestors = (root: string, folderId: string | null) => {
      let cursor = folderId;
      const guard = new Set<string>();
      while (cursor && !guard.has(cursor)) {
        guard.add(cursor);
        forcedFolders.add(cursor);
        cursor = input.folders.find((folder) => folder.id === cursor)?.parent_id ?? null;
      }
      forcedRoots.add(root);
    };
    for (const item of input.items) {
      if (!item.label.toLowerCase().includes(query)) continue;
      matchedItems.add(itemKey(item.root, item.id));
      keepAncestors(item.root, item.folderId);
    }
    // A folder name is a reasonable thing to search for, so a matching folder and
    // its ancestors stay visible even when no item inside matches.
    for (const folder of input.folders) {
      if (!folder.name.toLowerCase().includes(query)) continue;
      keepAncestors(folder.root, folder.id);
    }
  }

  const isExpanded = (key: string) => (query ? true : expanded.has(key));
  const childrenOf = (root: string, parentId: string | null) =>
    input.folders
      .filter((folder) => folder.root === root && (folder.parent_id ?? null) === parentId)
      .sort(compareNames);
  const visibleFolders = (root: string, parentId: string | null) =>
    childrenOf(root, parentId).filter((folder) => (query ? forcedFolders.has(folder.id) : true));

  const itemsIn = (root: string, folderId: string | null) =>
    input.items
      .filter((item) => item.root === root && item.folderId === folderId)
      .filter((item) => (query ? matchedItems.has(itemKey(item.root, item.id)) : true))
      .sort(compareItemLabels);

  function pushFolder(root: string, folder: FolderRecord, depth: number) {
    const childFolders = visibleFolders(root, folder.id);
    const childItems = itemsIn(root, folder.id);
    const hasChildren = childFolders.length > 0 || childItems.length > 0;
    const folderExpanded = isExpanded(folderKey(folder.id));
    rows.push({
      key: folderKey(folder.id),
      kind: "folder",
      depth,
      label: folder.name,
      root: root as ContentRoot,
      id: folder.id,
      folderId: folder.parent_id ?? null,
      hasChildren,
      expanded: folderExpanded,
    });
    if (!hasChildren || !folderExpanded) return;
    for (const child of childFolders) {
      pushFolder(root, child, depth + 1);
    }
    for (const item of childItems) {
      pushItem(item, depth + 1);
    }
  }

  function pushItem(item: TreeItem, depth: number) {
    rows.push({
      key: itemKey(item.root, item.id),
      kind: "item",
      depth,
      label: item.label,
      root: item.root,
      id: item.id,
      folderId: item.folderId,
      thumbnail: item.thumbnail,
      detail: item.detail,
      entityType: item.entityType,
      hasChildren: false,
      expanded: false,
    });
  }

  const projectExpanded = isExpanded(projectKey());
  rows.push({
    key: projectKey(),
    kind: "project",
    depth: 0,
    label: input.projectName,
    hasChildren: true,
    expanded: projectExpanded,
  });
  if (!projectExpanded) return rows;

  for (const definition of CONTENT_ROOTS) {
    if (query && !forcedRoots.has(definition.root)) continue;
    const rootFolders = visibleFolders(definition.root, null);
    const rootItems = itemsIn(definition.root, null);
    const rootExpanded = isExpanded(rootKey(definition.root));
    rows.push({
      key: rootKey(definition.root),
      kind: "root",
      depth: 1,
      label: definition.label,
      root: definition.root,
      hasChildren: rootFolders.length > 0 || rootItems.length > 0,
      expanded: rootExpanded,
    });
    if (!rootExpanded) continue;
    for (const folder of rootFolders) {
      pushFolder(definition.root, folder, 2);
    }
    for (const item of rootItems) {
      pushItem(item, 2);
    }
  }
  return rows;
}

export function folderBreadcrumb(folders: FolderRecord[], folderId: string | null): FolderRecord[] {
  if (!folderId) return [];
  const chain: FolderRecord[] = [];
  const guard = new Set<string>();
  let cursor: string | null = folderId;
  while (cursor && !guard.has(cursor)) {
    guard.add(cursor);
    const folder = folders.find((candidate) => candidate.id === cursor);
    if (!folder) break;
    chain.unshift(folder);
    cursor = folder.parent_id ?? null;
  }
  return chain;
}

export function folderContents(
  folders: FolderRecord[],
  items: TreeItem[],
  root: ContentRoot | string,
  folderId: string | null,
): { folders: FolderRecord[]; items: TreeItem[] } {
  return {
    folders: folders
      .filter((folder) => folder.root === root && (folder.parent_id ?? null) === folderId)
      .sort(compareNames),
    items: items
      .filter((item) => item.root === root && item.folderId === folderId)
      .sort(compareItemLabels),
  };
}

export function isFolderDescendant(
  folders: FolderRecord[],
  candidateId: string,
  ancestorId: string,
): boolean {
  const guard = new Set<string>();
  let cursor: string | null = folders.find((folder) => folder.id === candidateId)?.parent_id ?? null;
  while (cursor && !guard.has(cursor)) {
    if (cursor === ancestorId) return true;
    guard.add(cursor);
    cursor = folders.find((folder) => folder.id === cursor)?.parent_id ?? null;
  }
  return false;
}
