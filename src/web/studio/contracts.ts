import type { NarrativeGraph } from "./narrativeGraphTypes.ts";

export interface ProjectMetadata {
  id: string;
  name: string;
  revision: number;
}


export interface MicroPassionRecord {
  passion_id: string;
  current_value: number;
  weight: number;
  is_hidden: boolean;
}

export interface MacroPassionThresholdRecord {
  threshold_id: string;
  trigger_value: number;
  direction: "positive" | "negative";
  linked_graph_asset_id: string;
  is_currently_active?: boolean;
}

export interface ZeroSumPassionLinkRecord {
  target_character_id: string;
  source_passion_id: string;
  target_passion_id: string;
  weight: number;
}

export interface CharacterRecord {
  id: string;
  name: string;
  role: string;
  faction_id?: string;
  home_location: string;
  current_location: string;
  status_axes?: Record<string, unknown>;
  needs?: Record<string, number>;
  agenda?: string;
  red_line?: string;
  schedule?: Record<string, string>;
  passion_pool?: MicroPassionRecord[];
  active_passions?: string[];
  trust_inertia?: number;
  thresholds?: MacroPassionThresholdRecord[];
  zero_sum_passion_links?: ZeroSumPassionLinkRecord[];
  created_at?: number;
}

export interface LocationRecord {
  id: string;
  name: string;
  description: string;
  tags: string[];
  connected_zones?: string[];
  environmental_params?: Record<string, unknown>;
  devices?: Record<string, unknown>;
  created_at?: number;
}


export interface EventRecord {
  id: string;
  title: string;
  description: string;
  event_type: "canon" | "world";
  trigger: Record<string, unknown>;
  effects: Record<string, unknown>;
  delay_turns?: number;
  repeatable?: boolean;
  /** Canon events with a story time form the chronicle. */
  story_time?: number;
  story_time_label?: string;
  location_id?: string;
  /** Fact tags (under `fact.`) a participant learns when told this event. */
  facts?: string[];
  text?: string;
}

export interface StoryletParameterRecord {
  param_name: string;
  query: string;
  location_policy?: "current" | "any";
}

export interface StoryletChoiceRecord {
  choice_id: string;
  label: string;
  systemVerb?: string;
  conditions?: Array<{ variable: string; op: string; value: unknown }>;
  effects: Record<string, unknown>;
  divert_target?: string | null;
}




export interface StoryletRecord {
  id: string;
  precondition_graph?: NarrativeGraph | null;
  graph?: NarrativeGraph;
  revision: number;
  approved_revision?: number | null;
  title: string;
  ending: boolean;
  location_tags: string[];
  /** Participant entity ids who play this storylet; empty = any human. */
  played_by?: string[];
  group_tag?: string | null;
  cluster_type: "linear_progression" | "player_passage" | "wheel_deck" | "aftermath_accelerator";
  urgency_weight: number;
  repeatability: "once_ever" | "always" | "cooldown" | "persistent";
  cooldown_turns: number;
  parameters: StoryletParameterRecord[];
  content: {
    text_template: string;
    speaker_id?: string;
    sceneOpening?: string;
    dialogue_quips?: Array<{ speaker: string; text: string }>;
  };
  choices: StoryletChoiceRecord[];
  effects: Record<string, unknown>;
  folder_id?: string | null;
  thumbnail?: string | null;
  created_at: number;
}

export interface LibrarySummaryItem {
  id: string;
  title: string;
  excerpt: string;
  location_tags: string[];
}

export interface LibrarySummaryItem {
  id: string;
  title: string;
  excerpt: string;
  location_tags: string[];
  role_tags: string[];
  choice_count: number;
  revision: number;
  approved_revision?: number | null;
  ending: boolean;
  created_at: number;
  thumbnail?: string | null;
}

