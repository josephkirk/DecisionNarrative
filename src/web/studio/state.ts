import { reactive } from "vue";
import type {
  AssetRecord,
  AuthoringOptions,
  EntityRecord,
  EntityTemplateRecord,
  FolderRecord,
  EventRecord,
  ProjectMetadata,
  StoryletRecord,
  TraitTemplateRecord,
  GameplayTagRecord,
} from "./contracts.ts";

export interface GlobalStudioState {
  project: ProjectMetadata | null;
  authoringOptions: AuthoringOptions | null;
  entities: EntityRecord[];
  entityTemplates: EntityTemplateRecord[];
  gameplayTags: GameplayTagRecord[];
  folders: FolderRecord[];
  traitTemplates: TraitTemplateRecord[];
  assets: AssetRecord[];
  storylets: StoryletRecord[];
  events: EventRecord[];
  notification: { message: string; type: "info" | "success" | "error" } | null;
}

export const studioState = reactive<GlobalStudioState>({
  project: null,
  authoringOptions: null,
  entities: [],
  entityTemplates: [],
  gameplayTags: [],
  folders: [],
  traitTemplates: [],
  assets: [],
  storylets: [],
  events: [],
  notification: null,
});

export function setEntityTemplates(entityTemplates: EntityTemplateRecord[]) {
  studioState.entityTemplates = entityTemplates;
}

export function setEntities(entities: EntityRecord[]) {
  studioState.entities = entities;
}

export function notify(message: string, type: "info" | "success" | "error" = "info") {
  studioState.notification = { message, type };
  setTimeout(() => {
    if (studioState.notification?.message === message) {
      studioState.notification = null;
    }
  }, 4000);
}


