export interface ProjectMetadata {
  id: string;
  name: string;
  revision: number;
}

export interface QualityRecord {
  name: string;
  type: "currency" | "metric" | "tool" | "progress" | "menace" | "territory";
  value: number;
  min_value?: number;
  max_value?: number;
  is_hidden?: boolean;
  menace_crisis_storylet_id?: string;
  description?: string;
  metadata?: Record<string, unknown>;
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
  qualities?: Record<string, unknown>;
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
  local_variables?: LocalVariableDefinition[];
  graph?: unknown;
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


export interface LocalVariableDefinition {
  name: string;
  type: "boolean" | "number" | "string";
  default_value: boolean | number | string;
}

export interface QualityCondition {
  variable: string;
  op: "eq" | "neq" | "gt" | "gte" | "lt" | "lte" | string;
  value: unknown;
}

export type VisitedLocationPrecondition =
  | { kind: "location_id"; location_id: string }
  | { kind: "location_tag"; tag: string };

export type MetCharacterPrecondition =
  | { kind: "character_id"; character_id: string }
  | { kind: "role_tag"; role: string };

export interface StoryletPreconditions {
  all_of?: QualityCondition[];
  any_of?: QualityCondition[];
  visited_locations_any_of?: VisitedLocationPrecondition[];
  met_characters_any_of?: MetCharacterPrecondition[];
}

export interface ChronoQualityPredicate {
  target_id: string;
  subject: "metric" | "location" | "entity" | "visited" | "met_character";
  operator: "eq" | "neq" | "gt" | "gte" | "lt" | "lte";
  comparison_value: number;
  specificity_weight: number;
  entity_index?: number | null;
}

export interface ChronoStoryletEffect {
  target_id: string;
  value: number;
  incremental: boolean;
  add_tags: string[];
  remove_tags: string[];
}

export interface ChronoStoryletSettings {
  display_priority: number;
  urgency_color_tag: string;
  is_repeatable: boolean;
  cooldown_ms: number;
  preconditions: ChronoQualityPredicate[];
  required_tags: string[];
  required_any_tags: string[];
  forbidden_tags: string[];
  required_arc_phases: string[];
  ideal_state_vector: Record<string, number>;
  ideal_state_weights?: Record<string, number>;
  salience_bonus_tags: string[];
  effects_on_complete: ChronoStoryletEffect[];
}

export interface StoryletRecord {
  id: string;
  graph?: unknown;
  local_variables?: LocalVariableDefinition[];
  chrono?: ChronoStoryletSettings;
  revision: number;
  approved_revision?: number | null;
  title: string;
  ending: boolean;
  position?: [number, number];
  size?: [number, number];
  location_tags: string[];
  group_tag?: string | null;
  cluster_type: "linear_progression" | "player_passage" | "wheel_deck" | "aftermath_accelerator" | "linear" | "player_controlled_passage" | "wheel" | "aftermath";
  urgency_weight: number;
  repeatability: "once_ever" | "always" | "cooldown" | "persistent";
  cooldown_turns: number;
  parameters: StoryletParameterRecord[];
  preconditions: StoryletPreconditions;
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
  current_arc_phase?: string;
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
  location_tags: string[];
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
  available_location_tags: string[];
}

export interface AuthoringOptions {
  quality_types: string[];
  repeatability_modes: string[];
  cluster_types: string[];
  policy_modes: string[];
  event_types: string[];
}


export interface PlaytestSnapshot {
  session_id: string;
  project_id: string;
  world: {
    revision: number;
    turn: number;
    scenario_id?: string;
    current_location?: string | null;
    visited_locations?: string[];
    met_characters?: string[];
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
}
export interface ChronoPlaytestSessionSummary {
  sessionId: string;
  scenarioId: string | null;
  startLocationId: string | null;
  startStoryletId: string | null;
  seed: number;
  updatedAt: number;
  live: boolean;
  globalHistoryLength: number;
}

export interface ChronoSignalToken {
  token_id: string;
  graph_instance_id: string;
  target_node_guid: number;
  target_pin_name: string;
  time_budget_ms: number;
  is_active: boolean;
  deferred_until_ms: number;
  is_deferred: boolean;
}

export interface ChronoPlaytestSnapshot {
  operation: string;
  session: {
    sessionId: string;
    projectId: string;
    pinnedProjectRevision: number;
    scenarioId: string | null;
    startLocationId: string | null;
    startStoryletId: string | null;
    seed: number;
  };
  context: {
    instanceId: string;
    assetId: string;
    turn: number;
    clockMs: number;
    nodes: Array<{
      nodeGuid: number;
      nodeTypeId: string;
      state: unknown;
      storyletId: string | null;
      outputPins: string[];
      payload: { nodeData?: Record<string, unknown> } | null;
    }>;
    tokens: ChronoSignalToken[];
    activeInteractions: Array<{
      nodeGuid: number;
      kind: "choice" | "qbn" | "waypoint";
      options?: Array<{ outputPin: string; choiceId: string | null; label: string }>;
      currentTopicId?: string;
      topics?: Array<{
        edgeId: string;
        topicId: string;
        playerOnly: boolean;
        triggersSceneExit: boolean;
        revealedClueQualityId: string | null;
      }>;
      eligibleStorylets?: Array<{
        id: string;
        title: string;
        nodeGuid: number;
        displayPriority: number;
        urgencyColorTag: string;
      }>;
    }>;
    world: {
      currentLocation: string | null;
      metrics: Record<string, number>;
      globalTags: string[];
      entities: Record<string, string[]>;
      passions: {
        micro: Record<string, number>;
        macro: Record<string, number>;
        active: string[];
      };
    };
    storylets: Array<{ id: string; title: string }>;
    globalCallHistory: unknown[];
    scopedCallHistories: Record<string, unknown[]>;
    passionEvents: unknown[];
    passionThresholds: unknown[];
  };
  result: unknown;
}
