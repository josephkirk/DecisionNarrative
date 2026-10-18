import { reactive, ref, computed } from "vue";
import type {
  AuthoringOptions,
  CharacterRecord,
  EventRecord,
  LocationRecord,
  NpcBehaviorRecord,
  PlaytestSnapshot,
  ProjectMetadata,
  QualityRecord,
  ScenarioRecord,
} from "./contracts.ts";

export interface GlobalStudioState {
  project: ProjectMetadata | null;
  authoringOptions: AuthoringOptions | null;
  qualities: QualityRecord[];
  characters: CharacterRecord[];
  locations: LocationRecord[];
  npcBehaviors: NpcBehaviorRecord[];
  events: EventRecord[];
  scenarios: ScenarioRecord[];
  activePlaytestSessionId: string | null;
  activePlaytestSnapshot: PlaytestSnapshot | null;
  notification: { message: string; type: "info" | "success" | "error" } | null;
}

export const studioState = reactive<GlobalStudioState>({
  project: null,
  authoringOptions: null,
  qualities: [],
  characters: [],
  locations: [],
  npcBehaviors: [],
  events: [],
  scenarios: [],
  activePlaytestSessionId: null,
  activePlaytestSnapshot: null,
  notification: null,
});

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
