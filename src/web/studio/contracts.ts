export interface ProjectMetadata {
  id: string;
  name: string;
  revision: number;
}

export interface QualityRecord {
  name: string;
  type: "currency" | "metric" | "tool" | "territory" | "menace";
  value: number;
  description?: string;
  metadata?: Record<string, unknown>;
}

export interface CharacterRecord {
  id: string;
  name: string;
  role: string;
  home_location: string;
  current_location: string;
  status_axes?: Record<string, unknown>;
  needs?: Record<string, number>;
  agenda?: string;
  red_line?: string;
  schedule?: Record<string, string>;
  qualities?: Record<string, unknown>;
  created_at?: number;
}

export interface LocationRecord {
  id: string;
  name: string;
  description: string;
  connected_zones?: string[];
  environmental_params?: Record<string, unknown>;
  devices?: Record<string, unknown>;
  created_at?: number;
}

export interface NpcBehaviorRecord {
  id: string;
  actor_id: string;
  priority: number;
  conditions: Record<string, unknown>;
  action: string;
  effects: Array<Record<string, unknown>>;
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
}

export interface StoryletChoiceRecord {
  choice_id: string;
  label: string;
  systemVerb?: string;
  conditions?: Array<{ variable: string; op: string; value: unknown }>;
  effects: Record<string, unknown>;
  divert_target?: string | null;
  advisorPreview?: Record<string, unknown>;
}

export interface StoryletRecord {
  id: string;
  revision: number;
  approved_revision?: number | null;
  title: string;
  ending: boolean;
  location?: string | null;
  group_tag?: string | null;
  cluster_type: "linear" | "player_controlled_passage" | "wheel" | "aftermath";
  urgency_weight: number;
  repeatability: "once_ever" | "always" | "cooldown" | "persistent";
  cooldown_turns: number;
  parameters: StoryletParameterRecord[];
  preconditions: {
    all_of?: Array<{ variable: string; op: string; value: unknown }>;
    any_of?: Array<{ variable: string; op: string; value: unknown }>;
  };
  content: {
    text_template: string;
    speaker_id?: string;
    sceneOpening?: string;
    dialogue_quips?: Array<{ speaker: string; text: string }>;
  };
  choices: StoryletChoiceRecord[];
  effects: Record<string, unknown>;
  created_at: number;
}

export interface StartLocationPolicy {
  mode: "fixed" | "random" | "by_attribute";
  location_id?: string;
  attribute_key?: string;
  attribute_value?: unknown;
}

export interface ActorLocationPolicy {
  mode: "fixed" | "random" | "by_attribute";
  location_id?: string;
  attribute_key?: string;
  attribute_value?: unknown;
}

export interface ActorPopulationPolicy {
  mode: "all" | "random" | "by_attribute";
  count?: number;
  attribute_key?: string;
  attribute_value?: unknown;
  location_policy?: ActorLocationPolicy;
}

export interface ScenarioRecord {
  id: string;
  name: string;
  seed: number;
  start_location_policy: StartLocationPolicy;
  initial_qualities: Record<string, unknown>;
  actor_locations: Record<string, string>;
  actor_population_policy: ActorPopulationPolicy;
}

export interface ResolvedScenarioStart {
  player_start_location: string;
  actor_locations: Record<string, string>;
  active_actor_ids: string[];
  effective_qualities: Record<string, unknown>;
  seed: number;
  actors_per_location: Record<string, string[]>;
  issues: string[];
}

export interface LibrarySummaryItem {
  id: string;
  title: string;
  excerpt: string;
  location?: string | null;
  role_tags: string[];
  choice_count: number;
  revision: number;
  approved_revision?: number | null;
  ending: boolean;
  created_at: number;
}

export interface LibraryPageResult {
  items: LibrarySummaryItem[];
  total: number;
  page: number;
  page_size: number;
  total_pages: number;
  available_locations: string[];
}

export interface GenerationStageDescriptor {
  id: string;
  title: string;
}

export interface AuthoringOptions {
  quality_types: string[];
  repeatability_modes: string[];
  cluster_types: string[];
  policy_modes: string[];
  event_types: string[];
  stages: GenerationStageDescriptor[];
}

export interface GenerationJobRecord {
  id: string;
  revision: number;
  status: "ready" | "running" | "failed" | "complete";
  next_stage_index: number;
  request: Record<string, unknown>;
  stages: Array<{
    stage: string;
    artifact: unknown;
    source: string;
    model: string;
    promptVersion: string;
    completedAt: number;
  }>;
  attempts: Record<string, number>;
  total_retries: number;
  draft_storylet_id?: string;
  draft_revision?: number;
  failure?: { stage: string; message: string };
  created_at: number;
  updated_at: number;
}

export interface PlaytestSnapshot {
  session_id: string;
  project_id: string;
  world: {
    revision: number;
    turn: number;
    scenario_id?: string;
    current_location?: string | null;
    qualities: Record<string, unknown>;
    actors: Record<string, { role_tags: string[]; current_location: string; busy: boolean }>;
    events: Array<{
      id: string;
      actor_id: string;
      action: string;
      turn: number;
      revision: number;
      caused_by: string[];
      effects: Array<Record<string, unknown>>;
    }>;
  };
}

export interface CandidateEvaluation {
  storylet_id: string;
  eligible: boolean;
  rejection?: { reason: string };
  bound_roles: Record<string, string>;
  base_weight: number;
  urgency: number;
  arc_alignment_score: number;
  final_weight: number;
}

export interface DirectorEvaluation {
  candidates: CandidateEvaluation[];
  selected?: string | null;
  decision?: {
    formatVersion: number;
    source: string;
    no_action_probability: number;
    ranking: Array<{ storylet_id: string; score: number; probability: number }>;
  };
}
