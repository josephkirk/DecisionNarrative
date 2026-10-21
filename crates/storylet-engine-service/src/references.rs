//! Incoming-reference discovery for entities, assets, and trait templates.
//!
//! Each reference names the source record, the storylet graph node when the
//! source is a node, and how the link was made (`id`, `role`, or `tag`).

use serde_json::Value;

use crate::contracts::{
    AssetRecord, ContentReference, EntityRecord, EntityType, NpcBehaviorRecord, ReferenceVia,
    ScenarioRecord, StoryletRecord, TraitTemplateRecord,
};
use crate::narrative_graph::{DialogueItem, NarrativeGraph, NarrativeNode, NarrativeNodeData};
use crate::projection::parse_role_query;

pub struct ReferenceContext<'a> {
    pub entities: &'a [EntityRecord],
    pub templates: &'a [TraitTemplateRecord],
    pub assets: &'a [AssetRecord],
    pub storylets: &'a [StoryletRecord],
    pub scenarios: &'a [ScenarioRecord],
    pub npc_behaviors: &'a [NpcBehaviorRecord],
}

#[allow(clippy::too_many_arguments)]
fn record(
    out: &mut Vec<ContentReference>,
    source_kind: &str,
    source_id: &str,
    source_label: &str,
    node: Option<&NarrativeNode>,
    field: String,
    via: ReferenceVia,
) {
    out.push(ContentReference {
        source_kind: source_kind.to_string(),
        source_id: source_id.to_string(),
        source_label: source_label.to_string(),
        node_id: node.map(|node| node.id.clone()),
        node_label: node.map(|node| node.title.clone()),
        field,
        via,
    });
}

fn sort_records(mut references: Vec<ContentReference>) -> Vec<ContentReference> {
    references.sort_by(|left, right| {
        left.source_kind
            .cmp(&right.source_kind)
            .then_with(|| left.source_label.cmp(&right.source_label))
            .then_with(|| {
                left.node_label
                    .as_deref()
                    .unwrap_or("")
                    .cmp(right.node_label.as_deref().unwrap_or(""))
            })
            .then_with(|| left.field.cmp(&right.field))
    });
    references.dedup();
    references
}

fn preconditions<'a>(preconditions: &'a Value, key: &str) -> &'a [Value] {
    preconditions
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn effect_target<'a>(effects: &'a Value, key: &str) -> Option<&'a str> {
    effects.get(key).and_then(Value::as_str)
}

fn collect_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| collect_strings(item, out)),
        Value::Object(map) => map.values().for_each(|item| collect_strings(item, out)),
        _ => {}
    }
}

fn transform_mentions(rule: &Value, entity: &EntityRecord) -> bool {
    let layer = match entity.entity_type {
        EntityType::Location => "location",
        EntityType::Character => "character",
        EntityType::Item => "item",
        EntityType::Fact => "fact",
        EntityType::Other => return false,
    };
    let needle = format!("{layer}:{}", entity.id);
    let mut strings = Vec::new();
    collect_strings(rule, &mut strings);
    strings.iter().any(|value| value == &needle)
}

fn graph_of(storylet: &StoryletRecord) -> Option<NarrativeGraph> {
    match &storylet.graph {
        Some(value) => serde_json::from_value(value.clone()).ok(),
        None => Some(NarrativeGraph::from_storylet(storylet)),
    }
}

fn dialog_mentions(node: &NarrativeNode, role: &str) -> Vec<String> {
    let NarrativeNodeData::Dialog { speaker_role, sections, .. } = &node.data else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    if speaker_role.as_deref() == Some(role) {
        hits.push("Dialog speaker role".to_string());
    }
    for section in sections {
        for item in &section.items {
            match item {
                DialogueItem::Line {
                    speaker_role,
                    target_role,
                    action_target,
                    ..
                } => {
                    if speaker_role == role {
                        hits.push(format!("Line speaker role ({})", section.title));
                    }
                    if target_role == role {
                        hits.push(format!("Line target role ({})", section.title));
                    }
                    if action_target.as_deref() == Some(role) {
                        hits.push(format!("Line action target ({})", section.title));
                    }
                }
                DialogueItem::IdleAction {
                    actor_role, target, ..
                } => {
                    if actor_role == role {
                        hits.push(format!("Idle action actor ({})", section.title));
                    }
                    if target == role {
                        hits.push(format!("Idle action target ({})", section.title));
                    }
                }
            }
        }
    }
    hits
}

