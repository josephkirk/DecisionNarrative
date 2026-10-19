//! Lossless compilation of authored Studio storylets into the dense ChronoFlow dispatcher.
//!
//! The adapter keeps existing project records as the authoring boundary. Runtime IDs are
//! assigned in stable storylet-ID order and bounded by the engine's fixed completion mask.

use std::collections::{BTreeMap, BTreeSet};

use storylet_engine::chrono_dispatch::{
    EffectKind, IdealMetricTarget, PredicateOperator, PredicateSubject, QualityPredicate,
    RoleRequirement, StoryletClusterType, StoryletDef, StoryletEffect, UnifiedDispatcher,
};
use storylet_engine::chrono_state::{
    AttributeValue, GameplayTagRegistry, GraphLayer, MAX_ENTITIES, MAX_METRICS, MAX_TAGS,
    MetricRegistry, MetricThresholdRule, MicroPassionDef, PassionDirection, PassionRegistry,
    PassionThresholdRule, RivalPassionLink, StarNpcDef, TagBitset, WorldEdge, WorldStateTable,
    WorldVertex,
};

use crate::contracts::{
    CharacterRecord, ChronoStoryletSettings, LocationRecord, QualityRecord, ScenarioRecord,
    StoryletRecord,
};

#[derive(Clone, Debug)]
pub struct CompiledChronoDispatch {
    pub registry: GameplayTagRegistry,
    pub metrics: MetricRegistry,
    pub dispatcher: UnifiedDispatcher,
    pub storylet_ids: Vec<String>,
    pub storylet_indices: BTreeMap<String, u16>,
    pub metric_indices: BTreeMap<String, u16>,
    pub passion_registry: PassionRegistry,
    pub passion_indices: BTreeMap<(String, String), u16>,
}

fn menace_crisis_tag(quality_name: &str) -> String {
    format!("quality.crisis.{quality_name}")
}

/// Compile authored storylets and project names into bounded, deterministic dense IDs.
pub fn compile_chrono_dispatch(
    storylets: &[StoryletRecord],
    qualities: &[QualityRecord],
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
) -> Result<CompiledChronoDispatch, String> {
    compile_chrono_dispatch_inner(storylets, qualities, locations, characters, true)
}

/// Compile a candidate storylet without requiring its crisis targets to be in
/// the candidate slice. Full-project compilation remains strict.
pub fn compile_chrono_dispatch_for_storylet_validation(
    storylets: &[StoryletRecord],
    qualities: &[QualityRecord],
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
) -> Result<CompiledChronoDispatch, String> {
    compile_chrono_dispatch_inner(storylets, qualities, locations, characters, false)
}

