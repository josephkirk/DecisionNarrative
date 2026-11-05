//! Conversion from authored Storylet Studio content into engine ProjectBundle.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use storylet_engine::{
    CharacterDefinition, Effect, EventDefinition, EventType, FactRef, LocationDefinition,
    PROJECT_SCHEMA_VERSION, Predicate, PredicateOp, PredicateSet, ProjectBundle,
    Repeatability, RoleRequirement, ScenarioDefinition, SceneContent, StoryletChoice,
    StoryletDefinition,
};
use crate::contracts::{
    CharacterRecord, EventRecord, LocationRecord, ScenarioRecord,
    StoryletRecord,
};
use crate::scenario::resolve_scenario_start;

pub struct ProjectResult {
    pub bundle: ProjectBundle,
    pub scenario_id: String,
    pub issues: Vec<String>,
}

pub fn project_workspace(
    project_id: &str,
    project_name: &str,
    revision: u64,
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
    storylets: &[StoryletRecord],
    events: &[EventRecord],
    scenario: Option<&ScenarioRecord>,
) -> ProjectResult {
    let mut issues = Vec::new();
    // V1 has only a flat world-property map, not typed properties on Entities.
    // The V1 playtest entry point rejects this data instead of dropping it.
    let property_defs = BTreeMap::new();

    let mut location_defs = BTreeMap::new();
    let location_ids: BTreeSet<String> = locations.iter().map(|l| l.id.clone()).collect();
    for (index, loc) in locations.iter().enumerate() {
        let connected: Vec<String> = loc
            .connected_zones
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter(|id| location_ids.contains(id.as_str()))
            .cloned()
            .collect();

        let map_position = loc
            .environmental_params
            .as_ref()
            .and_then(|v| v.get("map_position"))
            .and_then(|pos| {
                if let Some(arr) = pos.as_array() {
                    if arr.len() == 2 {
                        let x = arr[0].as_f64().unwrap_or(0.0);
                        let y = arr[1].as_f64().unwrap_or(0.0);
                        return Some((x, y));
                    }
                }
                None
            })
            .unwrap_or_else(|| ((index % 5) as f64 * 3.0, (index / 5) as f64 * 2.0));

        location_defs.insert(
            loc.id.clone(),
            LocationDefinition {
                id: loc.id.clone(),
                name: loc.name.clone(),
                description: loc.description.clone(),
                tags: loc.tags.clone(),
                connected_locations: connected,
                map_position,
            },
        );
    }

    let mut character_defs = BTreeMap::new();
    for char in characters {
        character_defs.insert(
            char.id.clone(),
            CharacterDefinition {
                id: char.id.clone(),
                name: char.name.clone(),
                description: char.agenda.clone(),
                role_tags: if char.role.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![char.role.trim().to_string()]
                },
                home_location: char.home_location.clone(),
                goals: if char.agenda.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![char.agenda.clone()]
                },
                red_lines: if char.red_line.trim().is_empty() {
                    Vec::new()
                } else {
                    vec![char.red_line.clone()]
                },
                voice_profile: String::new(),
                schedule: char.schedule.clone().unwrap_or_default(),
            },
        );
    }

    // Only approved storylets enter the runtime bundle
    let mut storylet_defs = BTreeMap::new();
    let mut approved_storylet_ids = Vec::new();
    for st in storylets {
        if st.approved_revision != Some(st.revision) {
            continue;
        }
        approved_storylet_ids.push(st.id.clone());

        let mut required_roles = Vec::new();
        for param in &st.parameters {
            if let Some(role) = parse_role_query(&param.query) {
                required_roles.push(RoleRequirement { role });
            } else {
                issues.push(format!(
                    "storylets.{}.parameters.{}: unsupported role query '{}'",
                    st.id, param.param_name, param.query
                ));
            }
        }

        // Preconditions are evaluated from the authored NarrativeGraph by the graph runtime.
        // The legacy list representation is intentionally not projected here.
        let trigger = PredicateSet::default();

        let choices = st
            .choices
            .iter()
            .enumerate()
            .map(|(idx, choice)| {
                let reqs = if let Some(ref conds) = choice.conditions {
                    parse_predicates_array(
                        conds,
                        &property_defs.keys().cloned().collect(),
                        &mut issues,
                        &format!("storylets.{}.choices[{}].conditions", st.id, idx),
                    )
                } else {
                    Vec::new()
                };

                let effects = parse_effects(
                    &choice.effects,
                    &property_defs.keys().cloned().collect(),
                    &location_ids,
                    &mut issues,
                    &format!("storylets.{}.choices[{}].effects", st.id, idx),
                );

                StoryletChoice {
                    id: choice.choice_id.clone(),
                    label: choice.label.clone(),
                    requirements: PredicateSet {
                        all_of: reqs,
                        any_of: Vec::new(),
                    },
                    effects,
                    continues_scene: false,
                    divert_target: choice.divert_target.clone(),
                }
            })
            .collect();

        let repeatability = match st.repeatability.as_str() {
            "once_ever" => Repeatability::OnceEver,
            "persistent" => Repeatability::Persistent,
            "cooldown" => Repeatability::Cooldown {
                turns: st.cooldown_turns as u64,
            },
            _ => Repeatability::Always,
        };

        let cluster_type = match st.cluster_type.as_str() {
            "player_passage" => storylet_engine::StoryletClusterType::PlayerPassage,
            "linear_progression" => storylet_engine::StoryletClusterType::LinearProgression,
            "wheel_deck" => storylet_engine::StoryletClusterType::WheelDeck,
            "aftermath_accelerator" => storylet_engine::StoryletClusterType::AftermathAccelerator,
            unsupported => {
                issues.push(format!(
                    "storylets.{}.cluster_type: unsupported cluster type '{}'",
                    st.id, unsupported
                ));
                continue;
            }
        };

        let summary = st
            .content
            .scene_opening
            .as_deref()
            .unwrap_or(&st.content.text_template);
        let summary_snippet = summary
            .lines()
            .next()
            .unwrap_or("")
            .chars()
            .take(120)
            .collect();

        storylet_defs.insert(
            st.id.clone(),
            StoryletDefinition {
                id: st.id.clone(),
                revision: st.revision,
                title: st.title.clone(),
                summary: summary_snippet,
                approved: true,
                ending: st.ending,
                location_tags: st
                    .location_tags
                    .iter()
                    .map(|tag| tag.trim().to_string())
                    .filter(|tag| !tag.is_empty())
                    .collect(),
                urgency_weight: st.urgency_weight,
                cluster_type,
                repeatability,
                required_roles,
                trigger,
                content: SceneContent {
                    opening: st
                        .content
                        .scene_opening
                        .clone()
                        .unwrap_or_else(|| st.content.text_template.clone()),
                },
                choices,
            },
        );
    }

    let mut event_defs = BTreeMap::new();
    let mut project_event_ids = Vec::new();
    for ev in events {
        project_event_ids.push(ev.id.clone());
        let trigger = parse_predicate_set(
            &ev.trigger,
            &property_defs.keys().cloned().collect(),
            &mut issues,
            &format!("events.{}.trigger", ev.id),
        );
        let effects = parse_effects(
            &ev.effects,
            &property_defs.keys().cloned().collect(),
            &location_ids,
            &mut issues,
            &format!("events.{}.effects", ev.id),
        );
        let event_type = if ev.event_type == "canon" {
            EventType::Canon
        } else {
            EventType::World
        };

        event_defs.insert(
            ev.id.clone(),
            EventDefinition {
                id: ev.id.clone(),
                title: ev.title.clone(),
                description: ev.description.clone(),
                trigger,
                effects,
                delay_turns: ev.delay_turns.unwrap_or(0),
                repeatable: ev.repeatable.unwrap_or(false),
                event_type,
            },
        );
    }


    let default_scenario_id = format!("{project_id}:default");
    let (scenario_id, scenario_def) = if let Some(scen) = scenario {
        let resolved = resolve_scenario_start(scen, locations, characters);
        issues.extend(resolved.issues);

        // V1 exposes only one global property map. Entity-scoped scenario
        // overrides have no faithful representation, including the
        // single-entity case; retain no lossy projection and report it.
        let initial_properties = BTreeMap::new();
        if !resolved.effective_properties.is_empty() {
            issues.push(format!(
                "scenario '{}': entity-scoped initial properties cannot be represented by the runtime flat property map",
                scen.id
            ));
        }

        let s_def = ScenarioDefinition {
            id: scen.id.clone(),
            name: scen.name.clone(),
            player_start_location: resolved.player_start_location,
            initial_properties,
            actor_locations: resolved.actor_locations,
            storylet_ids: approved_storylet_ids,
            seed: scen.seed,
            event_ids: project_event_ids,
            active_actor_ids: Some(resolved.active_actor_ids),
        };
        (scen.id.clone(), s_def)
    } else {
        let first_loc = locations.first().map(|l| l.id.clone()).unwrap_or_default();
        let s_def = ScenarioDefinition {
            id: default_scenario_id.clone(),
            name: "Default Playtest".into(),
            player_start_location: first_loc,
            initial_properties: BTreeMap::new(),
            actor_locations: BTreeMap::new(),
            storylet_ids: approved_storylet_ids,
            seed: 1,
            event_ids: project_event_ids,
            active_actor_ids: None,
        };
        (default_scenario_id, s_def)
    };

    let mut scenarios = BTreeMap::new();
    scenarios.insert(scenario_id.clone(), scenario_def);

    let bundle = ProjectBundle {
        schema_version: PROJECT_SCHEMA_VERSION,
        id: project_id.to_string(),
        name: project_name.to_string(),
        revision,
        properties: property_defs,
        locations: location_defs,
        characters: character_defs,
        events: event_defs,
        storylets: storylet_defs,
        npc_behaviors: BTreeMap::new(),
        scenarios,
    };

    ProjectResult {
        bundle,
        scenario_id,
        issues,
    }
}