fn property_mentions(reference: &str, entity: &EntityRecord, ctx: &ReferenceContext) -> Vec<ContentReference> {
    let mut out = Vec::new();
    for source in ctx.entities {
        for property in &source.properties {
            if property.property_type != crate::contracts::PropertyType::Entity {
                continue;
            }
            let values: Vec<&Value> = if property.is_array {
                property
                    .value
                    .as_array()
                    .map(|items| items.iter().collect())
                    .unwrap_or_default()
            } else {
                vec![&property.value]
            };
            if values.iter().any(|value| value.as_str() == Some(reference)) {
                record(
                    &mut out,
                    "entity",
                    &source.id,
                    &source.name,
                    None,
                    format!("Property {}", property.name),
                    ReferenceVia::Id,
                );
            }
        }
        for instance in &source.traits {
            let Some(template) = ctx
                .templates
                .iter()
                .find(|template| template.id == instance.template_id)
            else {
                continue;
            };
            for definition in &template.properties {
                if definition.property_type != crate::contracts::PropertyType::Entity {
                    continue;
                }
                let effective = match instance.values.get(&definition.id) {
                    Some(value)
                        if crate::entities::value_shape_matches(
                            definition.property_type,
                            definition.is_array,
                            value,
                        ) =>
                    {
                        value.clone()
                    }
                    _ => definition.default.clone(),
                };
                let values: Vec<&Value> = if definition.is_array {
                    effective
                        .as_array()
                        .map(|items| items.iter().collect())
                        .unwrap_or_default()
                } else {
                    vec![&effective]
                };
                if values.iter().any(|value| value.as_str() == Some(reference)) {
                    record(
                        &mut out,
                        "entity",
                        &source.id,
                        &source.name,
                        None,
                        format!("Trait {}.{}", template.name, definition.name),
                        ReferenceVia::Id,
                    );
                }
            }
        }
    }
    let _ = entity;
    out
}

