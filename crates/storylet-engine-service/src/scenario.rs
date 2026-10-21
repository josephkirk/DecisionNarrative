//! Deterministic scenario starting condition resolution derived from Story Codex.

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::contracts::{
    CharacterRecord, LocationRecord, QualityRecord, ResolvedScenarioStart, ScenarioRecord,
    StartLocationPolicy,
};

pub struct XorShift32 {
    state: u32,
}

impl XorShift32 {
    pub fn new(seed: u64) -> Self {
        let mut s = (seed as u32) & 0xFFFF_FFFF;
        if s == 0 {
            s = 1;
        }
        Self { state: s }
    }

    pub fn next_f64(&mut self) -> f64 {
        let mut s = self.state;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.state = s;
        (s as f64) / 4294967296.0
    }
}

pub fn sample_deterministic<T: Clone>(items: &[T], count: usize, rng: &mut XorShift32) -> Vec<T> {
    if count >= items.len() {
        return items.to_vec();
    }
    if count == 0 {
        return Vec::new();
    }
    let mut pool = items.to_vec();
    let mut result = Vec::with_capacity(count);
    for _ in 0..count {
        let idx = (rng.next_f64() * pool.len() as f64).floor() as usize;
        result.push(pool.remove(idx));
    }
    result
}

pub fn match_attribute_value(actual: Option<&Value>, expected: Option<&Value>) -> bool {
    match (actual, expected) {
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
        (Some(act), Some(exp)) => {
            if act == exp {
                return true;
            }
            if let (Some(a_num), Some(e_num)) = (act.as_f64(), exp.as_f64()) {
                return (a_num - e_num).abs() < 0.000001;
            }
            if let (Some(a_str), Some(e_str)) = (act.as_str(), exp.as_str()) {
                return a_str.trim().eq_ignore_ascii_case(e_str.trim());
            }
            false
        }
    }
}

pub fn get_location_attribute(loc: &LocationRecord, key: &str) -> Option<Value> {
    let trimmed = key.trim();
    if trimmed == "id" {
        return Some(Value::String(loc.id.clone()));
    }
    if trimmed == "name" {
        return Some(Value::String(loc.name.clone()));
    }
    if trimmed == "description" {
        return Some(Value::String(loc.description.clone()));
    }
    if let Some(ref env) = loc.environmental_params {
        if let Some(val) = env.get(trimmed) {
            return Some(val.clone());
        }
    }
    if let Some(ref devices) = loc.devices {
        if let Some(val) = devices.get(trimmed) {
            return Some(val.clone());
        }
    }
    crate::entities::attribute_path(&loc.attributes, trimmed)
}

pub fn match_location(loc: &LocationRecord, key: &str, expected: Option<&Value>) -> bool {
    let act = get_location_attribute(loc, key);
    match_attribute_value(act.as_ref(), expected)
}

pub fn get_character_attribute(char: &CharacterRecord, key: &str) -> Option<Value> {
    let trimmed = key.trim();
    if trimmed == "id" {
        return Some(Value::String(char.id.clone()));
    }
    if trimmed == "name" {
        return Some(Value::String(char.name.clone()));
    }
    if trimmed == "role" {
        return Some(Value::String(char.role.clone()));
    }
    if trimmed == "home_location" || trimmed == "homeLocation" {
        return Some(Value::String(char.home_location.clone()));
    }
    if trimmed == "current_location" || trimmed == "currentLocation" {
        return Some(Value::String(char.current_location.clone()));
    }
    if trimmed == "agenda" {
        return Some(Value::String(char.agenda.clone()));
    }
    if trimmed == "red_line" || trimmed == "redLine" {
        return Some(Value::String(char.red_line.clone()));
    }
    if let Some(val) = char.status_axes.get(trimmed) {
        return Some(val.clone());
    }
    if let Some(val) = char.needs.get(trimmed) {
        return Some(val.clone());
    }
    if let Some(ref schedule) = char.schedule {
        if let Some(val) = schedule.get(trimmed) {
            return Some(Value::String(val.clone()));
        }
    }
    if let Some(ref qualities) = char.qualities {
        if let Some(val) = qualities.get(trimmed) {
            return Some(val.clone());
        }
    }
    crate::entities::attribute_path(&char.attributes, trimmed)
}