export interface LibraryPageResult {
  items: LibrarySummaryItem[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
  available_location_tags: string[];
}

export interface AuthoringOptions {
  repeatability_modes: string[];
  cluster_types: string[];
  policy_modes: string[];
  event_types: string[];
}

export const CONTENT_ROOT_KEYS = ["storylets", "entity_templates", "entities", "trait_templates", "assets"] as const;
export type ContentRoot = (typeof CONTENT_ROOT_KEYS)[number];

export interface FolderRecord {
  id: string;
  name: string;
  root: ContentRoot;
  parent_id?: string | null;
  created_at?: number;
}

export type PropertyType = "bool" | "string" | "float" | "int" | "asset" | "entity" | "storylet" | "tag" | "json";

export interface AssetLinkValue { asset_id: string; }
export type PropertyScalar = boolean | string | number | AssetLinkValue | Record<string, unknown> | null;
export type PropertyValue = PropertyScalar | PropertyScalar[];

export interface PropertyDefinition {
  id: string; key: string; name: string; type: PropertyType;
  is_array?: boolean; multiline?: boolean; default?: PropertyValue;
}
export interface EntityProperty extends PropertyDefinition { value?: PropertyValue; }
export interface TraitInstance { id: string; template_id: string; values: Record<string, PropertyValue>; }
export interface TraitPropertyDefinition extends PropertyDefinition { min?: number | null; max?: number | null; }
export interface TraitTemplateRecord {
  id: string; key: string; key_auto?: boolean; name: string; description?: string;
  folder_id?: string | null; includes?: string[]; properties: TraitPropertyDefinition[];
  allow_multiple?: boolean; system?: boolean; created_at?: number;
}
export interface GameplayTagRecord {
  id: string;
  name: string;
  description?: string;
  protected: boolean;
  created_at?: number;
}
export interface EntityTemplateRecord {
  id: string;
  object_id: string;
  game_id: string;
  game_id_auto?: boolean;
  name: string;
  root_tag: string;
  description?: string;
  thumbnail?: string | null;
  folder_id?: string | null;
  tags: string[];
  properties: EntityProperty[];
  traits: TraitInstance[];
  created_at?: number;
}

export interface EntityRecord {
  id: string;
  template_id?: string | null;
  object_id?: string;
  game_id?: string;
  game_id_auto?: boolean;
  name: string;
  root_tag: string;
  description?: string;
  thumbnail?: string | null;
  folder_id?: string | null;
  tags: string[];
  properties: EntityProperty[];
  traits: TraitInstance[];
  created_at?: number;
}

export interface StorageLocationRecord {
  storage_type: "disk" | "git" | "perforce" | string;
  storage_id: string;
  created_at: number;
  reference_type: "file" | "commit" | "changelist" | "snapshot" | string;
  path: string;
  metadata?: Record<string, unknown>;
}

export interface AssetVersionRecord {
  id: string;
  file_path: string;
  creator: string;
  tool_version: string;
  description?: string | null;
  created_at: number;
  tags: string[];
  custom_data?: Record<string, unknown>;
  thumbnail?: string | null;
  storage: StorageLocationRecord[];
}

export interface AssetRecord {
  id: string;
  name: string;
  folder_id?: string | null;
  thumbnail?: string | null;
  tags: string[];
  versions: AssetVersionRecord[];
  created_at?: number;
}

export type ReferenceVia = "id" | "role" | "tag";

export interface ContentReference {
  source_kind: string;
  source_id: string;
  source_label: string;
  node_id?: string | null;
  node_label?: string | null;
  field: string;
  via: ReferenceVia;
}

// ---- Run API (/api/v4/runs); mirrors crates/storylet-engine run.rs and storylet-engine-service runs.rs ----

export type RunAction =
  | { kind: "choose"; nodeGuid: number; outputPin: number }
  | { kind: "play_storylet"; nodeGuid: number; storylet: number }
  | { kind: "topic"; nodeGuid: number; edgeId: number }
  | { kind: "move"; to: number }
  | { kind: "wait" };

export type RunStart = { kind: "beginning" } | { kind: "storylet"; storylet: number };

export interface RunStartInput {
  start: RunStart;
  scenarioId?: string;
  /** Begin at this authored storylet instead of the scenario default. */
  storyletId?: string;
  seed?: number;
  /** Project revision the caller compiled against; the client fills it from the project when omitted. */
  expectedRevision?: number;
}

export interface StoryletRef { id: number; title: string; /** Authored storylet id; use for editor links. */ key?: string }
export interface ParticipantView { id: number; name: string }
export interface PlaceView { id: number; name: string; /** Present only when the project has a clock. */ clock?: string }
export interface LineView { text: string }
export interface ChoiceView { action: RunAction; label: string; enabled: boolean; requirement: string | null }
export interface SceneView { storylet: StoryletRef; lines: LineView[]; choices: ChoiceView[]; nodeGuid: number }
export interface RunStep { index: number; branch: number; participant: number; action: RunAction; label: string }
export interface BranchView { id: number; steps: RunStep[] }

export interface RunView {
  participant: number;
  participants: ParticipantView[];
  place: PlaceView | null;
  scene: SceneView | null;
  alsoHere: StoryletRef[];
  nothingHere: boolean;
  ending: StoryletRef | null;
  steps: RunStep[];
  currentStep: number;
  branches: BranchView[];
}

export interface RunEnvelope {
  id: string;
  revision: number;
  stale: boolean;
  contentRevision: number;
  view: RunView;
  stalled?: { storyletId: number | null; nodeGuid: number };
}

export interface RunReloadResult extends RunEnvelope { replayed: number; divergedAt: number | null }

export interface RunSummary {
  id: string; title: string; steps: number; ending: StoryletRef | null; stale: boolean; updatedAt: number;
}

export interface NearMiss { storylet: StoryletRef; reason: string }

export type KnowKind = "evidence" | "witnessed" | "told" | "rumor" | "recollection" | "testimony" | "inference";
export interface KnowRowView { fact: string; kind: KnowKind; confirmed: boolean; sourceStep: number | null }
export interface ChangeView { key: string; before: string; after: string }
export interface ChronicleRowView { key: string; label: string; place: string | null; storyTime: number; told: number | null }
export interface AdvancedView { runSeed: number; contentRevision: number; nodeGuid: number | null; interaction: string | null }
export interface InspectView {
  knows: KnowRowView[]; worldChanges: ChangeView[]; chronicle: ChronicleRowView[]; advanced: AdvancedView;
}
