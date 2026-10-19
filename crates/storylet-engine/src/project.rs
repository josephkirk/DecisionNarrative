use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::storylets::{ActorState, FactRef, MetCharacterRequirement, VisitedLocationRequirement};
use crate::{Effect, NpcBehaviorRule, PredicateSet, StoryletDefinition, WorldState};

pub const PROJECT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectBundle {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub revision: u64,
    #[serde(default)]
    pub qualities: BTreeMap<String, QualityDefinition>,
    #[serde(default)]
    pub locations: BTreeMap<String, LocationDefinition>,
    #[serde(default)]
    pub characters: BTreeMap<String, CharacterDefinition>,
    #[serde(default)]
    pub events: BTreeMap<String, EventDefinition>,
    #[serde(default)]
    pub storylets: BTreeMap<String, StoryletDefinition>,
    #[serde(default)]
    pub npc_behaviors: BTreeMap<String, NpcBehaviorRule>,
    #[serde(default)]
    pub scenarios: BTreeMap<String, ScenarioDefinition>,
}

impl ProjectBundle {
    pub fn empty(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            id: id.into(),
            name: name.into(),
            revision: 0,
            qualities: BTreeMap::new(),
            locations: BTreeMap::new(),
            characters: BTreeMap::new(),
            events: BTreeMap::new(),
            storylets: BTreeMap::new(),
            npc_behaviors: BTreeMap::new(),
            scenarios: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityType {
    Currency,
    Metric,
    Tool,
    Progress,
    Territory,
    Menace,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QualityDefinition {
    pub key: String,
    pub kind: QualityType,
    pub default_value: Value,
    pub description: String,
    #[serde(default)]
    pub min_value: f64,
    #[serde(default = "default_quality_max")]
    pub max_value: f64,
    #[serde(default)]
    pub is_hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub menace_crisis_storylet_id: Option<String>,
}

fn default_quality_max() -> f64 {
    100.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocationDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub connected_locations: Vec<String>,
    pub map_position: (f64, f64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CharacterDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub role_tags: Vec<String>,
    pub home_location: String,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub red_lines: Vec<String>,
    pub voice_profile: String,
    #[serde(default)]
    pub schedule: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EventDefinition {
    pub id: String,
    pub title: String,
    pub description: String,
    pub trigger: PredicateSet,
    #[serde(default)]
    pub effects: Vec<Effect>,
    #[serde(default)]
    pub delay_turns: u64,
    #[serde(default)]
    pub repeatable: bool,
    #[serde(default)]
    pub event_type: EventType,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Canon,
    #[default]
    World,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioDefinition {
    pub id: String,
    pub name: String,
    pub player_start_location: String,
    #[serde(default)]
    pub initial_qualities: BTreeMap<String, Value>,
    #[serde(default)]
    pub actor_locations: BTreeMap<String, String>,
    #[serde(default)]
    pub storylet_ids: Vec<String>,
    pub seed: u64,
    #[serde(default)]
    pub event_ids: Vec<String>,
    #[serde(default)]
    pub active_actor_ids: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectIssue {
    pub path: String,
    pub code: String,
    pub message: String,
}

pub fn validate_project(project: &ProjectBundle) -> Vec<ProjectIssue> {
    let mut issues = Vec::new();
    let mut global_ids = BTreeSet::new();
    if project.schema_version != PROJECT_SCHEMA_VERSION {
        issue(
            &mut issues,
            "schema_version",
            "unsupported_schema_version",
            format!("schema version {} is not supported", project.schema_version),
        );
    }
    if project.id.trim().is_empty() {
        issue(&mut issues, "id", "empty_id", "project id is required");
    }
    if project.name.trim().is_empty() {
        issue(
            &mut issues,
            "name",
            "empty_name",
            "project name is required",
        );
    }
    register_id(&mut issues, &mut global_ids, "project", &project.id);

    for (key, quality) in &project.qualities {
        let path = format!("qualities.{key}");
        register_record(&mut issues, &mut global_ids, &path, key, &quality.key);
        if quality.default_value.is_null() {
            issue(
                &mut issues,
                format!("{path}.default_value"),
                "null_default",
                "quality default must be a JSON value",
            );
        }
        if let Some(number) = quality.default_value.as_f64() {
            if !number.is_finite() {
                issue(
                    &mut issues,
                    format!("{path}.default_value"),
                    "non_finite_default",
                    "quality default must be finite",
                );
            }
        }
        if !quality.min_value.is_finite()
            || !quality.max_value.is_finite()
            || quality.min_value > quality.max_value
        {
            issue(
                &mut issues,
                format!("{path}.min_value"),
                "invalid_bounds",
                "quality bounds must be finite and minimum must not exceed maximum",
            );
        }
    }

    for (key, location) in &project.locations {
        let path = format!("locations.{key}");
        register_record(&mut issues, &mut global_ids, &path, key, &location.id);
        if !location.map_position.0.is_finite() || !location.map_position.1.is_finite() {
            issue(
                &mut issues,
                format!("{path}.map_position"),
                "non_finite_position",
                "map coordinates must be finite",
            );
        }
        for (index, connected) in location.connected_locations.iter().enumerate() {
            if !project.locations.contains_key(connected) {
                issue(
                    &mut issues,
                    format!("{path}.connected_locations[{index}]"),
                    "missing_location",
                    format!("location '{connected}' does not exist"),
                );
            }
        }
    }

    for (key, character) in &project.characters {
        let path = format!("characters.{key}");
        register_record(&mut issues, &mut global_ids, &path, key, &character.id);
        validate_location_ref(
            &mut issues,
            &project.locations,
            format!("{path}.home_location"),
            &character.home_location,
        );
        for (schedule, location) in &character.schedule {
            validate_location_ref(
                &mut issues,
                &project.locations,
                format!("{path}.schedule.{schedule}"),
                location,
            );
        }
        let mut roles = BTreeSet::new();
        for (index, role) in character.role_tags.iter().enumerate() {
            if role.trim().is_empty() || !roles.insert(role) {
                issue(
                    &mut issues,
                    format!("{path}.role_tags[{index}]"),
                    "invalid_role_tag",
                    "role tags must be nonempty and unique",
                );
            }
        }
    }

    for (key, event) in &project.events {
        let path = format!("events.{key}");
        register_record(&mut issues, &mut global_ids, &path, key, &event.id);
        validate_rules(
            &mut issues,
            project,
            &format!("{path}.trigger"),
            &event.trigger,
        );
        validate_effects(
            &mut issues,
            project,
            &format!("{path}.effects"),
            &event.effects,
        );
    }

    for (key, storylet) in &project.storylets {
        let path = format!("storylets.{key}");
        register_record(&mut issues, &mut global_ids, &path, key, &storylet.id);
        if !storylet.urgency_weight.is_finite() {
            issue(
                &mut issues,
                format!("{path}.urgency_weight"),
                "non_finite_weight",
                "urgency weight must be finite",
            );
        }
        validate_rules(
            &mut issues,
            project,
            &format!("{path}.trigger"),
            &storylet.trigger,
        );
        if storylet.ending && storylet.choices.is_empty() {
            issue(
                &mut issues,
                format!("{path}.choices"),
                "ending_without_choice",
                "ending storylets must have at least one choice",
            );
        }
        let mut choices = BTreeSet::new();
        for (index, choice) in storylet.choices.iter().enumerate() {
            let choice_path = format!("{path}.choices[{index}]");
            if choice.id.trim().is_empty() || !choices.insert(&choice.id) {
                issue(
                    &mut issues,
                    format!("{choice_path}.id"),
                    "duplicate_choice_id",
                    "choice ids must be nonempty and unique within the storylet",
                );
            }
            validate_rules(
                &mut issues,
                project,
                &format!("{choice_path}.requirements"),
                &choice.requirements,
            );
            validate_effects(
                &mut issues,
                project,
                &format!("{choice_path}.effects"),
                &choice.effects,
            );
            if choice
                .divert_target
                .as_ref()
                .is_some_and(|target| !project.storylets.contains_key(target))
            {
                issue(
                    &mut issues,
                    format!("{choice_path}.divert_target"),
                    "missing_storylet",
                    "divert target must reference a project storylet",
                );
            }
        }
        for (index, requirement) in storylet
            .preconditions
            .visited_locations_any_of
            .iter()
            .enumerate()
        {
            let (field, value, missing) = match requirement {
                VisitedLocationRequirement::LocationId { location_id } => (
                    "location_id",
                    location_id,
                    !project.locations.contains_key(location_id),
                ),
                VisitedLocationRequirement::LocationTag { tag } => ("tag", tag, false),
            };
            let requirement_path =
                format!("{path}.preconditions.visited_locations_any_of[{index}].{field}");
            if value.trim().is_empty() {
                issue(
                    &mut issues,
                    requirement_path,
                    "empty_precondition",
                    "visited-location requirements must be nonempty",
                );
            } else if missing {
                issue(
                    &mut issues,
                    requirement_path,
                    "missing_location",
                    format!("location '{value}' does not exist"),
                );
            }
        }
        for (index, requirement) in storylet
            .preconditions
            .met_characters_any_of
            .iter()
            .enumerate()
        {
            let (field, value, missing) = match requirement {
                MetCharacterRequirement::CharacterId { character_id } => (
                    "character_id",
                    character_id,
                    !project.characters.contains_key(character_id),
                ),
                MetCharacterRequirement::RoleTag { role } => ("role", role, false),
            };
            let requirement_path =
                format!("{path}.preconditions.met_characters_any_of[{index}].{field}");
            if value.trim().is_empty() {
                issue(
                    &mut issues,
                    requirement_path,
                    "empty_precondition",
                    "met-character requirements must be nonempty",
                );
            } else if missing {
                issue(
                    &mut issues,
                    requirement_path,
                    "missing_character",
                    format!("character '{value}' does not exist"),
                );
            }
        }
        for (index, role) in storylet.required_roles.iter().enumerate() {
            if role.role.trim().is_empty() {
                issue(
                    &mut issues,
                    format!("{path}.required_roles[{index}]"),
                    "empty_role",
                    "required role must be nonempty",
                );
            } else if !project
                .characters
                .values()
                .any(|character| character.role_tags.contains(&role.role))
            {
                issue(
                    &mut issues,
                    format!("{path}.required_roles[{index}]"),
                    "undefined_role",
                    format!("no project character defines role '{}'", role.role),
                );
            }
        }
    }

    for (key, rule) in &project.npc_behaviors {
        let path = format!("npc_behaviors.{key}");
        register_record(&mut issues, &mut global_ids, &path, key, &rule.id);
        if rule.action.trim().is_empty() {
            issue(
                &mut issues,
                format!("{path}.action"),
                "empty_action",
                "NPC behavior action is required",
            );
        }
        if !project.characters.contains_key(&rule.actor_id) {
            issue(
                &mut issues,
                format!("{path}.actor_id"),
                "missing_character",
                format!("character '{}' does not exist", rule.actor_id),
            );
        }
        validate_rules(
            &mut issues,
            project,
            &format!("{path}.conditions"),
            &rule.conditions,
        );
        validate_effects(
            &mut issues,
            project,
            &format!("{path}.effects"),
            &rule.effects,
        );
    }

    for (key, scenario) in &project.scenarios {
        let path = format!("scenarios.{key}");
        register_record(&mut issues, &mut global_ids, &path, key, &scenario.id);
        validate_location_ref(
            &mut issues,
            &project.locations,
            format!("{path}.player_start_location"),
            &scenario.player_start_location,
        );
        for (quality, value) in &scenario.initial_qualities {
            if !project.qualities.contains_key(quality) {
                issue(
                    &mut issues,
                    format!("{path}.initial_qualities.{quality}"),
                    "missing_quality",
                    "scenario quality is not defined by the project",
                );
            }
            if value.is_null() {
                issue(
                    &mut issues,
                    format!("{path}.initial_qualities.{quality}"),
                    "null_value",
                    "scenario quality values must not be null",
                );
            }
        }
        for (actor_id, location) in &scenario.actor_locations {
            if !project.characters.contains_key(actor_id) {
                issue(
                    &mut issues,
                    format!("{path}.actor_locations.{actor_id}"),
                    "missing_character",
                    "scenario actor does not exist",
                );
            }
            validate_location_ref(
                &mut issues,
                &project.locations,
                format!("{path}.actor_locations.{actor_id}"),
                location,
            );
        }
        if let Some(ref active) = scenario.active_actor_ids {
            for (index, actor_id) in active.iter().enumerate() {
                if !project.characters.contains_key(actor_id) {
                    issue(
                        &mut issues,
                        format!("{path}.active_actor_ids[{index}]"),
                        "missing_character",
                        format!("active character '{actor_id}' does not exist"),
                    );
                }
            }
        }
        for (index, event_id) in scenario.event_ids.iter().enumerate() {
            if !project.events.contains_key(event_id) {
                issue(
                    &mut issues,
                    format!("{path}.event_ids[{index}]"),
                    "missing_event",
                    "scenario event does not exist",
                );
            }
        }
        for (index, storylet_id) in scenario.storylet_ids.iter().enumerate() {
            match project.storylets.get(storylet_id) {
                None => issue(
                    &mut issues,
                    format!("{path}.storylet_ids[{index}]"),
                    "missing_storylet",
                    "scenario storylet does not exist",
                ),
                Some(storylet) if !storylet.approved => issue(
                    &mut issues,
                    format!("{path}.storylet_ids[{index}]"),
                    "storylet_not_approved",
                    "a playable scenario may include only approved storylets",
                ),
                Some(_) => {}
            }
        }
    }
    issues
}

pub fn build_world(
    project: &ProjectBundle,
    scenario_id: &str,
) -> Result<WorldState, Vec<ProjectIssue>> {
    let mut issues = validate_project(project);
    let Some(scenario) = project.scenarios.get(scenario_id) else {
        issue(
            &mut issues,
            format!("scenarios.{scenario_id}"),
            "missing_scenario",
            "scenario does not exist",
        );
        return Err(issues);
    };
    if !issues.is_empty() {
        return Err(issues);
    }

    let mut world = WorldState {
        scenario_id: Some(scenario.id.clone()),
        random_seed: scenario.seed,
        current_location: Some(scenario.player_start_location.clone()),
        location_tags: project
            .locations
            .iter()
            .map(|(id, location)| (id.clone(), location.tags.iter().cloned().collect()))
            .collect(),
        visited_locations: std::collections::BTreeSet::from([scenario
            .player_start_location
            .clone()]),
        ..WorldState::default()
    };
    for (key, definition) in &project.qualities {
        world
            .qualities
            .insert(key.clone(), definition.default_value.clone());
    }
    world.qualities.extend(scenario.initial_qualities.clone());
    for (id, character) in &project.characters {
        if let Some(ref active) = scenario.active_actor_ids {
            if !active.contains(id) {
                continue;
            }
        }
        let current_location = scenario
            .actor_locations
            .get(id)
            .unwrap_or(&character.home_location);
        world.actors.insert(
            id.clone(),
            ActorState {
                role_tags: character.role_tags.iter().cloned().collect(),
                current_location: current_location.clone(),
                busy: false,
            },
        );
    }
    if let Some(location_id) = &world.current_location {
        world.met_characters = world
            .actors
            .iter()
            .filter(|(_, actor)| actor.current_location == *location_id)
            .map(|(actor_id, _)| actor_id.clone())
            .collect();
    }
    Ok(world)
}

fn validate_rules(
    issues: &mut Vec<ProjectIssue>,
    project: &ProjectBundle,
    path: &str,
    rules: &PredicateSet,
) {
    for (index, predicate) in rules.all_of.iter().chain(rules.any_of.iter()).enumerate() {
        let fact = match &predicate.fact {
            FactRef::Quality { key } if !project.qualities.contains_key(key) => {
                Some(("missing_quality", format!("quality '{key}' is not defined")))
            }
            FactRef::ActorAt { actor_id, .. }
            | FactRef::ActorHasRole { actor_id, .. }
            | FactRef::Knows { actor_id, .. }
                if !project.characters.contains_key(actor_id) =>
            {
                Some((
                    "missing_character",
                    format!("character '{actor_id}' does not exist"),
                ))
            }
            FactRef::Relationship {
                source_id,
                target_id,
            } if !project.characters.contains_key(source_id)
                || !project.characters.contains_key(target_id) =>
            {
                Some((
                    "missing_character",
                    "relationship predicate references an unknown character".into(),
                ))
            }
            _ => None,
        };
        if let Some((code, message)) = fact {
            issue(issues, format!("{path}[{index}].fact"), code, message);
        }
    }
}

fn validate_effects(
    issues: &mut Vec<ProjectIssue>,
    project: &ProjectBundle,
    path: &str,
    effects: &[Effect],
) {
    for (index, effect) in effects.iter().enumerate() {
        match effect {
            Effect::SetQuality { key, .. } | Effect::AddQuality { key, .. } => {
                validate_quality_ref(issues, project, path, index, key)
            }
            Effect::TransferQuality { from, to, .. } => {
                validate_quality_ref(issues, project, path, index, from);
                validate_quality_ref(issues, project, path, index, to);
            }
            Effect::MovePlayer { location_id } => validate_location_ref(
                issues,
                &project.locations,
                format!("{path}[{index}].location_id"),
                location_id,
            ),
            Effect::MoveActor {
                actor_id,
                location_id,
            } => {
                if !project.characters.contains_key(actor_id) {
                    issue(
                        issues,
                        format!("{path}[{index}].actor_id"),
                        "missing_character",
                        format!("character '{actor_id}' does not exist"),
                    );
                }
                validate_location_ref(
                    issues,
                    &project.locations,
                    format!("{path}[{index}].location_id"),
                    location_id,
                );
            }
        }
    }
}

fn validate_quality_ref(
    issues: &mut Vec<ProjectIssue>,
    project: &ProjectBundle,
    path: &str,
    index: usize,
    key: &str,
) {
    if !project.qualities.contains_key(key) {
        issue(
            issues,
            format!("{path}[{index}]"),
            "missing_quality",
            format!("quality '{key}' is not defined"),
        );
    }
}

fn validate_location_ref(
    issues: &mut Vec<ProjectIssue>,
    locations: &BTreeMap<String, LocationDefinition>,
    path: String,
    target: &str,
) {
    if !locations.contains_key(target) {
        issue(
            issues,
            path,
            "missing_location",
            format!("location '{target}' does not exist"),
        );
    }
}

fn register_record(
    issues: &mut Vec<ProjectIssue>,
    ids: &mut BTreeSet<String>,
    path: &str,
    key: &str,
    id: &str,
) {
    if key != id {
        issue(
            issues,
            format!("{path}.id"),
            "map_key_mismatch",
            format!("map key '{key}' must equal record id '{id}'"),
        );
    }
    if id.trim().is_empty() {
        issue(
            issues,
            format!("{path}.id"),
            "empty_id",
            "record id is required",
        );
    }
    if !ids.insert(id.to_owned()) {
        issue(
            issues,
            format!("{path}.id"),
            "duplicate_id",
            format!("id '{id}' is already used in this project"),
        );
    }
}

fn register_id(issues: &mut Vec<ProjectIssue>, ids: &mut BTreeSet<String>, path: &str, id: &str) {
    if !ids.insert(id.to_owned()) {
        issue(
            issues,
            format!("{path}.id"),
            "duplicate_id",
            format!("id '{id}' is already used in this project"),
        );
    }
}

fn issue(
    issues: &mut Vec<ProjectIssue>,
    path: impl Into<String>,
    code: impl Into<String>,
    message: impl Into<String>,
) {
    issues.push(ProjectIssue {
        path: path.into(),
        code: code.into(),
        message: message.into(),
    });
}
