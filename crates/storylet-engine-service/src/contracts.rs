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
    /// Canon events with a story time form the chronicle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story_time_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location_id: Option<String>,
    /// Fact tags (under `fact.`) a participant learns when told this event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl EventRecord {
    /// A canon event with a story time is a chronicle entry.
    pub fn is_chronicle_entry(&self) -> bool {
        self.event_type == "canon" && self.story_time.is_some()
    }
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
    pub required_tags: Vec<String>,
    #[serde(default)]
    pub required_any_tags: Vec<String>,
    #[serde(default)]
    pub forbidden_tags: Vec<String>,
    #[serde(default)]
    pub required_arc_phases: Vec<String>,
    #[serde(default)]
    pub salience_bonus_tags: Vec<String>,
}


impl Default for ChronoStoryletSettings {
    fn default() -> Self {
        Self {
            display_priority: 0,
            urgency_color_tag: default_chrono_urgency_tag(),
            is_repeatable: default_chrono_repeatable(),
            cooldown_ms: 0,
            required_tags: Vec::new(),
            required_any_tags: Vec::new(),
            forbidden_tags: Vec::new(),
            required_arc_phases: Vec::new(),
            salience_bonus_tags: Vec::new(),
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
    #[serde(default = "default_storylet_origin")]
    pub origin: String,
    #[serde(default)]
    pub ending: bool,
    /// Filler offered when no progress storylet is available.
    #[serde(default)]
    pub is_fallback: bool,
    /// Participant entity ids who play this storylet; empty = any human.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub played_by: Vec<String>,
    /// Eligible clock slots; empty means any slot.
    #[serde(default)]
    pub window: Vec<u32>,
    #[serde(default = "default_learn_visibility")]
    pub default_learn_visibility: String,
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
    pub content: StoryletContentRecord,
    #[serde(default)]
    pub choices: Vec<StoryletChoiceRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precondition_graph: Option<crate::narrative_graph::NarrativeGraph>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<crate::narrative_graph::NarrativeGraph>,
    #[serde(
        default = "default_chrono_storylet_settings",
        skip_serializing_if = "Option::is_none"
    )]
    pub chrono: Option<ChronoStoryletSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    pub created_at: i64,
}

fn default_learn_visibility() -> String { "private".into() }
fn default_storylet_origin() -> String { "human".into() }
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
    pub initial_properties: BTreeMap<String, BTreeMap<String, Value>>,
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
    pub effective_properties: BTreeMap<String, BTreeMap<String, Value>>,
    pub seed: u64,
    pub actors_per_location: BTreeMap<String, Vec<String>>,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LibrarySummaryItem {
    pub id: String,
    pub title: String,
    #[serde(default = "default_storylet_origin")]
    pub origin: String,
    pub excerpt: String,
    pub location_tags: Vec<String>,
    pub role_tags: Vec<String>,
    #[serde(default)]
    pub window: Vec<u32>,
    #[serde(default)]
    pub played_by: Vec<String>,
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
    pub repeatability_modes: Vec<String>,
    pub cluster_types: Vec<String>,
    pub policy_modes: Vec<String>,
    pub event_types: Vec<String>,
}
/// Authored content kinds addressable through the service.
pub const CONTENT_KINDS: [&str; 9] = [
    "entities",
    "entity_templates",
    "gameplay_tags",
    "folders",
    "trait_templates",
    "assets",
    "storylets",
    "events",
    "scenarios",
];

/// Project-tree roots that own folders.
pub const FOLDER_ROOTS: [&str; 5] = [
    "storylets",
    "entity_templates",
    "entities",
    "trait_templates",
    "assets",
];
/// Canonical storylet pacing categories accepted by authored projects.
pub const STORYLET_CLUSTER_TYPES: [&str; 4] = [
    "player_passage",
    "linear_progression",
    "wheel_deck",
    "aftermath_accelerator",
];

/// The persisted project interchange schema.
pub const PROJECT_BUNDLE_SCHEMA_VERSION: u32 = 5;


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
pub enum PropertyType {
    Bool,
    String,
    Float,
    Int,
    Asset,
    Entity,
    Storylet,
    Tag,
    Json,
}

/// A reusable typed property definition for trait templates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraitPropertyDefinition {
    #[serde(flatten)]
    pub property: PropertyDefinition,
}

