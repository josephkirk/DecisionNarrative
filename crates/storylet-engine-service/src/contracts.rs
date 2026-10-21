//! Authored contracts and API payloads for Storylet Studio.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub id: String,
    pub name: String,
    pub revision: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityRecord {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String, // currency | metric | tool | progress | menace; territory is retained for legacy imports
    pub value: f64,
    #[serde(default = "default_quality_min")]
    pub min_value: f64,
    #[serde(default = "default_quality_max")]
    pub max_value: f64,
    #[serde(default)]
    pub is_hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menace_crisis_storylet_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
}

fn default_quality_min() -> f64 {
    0.0
}

fn default_quality_max() -> f64 {
    100.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MicroPassionRecord {
    pub passion_id: String,
    #[serde(default)]
    pub current_value: f64,
    #[serde(default = "default_passion_weight")]
    pub weight: f64,
    #[serde(default)]
    pub is_hidden: bool,
}

fn default_passion_weight() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MacroPassionThresholdRecord {
    #[serde(default)]
    pub threshold_id: String,
    pub trigger_value: f64,
    pub direction: String,
    pub linked_graph_asset_id: String,
    #[serde(default)]
    pub is_currently_active: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ZeroSumPassionLinkRecord {
    pub target_character_id: String,
    pub source_passion_id: String,
    pub target_passion_id: String,
    pub weight: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterRecord {
    pub id: String,
    pub name: String,
    pub role: String,
    pub home_location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<String>,
    pub current_location: String,
    #[serde(default)]
    pub status_axes: Value,
    #[serde(default)]
    pub needs: Value,
    #[serde(default)]
    pub agenda: String,
    #[serde(default)]
    pub red_line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualities: Option<Value>,
    #[serde(default)]
    pub passion_pool: Vec<MicroPassionRecord>,
    #[serde(default)]
    pub active_passions: Vec<String>,
    #[serde(default)]
    pub trust_inertia: f64,
    #[serde(default)]
    pub thresholds: Vec<MacroPassionThresholdRecord>,
    #[serde(default)]
    pub zero_sum_passion_links: Vec<ZeroSumPassionLinkRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    /// Derived runtime attributes; never persisted directly.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub attributes: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocationRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_zones: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environmental_params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
    /// Derived runtime attributes; never persisted directly.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub attributes: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NpcBehaviorRecord {
    pub id: String,
    pub actor_id: String,
    pub priority: i32,
    pub conditions: Value,
    pub action: String,
    pub effects: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub title: String,
    pub description: String,
    pub event_type: String, // "canon" | "world"
    pub trigger: Value,
    pub effects: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_turns: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repeatable: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryletParameterRecord {
    pub param_name: String,
    pub query: String,
    /// Role binding scope: `current` keeps the actor at the player's current
    /// location; `any` allows a role to bind a remote or abstract actor.
    #[serde(
        default = "default_role_location_policy",
        skip_serializing_if = "is_default_role_location_policy"
    )]
    pub location_policy: String,
}

fn default_role_location_policy() -> String {
    "current".into()
}

fn is_default_role_location_policy(value: &str) -> bool {
    value == "current"
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryletContentRecord {
    pub text_template: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker_id: Option<String>,
    #[serde(
        default,
        rename = "sceneOpening",
        skip_serializing_if = "Option::is_none"
    )]
    pub scene_opening: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dialogue_quips: Option<Vec<Value>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryletChoiceRecord {
    pub choice_id: String,
    pub label: String,
    #[serde(
        default,
        rename = "systemVerb",
        skip_serializing_if = "Option::is_none"
    )]
    pub system_verb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Vec<Value>>,
    pub effects: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub divert_target: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChronoQualityPredicate {
    pub target_id: String,
    #[serde(default = "default_chrono_predicate_subject")]
    pub subject: String,
    pub operator: String,
    pub comparison_value: f64,
    #[serde(default = "default_specificity_weight")]
    pub specificity_weight: f64,
    #[serde(default)]
    pub entity_index: Option<usize>,
}

fn default_chrono_predicate_subject() -> String {
    "metric".into()
}

fn default_specificity_weight() -> f64 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChronoStoryletEffect {
    pub target_id: String,
    pub value: f64,
    #[serde(default)]
    pub incremental: bool,
    #[serde(default)]
    pub add_tags: Vec<String>,
    #[serde(default)]
    pub remove_tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChronoStoryletSettings {
    #[serde(default)]
    pub display_priority: i32,
    #[serde(default = "default_chrono_urgency_tag", alias = "urgency_tag")]
    pub urgency_color_tag: String,
    #[serde(default = "default_chrono_repeatable")]
    pub is_repeatable: bool,
    #[serde(default)]
    pub cooldown_ms: u64,
    #[serde(default)]
    pub preconditions: Vec<ChronoQualityPredicate>,
    #[serde(default)]
    pub required_tags: Vec<String>,
    #[serde(default)]
    pub required_any_tags: Vec<String>,
    #[serde(default)]
    pub forbidden_tags: Vec<String>,
    #[serde(default)]
    pub required_arc_phases: Vec<String>,
    #[serde(default)]
    pub ideal_state_vector: BTreeMap<String, f64>,
    /// Per-dimension salience distance weight (`v_j`); missing means 1.0.
    #[serde(default)]
    pub ideal_state_weights: BTreeMap<String, f64>,
    #[serde(default)]
    pub salience_bonus_tags: Vec<String>,
    #[serde(default)]
    pub effects_on_complete: Vec<ChronoStoryletEffect>,
}

impl Default for ChronoStoryletSettings {
    fn default() -> Self {
        Self {
            display_priority: 0,
            urgency_color_tag: default_chrono_urgency_tag(),
            is_repeatable: default_chrono_repeatable(),
            cooldown_ms: 0,
            preconditions: Vec::new(),
            required_tags: Vec::new(),
            required_any_tags: Vec::new(),
            forbidden_tags: Vec::new(),
            required_arc_phases: Vec::new(),
            ideal_state_vector: BTreeMap::new(),
            ideal_state_weights: BTreeMap::new(),
            salience_bonus_tags: Vec::new(),
            effects_on_complete: Vec::new(),
        }
    }
}

fn default_chrono_urgency_tag() -> String {
    "Standard".into()
}

fn default_chrono_repeatable() -> bool {
    true
}

fn default_chrono_storylet_settings() -> Option<ChronoStoryletSettings> {
    Some(ChronoStoryletSettings::default())
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoryletRecord {
    pub id: String,
    #[serde(default = "default_revision")]
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_revision: Option<u64>,
    pub title: String,
    #[serde(default)]
    pub ending: bool,
    #[serde(default)]
    pub location_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_tag: Option<String>,
    #[serde(default = "default_cluster_type")]
    pub cluster_type: String,
    #[serde(default = "default_urgency")]
    pub urgency_weight: f64,
    #[serde(default = "default_repeatability")]
    pub repeatability: String,
    #[serde(default)]
    pub cooldown_turns: u32,
    #[serde(default)]
    pub parameters: Vec<StoryletParameterRecord>,
    #[serde(default)]
    pub preconditions: Value,
    pub content: StoryletContentRecord,
    #[serde(default)]
    pub choices: Vec<StoryletChoiceRecord>,
    #[serde(default)]
    pub effects: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_variables: Option<Vec<crate::narrative_graph::LocalVariableDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<Value>,
    #[serde(
        default = "default_chrono_storylet_settings",
        skip_serializing_if = "Option::is_none"
    )]
    pub chrono: Option<ChronoStoryletSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    pub created_at: i64,
}

fn default_revision() -> u64 {
    1
}
fn default_cluster_type() -> String {
    "player_passage".into()
}
fn default_urgency() -> f64 {
    50.0
}
fn default_repeatability() -> String {
    "always".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StartLocationPolicy {
    pub mode: String, // "fixed" | "random" | "by_attribute"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_value: Option<Value>,
}

impl Default for StartLocationPolicy {
    fn default() -> Self {
        Self {
            mode: "fixed".into(),
            location_id: None,
            attribute_key: None,
            attribute_value: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActorLocationPolicy {
    pub mode: String, // "fixed" | "random" | "by_attribute"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_value: Option<Value>,
}

impl Default for ActorLocationPolicy {
    fn default() -> Self {
        Self {
            mode: "fixed".into(),
            location_id: None,
            attribute_key: None,
            attribute_value: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActorPopulationPolicy {
    pub mode: String, // "all" | "random" | "by_attribute"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute_value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_policy: Option<ActorLocationPolicy>,
}

impl Default for ActorPopulationPolicy {
    fn default() -> Self {
        Self {
            mode: "all".into(),
            count: None,
            attribute_key: None,
            attribute_value: None,
            location_policy: Some(ActorLocationPolicy::default()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioRecord {
    pub id: String,
    pub name: String,
    pub seed: u64,
    #[serde(default)]
    pub start_location_policy: StartLocationPolicy,
    #[serde(default)]
    pub initial_qualities: BTreeMap<String, Value>,
    #[serde(default)]
    pub actor_locations: BTreeMap<String, String>,
    #[serde(default)]
    pub actor_population_policy: ActorPopulationPolicy,
    /// Gameplay tag that seeds `Blackboard.current_dramatic_arc_phase`.
    #[serde(default)]
    pub current_arc_phase: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResolvedScenarioStart {
    pub player_start_location: String,
    pub actor_locations: BTreeMap<String, String>,
    pub active_actor_ids: Vec<String>,
    pub effective_qualities: BTreeMap<String, Value>,
    pub seed: u64,
    pub actors_per_location: BTreeMap<String, Vec<String>>,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibrarySummaryItem {
    pub id: String,
    pub title: String,
    pub excerpt: String,
    pub location_tags: Vec<String>,
    pub role_tags: Vec<String>,
    pub choice_count: usize,
    pub revision: u64,
    pub approved_revision: Option<u64>,
    pub ending: bool,
    pub created_at: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LibraryQuery {
    pub q: Option<String>,
    pub status: Option<String>, // "all" | "draft" | "approved"
    pub location_tag: Option<String>,
    pub ending: Option<String>, // "all" | "ending" | "non-ending"
    pub sort: Option<String>,   // "title" | "created_desc"
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibraryPageResult {
    pub items: Vec<LibrarySummaryItem>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
    pub available_location_tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuthoringOptions {
    pub quality_types: Vec<String>,
    pub repeatability_modes: Vec<String>,
    pub cluster_types: Vec<String>,
    pub policy_modes: Vec<String>,
    pub event_types: Vec<String>,
}
/// Authored content kinds addressable through `/api/v2/content/{entity}`.
pub const CONTENT_KINDS: [&str; 9] = [
    "qualities",
    "entities",
    "folders",
    "trait_templates",
    "assets",
    "storylets",
    "npc_behaviors",
    "events",
    "scenarios",
];

/// Project-tree roots that own folders.
pub const FOLDER_ROOTS: [&str; 5] = [
    "storylets",
    "entities",
    "qualities",
    "trait_templates",
    "assets",
];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FolderRecord {
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// One of [`FOLDER_ROOTS`]; a folder never leaves its root.
    pub root: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Character,
    Location,
    Item,
    Fact,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PropertyType {
    Bool,
    String,
    Float,
    Int,
    Asset,
    Entity,
}

/// A trait-template variable: the Unreal-Blueprint-style authoring unit.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PropertyDefinition {
    #[serde(default)]
    pub id: String,
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub property_type: PropertyType,
    #[serde(default)]
    pub is_array: bool,
    #[serde(default)]
    pub multiline: bool,
    #[serde(default)]
    pub default: Value,
}

/// An entity-local variable with the same shape as [`PropertyDefinition`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityProperty {
    #[serde(default)]
    pub id: String,
    pub key: String,
    pub name: String,
    #[serde(rename = "type")]
    pub property_type: PropertyType,
    #[serde(default)]
    pub is_array: bool,
    #[serde(default)]
    pub multiline: bool,
    #[serde(default)]
    pub value: Value,
}

/// One composed trait template; override keys are [`PropertyDefinition::id`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraitInstance {
    pub template_id: String,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraitTemplateRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub key: String,
    /// True when the server generated `key` and may regenerate it on rename.
    #[serde(default)]
    pub key_auto: bool,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub properties: Vec<PropertyDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

/// Every [`CharacterRecord`] field except identity and timestamps; persisted
/// inside an entity of type `character`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterSection {
    pub role: String,
    pub home_location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faction_id: Option<String>,
    pub current_location: String,
    #[serde(default)]
    pub status_axes: Value,
    #[serde(default)]
    pub needs: Value,
    #[serde(default)]
    pub agenda: String,
    #[serde(default)]
    pub red_line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schedule: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualities: Option<Value>,
    #[serde(default)]
    pub passion_pool: Vec<MicroPassionRecord>,
    #[serde(default)]
    pub active_passions: Vec<String>,
    #[serde(default)]
    pub trust_inertia: f64,
    #[serde(default)]
    pub thresholds: Vec<MacroPassionThresholdRecord>,
    #[serde(default)]
    pub zero_sum_passion_links: Vec<ZeroSumPassionLinkRecord>,
}

impl Default for CharacterSection {
    fn default() -> Self {
        Self {
            role: String::new(),
            home_location: String::new(),
            faction_id: None,
            current_location: String::new(),
            status_axes: Value::Null,
            needs: Value::Null,
            agenda: String::new(),
            red_line: String::new(),
            schedule: None,
            qualities: None,
            passion_pool: Vec::new(),
            active_passions: Vec::new(),
            trust_inertia: 0.0,
            thresholds: Vec::new(),
            zero_sum_passion_links: Vec::new(),
        }
    }
}

/// Every [`LocationRecord`] field except identity, name, description, and
/// timestamps; persisted inside an entity of type `location`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LocationSection {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connected_zones: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environmental_params: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub devices: Option<Value>,
}

/// The single authorable world object: characters, locations, items, facts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub object_id: String,
    #[serde(default)]
    pub game_id: String,
    /// True when the server generated `game_id` and may regenerate it on rename.
    #[serde(default)]
    pub game_id_auto: bool,
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: EntityType,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub properties: Vec<EntityProperty>,
    #[serde(default)]
    pub traits: Vec<TraitInstance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<CharacterSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<LocationSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StorageLocationRecord {
    pub storage_type: String,
    pub storage_id: String,
    pub created_at: i64,
    pub reference_type: String,
    pub path: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssetVersionRecord {
    pub id: String,
    pub file_path: String,
    pub creator: String,
    pub tool_version: String,
    #[serde(default)]
    pub description: Option<String>,
    pub created_at: i64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub custom_data: Value,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub storage: Vec<StorageLocationRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssetRecord {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub versions: Vec<AssetVersionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldObjectLayer {
    Item,
    Fact,
}

/// An item or fact entity projected into the runtime graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldObjectRecord {
    pub id: String,
    pub name: String,
    pub layer: WorldObjectLayer,
    pub attributes: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceVia {
    Id,
    Role,
    Tag,
}

/// Where an entity, asset, or trait template is used.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContentReference {
    pub source_kind: String,
    pub source_id: String,
    pub source_label: String,
    pub node_id: Option<String>,
    pub node_label: Option<String>,
    pub field: String,
    pub via: ReferenceVia,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProjectBundleContent {
    #[serde(default)]
    pub qualities: Vec<QualityRecord>,
    #[serde(default)]
    pub entities: Vec<EntityRecord>,
    #[serde(default)]
    pub folders: Vec<FolderRecord>,
    #[serde(default)]
    pub trait_templates: Vec<TraitTemplateRecord>,
    #[serde(default)]
    pub assets: Vec<AssetRecord>,
    #[serde(default)]
    pub storylets: Vec<StoryletRecord>,
    #[serde(default, rename = "npcBehaviors")]
    pub npc_behaviors: Vec<NpcBehaviorRecord>,
    #[serde(default)]
    pub events: Vec<EventRecord>,
    #[serde(default)]
    pub scenarios: Vec<ScenarioRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectBundle {
    pub format: String,
    pub schema_version: u32,
    pub project: ProjectMetadata,
    #[serde(default)]
    pub content: ProjectBundleContent,
}

#[cfg(test)]
mod chrono_contract_tests {
    use super::StoryletRecord;
    use serde_json::json;

    #[test]
    fn storylet_pacing_metadata_is_persisted_as_v5_fields() {
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id": "high-priority",
            "title": "High Priority",
            "content": { "text_template": "" },
            "created_at": 1,
            "chrono": {
                "display_priority": 70_000,
                "urgency_tag": "Urgency.MustPlay"
            }
        }))
        .unwrap();

        let chrono = storylet.chrono.unwrap();
        assert_eq!(chrono.display_priority, 70_000);
        assert_eq!(chrono.urgency_color_tag, "Urgency.MustPlay");

        let serialized = serde_json::to_value(chrono).unwrap();
        assert_eq!(serialized["display_priority"], 70_000);
        assert_eq!(serialized["urgency_color_tag"], "Urgency.MustPlay");
        assert!(serialized.get("urgency_tag").is_none());
    }

    #[test]
    fn older_storylets_receive_required_chrono_display_defaults() {
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id": "legacy",
            "title": "Legacy",
            "content": { "text_template": "" },
            "created_at": 1
        }))
        .unwrap();
        let chrono = storylet.chrono.unwrap();
        assert_eq!(chrono.display_priority, 0);
        assert_eq!(chrono.urgency_color_tag, "Standard");
    }
}