pub(crate) fn parse_role_query(query: &str) -> Option<String> {
    let trimmed = query.trim();
    let re = regex::Regex::new(r#"^role\s*==\s*(?:'([^']*)'|"([^"]*)")$"#).ok()?;
    let caps = re.captures(trimmed)?;
    Some(
        caps.get(1)
            .or_else(|| caps.get(2))?
            .as_str()
            .trim()
            .to_string(),
    )
}

fn normalize_op(op_str: &str) -> Option<PredicateOp> {
    match op_str.trim().to_lowercase().as_str() {
        "=" | "==" | "eq" => Some(PredicateOp::Eq),
        "!=" | "<>" | "neq" => Some(PredicateOp::Neq),
        ">" | "gt" => Some(PredicateOp::Gt),
        ">=" | "gte" => Some(PredicateOp::Gte),
        "<" | "lt" => Some(PredicateOp::Lt),
        "<=" | "lte" => Some(PredicateOp::Lte),
        "contains" => Some(PredicateOp::Contains),
        _ => None,
    }
}

fn parse_predicate(
    val: &Value,
    _property_ids: &BTreeSet<String>,
    issues: &mut Vec<String>,
    path: &str,
) -> Option<Predicate> {
    let obj = val.as_object()?;
    let var = obj.get("variable")?.as_str()?.trim();
    let op_str = obj.get("op")?.as_str()?.trim();
    let expected = obj.get("value")?.clone();

    let op = match normalize_op(op_str) {
        Some(o) => o,
        None => {
            issues.push(format!("{path}: unsupported predicate operator '{op_str}'"));
            return None;
        }
    };

    let fact = if var == "location" || var == "current_location" {
        FactRef::CurrentLocation
    } else {
        FactRef::Property {
            key: var.to_string(),
        }
    };

    Some(Predicate {
        fact,
        op,
        value: expected,
    })
}

fn parse_predicates_array(
    arr: &[Value],
    _property_ids: &BTreeSet<String>,
    issues: &mut Vec<String>,
    path: &str,
) -> Vec<Predicate> {
    arr.iter()
        .enumerate()
        .filter_map(|(idx, v)| parse_predicate(v, _property_ids, issues, &format!("{path}[{idx}]")))
        .collect()
}

fn parse_predicate_set(
    val: &Value,
    property_ids: &BTreeSet<String>,
    issues: &mut Vec<String>,
    path: &str,
) -> PredicateSet {
    let mut all_of = Vec::new();
    let mut any_of = Vec::new();

    if let Some(obj) = val.as_object() {
        if let Some(all_arr) = obj.get("all_of").and_then(|v| v.as_array()) {
            all_of =
                parse_predicates_array(all_arr, property_ids, issues, &format!("{path}.all_of"));
        }
        if let Some(any_arr) = obj.get("any_of").and_then(|v| v.as_array()) {
            any_of =
                parse_predicates_array(any_arr, property_ids, issues, &format!("{path}.any_of"));
        }
    }

    PredicateSet { all_of, any_of }
}


fn parse_effects(
    val: &Value,
    _property_ids: &BTreeSet<String>,
    location_ids: &BTreeSet<String>,
    issues: &mut Vec<String>,
    path: &str,
) -> Vec<Effect> {
    let mut effects = Vec::new();

    if let Some(obj) = val.as_object() {
        for (key, v) in obj {
            if key == "$move_player" {
                match v
                    .as_str()
                    .filter(|location_id| location_ids.contains(*location_id))
                {
                    Some(location_id) => effects.push(Effect::MovePlayer {
                        location_id: location_id.to_owned(),
                    }),
                    None => issues.push(format!(
                        "{path}.{key}: location must reference an existing project location"
                    )),
                }
                continue;
            }
            if let Some(num) = v.as_f64() {
                effects.push(Effect::AddProperty {
                    key: key.clone(),
                    amount: num,
                });
            } else {
                effects.push(Effect::SetProperty {
                    key: key.clone(),
                    value: v.clone(),
                });
            }
        }
    }

    effects
}

#[cfg(test)]
mod parse_role_query_tests {
    use super::{parse_effects, parse_predicate_set, parse_role_query};

    #[test]
    fn parses_player_movement_only_to_existing_locations() {
        use serde_json::json;
        use std::collections::BTreeSet;
        use storylet_engine::Effect;

        let locations = BTreeSet::from(["market".to_string()]);
        let mut issues = Vec::new();
        let effects = parse_effects(
            &json!({"$move_player": "market"}),
            &BTreeSet::new(),
            &locations,
            &mut issues,
            "choice.effects",
        );
        assert_eq!(
            effects,
            vec![Effect::MovePlayer {
                location_id: "market".into()
            }]
        );
        assert!(issues.is_empty());

        let effects = parse_effects(
            &json!({"$move_player": "missing"}),
            &BTreeSet::new(),
            &locations,
            &mut issues,
            "choice.effects",
        );
        assert!(effects.is_empty());
        assert!(
            issues
                .iter()
                .any(|issue| issue.contains("existing project location"))
        );
    }

    #[test]
    fn accepts_only_the_canonical_role_predicate() {
        assert_eq!(
            parse_role_query(r#"role == "character.witness""#),
            Some("character.witness".to_string())
        );
        assert_eq!(parse_role_query("character.role == 'Guide'"), None);
        assert_eq!(parse_role_query("character_role == 'Guard'"), None);
        assert_eq!(parse_role_query("role matches 'Guard'"), None);
    }

}