pub fn match_character(char: &CharacterRecord, key: &str, expected: Option<&Value>) -> bool {
    let act = get_character_attribute(char, key);
    match_attribute_value(act.as_ref(), expected)
}

pub fn resolve_location_from_policy(
    policy: Option<&StartLocationPolicy>,
    fallback: &str,
    locations: &[LocationRecord],
    rng: &mut XorShift32,
    issues: &mut Vec<String>,
    path_prefix: &str,
) -> String {
    let mode = policy.map(|p| p.mode.as_str()).unwrap_or("fixed");
    if mode == "fixed" {
        let loc_id = policy
            .and_then(|p| p.location_id.as_deref())
            .filter(|id| !id.trim().is_empty())
            .unwrap_or(fallback);

        if loc_id.is_empty() {
            issues.push(format!(
                "{path_prefix}: fixed location mode requires a location"
            ));
            return String::new();
        }
        if let Some(found) = locations.iter().find(|l| l.id == loc_id) {
            return found.id.clone();
        }
        issues.push(format!("{path_prefix}: unknown fixed location '{loc_id}'"));
        return String::new();
    }

    if mode == "random" {
        if locations.is_empty() {
            issues.push(format!(
                "{path_prefix}: cannot select random location from empty locations list"
            ));
            return String::new();
        }
        let idx = (rng.next_f64() * locations.len() as f64).floor() as usize;
        return locations[idx].id.clone();
    }

    if mode == "by_attribute" {
        let attr_key = policy
            .and_then(|p| p.attribute_key.as_deref())
            .unwrap_or("")
            .trim();
        if attr_key.is_empty() {
            issues.push(format!(
                "{path_prefix}: by_attribute mode requires an attribute key"
            ));
            return String::new();
        }
        let attr_val = policy.and_then(|p| p.attribute_value.as_ref());
        let matches: Vec<&LocationRecord> = locations
            .iter()
            .filter(|l| match_location(l, attr_key, attr_val))
            .collect();

        if matches.is_empty() {
            issues.push(format!(
                "{path_prefix}: no location matches attribute '{attr_key}' = {:?}",
                attr_val
            ));
            return String::new();
        }
        let idx = (rng.next_f64() * matches.len() as f64).floor() as usize;
        return matches[idx].id.clone();
    }

    issues.push(format!(
        "{path_prefix}: unsupported location policy mode '{mode}'"
    ));
    fallback.to_string()
}

