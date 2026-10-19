//! Conversion from authored Storylet Studio content into engine ProjectBundle.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use storylet_engine::{
    CharacterDefinition, Effect, EventDefinition, EventType, FactRef, LocationDefinition,
    MetCharacterRequirement, NpcBehaviorRule, PROJECT_SCHEMA_VERSION, Predicate, PredicateOp,
    PredicateSet, ProjectBundle, QualityDefinition, QualityType, Repeatability, RoleRequirement,
    ScenarioDefinition, SceneContent, StoryletChoice, StoryletDefinition, StoryletPreconditions,
    VisitedLocationRequirement,
};

use crate::contracts::{
    CharacterRecord, EventRecord, LocationRecord, NpcBehaviorRecord, QualityRecord, ScenarioRecord,
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
    qualities: &[QualityRecord],
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
    storylets: &[StoryletRecord],
    npc_behaviors: &[NpcBehaviorRecord],
    events: &[EventRecord],
    scenario: Option<&ScenarioRecord>,
) -> ProjectResult {
    let mut issues = Vec::new();

    let mut quality_defs = BTreeMap::new();
    let quality_ids: BTreeSet<String> = qualities.iter().map(|q| q.name.clone()).collect();
    for q in qualities {
        let kind = match q.kind.as_str() {
            "currency" => QualityType::Currency,
            "metric" => QualityType::Metric,
            "tool" => QualityType::Tool,
            "progress" => QualityType::Progress,
            "territory" => QualityType::Territory,
            "menace" => QualityType::Menace,
            _ => QualityType::Custom,
        };
        quality_defs.insert(
            q.name.clone(),
            QualityDefinition {
                key: q.name.clone(),
                kind,
                default_value: Value::from(q.value),
                description: q.description.clone().unwrap_or_default(),
                min_value: q.min_value,
                max_value: q.max_value,
                is_hidden: q.is_hidden,
                menace_crisis_storylet_id: q.menace_crisis_storylet_id.clone(),
            },
        );
    }

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

        let trigger = parse_predicate_set(
            &st.preconditions,
            &quality_ids,
            &mut issues,
            &format!("storylets.{}.preconditions", st.id),
        );
        let preconditions = parse_storylet_preconditions(
            &st.preconditions,
            &mut issues,
            &format!("storylets.{}.preconditions", st.id),
        );

        let choices = st
            .choices
            .iter()
            .enumerate()
            .map(|(idx, choice)| {
                let reqs = if let Some(ref conds) = choice.conditions {
                    parse_predicates_array(
                        conds,
                        &quality_ids,
                        &mut issues,
                        &format!("storylets.{}.choices[{}].conditions", st.id, idx),
                    )
                } else {
                    Vec::new()
                };

                let effects = parse_effects(
                    &choice.effects,
                    &quality_ids,
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
            "linear" | "linear_progression" => storylet_engine::ClusterType::Linear,
            "wheel" | "wheel_deck" => storylet_engine::ClusterType::Wheel,
            "aftermath" | "aftermath_accelerator" => storylet_engine::ClusterType::Aftermath,
            _ => storylet_engine::ClusterType::PlayerControlledPassage,
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
                preconditions,
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
            &quality_ids,
            &mut issues,
            &format!("events.{}.trigger", ev.id),
        );
        let effects = parse_effects(
            &ev.effects,
            &quality_ids,
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

    let mut npc_behavior_defs = BTreeMap::new();
    for npc in npc_behaviors {
        let conditions = parse_predicate_set(
            &npc.conditions,
            &quality_ids,
            &mut issues,
            &format!("npc_behaviors.{}.conditions", npc.id),
        );
        let mut effects = Vec::new();
        for (idx, eff_val) in npc.effects.iter().enumerate() {
            let eff_list = parse_effects(
                eff_val,
                &quality_ids,
                &location_ids,
                &mut issues,
                &format!("npc_behaviors.{}.effects[{}]", npc.id, idx),
            );
            effects.extend(eff_list);
        }

        npc_behavior_defs.insert(
            npc.id.clone(),
            NpcBehaviorRule {
                id: npc.id.clone(),
                actor_id: npc.actor_id.clone(),
                priority: npc.priority,
                conditions,
                action: npc.action.clone(),
                effects,
            },
        );
    }

    // Resolve scenario
    let default_scenario_id = "workspace-playtest".to_string();
    let (scenario_id, scenario_def) = if let Some(scen) = scenario {
        let resolved = resolve_scenario_start(scen, qualities, locations, characters);
        issues.extend(resolved.issues);

        let s_def = ScenarioDefinition {
            id: scen.id.clone(),
            name: scen.name.clone(),
            player_start_location: resolved.player_start_location,
            initial_qualities: resolved.effective_qualities,
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
            initial_qualities: BTreeMap::new(),
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
        qualities: quality_defs,
        locations: location_defs,
        characters: character_defs,
        events: event_defs,
        storylets: storylet_defs,
        npc_behaviors: npc_behavior_defs,
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
    // Accept the canonical role predicate plus legacy predicates emitted by the tag editor.
    let re = regex::Regex::new(
        r#"(?i)^(?:(?:character\.)?role|character_role)\s*==\s*(?:'([^']*)'|"([^"]*)")$"#,
    )
    .ok()?;
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
    quality_ids: &BTreeSet<String>,
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
    } else if quality_ids.contains(var) {
        FactRef::Quality {
            key: var.to_string(),
        }
    } else {
        issues.push(format!("{path}: unknown quality '{var}'"));
        return None;
    };

    Some(Predicate {
        fact,
        op,
        value: expected,
    })
}

fn parse_predicates_array(
    arr: &[Value],
    quality_ids: &BTreeSet<String>,
    issues: &mut Vec<String>,
    path: &str,
) -> Vec<Predicate> {
    arr.iter()
        .enumerate()
        .filter_map(|(idx, v)| parse_predicate(v, quality_ids, issues, &format!("{path}[{idx}]")))
        .collect()
}

fn parse_predicate_set(
    val: &Value,
    quality_ids: &BTreeSet<String>,
    issues: &mut Vec<String>,
    path: &str,
) -> PredicateSet {
    let mut all_of = Vec::new();
    let mut any_of = Vec::new();

    if let Some(obj) = val.as_object() {
        if let Some(all_arr) = obj.get("all_of").and_then(|v| v.as_array()) {
            all_of =
                parse_predicates_array(all_arr, quality_ids, issues, &format!("{path}.all_of"));
        }
        if let Some(any_arr) = obj.get("any_of").and_then(|v| v.as_array()) {
            any_of =
                parse_predicates_array(any_arr, quality_ids, issues, &format!("{path}.any_of"));
        }
    }

    PredicateSet { all_of, any_of }
}

fn parse_storylet_preconditions(
    val: &Value,
    issues: &mut Vec<String>,
    path: &str,
) -> StoryletPreconditions {
    let mut preconditions = StoryletPreconditions::default();
    let Some(object) = val.as_object() else {
        return preconditions;
    };

    for (field, target) in [
        ("visited_locations_any_of", "visited location"),
        ("met_characters_any_of", "met character"),
    ] {
        let Some(entries) = object.get(field).and_then(Value::as_array) else {
            continue;
        };
        for (index, entry) in entries.iter().enumerate() {
            let entry_path = format!("{path}.{field}[{index}]");
            let Some(entry) = entry.as_object() else {
                issues.push(format!(
                    "{entry_path}: {target} requirement must be an object"
                ));
                continue;
            };
            let Some(kind) = entry.get("kind").and_then(Value::as_str) else {
                issues.push(format!("{entry_path}.kind: requirement kind is required"));
                continue;
            };
            let (value_key, value_kind) = match kind {
                "location_id" if field == "visited_locations_any_of" => {
                    ("location_id", "location id")
                }
                "location_tag" if field == "visited_locations_any_of" => ("tag", "location tag"),
                "character_id" if field == "met_characters_any_of" => {
                    ("character_id", "character id")
                }
                "role_tag" if field == "met_characters_any_of" => ("role", "character role"),
                _ => {
                    issues.push(format!(
                        "{entry_path}.kind: unsupported {target} requirement kind '{kind}'"
                    ));
                    continue;
                }
            };
            let Some(value) = entry
                .get(value_key)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                issues.push(format!(
                    "{entry_path}.{value_key}: nonempty {value_kind} is required"
                ));
                continue;
            };
            if field == "visited_locations_any_of" {
                preconditions.visited_locations_any_of.push(match kind {
                    "location_id" => VisitedLocationRequirement::LocationId {
                        location_id: value.to_string(),
                    },
                    _ => VisitedLocationRequirement::LocationTag {
                        tag: value.to_string(),
                    },
                });
            } else {
                preconditions.met_characters_any_of.push(match kind {
                    "character_id" => MetCharacterRequirement::CharacterId {
                        character_id: value.to_string(),
                    },
                    _ => MetCharacterRequirement::RoleTag {
                        role: value.to_string(),
                    },
                });
            }
        }
    }
    preconditions
}

fn parse_effects(
    val: &Value,
    quality_ids: &BTreeSet<String>,
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
            if !quality_ids.contains(key) {
                issues.push(format!("{path}.{key}: unknown quality '{key}'"));
                continue;
            }
            if let Some(num) = v.as_f64() {
                effects.push(Effect::AddQuality {
                    key: key.clone(),
                    amount: num,
                });
            } else if let Some(b) = v.as_bool() {
                effects.push(Effect::SetQuality {
                    key: key.clone(),
                    value: Value::Bool(b),
                });
            } else if let Some(s) = v.as_str() {
                effects.push(Effect::SetQuality {
                    key: key.clone(),
                    value: Value::String(s.to_string()),
                });
            } else {
                issues.push(format!("{path}.{key}: only numeric deltas, booleans, and strings supported as quality effects"));
            }
        }
    }

    effects
}

#[cfg(test)]
mod parse_role_query_tests {
    use super::{
        parse_effects, parse_predicate_set, parse_role_query, parse_storylet_preconditions,
    };

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
    fn accepts_canonical_and_previously_emitted_role_predicates() {
        assert_eq!(
            parse_role_query(r#"role == "Sentinel""#),
            Some("Sentinel".to_string())
        );
        assert_eq!(
            parse_role_query("character.role == 'Guide'"),
            Some("Guide".to_string())
        );
        assert_eq!(
            parse_role_query("character_role == 'Guard'"),
            Some("Guard".to_string())
        );
        assert_eq!(parse_role_query("role matches 'Guard'"), None);
    }

    #[test]
    fn parses_quality_and_entity_preconditions_from_authored_json() {
        use serde_json::json;
        use std::collections::BTreeSet;
        use storylet_engine::{MetCharacterRequirement, PredicateOp, VisitedLocationRequirement};

        let quality_ids = BTreeSet::from(["trust".to_string()]);
        let mut issues = Vec::new();
        let value = json!({
            "all_of": [{"variable": "trust", "op": "gte", "value": 2}],
            "any_of": [{"variable": "trust", "op": "lt", "value": 0}],
            "visited_locations_any_of": [
                {"kind": "location_id", "location_id": "market"},
                {"kind": "location_tag", "tag": "archive"}
            ],
            "met_characters_any_of": [
                {"kind": "character_id", "character_id": "nora"},
                {"kind": "role_tag", "role": "clerk"}
            ]
        });
        let trigger = parse_predicate_set(&value, &quality_ids, &mut issues, "preconditions");
        let typed = parse_storylet_preconditions(&value, &mut issues, "preconditions");

        assert_eq!(trigger.all_of.len(), 1);
        assert_eq!(trigger.all_of[0].op, PredicateOp::Gte);
        assert_eq!(trigger.any_of.len(), 1);
        assert_eq!(trigger.any_of[0].op, PredicateOp::Lt);
        assert_eq!(
            typed.visited_locations_any_of,
            vec![
                VisitedLocationRequirement::LocationId {
                    location_id: "market".into()
                },
                VisitedLocationRequirement::LocationTag {
                    tag: "archive".into()
                },
            ]
        );
        assert_eq!(
            typed.met_characters_any_of,
            vec![
                MetCharacterRequirement::CharacterId {
                    character_id: "nora".into()
                },
                MetCharacterRequirement::RoleTag {
                    role: "clerk".into()
                },
            ]
        );
        assert!(issues.is_empty());
    }
}
