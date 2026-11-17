import { StudioApi } from "../api.ts";
import { notify, setEntities, studioState } from "../state.ts";
import { emit } from "../plugins/runtime.ts";
import type {
  AssetRecord,
  EntityRecord,
  EventRecord,
  FolderRecord,
  StoryletRecord,
  TraitTemplateRecord,
  GameplayTagRecord,
} from "../contracts.ts";

export type WorkspaceKind =
  | "entities"
  | "gameplay_tags"
  | "trait_templates"
  | "assets"
  | "storylets"
  | "events"
  | "folders";

export const ALL_WORKSPACE_KINDS: readonly WorkspaceKind[] = [
  "entities",
  "gameplay_tags",
  "trait_templates",
  "assets",
  "storylets",
  "events",
  "folders",
];

function assign(kind: WorkspaceKind, rows: unknown[]) {
  switch (kind) {
    case "entities": setEntities(rows as EntityRecord[]); break;
    case "gameplay_tags":
      studioState.gameplayTags = rows as GameplayTagRecord[];
      break;
    case "folders":
      studioState.folders = rows as FolderRecord[];
      break;
    case "trait_templates":
      studioState.traitTemplates = rows as TraitTemplateRecord[];
      break;
    case "assets":
      studioState.assets = rows as AssetRecord[];
      break;
    case "storylets":
      studioState.storylets = rows as StoryletRecord[];
      break;
    case "events":
      studioState.events = rows as EventRecord[];
      break;
  }
}

export function currentFolderId(kind: WorkspaceKind, key: string): string | null | undefined {
  switch (kind) {
    case "entities": {
      const row = studioState.entities.find((entity) => entity.id === key);
      return row ? row.folder_id ?? null : undefined;
    }
    case "storylets": {
      const row = studioState.storylets.find((storylet) => storylet.id === key);
      return row ? row.folder_id ?? null : undefined;
    }
    case "trait_templates": {
      const row = studioState.traitTemplates.find((template) => template.id === key);
      return row ? row.folder_id ?? null : undefined;
    }
    case "assets": {
      const row = studioState.assets.find((asset) => asset.id === key);
      return row ? row.folder_id ?? null : undefined;
    }
    default:
      return undefined;
  }
}

export async function refreshWorkspace(
  kinds: readonly WorkspaceKind[] = ALL_WORKSPACE_KINDS,
): Promise<void> {
  try {
    const [project, rows] = await Promise.all([
      StudioApi.getProject(),
      Promise.all(kinds.map((kind) => StudioApi.listContent<unknown>(kind))),
    ]);
    studioState.project = project;
    kinds.forEach((kind, index) => assign(kind, rows[index] ?? []));
    emit("workspace/loaded");
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  }
}