pub fn entity_references(target: &EntityRecord, ctx: &ReferenceContext) -> Vec<ContentReference> {
    let mut out: Vec<ContentReference> = Vec::new();
    let is_location = target.entity_type == EntityType::Location;
    let is_character = target.entity_type == EntityType::Character;
    let location_tags: Vec<&str> = target
        .location
        .as_ref()
        .map(|section| section.tags.iter().map(String::as_str).collect())
        .unwrap_or_default();
    let role = target
        .character
        .as_ref()
        .map(|section| section.role.clone())
        .unwrap_or_default();

    for storylet in ctx.storylets {
        let graph = graph_of(storylet);
        if let Some(graph) = &graph {
            for node in &graph.nodes {
                match &node.data {
                    NarrativeNodeData::GoToLocation { location_id }
                        if is_location && location_id == &target.id =>
                    {
                        record(&mut out, "storylet", &storylet.id, &storylet.title, Some(node), "Go To Location node".into(), ReferenceVia::Id);
                    }
                    NarrativeNodeData::Choice { choices } => {
                        for choice in choices {
                            if let Some(effects) = &choice.effects
                                && is_location
                                && effect_target(effects, "$move_player") == Some(target.id.as_str())
                            {
                                record(&mut out, "storylet", &storylet.id, &storylet.title, Some(node), "Choice effect $move_player".into(), ReferenceVia::Id);
                            }
                        }
                    }
                    NarrativeNodeData::GraphTransform { rule } => {
                        let rule_value = serde_json::to_value(rule).unwrap_or(Value::Null);
                        if transform_mentions(&rule_value, target) {
                            record(&mut out, "storylet", &storylet.id, &storylet.title, Some(node), "Graph transform".into(), ReferenceVia::Id);
                        }
                    }
                    NarrativeNodeData::Dialog { .. } if is_character && !role.trim().is_empty() => {
                        for field in dialog_mentions(node, &role) {
                            record(&mut out, "storylet", &storylet.id, &storylet.title, Some(node), field, ReferenceVia::Role);
                        }
                    }
                    _ => {}
                }
            }
        }

        for choice in &storylet.choices {
            if is_location && effect_target(&choice.effects, "$move_player") == Some(target.id.as_str())
            {
                record(&mut out, "storylet", &storylet.id, &storylet.title, None, "Storylet choice effect $move_player".into(), ReferenceVia::Id);
            }
        }

        if is_location {
            for entry in preconditions(&storylet.preconditions, "visited_locations_any_of") {
                if entry.get("location_id").and_then(Value::as_str) == Some(target.id.as_str()) {
                    record(&mut out, "storylet", &storylet.id, &storylet.title, None, "Visited location precondition".into(), ReferenceVia::Id);
                }
                if let Some(tag) = entry.get("tag").and_then(Value::as_str)
                    && location_tags.contains(&tag)
                {
                    record(&mut out, "storylet", &storylet.id, &storylet.title, None, format!("Visited location tag '{tag}'"), ReferenceVia::Tag);
                }
            }
            let matching_tags: Vec<&str> = storylet
                .location_tags
                .iter()
                .map(String::as_str)
                .filter(|tag| location_tags.contains(tag))
                .collect();
            if !matching_tags.is_empty() {
                record(&mut out, "storylet", &storylet.id, &storylet.title, None, format!("Location tags: {}", matching_tags.join(", ")), ReferenceVia::Tag);
            }
        }

        if is_character {
            for parameter in &storylet.parameters {
                if !role.trim().is_empty() && parse_role_query(&parameter.query).as_deref() == Some(role.as_str()) {
                    record(&mut out, "storylet", &storylet.id, &storylet.title, None, format!("Role parameter {}", parameter.param_name), ReferenceVia::Role);
                }
            }
            for entry in preconditions(&storylet.preconditions, "met_characters_any_of") {
                if entry.get("character_id").and_then(Value::as_str) == Some(target.id.as_str()) {
                    record(&mut out, "storylet", &storylet.id, &storylet.title, None, "Met character precondition".into(), ReferenceVia::Id);
                }
                if !role.trim().is_empty()
                    && entry.get("role").and_then(Value::as_str) == Some(role.as_str())
                {
                    record(&mut out, "storylet", &storylet.id, &storylet.title, None, "Met character role precondition".into(), ReferenceVia::Role);
                }
            }
        }
    }

    for scenario in ctx.scenarios {
        if is_location {
            if scenario.start_location_policy.location_id.as_deref() == Some(target.id.as_str()) {
                record(&mut out, "scenario", &scenario.id, &scenario.name, None, "Start location".into(), ReferenceVia::Id);
            }
            if let Some(policy) = &scenario.actor_population_policy.location_policy
                && policy.location_id.as_deref() == Some(target.id.as_str())
            {
                record(&mut out, "scenario", &scenario.id, &scenario.name, None, "Actor location".into(), ReferenceVia::Id);
            }
            if scenario
                .actor_locations
                .values()
                .any(|value| value == &target.id)
            {
                record(&mut out, "scenario", &scenario.id, &scenario.name, None, "Actor location override".into(), ReferenceVia::Id);
            }
        }
        if is_character && scenario.actor_locations.contains_key(&target.id) {
            record(&mut out, "scenario", &scenario.id, &scenario.name, None, "Actor location override".into(), ReferenceVia::Id);
        }
    }

    if is_character {
        for behavior in ctx.npc_behaviors {
            if behavior.actor_id == target.id {
                record(&mut out, "npc_behavior", &behavior.id, &behavior.action, None, "Actor".into(), ReferenceVia::Id);
            }
        }
        for other in ctx.entities {
            if other.id == target.id {
                continue;
            }
            if let Some(section) = &other.character
                && section
                    .zero_sum_passion_links
                    .iter()
                    .any(|link| link.target_character_id == target.id)
            {
                record(&mut out, "entity", &other.id, &other.name, None, "Zero-sum passion link".into(), ReferenceVia::Id);
            }
        }
    }

    if is_location {
        for other in ctx.entities {
            if other.id == target.id {
                continue;
            }
            if let Some(section) = &other.location
                && section
                    .connected_zones
                    .as_ref()
                    .is_some_and(|zones| zones.iter().any(|zone| zone == &target.id))
            {
                record(&mut out, "entity", &other.id, &other.name, None, "Connected zone".into(), ReferenceVia::Id);
            }
            if let Some(section) = &other.character {
                if section.home_location == target.id {
                    record(&mut out, "entity", &other.id, &other.name, None, "Home location".into(), ReferenceVia::Id);
                }
                if section.current_location == target.id {
                    record(&mut out, "entity", &other.id, &other.name, None, "Current location".into(), ReferenceVia::Id);
                }
            }
        }
    }

    out.extend(property_mentions(&target.id, target, ctx));

    for template in ctx.templates {
        for definition in &template.properties {
            if definition.property_type != crate::contracts::PropertyType::Entity {
                continue;
            }
            let values: Vec<&Value> = if definition.is_array {
                definition
                    .default
                    .as_array()
                    .map(|items| items.iter().collect())
                    .unwrap_or_default()
            } else {
                vec![&definition.default]
            };
            if values.iter().any(|value| value.as_str() == Some(target.id.as_str())) {
                record(
                    &mut out,
                    "trait_template",
                    &template.id,
                    &template.name,
                    None,
                    format!("Default {}", definition.name),
                    ReferenceVia::Id,
                );
            }
        }
    }

    sort_records(out)
}