pub fn resolve_actor_population(
    scenario: &ScenarioRecord,
    characters: &[CharacterRecord],
    locations: &[LocationRecord],
    seed: u64,
    issues: &mut Vec<String>,
    path_prefix: &str,
) -> (BTreeMap<String, String>, Vec<String>) {
    let mut rng = XorShift32::new(seed);
    let location_ids: BTreeSet<&str> = locations.iter().map(|l| l.id.as_str()).collect();
    let policy = &scenario.actor_population_policy;

    let selected_characters: Vec<&CharacterRecord> = match policy.mode.as_str() {
        "all" => characters.iter().collect(),
        "random" => {
            let count = policy.count.unwrap_or(characters.len());
            let items: Vec<&CharacterRecord> = characters.iter().collect();
            sample_deterministic(&items, count, &mut rng)
        }
        "by_attribute" => {
            let attr_key = policy.attribute_key.as_deref().unwrap_or("").trim();
            if attr_key.is_empty() {
                issues.push(format!("{path_prefix}.attribute_key: by_attribute actor population requires an attribute key"));
                Vec::new()
            } else {
                let attr_val = policy.attribute_value.as_ref();
                let matches: Vec<&CharacterRecord> = characters
                    .iter()
                    .filter(|c| match_character(c, attr_key, attr_val))
                    .collect();
                if matches.is_empty() {
                    issues.push(format!(
                        "{path_prefix}: no character matches attribute '{attr_key}' = {:?}",
                        attr_val
                    ));
                }
                matches
            }
        }
        other => {
            issues.push(format!(
                "{path_prefix}.mode: unsupported actor population mode '{other}'"
            ));
            characters.iter().collect()
        }
    };

    let mut result_locations = BTreeMap::new();
    let mut active_actor_ids = Vec::with_capacity(selected_characters.len());

    for char in selected_characters {
        active_actor_ids.push(char.id.clone());

        // Check if there is an explicit override in scenario.actor_locations
        if let Some(override_loc) = scenario.actor_locations.get(&char.id) {
            if location_ids.contains(override_loc.as_str()) {
                result_locations.insert(char.id.clone(), override_loc.clone());
                continue;
            } else {
                issues.push(format!(
                    "{path_prefix}.{}: unknown override location '{override_loc}'",
                    char.id
                ));
            }
        }

        // Apply policy
        let loc_policy = policy.location_policy.as_ref();
        let mode = loc_policy.map(|p| p.mode.as_str()).unwrap_or("fixed");

        if mode == "fixed" {
            let fixed_loc = loc_policy
                .and_then(|p| p.location_id.as_deref())
                .filter(|id| !id.trim().is_empty())
                .unwrap_or(&char.current_location);

            let chosen = if location_ids.contains(fixed_loc) {
                fixed_loc
            } else if location_ids.contains(char.home_location.as_str()) {
                &char.home_location
            } else {
                ""
            };

            if chosen.is_empty() {
                issues.push(format!(
                    "{path_prefix}.{}: unknown location for actor",
                    char.id
                ));
            } else {
                result_locations.insert(char.id.clone(), chosen.to_string());
            }
        } else {
            let temp_start_policy = StartLocationPolicy {
                mode: mode.to_string(),
                location_id: loc_policy.and_then(|p| p.location_id.clone()),
                attribute_key: loc_policy.and_then(|p| p.attribute_key.clone()),
                attribute_value: loc_policy.and_then(|p| p.attribute_value.clone()),
            };
            let fallback = if location_ids.contains(char.current_location.as_str()) {
                &char.current_location
            } else {
                &char.home_location
            };
            let loc = resolve_location_from_policy(
                Some(&temp_start_policy),
                fallback,
                locations,
                &mut rng,
                issues,
                &format!("{path_prefix}.{}.location", char.id),
            );
            if !loc.is_empty() {
                result_locations.insert(char.id.clone(), loc);
            }
        }
    }

    (result_locations, active_actor_ids)
}

pub fn resolve_scenario_start(
    scenario: &ScenarioRecord,
    qualities: &[QualityRecord],
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
) -> ResolvedScenarioStart {
    let mut issues = Vec::new();
    let seed = scenario.seed;
    let mut start_rng = XorShift32::new(seed);

    let fallback_start = locations.first().map(|l| l.id.as_str()).unwrap_or("");
    let player_start_location = resolve_location_from_policy(
        Some(&scenario.start_location_policy),
        fallback_start,
        locations,
        &mut start_rng,
        &mut issues,
        "scenario.start_location_policy",
    );

    let mut sorted_characters = characters.to_vec();
    sorted_characters.sort_by(|a, b| a.id.cmp(&b.id));

    let (actor_locations, active_actor_ids) = resolve_actor_population(
        scenario,
        &sorted_characters,
        locations,
        seed,
        &mut issues,
        "scenario.actor_population",
    );

    // Effective qualities: default values from qualities, overridden by initial_qualities
    let mut effective_qualities = BTreeMap::new();
    for q in qualities {
        effective_qualities.insert(q.name.clone(), Value::from(q.value));
    }
    for (k, v) in &scenario.initial_qualities {
        effective_qualities.insert(k.clone(), v.clone());
    }

    // Actors per location
    let mut actors_per_location: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for loc in locations {
        actors_per_location.insert(loc.id.clone(), Vec::new());
    }
    for (actor_id, loc_id) in &actor_locations {
        actors_per_location
            .entry(loc_id.clone())
            .or_default()
            .push(actor_id.clone());
    }

    ResolvedScenarioStart {
        player_start_location,
        actor_locations,
        active_actor_ids,
        effective_qualities,
        seed,
        actors_per_location,
        issues,
    }
}