fn compile_chrono_dispatch_inner(
    storylets: &[StoryletRecord],
    qualities: &[QualityRecord],
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
    strict_crisis_targets: bool,
) -> Result<CompiledChronoDispatch, String> {
    let mut ordered_storylets: Vec<&StoryletRecord> = storylets.iter().collect();
    ordered_storylets.sort_by(|left, right| left.id.cmp(&right.id));
    const MAX_DISPATCHABLE_STORYLETS: usize = u16::MAX as usize + 1;
    if ordered_storylets.len() > MAX_DISPATCHABLE_STORYLETS {
        return Err(format!(
            "ChronoFlow supports at most {MAX_DISPATCHABLE_STORYLETS} dispatchable storylets; got {}",
            ordered_storylets.len()
        ));
    }

    let mut names = BTreeSet::new();
    names.insert("location.current".into());
    for location in locations {
        names.insert(format!("location.{}", location.id));
        names.extend(location.tags.iter().cloned());
    }
    for character in characters {
        names.insert(format!("character.{}", character.id));
        names.insert(character.role.clone());
        for (index, threshold) in character.thresholds.iter().enumerate() {
            names.insert(passion_threshold_tag(character, threshold, index));
        }
    }
    for quality in qualities.iter().filter(|quality| quality.kind == "tool") {
        names.insert(format!("quality.{}", quality.name));
    }
    for quality in qualities
        .iter()
        .filter(|quality| quality.kind == "menace" && quality.menace_crisis_storylet_id.is_some())
    {
        names.insert(menace_crisis_tag(&quality.name));
    }
    for storylet in &ordered_storylets {
        names.extend(storylet.location_tags.iter().cloned());
        names.extend(
            storylet
                .parameters
                .iter()
                .map(|parameter| parameter.param_name.clone()),
        );
        // The compiled role tag comes from `query`; register it too so a query
        // naming a role different from the parameter label still resolves.
        names.extend(
            storylet
                .parameters
                .iter()
                .filter_map(|parameter| crate::projection::parse_role_query(&parameter.query)),
        );
        if let Some(settings) = &storylet.chrono {
            collect_settings_names(settings, &mut names);
        }
        if let Some(graph) = &storylet.graph {
            collect_graph_tag_names(graph, &mut names);
        }
        collect_legacy_precondition_names(&storylet.preconditions, &mut names);
        validate_legacy_reference_ids(storylet, locations, characters)?;
    }
    if names.len() > MAX_TAGS {
        return Err(format!(
            "ChronoFlow tag registry supports at most {MAX_TAGS} names; got {}",
            names.len()
        ));
    }

    let registry = GameplayTagRegistry::compile(names.into_iter())
        .map_err(|error| format!("Invalid ChronoFlow tag registry: {error}"))?;
    let metrics = MetricRegistry::compile(qualities.iter().map(|quality| quality.name.clone()))
        .map_err(|error| format!("Invalid ChronoFlow metric registry: {error}"))?;
    let mut metric_indices = BTreeMap::new();
    for quality in qualities {
        let metric_id = metrics.id(&quality.name).ok_or_else(|| {
            format!(
                "quality {} is missing from the ChronoFlow metric registry",
                quality.name
            )
        })?;
        metric_indices.insert(quality.name.clone(), metric_id);
    }

    let storylet_ids: Vec<String> = ordered_storylets
        .iter()
        .map(|storylet| storylet.id.clone())
        .collect();
    let storylet_indices: BTreeMap<String, u16> = storylet_ids
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), index as u16))
        .collect();
    let mut threshold_rules = Vec::new();
    for quality in qualities {
        let Some(crisis_storylet_id) = &quality.menace_crisis_storylet_id else {
            continue;
        };
        if quality.kind != "menace" {
            return Err(format!(
                "quality {} sets a crisis storylet but is not a menace",
                quality.name
            ));
        }
        if strict_crisis_targets && !storylet_indices.contains_key(crisis_storylet_id) {
            return Err(format!(
                "menace quality {} references unknown crisis storylet {}",
                quality.name, crisis_storylet_id
            ));
        }
        let metric_id = metric_indices[&quality.name];
        let threshold = quality.max_value as f32;
        if !threshold.is_finite() {
            return Err(format!(
                "menace quality {} threshold exceeds f32 range",
                quality.name
            ));
        }
        let tag = menace_crisis_tag(&quality.name);
        let tag_id = registry
            .id(&tag)
            .ok_or_else(|| format!("unregistered menace crisis tag {tag}"))?;
        threshold_rules.push(MetricThresholdRule {
            metric_id,
            threshold_tag_bit: tag_id,
            trigger_value: threshold,
            hysteresis: 0.0,
        });
    }
    let registry = registry
        .with_threshold_rules(threshold_rules)
        .map_err(|error| format!("Invalid ChronoFlow menace thresholds: {error}"))?;
    let (passion_registry, passion_indices) =
        compile_passion_registry(characters, locations, &registry)?;

    let mut definitions = Vec::with_capacity(ordered_storylets.len());
    for (storylet_index, storylet) in ordered_storylets.iter().enumerate() {
        let settings = storylet.chrono.as_ref();
        let mut definition = StoryletDef {
            storylet_id: storylet_index as u16,
            display_priority: settings
                .map(|settings| settings.display_priority)
                .unwrap_or(0),
            urgency_tag: settings
                .map(|settings| settings.urgency_color_tag.trim())
                .filter(|tag| !tag.is_empty())
                .unwrap_or("Standard")
                .to_owned(),
            cluster_type: map_cluster_type(&storylet.cluster_type),
            payload_node_guid: storylet_index as u32,
            is_repeatable: settings
                .map(|settings| settings.is_repeatable)
                .unwrap_or(storylet.repeatability != "once_ever")
                && storylet.repeatability != "once_ever",
            cooldown_ms: settings
                .map(|settings| i64::try_from(settings.cooldown_ms))
                .transpose()
                .map_err(|_| format!("storylet {} cooldown_ms exceeds i64 range", storylet.id))?
                .unwrap_or(0),
            cooldown_turns: if storylet.repeatability == "cooldown" {
                u64::from(storylet.cooldown_turns)
            } else {
                0
            },
            required_roles: storylet
                .parameters
                .iter()
                .map(|parameter| {
                    // The role is authored in `query` (`role == "Detective"`);
                    // the parameter name is only a variable label.
                    let role = crate::projection::parse_role_query(&parameter.query).ok_or_else(
                        || {
                            format!(
                                "storylet {} parameter {} has unsupported role query '{}'",
                                storylet.id, parameter.param_name, parameter.query
                            )
                        },
                    )?;
                    Ok(RoleRequirement {
                        role_tag: role,
                        location_tag: Some("location.current".into()),
                        ..RoleRequirement::default()
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
            ..StoryletDef::default()
        };
        for quality in qualities.iter().filter(|quality| {
            quality.kind == "menace"
                && quality.menace_crisis_storylet_id.as_deref() == Some(storylet.id.as_str())
        }) {
            let tag = menace_crisis_tag(&quality.name);
            let tag_id = registry
                .id(&tag)
                .ok_or_else(|| format!("unregistered menace crisis tag {tag}"))?;
            definition
                .required_tags
                .set_bit(tag_id)
                .map_err(|error| error.to_string())?;
        }
        if let Some(settings) = settings {
            add_tags(
                &registry,
                &mut definition.required_tags,
                &settings.required_tags,
                &storylet.id,
                "required_tags",
            )?;
            add_any_tags(
                &registry,
                &mut definition.required_any_tags,
                &settings.required_any_tags,
                &storylet.id,
                "required_any_tags",
            )?;
            add_any_tags(
                &registry,
                &mut definition.forbidden_tags,
                &settings.forbidden_tags,
                &storylet.id,
                "forbidden_tags",
            )?;
            add_any_tags(
                &registry,
                &mut definition.required_arc_phases,
                &settings.required_arc_phases,
                &storylet.id,
                "required_arc_phases",
            )?;
            // Bonus specificity counts exact tags. Expanding to the ancestor
            // closure would score one authored bonus per ancestor and let
            // sibling tags cross-trigger.
            for name in &settings.salience_bonus_tags {
                let index = registry.id(name).ok_or_else(|| {
                    format!(
                        "storylet {} salience_bonus_tags references unknown tag {name}",
                        storylet.id
                    )
                })?;
                definition
                    .salience_bonus_tags
                    .set_bit(index)
                    .map_err(|error| {
                        format!("storylet {} salience_bonus_tags: {error}", storylet.id)
                    })?;
            }

            for (target, value) in &settings.ideal_state_vector {
                let metric_id = metric_indices.get(target).copied().ok_or_else(|| {
                    format!(
                        "storylet {} ideal_state_vector references unknown quality {target}",
                        storylet.id
                    )
                })?;
                if !value.is_finite() {
                    return Err(format!(
                        "storylet {} ideal target for {target} must be finite",
                        storylet.id
                    ));
                }
                let target_value = *value as f32;
                if !target_value.is_finite() {
                    return Err(format!(
                        "storylet {} ideal target for {target} exceeds f32 range",
                        storylet.id
                    ));
                }
                let weight = settings
                    .ideal_state_weights
                    .get(target)
                    .copied()
                    .unwrap_or(1.0);
                if !weight.is_finite() || weight <= 0.0 {
                    return Err(format!(
                        "storylet {} ideal weight for {target} must be finite and positive",
                        storylet.id
                    ));
                }
                let weight = weight as f32;
                if !weight.is_finite() {
                    return Err(format!(
                        "storylet {} ideal weight for {target} exceeds f32 range",
                        storylet.id
                    ));
                }
                definition.ideal_vector.push(IdealMetricTarget {
                    metric_id,
                    target: target_value,
                    weight,
                });
            }
            for predicate in &settings.preconditions {
                let operator = map_operator(&predicate.operator).ok_or_else(|| {
                    format!(
                        "storylet {} has unsupported predicate operator {:?}",
                        storylet.id, predicate.operator
                    )
                })?;
                let subject = map_subject(&predicate.subject).ok_or_else(|| {
                    format!(
                        "storylet {} has unsupported predicate subject {:?}",
                        storylet.id, predicate.subject
                    )
                })?;
                if subject == PredicateSubject::Metric
                    && !metric_indices.contains_key(&predicate.target_id)
                {
                    return Err(format!(
                        "storylet {} metric predicate references unknown quality {}",
                        storylet.id, predicate.target_id
                    ));
                }
                definition.preconditions.push(QualityPredicate {
                    target_id: predicate.target_id.clone(),
                    subject,
                    operator,
                    comparison_value: predicate.comparison_value,
                    specificity_weight: predicate.specificity_weight,
                    entity_index: predicate.entity_index,
                    target_index: None,
                    tag_mask: TagBitset::new(),
                });
            }
            for effect in &settings.effects_on_complete {
                let kind = if effect.incremental {
                    EffectKind::Increment
                } else {
                    EffectKind::Set
                };
                definition.effects_on_complete.push(StoryletEffect {
                    target_id: effect.target_id.clone(),
                    value: effect.value,
                    kind,
                    target_index: None,
                    tag_mask: TagBitset::new(),
                });
                for tag in &effect.add_tags {
                    let id = registry.id(tag).ok_or_else(|| {
                        format!(
                            "storylet {} completion effect references unknown tag {tag}",
                            storylet.id
                        )
                    })?;
                    registry
                        .add_hierarchical(&mut definition.on_complete_mutation.add_global_tags, id)
                        .map_err(|error| error.to_string())?;
                }
                for tag in &effect.remove_tags {
                    let id = registry.id(tag).ok_or_else(|| {
                        format!(
                            "storylet {} completion effect references unknown tag {tag}",
                            storylet.id
                        )
                    })?;
                    registry
                        .add_hierarchical(
                            &mut definition.on_complete_mutation.remove_global_tags,
                            id,
                        )
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        let legacy_all_of =
            legacy_quality_predicates(&storylet.preconditions, "all_of", &storylet.id)?;
        for predicate in legacy_all_of {
            if !metric_indices.contains_key(&predicate.target_id) {
                return Err(format!(
                    "storylet {} legacy condition references unknown quality {}",
                    storylet.id, predicate.target_id
                ));
            }
            definition.preconditions.push(predicate);
        }
        for (group_name, subject) in [
            ("any_of", PredicateSubject::Metric),
            ("visited_locations_any_of", PredicateSubject::Visited),
            ("met_characters_any_of", PredicateSubject::MetCharacter),
        ] {
            let group = match subject {
                PredicateSubject::Metric => {
                    legacy_quality_predicates(&storylet.preconditions, group_name, &storylet.id)?
                }
                PredicateSubject::Visited | PredicateSubject::MetCharacter => {
                    legacy_tag_predicates(
                        &storylet.preconditions,
                        group_name,
                        subject,
                        &storylet.id,
                    )?
                }
                _ => Vec::new(),
            };
            if !group.is_empty() {
                definition.any_of_precondition_groups.push(group);
            }
        }
        if !storylet.location_tags.is_empty() {
            let group = storylet
                .location_tags
                .iter()
                .map(|tag| QualityPredicate {
                    target_id: tag.clone(),
                    subject: PredicateSubject::Location,
                    operator: PredicateOperator::Eq,
                    comparison_value: 1.0,
                    specificity_weight: 1.0,
                    entity_index: None,
                    target_index: None,
                    tag_mask: TagBitset::new(),
                })
                .collect();
            definition.any_of_precondition_groups.push(group);
        }
        definitions.push(definition);
    }

    let mut dispatcher = UnifiedDispatcher::new(definitions);
    dispatcher
        .resolve(&registry, &metrics)
        .map_err(|error| format!("ChronoFlow dispatcher compile failed: {error}"))?;
    Ok(CompiledChronoDispatch {
        registry,
        metrics,
        dispatcher,
        storylet_ids,
        storylet_indices,
        metric_indices,
        passion_registry,
        passion_indices,
    })
}

fn passion_threshold_tag(
    character: &CharacterRecord,
    threshold: &crate::contracts::MacroPassionThresholdRecord,
    index: usize,
) -> String {
    let threshold_id = if threshold.threshold_id.trim().is_empty() {
        index.to_string()
    } else {
        threshold.threshold_id.trim().to_owned()
    };
    format!("passion.threshold.{}.{}", character.id, threshold_id)
}

fn compile_passion_registry(
    characters: &[CharacterRecord],
    locations: &[LocationRecord],
    tags: &GameplayTagRegistry,
) -> Result<(PassionRegistry, BTreeMap<(String, String), u16>), String> {
    let mut ordered: Vec<&CharacterRecord> = characters.iter().collect();
    ordered.sort_by(|left, right| left.id.cmp(&right.id));
    let mut npc_indices = BTreeMap::new();
    for (index, character) in ordered.iter().enumerate() {
        let npc_id = locations
            .len()
            .checked_add(index)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (*value as usize) < MAX_ENTITIES)
            .ok_or_else(|| "ChronoFlow passion NPC index exceeds the entity limit".to_string())?;
        npc_indices.insert(character.id.clone(), npc_id);
    }

    let mut passion_indices = BTreeMap::new();
    let mut next_passion_id = 0usize;
    for character in &ordered {
        let mut local_ids = BTreeSet::new();
        for passion in &character.passion_pool {
            if passion.passion_id.trim().is_empty() || !local_ids.insert(passion.passion_id.clone())
            {
                return Err(format!(
                    "character {} has an empty or duplicate passion id",
                    character.id
                ));
            }
            if !passion.current_value.is_finite()
                || !(-100.0..=100.0).contains(&passion.current_value)
            {
                return Err(format!(
                    "character {} passion {} value must be finite and in [-100, 100]",
                    character.id, passion.passion_id
                ));
            }
            if !passion.weight.is_finite() {
                return Err(format!(
                    "character {} passion {} weight must be finite",
                    character.id, passion.passion_id
                ));
            }
            let passion_id = u16::try_from(next_passion_id).map_err(|_| {
                "ChronoFlow passion registry exceeds its fixed metric limit".to_string()
            })?;
            passion_indices.insert(
                (character.id.clone(), passion.passion_id.clone()),
                passion_id,
            );
            next_passion_id += 1;
        }
        if !character.passion_pool.is_empty() {
            if character.active_passions.is_empty() {
                return Err(format!(
                    "character {} has a passion pool but no active passions",
                    character.id
                ));
            }
            for active in &character.active_passions {
                if !local_ids.contains(active) {
                    return Err(format!(
                        "character {} activates unknown passion {active}",
                        character.id
                    ));
                }
            }
        }
        if !character.trust_inertia.is_finite()
            || !(0.0..=1.0).contains(&character.trust_inertia)
        {
            return Err(format!(
                "character {} trust inertia must be finite and in [0, 1]",
                character.id
            ));
        }
    }
    if next_passion_id > MAX_METRICS {
        return Err(format!(
            "ChronoFlow supports at most {MAX_METRICS} micro passions"
        ));
    }

    let metric_threshold_bits: BTreeSet<u16> = tags
        .threshold_rules()
        .iter()
        .map(|rule| rule.threshold_tag_bit)
        .collect();
    let mut stars = Vec::new();
    let mut thresholds = Vec::new();
    for character in ordered {
        if character.passion_pool.is_empty() {
            continue;
        }
        let npc_id = npc_indices[&character.id];
        let mut micro_passions = Vec::with_capacity(character.passion_pool.len());
        for passion in &character.passion_pool {
            micro_passions.push(MicroPassionDef {
                npc_id,
                passion_id: passion_indices[&(character.id.clone(), passion.passion_id.clone())],
                weight: passion.weight as f32,
                is_hidden: passion.is_hidden,
            });
            if !(passion.weight as f32).is_finite() {
                return Err(format!(
                    "character {} passion {} weight exceeds f32 range",
                    character.id, passion.passion_id
                ));
            }
        }
        let mut zero_sum_links = Vec::with_capacity(character.zero_sum_passion_links.len());
        for link in &character.zero_sum_passion_links {
            let target_id = *npc_indices.get(&link.target_character_id).ok_or_else(|| {
                format!(
                    "character {} links to unknown rival {}",
                    character.id, link.target_character_id
                )
            })?;
            let source_passion_id = *passion_indices
                .get(&(character.id.clone(), link.source_passion_id.clone()))
                .ok_or_else(|| {
                    format!(
                        "character {} links from unknown passion {}",
                        character.id, link.source_passion_id
                    )
                })?;
            let target_passion_id = *passion_indices
                .get(&(
                    link.target_character_id.clone(),
                    link.target_passion_id.clone(),
                ))
                .ok_or_else(|| {
                    format!(
                        "rival {} has no passion {}",
                        link.target_character_id, link.target_passion_id
                    )
                })?;
            if !link.weight.is_finite() || !(link.weight as f32).is_finite() {
                return Err(format!(
                    "character {} has non-finite zero-sum link weight",
                    character.id
                ));
            }
            zero_sum_links.push(RivalPassionLink {
                target_character_id: target_id,
                source_passion_id,
                target_passion_id,
                weight: link.weight as f32,
            });
        }
        stars.push(StarNpcDef {
            npc_id,
            trust_inertia: character.trust_inertia as f32,
            micro_passions,
            macro_bounds: storylet_engine::chrono_state::MetricBound {
                min: -100.0,
                max: 100.0,
            },
            zero_sum_links,
        });
        for (index, threshold) in character.thresholds.iter().enumerate() {
            let direction = match threshold.direction.trim().to_ascii_lowercase().as_str() {
                "positive" | "up" => PassionDirection::Up,
                "negative" | "down" => PassionDirection::Down,
                other => {
                    return Err(format!(
                        "character {} has invalid passion threshold direction {other}",
                        character.id
                    ));
                }
            };
            let name = passion_threshold_tag(character, threshold, index);
            let threshold_tag_bit = tags
                .id(&name)
                .ok_or_else(|| format!("unregistered passion threshold tag {name}"))?;
            if metric_threshold_bits.contains(&threshold_tag_bit) {
                return Err(format!(
                    "character {} threshold {name} shares a tag bit with a quality threshold",
                    character.id
                ));
            }
            thresholds.push(PassionThresholdRule {
                npc_id,
                threshold_tag_bit,
                trigger_value: threshold.trigger_value as f32,
                direction,
                linked_graph_asset_id: threshold.linked_graph_asset_id.clone(),
            });
        }
    }
    let registry = PassionRegistry::compile(stars, thresholds)
        .map_err(|error| format!("Invalid ChronoFlow passion registry: {error}"))?;
    Ok((registry, passion_indices))
}

fn collect_legacy_precondition_names(
    preconditions: &serde_json::Value,
    names: &mut BTreeSet<String>,
) {
    if let Some(items) = preconditions
        .get("visited_locations_any_of")
        .and_then(serde_json::Value::as_array)
    {
        for item in items {
            match item.get("kind").and_then(serde_json::Value::as_str) {
                Some("location_id") => {
                    if let Some(id) = item.get("location_id").and_then(serde_json::Value::as_str) {
                        names.insert(format!("location.{id}"));
                    }
                }
                Some("location_tag") => {
                    if let Some(tag) = item.get("tag").and_then(serde_json::Value::as_str) {
                        names.insert(tag.to_owned());
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(items) = preconditions
        .get("met_characters_any_of")
        .and_then(serde_json::Value::as_array)
    {
        for item in items {
            match item.get("kind").and_then(serde_json::Value::as_str) {
                Some("character_id") => {
                    if let Some(id) = item.get("character_id").and_then(serde_json::Value::as_str) {
                        names.insert(format!("character.{id}"));
                    }
                }
                Some("role_tag") => {
                    if let Some(role) = item.get("role").and_then(serde_json::Value::as_str) {
                        names.insert(role.to_owned());
                    }
                }
                _ => {}
            }
        }
    }
}

fn validate_legacy_reference_ids(
    storylet: &StoryletRecord,
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
) -> Result<(), String> {
    let visited = storylet
        .preconditions
        .get("visited_locations_any_of")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten();
    for (index, condition) in visited.enumerate() {
        match condition.get("kind").and_then(serde_json::Value::as_str) {
            Some("location_id") => {
                let id = condition
                    .get("location_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if !locations.iter().any(|location| location.id == id) {
                    return Err(format!(
                        "storylet {} preconditions.visited_locations_any_of[{index}] references unknown location {id}",
                        storylet.id
                    ));
                }
            }
            Some("location_tag") => {
                let tag = condition
                    .get("tag")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if !locations.iter().any(|location| {
                    location
                        .tags
                        .iter()
                        .any(|authored| authored == tag || authored.starts_with(&format!("{tag}.")))
                }) {
                    return Err(format!(
                        "storylet {} preconditions.visited_locations_any_of[{index}] references unknown location tag {tag}",
                        storylet.id
                    ));
                }
            }
            _ => {}
        }
    }
    let met = storylet
        .preconditions
        .get("met_characters_any_of")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten();
    for (index, condition) in met.enumerate() {
        match condition.get("kind").and_then(serde_json::Value::as_str) {
            Some("character_id") => {
                let id = condition
                    .get("character_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if !characters.iter().any(|character| character.id == id) {
                    return Err(format!(
                        "storylet {} preconditions.met_characters_any_of[{index}] references unknown character {id}",
                        storylet.id
                    ));
                }
            }
            Some("role_tag") => {
                let role = condition
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                if !characters.iter().any(|character| {
                    character.role == role || character.role.starts_with(&format!("{role}."))
                }) {
                    return Err(format!(
                        "storylet {} preconditions.met_characters_any_of[{index}] references unknown character role {role}",
                        storylet.id
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_graph_tag_names(value: &serde_json::Value, names: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                let is_tag_field = matches!(
                    key.as_str(),
                    "tag"
                        | "tags"
                        | "emotion_tag"
                        | "action_tag"
                        | "emote"
                        | "arc_phase"
                        | "urgency_tag"
                        | "urgency_color_tag"
                        | "speaker_role"
                        | "actor_role"
                        | "target_role"
                        | "action_target"
                ) || key.ends_with("_tags");
                if is_tag_field {
                    match value {
                        serde_json::Value::String(tag) if !tag.trim().is_empty() => {
                            names.insert(tag.clone());
                        }
                        serde_json::Value::Array(tags) => {
                            names.extend(
                                tags.iter()
                                    .filter_map(serde_json::Value::as_str)
                                    .map(str::to_owned),
                            );
                        }
                        _ => {}
                    }
                }
                collect_graph_tag_names(value, names);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_graph_tag_names(item, names);
            }
        }
        _ => {}
    }
}

fn collect_settings_names(settings: &ChronoStoryletSettings, names: &mut BTreeSet<String>) {
    names.extend(settings.required_tags.iter().cloned());
    names.extend(settings.required_any_tags.iter().cloned());
    names.extend(settings.forbidden_tags.iter().cloned());
    names.extend(settings.required_arc_phases.iter().cloned());
    names.extend(settings.salience_bonus_tags.iter().cloned());
    for predicate in &settings.preconditions {
        if predicate.subject != "metric" {
            names.insert(predicate.target_id.clone());
        }
    }
    for effect in &settings.effects_on_complete {
        names.extend(effect.add_tags.iter().cloned());
        names.extend(effect.remove_tags.iter().cloned());
    }
}

fn add_tags(
    registry: &GameplayTagRegistry,
    target: &mut TagBitset,
    names: &[String],
    storylet_id: &str,
    field: &str,
) -> Result<(), String> {
    for name in names {
        let index = registry.id(name).ok_or_else(|| {
            format!("storylet {storylet_id} {field} references unknown tag {name}")
        })?;
        registry
            .add_hierarchical(target, index)
            .map_err(|error| format!("storylet {storylet_id} {field}: {error}"))?;
    }
    Ok(())
}

fn add_any_tags(
    registry: &GameplayTagRegistry,
    target: &mut TagBitset,
    names: &[String],
    storylet_id: &str,
    field: &str,
) -> Result<(), String> {
    let mut words = *target.words();
    for name in names {
        let index = registry.id(name).ok_or_else(|| {
            format!("storylet {storylet_id} {field} references unknown tag {name}")
        })?;
        let mask = registry.descendants_mask(index).ok_or_else(|| {
            format!("storylet {storylet_id} {field} has invalid tag index {index}")
        })?;
        for (word, descendants) in words.iter_mut().zip(mask.words()) {
            *word |= *descendants;
        }
    }
    *target = TagBitset::from_words(words);
    Ok(())
}

fn map_operator(operator: &str) -> Option<PredicateOperator> {
    match operator {
        "eq" => Some(PredicateOperator::Eq),
        "neq" => Some(PredicateOperator::Neq),
        "gt" => Some(PredicateOperator::Gt),
        "gte" => Some(PredicateOperator::Gte),
        "lt" => Some(PredicateOperator::Lt),
        "lte" => Some(PredicateOperator::Lte),
        _ => None,
    }
}

fn legacy_quality_predicates(
    preconditions: &serde_json::Value,
    group: &str,
    storylet_id: &str,
) -> Result<Vec<QualityPredicate>, String> {
    let Some(value) = preconditions.get(group) else {
        return Ok(Vec::new());
    };
    let predicates = value
        .as_array()
        .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group} must be an array"))?;
    predicates
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let target_id = value.get("variable").and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] needs a quality variable"))?;
            let operator_name = value.get("op").and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] needs an operator"))?;
            let operator = map_operator(operator_name)
                .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] has unsupported operator {operator_name}"))?;
            let comparison_value = value.get("value").and_then(serde_json::Value::as_f64)
                .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] needs a numeric value"))?;
            Ok(QualityPredicate {
                target_id: target_id.to_owned(),
                subject: PredicateSubject::Metric,
                operator,
                comparison_value,
                specificity_weight: 1.0,
                entity_index: None,
                target_index: None,
                tag_mask: TagBitset::new(),
            })
        })
        .collect()
}
fn legacy_tag_predicates(
    preconditions: &serde_json::Value,
    group: &str,
    subject: PredicateSubject,
    storylet_id: &str,
) -> Result<Vec<QualityPredicate>, String> {
    let Some(value) = preconditions.get(group) else {
        return Ok(Vec::new());
    };
    let items = value
        .as_array()
        .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group} must be an array"))?;
    items.iter().enumerate().map(|(index, item)| {
        let kind = item.get("kind").and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] needs a kind"))?;
        let target_id = match (subject, kind) {
            (PredicateSubject::Visited, "location_id") => {
                let id = item.get("location_id").and_then(serde_json::Value::as_str)
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] needs a location_id"))?;
                format!("location.{id}")
            }
            (PredicateSubject::Visited, "location_tag") => item.get("tag").and_then(serde_json::Value::as_str)
                .filter(|tag| !tag.trim().is_empty())
                .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] needs a tag"))?
                .to_owned(),
            (PredicateSubject::MetCharacter, "character_id") => {
                let id = item.get("character_id").and_then(serde_json::Value::as_str)
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] needs a character_id"))?;
                format!("character.{id}")
            }
            (PredicateSubject::MetCharacter, "role_tag") => item.get("role").and_then(serde_json::Value::as_str)
                .filter(|role| !role.trim().is_empty())
                .ok_or_else(|| format!("storylet {storylet_id} preconditions.{group}[{index}] needs a role"))?
                .to_owned(),
            _ => return Err(format!(
                "storylet {storylet_id} preconditions.{group}[{index}] has unsupported kind {kind}"
            )),
        };
        Ok(QualityPredicate {
            target_id,
            subject,
            operator: PredicateOperator::Eq,
            comparison_value: 1.0,
            specificity_weight: 1.0,
            entity_index: None,
            target_index: None,
            tag_mask: TagBitset::new(),
        })
    }).collect()
}

fn map_subject(subject: &str) -> Option<PredicateSubject> {
    match subject {
        "metric" => Some(PredicateSubject::Metric),
        "location" => Some(PredicateSubject::Location),
        "entity" => Some(PredicateSubject::Entity),
        "visited" => Some(PredicateSubject::Visited),
        "met_character" => Some(PredicateSubject::MetCharacter),
        _ => None,
    }
}

fn map_cluster_type(value: &str) -> StoryletClusterType {
    match value {
        "linear" | "linear_progression" => StoryletClusterType::LinearProgression,
        "wheel" | "wheel_deck" => StoryletClusterType::WheelDeck,
        "aftermath" | "aftermath_accelerator" => StoryletClusterType::AftermathAccelerator,
        "player_controlled_passage" | "player_passage" => StoryletClusterType::PlayerPassage,
        _ => StoryletClusterType::PlayerPassage,
    }
}

#[derive(Clone, Debug)]
pub struct ChronoWorldSeed {
    pub state: WorldStateTable,
    /// Stable dense row for each authored location or character ID.
    pub entity_indices: BTreeMap<String, u16>,
}

/// Build the fixed-capacity world rows from the current project and resolved start.
pub fn initialize_chrono_world(
    compiled: &CompiledChronoDispatch,
    qualities: &[QualityRecord],
    locations: &[LocationRecord],
    characters: &[CharacterRecord],
    scenario: Option<&ScenarioRecord>,
    start_location_id: &str,
) -> Result<ChronoWorldSeed, String> {
    let mut ordered_locations: Vec<&LocationRecord> = locations.iter().collect();
    ordered_locations.sort_by(|left, right| left.id.cmp(&right.id));
    let mut ordered_characters: Vec<&CharacterRecord> = characters.iter().collect();
    ordered_characters.sort_by(|left, right| left.id.cmp(&right.id));
    let mut ordered_tool_qualities: Vec<&QualityRecord> = qualities
        .iter()
        .filter(|quality| quality.kind == "tool")
        .collect();
    ordered_tool_qualities.sort_by(|left, right| left.name.cmp(&right.name));
    let entity_count =
        ordered_locations.len() + ordered_characters.len() + ordered_tool_qualities.len();
    if entity_count > MAX_ENTITIES {
        return Err(format!(
            "ChronoFlow supports at most {MAX_ENTITIES} location/character/tool entities; got {entity_count}"
        ));
    }
    let location_indices: BTreeMap<String, u16> = ordered_locations
        .iter()
        .enumerate()
        .map(|(index, location)| (location.id.clone(), index as u16))
        .collect();
    let mut entity_indices = BTreeMap::new();
    for (index, location) in ordered_locations.iter().enumerate() {
        entity_indices.insert(format!("location:{}", location.id), index as u16);
    }
    let mut character_indices = BTreeMap::new();
    for (offset, character) in ordered_characters.iter().enumerate() {
        let index = (ordered_locations.len() + offset) as u16;
        entity_indices.insert(format!("character:{}", character.id), index);
        character_indices.insert(character.id.clone(), index);
    }
    for (offset, quality) in ordered_tool_qualities.iter().enumerate() {
        let index = (ordered_locations.len() + ordered_characters.len() + offset) as u16;
        entity_indices.insert(format!("quality:{}", quality.name), index);
    }
    let current_location_entity = location_indices
        .get(start_location_id)
        .copied()
        .ok_or_else(|| format!("ChronoFlow start location {start_location_id} does not exist"))?;
    let mut state = WorldStateTable::with_capacity(entity_count, MAX_METRICS);
    state.current_location_entity = Some(current_location_entity);

    // Seed the dramatic arc phase so arc-gated content is reachable without an
    // unrelated mutation writing the tag.
    if let Some(phase) = scenario
        .map(|scenario| scenario.current_arc_phase.trim())
        .filter(|phase| !phase.is_empty())
    {
        let phase_id = compiled.registry.id(phase).ok_or_else(|| {
            format!("scenario current_arc_phase '{phase}' is not a registered tag")
        })?;
        compiled
            .registry
            .add_hierarchical(&mut state.global_tags, phase_id)
            .map_err(|error| format!("scenario current_arc_phase: {error}"))?;
    }

    for quality in qualities {
        let metric_id = *compiled.metric_indices.get(&quality.name).ok_or_else(|| {
            format!(
                "quality {} was not compiled into the metric registry",
                quality.name
            )
        })?;
        let value =
            match scenario.and_then(|scenario| scenario.initial_qualities.get(&quality.name)) {
                Some(value) => value.as_f64().ok_or_else(|| {
                    format!(
                        "scenario quality {} must be numeric for ChronoFlow",
                        quality.name
                    )
                })?,
                None => quality.value,
            };
        let value = value as f32;
        let min = quality.min_value as f32;
        let max = quality.max_value as f32;
        if !value.is_finite() || !min.is_finite() || !max.is_finite() || min > max {
            return Err(format!(
                "quality {} has invalid ChronoFlow value bounds",
                quality.name
            ));
        }
        state
            .set_metric_bound(metric_id, min, max)
            .map_err(|error| format!("invalid bounds for quality {}: {error}", quality.name))?;
        let value = value.clamp(min, max);
        state.metrics[metric_id as usize] = value;
        if quality.kind == "menace" && quality.menace_crisis_storylet_id.is_some() {
            let threshold = quality.max_value as f32;
            if !threshold.is_finite() {
                return Err(format!(
                    "menace quality {} threshold exceeds f32 range",
                    quality.name
                ));
            }
            if value >= threshold {
                let tag = menace_crisis_tag(&quality.name);
                let tag_id = compiled
                    .registry
                    .id(&tag)
                    .ok_or_else(|| format!("unregistered menace crisis tag {tag}"))?;
                state
                    .global_tags
                    .set_bit(tag_id)
                    .map_err(|error| error.to_string())?;
            }
        }
    }

    let mut vertices = Vec::with_capacity(entity_count + qualities.len());
    let mut edges = Vec::new();
    for location in &ordered_locations {
        let entity_index = location_indices[&location.id];
        let location_tag = format!("location.{}", location.id);
        add_entity_tag(&compiled.registry, &mut state, entity_index, &location_tag)?;
        for tag in &location.tags {
            add_entity_tag(&compiled.registry, &mut state, entity_index, tag)?;
        }
        let mut attributes = BTreeMap::new();
        if let Some(value) = &location.environmental_params {
            insert_json_attributes(value, "environment", &mut attributes);
        }
        if let Some(value) = &location.devices {
            insert_json_attributes(value, "device", &mut attributes);
        }
        vertices.push(WorldVertex {
            vertex_id: format!("location:{}", location.id),
            layer: GraphLayer::Location,
            entity_index: Some(entity_index),
            attributes,
        });
        for target_id in location.connected_zones.as_deref().unwrap_or_default() {
            if !location_indices.contains_key(target_id) {
                return Err(format!(
                    "location {} connects to unknown location {target_id}",
                    location.id
                ));
            }
            edges.push(WorldEdge {
                edge_id: format!("connected:{}:{target_id}", location.id),
                source_id: format!("location:{}", location.id),
                target_id: format!("location:{target_id}"),
                relation_type: "connected".into(),
                weight: 1.0,
            });
        }
    }
    let start_location = ordered_locations
        .iter()
        .find(|location| location.id == start_location_id)
        .copied()
        .ok_or_else(|| format!("ChronoFlow start location {start_location_id} does not exist"))?;
    add_global_tag(
        &compiled.registry,
        &mut state.visited_location_tags.tag_mask,
        &format!("location.{}", start_location.id),
    )?;
    add_hierarchical_tags(
        &compiled.registry,
        &mut state.visited_location_tags.tag_mask,
        &start_location.tags,
    )?;
    state
        .visited_location_tags
        .entity_mask
        .set_bit(current_location_entity)
        .map_err(|error| error.to_string())?;
    state.current_location_tag_mask = state.entity_tags[current_location_entity as usize];
    state.global_tags.merge(&state.current_location_tag_mask);

    for character in &ordered_characters {
        let entity_index = character_indices[&character.id];
        add_entity_tag(
            &compiled.registry,
            &mut state,
            entity_index,
            &format!("character.{}", character.id),
        )?;
        add_entity_tag(
            &compiled.registry,
            &mut state,
            entity_index,
            &character.role,
        )?;
        let mut attributes = BTreeMap::new();
        insert_json_attributes(&character.status_axes, "status", &mut attributes);
        insert_json_attributes(&character.needs, "need", &mut attributes);
        if let Some(value) = &character.qualities {
            insert_json_attributes(value, "quality", &mut attributes);
        }
        if !character.agenda.is_empty() {
            attributes.insert(
                "agenda".into(),
                AttributeValue::String(character.agenda.clone()),
            );
        }
        if !character.red_line.is_empty() {
            attributes.insert(
                "red_line".into(),
                AttributeValue::String(character.red_line.clone()),
            );
        }
        vertices.push(WorldVertex {
            vertex_id: format!("character:{}", character.id),
            layer: GraphLayer::Character,
            entity_index: Some(entity_index),
            attributes,
        });
        let actor_location = scenario
            .and_then(|scenario| scenario.actor_locations.get(&character.id))
            .map(String::as_str)
            .filter(|location| !location.is_empty())
            .unwrap_or(&character.current_location);
        if !actor_location.is_empty() {
            if !location_indices.contains_key(actor_location) {
                return Err(format!(
                    "character {} references unknown current location {actor_location}",
                    character.id
                ));
            }
            if actor_location == start_location_id {
                add_entity_tag(
                    &compiled.registry,
                    &mut state,
                    entity_index,
                    "location.current",
                )?;
            }
            edges.push(WorldEdge {
                edge_id: format!("located:{}:{actor_location}", character.id),
                source_id: format!("character:{}", character.id),
                target_id: format!("location:{actor_location}"),
                relation_type: "located_at".into(),
                weight: 1.0,
            });
        }
    }
    seed_passion_state(
        compiled,
        &ordered_characters,
        &character_indices,
        &mut state,
    )?;
    for quality in qualities {
        let mut attributes = BTreeMap::new();
        let metric_id = compiled
            .metric_indices
            .get(&quality.name)
            .copied()
            .ok_or_else(|| {
                format!(
                    "quality {} was not compiled into the metric registry",
                    quality.name
                )
            })?;
        attributes.insert(
            "category".into(),
            AttributeValue::String(quality.kind.clone()),
        );
        attributes.insert("metric_id".into(), AttributeValue::Int(metric_id as i64));
        attributes.insert("min_value".into(), AttributeValue::Float(quality.min_value));
        attributes.insert("max_value".into(), AttributeValue::Float(quality.max_value));
        attributes.insert("is_hidden".into(), AttributeValue::Bool(quality.is_hidden));
        if let Some(storylet_id) = &quality.menace_crisis_storylet_id {
            attributes.insert(
                "menace_crisis_storylet_id".into(),
                AttributeValue::String(storylet_id.clone()),
            );
        }
        let entity_index = if quality.kind == "tool" {
            let index = *entity_indices
                .get(&format!("quality:{}", quality.name))
                .ok_or_else(|| {
                    format!("tool quality {} has no assigned entity index", quality.name)
                })?;
            add_entity_tag(
                &compiled.registry,
                &mut state,
                index,
                &format!("quality.{}", quality.name),
            )?;
            Some(index)
        } else {
            None
        };
        vertices.push(WorldVertex {
            vertex_id: format!("quality:{}", quality.name),
            layer: if quality.kind == "tool" {
                GraphLayer::Item
            } else {
                GraphLayer::Fact
            },
            entity_index,
            attributes,
        });
    }
    vertices.sort_by(|left, right| left.vertex_id.cmp(&right.vertex_id));
    edges.sort_by(|left, right| left.edge_id.cmp(&right.edge_id));
    state.vertices = vertices;
    state.edges = edges;
    Ok(ChronoWorldSeed {
        state,
        entity_indices,
    })
}

fn seed_passion_state(
    compiled: &CompiledChronoDispatch,
    characters: &[&CharacterRecord],
    character_indices: &BTreeMap<String, u16>,
    state: &mut WorldStateTable,
) -> Result<(), String> {
    for character in characters {
        if character.passion_pool.is_empty() {
            continue;
        }
        let npc_id = character_indices[&character.id];
        let mut weighted_sum = 0.0f32;
        let mut total_weight = 0.0f32;
        for passion in &character.passion_pool {
            let passion_id = *compiled
                .passion_indices
                .get(&(character.id.clone(), passion.passion_id.clone()))
                .ok_or_else(|| {
                    format!(
                        "character {} passion {} was not compiled",
                        character.id, passion.passion_id
                    )
                })?;
            let value = passion.current_value as f32;
            let weight = passion.weight as f32;
            state.passions.micro[passion_id as usize] = value;
            let active = character
                .active_passions
                .iter()
                .any(|id| id == &passion.passion_id);
            state.passions.active[passion_id as usize] = active;
            if active {
                weighted_sum += value * weight;
                total_weight += weight;
            }
        }
        if !weighted_sum.is_finite() || !total_weight.is_finite() {
            return Err(format!(
                "character {} has overflowing active passion weights",
                character.id
            ));
        }
        if total_weight > 0.0 {
            let macro_value = weighted_sum / total_weight + character.trust_inertia as f32;
            if !macro_value.is_finite() || npc_id as usize >= state.passions.macro_values.len() {
                return Err(format!(
                    "character {} has invalid initial macro passion",
                    character.id
                ));
            }
            state.passions.macro_values[npc_id as usize] = macro_value.clamp(-100.0, 100.0);
        }
        for (index, threshold) in character.thresholds.iter().enumerate() {
            let direction = threshold.direction.trim().to_ascii_lowercase();
            let trigger = threshold.trigger_value as f32;
            let macro_value = state.passions.macro_values[npc_id as usize];
            let active = match direction.as_str() {
                "positive" | "up" => macro_value >= trigger,
                "negative" | "down" => macro_value <= trigger,
                _ => {
                    return Err(format!(
                        "character {} has invalid passion threshold direction {direction}",
                        character.id
                    ));
                }
            };
            let tag = passion_threshold_tag(character, threshold, index);
            let tag_id = compiled
                .registry
                .id(&tag)
                .ok_or_else(|| format!("unregistered passion threshold tag {tag}"))?;
            if active {
                state
                    .global_tags
                    .set_bit(tag_id)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn add_entity_tag(
    registry: &GameplayTagRegistry,
    state: &mut WorldStateTable,
    entity_index: u16,
    tag: &str,
) -> Result<(), String> {
    let index = registry
        .id(tag)
        .ok_or_else(|| format!("unregistered entity tag {tag}"))?;
    let entity = state
        .entity_tags
        .get_mut(entity_index as usize)
        .ok_or_else(|| {
            format!("entity index {entity_index} is outside the ChronoFlow state table")
        })?;
    registry
        .add_hierarchical(entity, index)
        .map_err(|error| error.to_string())
}

fn add_global_tag(
    registry: &GameplayTagRegistry,
    target: &mut TagBitset,
    tag: &str,
) -> Result<(), String> {
    let index = registry
        .id(tag)
        .ok_or_else(|| format!("unregistered global tag {tag}"))?;
    registry
        .add_hierarchical(target, index)
        .map_err(|error| error.to_string())
}

fn add_hierarchical_tags(
    registry: &GameplayTagRegistry,
    target: &mut TagBitset,
    tags: &[String],
) -> Result<(), String> {
    for tag in tags {
        let index = registry
            .id(tag)
            .ok_or_else(|| format!("unregistered visited tag {tag}"))?;
        registry
            .add_hierarchical(target, index)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::compile_chrono_dispatch;
    use crate::contracts::{CharacterRecord, LocationRecord, QualityRecord, StoryletRecord};
    use storylet_engine::chrono_state::{MutationOp, apply_mutation_with_registry, revert_delta};

    #[test]
    fn compiles_each_independent_legacy_precondition_group() {
        let storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "scene-a",
            "title": "Scene A",
            "content": { "text_template": "A scene." },
            "effects": {},
            "created_at": 1,
            "location_tags": ["Place.Museum"],
            "preconditions": {
                "any_of": [{ "variable": "Trust", "op": "gte", "value": 3 }],
                "visited_locations_any_of": [{ "kind": "location_tag", "tag": "Place.City" }],
                "met_characters_any_of": [{ "kind": "role_tag", "role": "Role.Guide" }]
            }
        }))
        .expect("valid storylet fixture");
        let quality = QualityRecord {
            name: "Trust".into(),
            kind: "metric".into(),
            value: 0.0,
            min_value: 0.0,
            max_value: 100.0,
            is_hidden: false,
            menace_crisis_storylet_id: None,
            description: None,
            metadata: None,
        };
        let location: LocationRecord = serde_json::from_value(serde_json::json!({
            "id": "museum",
            "name": "Museum",
            "description": "",
            "tags": ["Place.City", "Place.Museum"]
        }))
        .expect("valid location fixture");
        let character: CharacterRecord = serde_json::from_value(serde_json::json!({
            "id": "guide",
            "name": "Guide",
            "role": "Role.Guide",
            "home_location": "museum",
            "current_location": "museum",
            "status_axes": {},
            "needs": {}
        }))
        .expect("valid character fixture");

        let compiled = compile_chrono_dispatch(&[storylet], &[quality], &[location], &[character])
            .expect("storylet should compile");

        assert_eq!(compiled.dispatcher.storylets.len(), 1);
        let groups = &compiled.dispatcher.storylets[0].any_of_precondition_groups;
        assert_eq!(groups.len(), 4);
        assert_eq!(
            groups[0][0].subject,
            storylet_engine::chrono_dispatch::PredicateSubject::Metric
        );
        assert_eq!(
            groups[1][0].subject,
            storylet_engine::chrono_dispatch::PredicateSubject::Visited
        );
        assert_eq!(
            groups[2][0].subject,
            storylet_engine::chrono_dispatch::PredicateSubject::MetCharacter
        );
        assert_eq!(
            groups[3][0].subject,
            storylet_engine::chrono_dispatch::PredicateSubject::Location
        );
    }

    #[test]
    fn compiles_more_than_256_dispatchable_storylets() {
        let storylets = (0..501)
            .map(|index| {
                serde_json::from_value(serde_json::json!({
                    "id": format!("storylet-{index:03}"),
                    "title": format!("Storylet {index}"),
                    "content": { "text_template": "A scene." },
                    "effects": {},
                    "created_at": 1
                }))
                .expect("valid storylet fixture")
            })
            .collect::<Vec<StoryletRecord>>();

        let compiled = compile_chrono_dispatch(&storylets, &[], &[], &[])
            .expect("501 dispatchable storylets fit the u16 ID range");

        assert_eq!(compiled.dispatcher.storylets.len(), 501);
        assert_eq!(
            compiled.dispatcher.storylets.last().unwrap().storylet_id,
            500
        );
    }

    #[test]
    fn seeds_location_roles_attributes_and_metrics_in_separate_state_spaces() {
        let storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "scene-a",
            "title": "Scene A",
            "content": { "text_template": "A scene." },
            "effects": {},
            "created_at": 1
        }))
        .expect("valid storylet fixture");
        let quality = QualityRecord {
            name: "Trust".into(),
            kind: "metric".into(),
            value: 4.0,
            min_value: 0.0,
            max_value: 100.0,
            is_hidden: false,
            menace_crisis_storylet_id: None,
            description: None,
            metadata: None,
        };
        let tool_quality = QualityRecord {
            name: "Clue".into(),
            kind: "tool".into(),
            value: 1.0,
            min_value: 0.0,
            max_value: 1.0,
            is_hidden: false,
            menace_crisis_storylet_id: None,
            description: None,
            metadata: None,
        };
        let location: LocationRecord = serde_json::from_value(serde_json::json!({
            "id": "square",
            "name": "Square",
            "description": "",
            "tags": ["Place.City"],
            "environmental_params": { "danger": 2, "open": true }
        }))
        .expect("valid location fixture");
        let character: CharacterRecord = serde_json::from_value(serde_json::json!({
            "id": "guide",
            "name": "Guide",
            "role": "Role.Guide",
            "home_location": "square",
            "current_location": "square",
            "status_axes": { "trust": 0.75 },
            "needs": {}
        }))
        .expect("valid character fixture");
        let compiled = compile_chrono_dispatch(
            &[storylet],
            &[quality.clone(), tool_quality.clone()],
            &[location],
            &[character],
        )
        .expect("storylet should compile");

        let seed = super::initialize_chrono_world(
            &compiled,
            &[quality.clone(), tool_quality.clone()],
            &[serde_json::from_value(serde_json::json!({
                "id": "square",
                "name": "Square",
                "description": "",
                "tags": ["Place.City"],
                "environmental_params": { "danger": 2, "open": true }
            }))
            .expect("valid location fixture")],
            &[serde_json::from_value(serde_json::json!({
                "id": "guide",
                "name": "Guide",
                "role": "Role.Guide",
                "home_location": "square",
                "current_location": "square",
                "status_axes": { "trust": 0.75 },
                "needs": {}
            }))
            .expect("valid character fixture")],
            None,
            "square",
        )
        .expect("valid initial world");

        let metric_id = compiled.metric_indices["Trust"] as usize;
        assert_eq!(seed.state.metrics[metric_id], 4.0);
        let location_tag = compiled.registry.id("Place.City").expect("tag registered");
        assert!(seed.state.global_tags.bit(location_tag));
        let current_tag = compiled
            .registry
            .id("location.current")
            .expect("current marker registered");
        let guide = seed.entity_indices["character:guide"] as usize;
        assert!(seed.state.entity_tags[guide].bit(current_tag));
        let location_vertex = seed
            .state
            .vertex("location:square")
            .expect("location vertex");
        assert_eq!(
            location_vertex.attributes.get("environment.danger"),
            Some(&storylet_engine::chrono_state::AttributeValue::Int(2)),
        );
        let guide_vertex = seed
            .state
            .vertex("character:guide")
            .expect("character vertex");
        assert_eq!(
            guide_vertex.attributes.get("status.trust"),
            Some(&storylet_engine::chrono_state::AttributeValue::Float(0.75)),
        );
        let quality_vertex = seed
            .state
            .vertex("quality:Trust")
            .expect("quality fact vertex");
        assert_eq!(
            quality_vertex.layer,
            storylet_engine::chrono_state::GraphLayer::Fact
        );
        assert_eq!(
            quality_vertex.attributes.get("category"),
            Some(&storylet_engine::chrono_state::AttributeValue::String(
                "metric".into()
            )),
        );
        let item_vertex = seed
            .state
            .vertex("quality:Clue")
            .expect("quality item vertex");
        assert_eq!(
            item_vertex.layer,
            storylet_engine::chrono_state::GraphLayer::Item
        );
        let item_index = seed.entity_indices["quality:Clue"] as usize;
        assert_eq!(item_vertex.entity_index, Some(item_index as u16));
        let tool_tag = compiled
            .registry
            .id("quality.Clue")
            .expect("tool tag registered");
        assert!(seed.state.entity_tags[item_index].bit(tool_tag));
    }
    #[test]
    fn seeds_menace_crisis_tag_and_makes_crisis_storylet_eligible_at_threshold() {
        let storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "crisis-scene",
            "title": "Crisis",
            "content": { "text_template": "The menace peaks." },
            "effects": {},
            "created_at": 1
        }))
        .expect("valid crisis storylet");
        let menace = QualityRecord {
            name: "Doom".into(),
            kind: "menace".into(),
            value: 10.0,
            min_value: 0.0,
            max_value: 5.0,
            is_hidden: false,
            menace_crisis_storylet_id: Some("crisis-scene".into()),
            description: None,
            metadata: None,
        };
        let location: LocationRecord = serde_json::from_value(serde_json::json!({
            "id": "start",
            "name": "Start",
            "description": "",
            "tags": []
        }))
        .expect("valid location fixture");
        let compiled =
            compile_chrono_dispatch(&[storylet], &[menace.clone()], &[location.clone()], &[])
                .expect("menace crisis should compile");
        let mut seed =
            super::initialize_chrono_world(&compiled, &[menace], &[location], &[], None, "start")
                .expect("world should seed");
        let crisis_tag = compiled
            .registry
            .id(&super::menace_crisis_tag("Doom"))
            .expect("crisis tag registered");
        assert!(seed.state.global_tags.bit(crisis_tag));
        let metric_id = compiled.metric_indices["Doom"] as usize;
        assert_eq!(seed.state.metrics[metric_id], 5.0);
        assert_eq!(seed.state.metric_bounds[metric_id].max, 5.0);
        assert!(
            compiled
                .dispatcher
                .is_eligible(&compiled.dispatcher.storylets[0], &seed.state,)
        );
        let delta = apply_mutation_with_registry(
            &mut seed.state,
            &MutationOp {
                metric_deltas: vec![(metric_id as u16, -1.0)],
                ..MutationOp::default()
            },
            &compiled.registry,
        )
        .expect("falling below the crisis threshold clears its tag");
        assert!(!seed.state.global_tags.bit(crisis_tag));
        assert!(
            !compiled
                .dispatcher
                .is_eligible(&compiled.dispatcher.storylets[0], &seed.state,)
        );
        revert_delta(&mut seed.state, &delta).expect("undo restores the crisis");
        assert!(seed.state.global_tags.bit(crisis_tag));
        assert_eq!(seed.state.metrics[metric_id], 5.0);
    }
    #[test]
    fn partial_storylet_validation_defers_crisis_cross_reference_to_full_compile() {
        let storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "unrelated-scene",
            "title": "Unrelated",
            "content": { "text_template": "Scene." },
            "effects": {},
            "created_at": 1
        }))
        .expect("valid storylet fixture");
        let menace = QualityRecord {
            name: "Doom".into(),
            kind: "menace".into(),
            value: 0.0,
            min_value: 0.0,
            max_value: 5.0,
            is_hidden: false,
            menace_crisis_storylet_id: Some("missing-crisis".into()),
            description: None,
            metadata: None,
        };
        assert!(
            super::compile_chrono_dispatch(
                std::slice::from_ref(&storylet),
                std::slice::from_ref(&menace),
                &[],
                &[],
            )
            .is_err()
        );
        super::compile_chrono_dispatch_for_storylet_validation(&[storylet], &[menace], &[], &[])
            .expect("an unrelated candidate must compile independently");
    }
    #[test]
    fn seeds_active_weighted_passion_macro_and_threshold_state() {
        let location: LocationRecord = serde_json::from_value(serde_json::json!({
            "id": "harbor", "name": "Harbor", "description": "", "tags": []
        }))
        .unwrap();
        let character: CharacterRecord = serde_json::from_value(serde_json::json!({
            "id": "captain", "name": "Captain", "role": "Role.Captain",
            "home_location": "harbor", "current_location": "harbor",
            "passion_pool": [
                {"passion_id": "loyalty", "current_value": 60.0, "weight": 0.5},
                {"passion_id": "fear", "current_value": -100.0, "weight": 1.0}
            ],
            "active_passions": ["loyalty"],
            "trust_inertia": 1.0,
            "thresholds": [{
                "threshold_id": "crisis", "trigger_value": 60.0,
                "direction": "positive", "linked_graph_asset_id": "captain-crisis"
            }]
        }))
        .unwrap();
        let compiled = compile_chrono_dispatch(
            &[],
            &[],
            std::slice::from_ref(&location),
            std::slice::from_ref(&character),
        )
        .expect("valid passion records compile");
        let seed = super::initialize_chrono_world(
            &compiled,
            &[],
            std::slice::from_ref(&location),
            std::slice::from_ref(&character),
            None,
            "harbor",
        )
        .expect("valid world seed");
        let npc_id = seed.entity_indices["character:captain"] as usize;
        let loyalty_id = compiled.passion_indices[&("captain".into(), "loyalty".into())] as usize;
        let fear_id = compiled.passion_indices[&("captain".into(), "fear".into())] as usize;
        assert_eq!(seed.state.passions.macro_values[npc_id], 61.0);
        assert!(seed.state.passions.active[loyalty_id]);
        assert!(!seed.state.passions.active[fear_id]);
        let threshold_bit = compiled
            .registry
            .id("passion.threshold.captain.crisis")
            .unwrap();
        assert!(seed.state.global_tags.bit(threshold_bit));
    }
}

fn insert_json_attributes(
    value: &serde_json::Value,
    prefix: &str,
    attributes: &mut BTreeMap<String, AttributeValue>,
) {
    match value {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                insert_json_attributes(value, &path, attributes);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                let path = format!("{prefix}.{index}");
                insert_json_attributes(value, &path, attributes);
            }
        }
        serde_json::Value::Bool(value) => {
            attributes.insert(prefix.to_owned(), AttributeValue::Bool(*value));
        }
        serde_json::Value::Number(value) => {
            if let Some(integer) = value.as_i64() {
                attributes.insert(prefix.to_owned(), AttributeValue::Int(integer));
            } else if let Some(float) = value.as_f64().filter(|float| float.is_finite()) {
                attributes.insert(prefix.to_owned(), AttributeValue::Float(float));
            }
        }
        serde_json::Value::String(value) => {
            attributes.insert(prefix.to_owned(), AttributeValue::String(value.clone()));
        }
        serde_json::Value::Null => {}
    }
}