impl TraitPropertyDefinition {
    pub fn key(&self) -> &str {
        &self.property.key
    }
}

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemTraitPropertySpec {
    pub key: &'static str,
    pub name: &'static str,
    pub property_type: PropertyType,
    pub is_array: bool,
    pub default_json: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemTraitTemplateSpec {
    pub key: &'static str,
    pub name: &'static str,
    pub allow_multiple: bool,
    pub properties: &'static [SystemTraitPropertySpec],
}

/// System traits added after v5 shipped. Older projects get them virtually and
/// bundles without them still validate.
pub const ADDITIVE_SYSTEM_TRAITS: &[&str] = &["participant", "connection", "clock", "milestone", "intent_probe"];

pub const SYSTEM_TRAIT_TEMPLATES: &[SystemTraitTemplateSpec] = &[
    SystemTraitTemplateSpec { key: "participant", name: "Participant", allow_multiple: false, properties: &[
        SystemTraitPropertySpec { key: "controller", name: "Controller", property_type: PropertyType::String, is_array: false, default_json: "\"human\"" },
        SystemTraitPropertySpec { key: "seat", name: "Seat", property_type: PropertyType::Int, is_array: false, default_json: "0" },
        SystemTraitPropertySpec { key: "start_location", name: "Start Location", property_type: PropertyType::Entity, is_array: false, default_json: "null" },
    ] },
    SystemTraitTemplateSpec { key: "connection", name: "Connection", allow_multiple: true, properties: &[
        SystemTraitPropertySpec { key: "to", name: "To", property_type: PropertyType::Entity, is_array: false, default_json: "null" },
        SystemTraitPropertySpec { key: "open_slots", name: "Open Slots", property_type: PropertyType::Int, is_array: true, default_json: "[]" },
        SystemTraitPropertySpec { key: "requires_fact", name: "Requires Fact", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "on_close", name: "On Close", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "return_to", name: "Return To", property_type: PropertyType::Entity, is_array: false, default_json: "null" },
    ] },
    SystemTraitTemplateSpec { key: "clock", name: "Clock", allow_multiple: false, properties: &[
        SystemTraitPropertySpec { key: "slot_labels", name: "Slot Labels", property_type: PropertyType::String, is_array: true, default_json: "[]" },
        SystemTraitPropertySpec { key: "actions_per_slot", name: "Actions Per Slot", property_type: PropertyType::Int, is_array: false, default_json: "2" },
    ] },
    SystemTraitTemplateSpec { key: "milestone", name: "Milestone", allow_multiple: true, properties: &[
        SystemTraitPropertySpec { key: "key", name: "Key", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "name", name: "Name", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "routes", name: "Routes", property_type: PropertyType::Storylet, is_array: true, default_json: "[]" },
        SystemTraitPropertySpec { key: "reachable_for", name: "Reachable For", property_type: PropertyType::String, is_array: true, default_json: "[]" },
        SystemTraitPropertySpec { key: "deadline_slot", name: "Deadline Slot", property_type: PropertyType::Int, is_array: false, default_json: "-1" },
    ] },
    SystemTraitTemplateSpec { key: "intent_probe", name: "Intent Probe", allow_multiple: true, properties: &[
        SystemTraitPropertySpec { key: "name", name: "Name", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "holder_knows", name: "Holder Knows", property_type: PropertyType::String, is_array: true, default_json: "[]" },
        SystemTraitPropertySpec { key: "colocated_with", name: "Colocated With", property_type: PropertyType::Entity, is_array: false, default_json: "null" },
        SystemTraitPropertySpec { key: "expected_routes", name: "Expected Routes", property_type: PropertyType::Storylet, is_array: true, default_json: "[]" },
    ] },    SystemTraitTemplateSpec { key: "character_settings", name: "Character Settings", allow_multiple: false, properties: &[
        SystemTraitPropertySpec { key: "role", name: "Role", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "home_location", name: "Home Location", property_type: PropertyType::Entity, is_array: false, default_json: "null" },
        SystemTraitPropertySpec { key: "current_location", name: "Current Location", property_type: PropertyType::Entity, is_array: false, default_json: "null" },
    ] },
    SystemTraitTemplateSpec { key: "location_settings", name: "Location Settings", allow_multiple: false, properties: &[
        SystemTraitPropertySpec { key: "connected_zones", name: "Connected Zones", property_type: PropertyType::Entity, is_array: true, default_json: "[]" },
    ] },
    SystemTraitTemplateSpec { key: "passion", name: "Passion", allow_multiple: true, properties: &[
        SystemTraitPropertySpec { key: "passion_id", name: "Passion ID", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "current_value", name: "Current Value", property_type: PropertyType::Float, is_array: false, default_json: "0.0" },
        SystemTraitPropertySpec { key: "weight", name: "Weight", property_type: PropertyType::Float, is_array: false, default_json: "1.0" },
        SystemTraitPropertySpec { key: "is_hidden", name: "Hidden", property_type: PropertyType::Bool, is_array: false, default_json: "false" },
    ] },
    SystemTraitTemplateSpec { key: "passion_threshold", name: "Passion Threshold", allow_multiple: true, properties: &[
        SystemTraitPropertySpec { key: "threshold_id", name: "Threshold ID", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "trigger_value", name: "Trigger Value", property_type: PropertyType::Float, is_array: false, default_json: "0.0" },
        SystemTraitPropertySpec { key: "direction", name: "Direction", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "linked_graph_asset_id", name: "Linked Graph", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "is_currently_active", name: "Active", property_type: PropertyType::Bool, is_array: false, default_json: "false" },
    ] },
    SystemTraitTemplateSpec { key: "passion_link", name: "Passion Link", allow_multiple: true, properties: &[
        SystemTraitPropertySpec { key: "target_character_id", name: "Target Character", property_type: PropertyType::Entity, is_array: false, default_json: "null" },
        SystemTraitPropertySpec { key: "source_passion_id", name: "Source Passion", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "target_passion_id", name: "Target Passion", property_type: PropertyType::String, is_array: false, default_json: "\"\"" },
        SystemTraitPropertySpec { key: "weight", name: "Weight", property_type: PropertyType::Float, is_array: false, default_json: "0.0" },
    ] },
];

/// An entity-owned typed property value.
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
    #[serde(default, alias = "default")]
    pub value: Value,
}

