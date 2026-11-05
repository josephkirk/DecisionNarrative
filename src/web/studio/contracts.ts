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