pub fn asset_references(asset_id: &str, ctx: &ReferenceContext) -> Vec<ContentReference> {
    let mut out = Vec::new();
    for template in ctx.templates {
        for definition in &template.properties {
            if definition.property_type != crate::contracts::PropertyType::Asset {
                continue;
            }
            if property_link_matches(&definition.default, definition.is_array, asset_id) {
                record(
                    &mut out,
                    "trait_template",
                    &template.id,
                    &template.name,
                    None,
                    format!("Default {}", definition.name),
                    ReferenceVia::Id,
                );
            }
        }
    }
    for source in ctx.entities {
        for property in &source.properties {
            if property.property_type != crate::contracts::PropertyType::Asset {
                continue;
            }
            if property_link_matches(&property.value, property.is_array, asset_id) {
                record(&mut out, "entity", &source.id, &source.name, None, format!("Property {}", property.name), ReferenceVia::Id);
            }
        }
        for instance in &source.traits {
            let Some(template) = ctx
                .templates
                .iter()
                .find(|template| template.id == instance.template_id)
            else {
                continue;
            };
            for definition in &template.properties {
                if definition.property_type != crate::contracts::PropertyType::Asset {
                    continue;
                }
                let effective = match instance.values.get(&definition.id) {
                    Some(value)
                        if crate::entities::value_shape_matches(
                            definition.property_type,
                            definition.is_array,
                            value,
                        ) =>
                    {
                        value.clone()
                    }
                    _ => definition.default.clone(),
                };
                if property_link_matches(&effective, definition.is_array, asset_id) {
                    record(&mut out, "entity", &source.id, &source.name, None, format!("Trait {}.{}", template.name, definition.name), ReferenceVia::Id);
                }
            }
        }
    }
    sort_records(out)
}

fn property_link_matches(value: &Value, is_array: bool, asset_id: &str) -> bool {
    let values: Vec<&Value> = if is_array {
        value
            .as_array()
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    } else {
        vec![value]
    };
    values.iter().any(|item| {
        crate::entities::asset_link(item)
            .is_some_and(|(linked, _)| linked == asset_id)
    })
}

pub fn trait_template_references(template_id: &str, ctx: &ReferenceContext) -> Vec<ContentReference> {
    let mut out = Vec::new();
    for source in ctx.entities {
        if !source
            .traits
            .iter()
            .any(|instance| instance.template_id == template_id)
        {
            continue;
        }
        let name = ctx
            .templates
            .iter()
            .find(|template| template.id == template_id)
            .map(|template| template.name.clone())
            .unwrap_or_else(|| "trait".to_string());
        record(&mut out, "entity", &source.id, &source.name, None, format!("Trait {name}"), ReferenceVia::Id);
    }
    sort_records(out)
}
