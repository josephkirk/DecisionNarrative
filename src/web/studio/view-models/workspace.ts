import { StudioApi } from "../api.ts";
import { notify, setEntities, studioState } from "../state.ts";
import type {
  AssetRecord,
  EntityRecord,
  EventRecord,
  FolderRecord,
  NpcBehaviorRecord,
  QualityRecord,
  StoryletRecord,
  TraitTemplateRecord,
} from "../contracts.ts";

export type WorkspaceKind =
  | "qualities"
  | "entities"
  | "folders"
  | "trait_templates"
  | "assets"
  | "storylets"
  | "npc_behaviors"
  | "events";

export const ALL_WORKSPACE_KINDS: readonly WorkspaceKind[] = [
  "qualities",
  "entities",
  "folders",
  "trait_templates",
  "assets",
  "storylets",
  "npc_behaviors",
  "events",
];

function assign(kind: WorkspaceKind, rows: unknown[]) {
  switch (kind) {
    case "qualities":
      studioState.qualities = rows as QualityRecord[];
      break;
    case "entities":
      setEntities(rows as EntityRecord[]);
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
    case "npc_behaviors":
      studioState.npcBehaviors = rows as NpcBehaviorRecord[];
      break;
    case "events":
      studioState.events = rows as EventRecord[];
      break;
  }
}

/**
 * The folder a record currently sits in, as last loaded from the server.
 * Returns `undefined` when the record is unknown (e.g. an unsaved draft).
 */
export function currentFolderId(kind: WorkspaceKind, key: string): string | null | undefined {
  switch (kind) {
    case "entities": {
      const row = studioState.entities.find((entity) => entity.id === key);
      return row ? row.folder_id ?? null : undefined;
    }
    case "qualities": {
      const row = studioState.qualities.find((quality) => quality.name === key);
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

/** Reload the requested authored kinds (and the project revision) into the studio state. */
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
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
  }
}
