import { reactive, computed } from "vue";
import type {
  AssetRecord,
  AuthoringOptions,
  CharacterRecord,
  EntityRecord,
  EventRecord,
  FolderRecord,
  LocationRecord,
  NpcBehaviorRecord,
  ChronoPlaytestSnapshot,
  PlaytestSnapshot,
  ProjectMetadata,
  QualityRecord,
  ScenarioRecord,
  StoryletRecord,
  TraitTemplateRecord,
} from "./contracts.ts";
import { characterRecordFromEntity, locationRecordFromEntity } from "./entityModel.ts";

export interface GlobalStudioState {
  project: ProjectMetadata | null;
  authoringOptions: AuthoringOptions | null;
  qualities: QualityRecord[];
  /** Authoritative world objects. Assign only through `setEntities`. */
  entities: EntityRecord[];
  folders: FolderRecord[];
  traitTemplates: TraitTemplateRecord[];
  assets: AssetRecord[];
  storylets: StoryletRecord[];
  /** Derived from `entities`; assign only through `setEntities`. */
  characters: CharacterRecord[];
  /** Derived from `entities`; assign only through `setEntities`. */
  locations: LocationRecord[];
  npcBehaviors: NpcBehaviorRecord[];
  events: EventRecord[];
  scenarios: ScenarioRecord[];
  activeChronoPlaytestSessionId: string | null;
  activeChronoPlaytestSnapshot: ChronoPlaytestSnapshot | null;
  activePlaytestSessionId: string | null;
  activePlaytestSnapshot: PlaytestSnapshot | null;
  notification: { message: string; type: "info" | "success" | "error" } | null;
}

export const studioState = reactive<GlobalStudioState>({
  project: null,
  authoringOptions: null,
  qualities: [],
  entities: [],
  folders: [],
  traitTemplates: [],
  assets: [],
  storylets: [],
  characters: [],
  locations: [],
  npcBehaviors: [],
  events: [],
  scenarios: [],
  activeChronoPlaytestSessionId: null,
  activeChronoPlaytestSnapshot: null,
  activePlaytestSessionId: null,
  activePlaytestSnapshot: null,
  notification: null,
});

/** Replace the entity list and rebuild the derived character/location views. */
export function setEntities(entities: EntityRecord[]) {
  studioState.entities = entities;
  studioState.characters = entities
    .map(characterRecordFromEntity)
    .filter((record): record is CharacterRecord => record !== null);
  studioState.locations = entities
    .map(locationRecordFromEntity)
    .filter((record): record is LocationRecord => record !== null);
}

export function notify(message: string, type: "info" | "success" | "error" = "info") {
  studioState.notification = { message, type };
  setTimeout(() => {
    if (studioState.notification?.message === message) {
      studioState.notification = null;
    }
  }, 4000);
}

export const locationMap = computed(() => {
  const map = new Map<string, LocationRecord>();
  for (const loc of studioState.locations) {
    map.set(loc.id, loc);
  }
  return map;
});

export const characterMap = computed(() => {
  const map = new Map<string, CharacterRecord>();
  for (const c of studioState.characters) {
    map.set(c.id, c);
  }
  return map;
});