/// One composed trait instance. Stable IDs make repeatable traits independently editable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraitInstance {
    #[serde(default)]
    pub id: String,
    pub template_id: String,
    #[serde(default)]
    pub values: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GameplayTagRecord {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub protected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}
/// Concrete entity instances (e.g. David using Character template, or empty entities).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub root_tag: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub properties: Vec<EntityProperty>,
    #[serde(default)]
    pub traits: Vec<TraitInstance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}


/// Authored entity consumed directly by runtime projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityTemplateRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub object_id: String,
    #[serde(default)]
    pub game_id: String,
    #[serde(default)]
    pub game_id_auto: bool,
    pub name: String,
    pub root_tag: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub properties: Vec<EntityProperty>,
    #[serde(default)]
    pub traits: Vec<TraitInstance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,
}

impl EntityTemplateRecord {
    /// Validate intrinsic category and canonical tag namespace invariants.
    pub fn validate_tags(&self, registered: &[GameplayTagRecord]) -> Result<(), String> {
        let root = self.root_tag.trim();
        if root.is_empty() || root.contains('.') {
            return Err("root_tag must be a non-empty name without dots".into());
        }
        let root_match = registered.iter().find(|tag| tag.name.eq_ignore_ascii_case(root))
            .ok_or_else(|| format!("root gameplay tag '{root}' is not registered"))?;
        if root_match.name.contains('.') {
            return Err("root gameplay tag must not contain dots".into());
        }
        for tag in &self.tags {
            let canonical = tag.trim();
            if canonical.is_empty() || (!canonical.eq_ignore_ascii_case(root)
                && !canonical.to_ascii_lowercase().starts_with(&(root.to_ascii_lowercase() + ".")))
            {
                return Err(format!("tag '{canonical}' is outside root namespace '{root}'"));
            }
            if !registered.iter().any(|item| item.name.eq_ignore_ascii_case(canonical)) {
                return Err(format!("gameplay tag '{canonical}' is not registered"));
            }
        }
        if self.tags.iter().any(|tag| tag.eq_ignore_ascii_case(root)) {
            return Err("root_tag must not be duplicated in tags".into());
        }
        Ok(())
    }
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
    pub properties: Vec<TraitPropertyDefinition>,
    #[serde(default)]
    pub allow_multiple: bool,
    #[serde(default)]
    pub system: bool,
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
    pub entities: Vec<EntityRecord>,
    #[serde(default)]
    pub entity_templates: Vec<EntityTemplateRecord>,
    #[serde(default)]
    pub gameplay_tags: Vec<GameplayTagRecord>,
    #[serde(default)]
    pub folders: Vec<FolderRecord>,
    #[serde(default)]
    pub trait_templates: Vec<TraitTemplateRecord>,
    #[serde(default)]
    pub assets: Vec<AssetRecord>,
    #[serde(default)]
    pub storylets: Vec<StoryletRecord>,
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
    use super::{ScenarioRecord, StoryletRecord};
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
    #[test]
    fn storylet_and_scenario_use_typed_property_shapes() {
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id": "s1",
            "title": "A storylet",
            "content": { "text_template": "" },
            "precondition_graph": { "nodes": [], "edges": [] },
            "created_at": 1
        }))
        .unwrap();
        assert!(storylet.precondition_graph.is_some());

        let scenario: ScenarioRecord = serde_json::from_value(json!({
            "id": "scenario",
            "name": "Start",
            "seed": 1,
            "start_location_policy": { "mode": "fixed" },
            "initial_properties": { "entity-1": { "mood": "tense" } }
        }))
        .unwrap();
        assert_eq!(scenario.initial_properties["entity-1"]["mood"], "tense");
    }
}
