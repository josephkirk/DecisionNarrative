//! Incoming-reference discovery for authored entity templates, assets, and trait templates.
use serde_json::Value;
use crate::contracts::{
    AssetRecord, ContentReference, EntityTemplateRecord, GameplayTagRecord, PropertyType,
    ReferenceVia, ScenarioRecord, StoryletRecord, TraitTemplateRecord,
};
use crate::narrative_graph::{DialogueItem, NarrativeGraph, NarrativeNode, NarrativeNodeData};

pub struct ReferenceContext<'a> {
    pub entities: &'a [EntityTemplateRecord],
    pub templates: &'a [TraitTemplateRecord],
    pub assets: &'a [AssetRecord],
    pub storylets: &'a [StoryletRecord],
    pub scenarios: &'a [ScenarioRecord],
    pub gameplay_tags: &'a [GameplayTagRecord],
}

fn record(out: &mut Vec<ContentReference>, kind: &str, id: &str, label: &str, node: Option<&NarrativeNode>, field: String, via: ReferenceVia) {
    out.push(ContentReference { source_kind: kind.into(), source_id: id.into(), source_label: label.into(), node_id: node.map(|n| n.id.clone()), node_label: node.map(|n| n.title.clone()), field, via });
}
fn sorted(mut out: Vec<ContentReference>) -> Vec<ContentReference> {
    out.sort_by(|a,b| a.source_kind.cmp(&b.source_kind).then(a.source_label.cmp(&b.source_label)).then(a.field.cmp(&b.field))); out.dedup(); out
}
fn graph(s: &StoryletRecord) -> NarrativeGraph { s.graph.clone().unwrap_or_else(|| NarrativeGraph::from_storylet(s)) }
fn strings(v: &Value, out: &mut Vec<String>) { match v { Value::String(s)=>out.push(s.clone()), Value::Array(a)=>a.iter().for_each(|v|strings(v,out)), Value::Object(o)=>o.values().for_each(|v|strings(v,out)), _=>{} } }
fn value_has(v: &Value, needle: &str) -> bool { let mut ss=Vec::new(); strings(v,&mut ss); ss.iter().any(|s|s==needle) }

pub fn entity_references(
    target: &EntityTemplateRecord,
    ctx: &ReferenceContext,
) -> Vec<ContentReference> {
    let mut out = Vec::new();
    for storylet in ctx.storylets {
        for (graph, preconditions) in [(graph(storylet), false)]
            .into_iter()
            .chain(
                storylet
                    .precondition_graph
                    .clone()
                    .into_iter()
                    .map(|graph| (graph, true)),
            )
        {
            let prefix = if preconditions { "Preconditions " } else { "" };
            for node in &graph.nodes {
                match &node.data {
                    NarrativeNodeData::GetEntity { entity_path }
                        if entity_path == &target.id || entity_path == &target.game_id =>
                    {
                        record(
                            &mut out,
                            "storylet",
                            &storylet.id,
                            &storylet.title,
                            Some(node),
                            format!("{prefix}Get Entity node"),
                            ReferenceVia::Id,
                        );
                    }
                    NarrativeNodeData::AddEntityTag { target_entity_id, .. }
                    | NarrativeNodeData::RemoveEntityTag { target_entity_id, .. }
                    | NarrativeNodeData::HasEntityTag { target_entity_id, .. }
                        if target_entity_id == &target.id =>
                    {
                        record(
                            &mut out,
                            "storylet",
                            &storylet.id,
                            &storylet.title,
                            Some(node),
                            format!("{prefix}Entity tag"),
                            ReferenceVia::Id,
                        );
                    }
                    _ => {}
                }
            }
        }
        if storylet
            .location_tags
            .iter()
            .any(|tag| target.tags.iter().any(|entity_tag| entity_tag == tag) || target.root_tag == *tag)
        {
            record(
                &mut out,
                "storylet",
                &storylet.id,
                &storylet.title,
                None,
                "Storylet location tags".into(),
                ReferenceVia::Tag,
            );
        }
    }

    for source in ctx.entities {
        if source.id == target.id {
            continue;
        }
        let direct_reference = direct_property_reference(source, PropertyType::Entity, &target.id);
        let trait_reference = typed_trait_reference(source, ctx, PropertyType::Entity, &target.id);
        if let Some(field) = direct_reference.or(trait_reference) {
            record(
                &mut out,
                "entity_template",
                &source.id,
                &source.name,
                None,
                field,
                ReferenceVia::Id,
            );
        }
        if source.tags.iter().any(|tag| target.tags.iter().any(|other| other == tag)) {
            record(
                &mut out,
                "entity_template",
                &source.id,
                &source.name,
                None,
                "Gameplay tag".into(),
                ReferenceVia::Tag,
            );
        }
    }
    for scenario in ctx.scenarios {
        if scenario.start_location_policy.location_id.as_deref() == Some(&target.id) {
            record(
                &mut out,
                "scenario",
                &scenario.id,
                &scenario.name,
                None,
                "Start location".into(),
                ReferenceVia::Id,
            );
        }
    }
    sorted(out)
}

/// A direct entity property of `property_type` whose value contains `needle`;
/// returns the property name as the reference field.
fn direct_property_reference(
    entity: &EntityTemplateRecord,
    property_type: PropertyType,
    needle: &str,
) -> Option<String> {
    entity
        .properties
        .iter()
        .find(|property| property.property_type == property_type && value_has(&property.value, needle))
        .map(|property| {
            let name = property.name.trim();
            if name.is_empty() { format!("{} property", type_label(&property_type)) } else { name.to_string() }
        })
}

fn type_label(property_type: &PropertyType) -> &'static str {
    match property_type {
        PropertyType::Entity => "Entity",
        PropertyType::Storylet => "Storylet",
        PropertyType::Asset => "Asset",
        _ => "Typed",
    }
}

/// Where a storylet is linked from: entity storylet properties (direct or trait).
pub fn storylet_references(storylet_id: &str, ctx: &ReferenceContext) -> Vec<ContentReference> {
    let mut out = Vec::new();
    for entity in ctx.entities {
        let direct = direct_property_reference(entity, PropertyType::Storylet, storylet_id);
        let via_trait = typed_trait_reference(entity, ctx, PropertyType::Storylet, storylet_id);
        if let Some(field) = direct.or(via_trait) {
            record(&mut out, "entity_template", &entity.id, &entity.name, None, field, ReferenceVia::Id);
        }
    }
    sorted(out)
}

fn typed_trait_reference(
    entity: &EntityTemplateRecord,
    ctx: &ReferenceContext,
    property_type: PropertyType,
    needle: &str,
) -> Option<String> {
    for instance in &entity.traits {
        let Some(template) = ctx
            .templates
            .iter()
            .find(|template| template.id == instance.template_id)
        else {
            continue;
        };
        let mut properties = Vec::new();
        crate::entities::collect_template_properties(
            template,
            ctx.templates,
            &mut std::collections::BTreeSet::new(),
            &mut properties,
        );
        for property in properties {
            let definition = &property.property;
            let value = instance
                .values
                .get(property.key())
                .unwrap_or(&definition.default);
            if definition.property_type == property_type && value_has(value, needle) {
                return Some(format!("Trait {} {}", template.key, definition.name));
            }
        }
    }
    None
}

pub fn asset_references(asset_id: &str, ctx: &ReferenceContext) -> Vec<ContentReference> {
    let mut out = Vec::new();
    for entity in ctx.entities {
        let direct_reference = entity.properties.iter().any(|property| {
            property.property_type == PropertyType::Asset && value_has(&property.value, asset_id)
        });
        let trait_reference = typed_trait_reference(entity, ctx, PropertyType::Asset, asset_id);
        if direct_reference || trait_reference.is_some() {
            record(
                &mut out,
                "entity_template",
                &entity.id,
                &entity.name,
                None,
                trait_reference.unwrap_or_else(|| "Asset property".into()),
                ReferenceVia::Id,
            );
        }
    }
    sorted(out)
}
pub fn trait_template_references(id: &str, ctx: &ReferenceContext) -> Vec<ContentReference> {
    let mut out=Vec::new();
    for s in ctx.entities {
        if s.traits.iter().any(|t|t.template_id==id) {
            record(&mut out,"entity_template",&s.id,&s.name,None,format!("Trait {id}"),ReferenceVia::Id);
        }
    }
    sorted(out)
}

fn tagged_value(v: &Value, name: &str) -> bool {
    match v {
        Value::String(s) => s == name,
        Value::Array(values) => values.iter().any(|value| tagged_value(value, name)),
        Value::Object(values) => values.values().any(|value| tagged_value(value, name)),
        _ => false,
    }
}

fn role_query(query: &str, name: &str) -> bool {
    let q = query.trim();
    (q.contains("role") || q.contains("Role"))
        && q.split(|c| c == '"' || c == '\'').any(|part| part == name)
}

/// Find typed authored uses of a gameplay tag, excluding arbitrary prose.
pub fn gameplay_tag_references(target: &GameplayTagRecord, ctx: &ReferenceContext) -> Vec<ContentReference> {
    let mut out = Vec::new();
    for entity in ctx.entities {
        if entity.root_tag == target.name {
            record(&mut out, "entity_template", &entity.id, &entity.name, None, "Root gameplay tag".into(), ReferenceVia::Tag);
        }
        if entity.tags.iter().any(|tag| tag == &target.name) {
            record(&mut out, "entity_template", &entity.id, &entity.name, None, "Gameplay tag".into(), ReferenceVia::Tag);
        }
        for trait_instance in &entity.traits {
        let Some(template) = ctx.templates.iter().find(|template| template.id == trait_instance.template_id) else {
            continue;
        };
        let mut properties = Vec::new();
        crate::entities::collect_template_properties(
            template,
            ctx.templates,
            &mut std::collections::BTreeSet::new(),
            &mut properties,
        );
        for property in properties {
            let definition = &property.property;
            if !(property.key().to_ascii_lowercase().contains("tag")
                || property.key().to_ascii_lowercase().contains("role"))
            {
                continue;
            }
            let value = trait_instance
                .values
                .get(property.key())
                .unwrap_or(&definition.default);
            if tagged_value(value, &target.name) {
                record(&mut out, "entity_template", &entity.id, &entity.name, None,
                    format!("Trait {} {}", template.key, definition.name), ReferenceVia::Tag);
            }
        }
    }
    }
    for template in ctx.templates {
        for property in &template.properties {
            let definition = &property.property;
            if (property.key().to_ascii_lowercase().contains("tag") || property.key().to_ascii_lowercase().contains("role"))
                && tagged_value(&definition.default, &target.name)
            {
                record(&mut out, "trait_template", &template.id, &template.name, None,
                    format!("Default {}", definition.name), ReferenceVia::Tag);
            }
        }
    }
    for storylet in ctx.storylets {
        if storylet.location_tags.iter().any(|tag| tag == &target.name) {
            record(&mut out, "storylet", &storylet.id, &storylet.title, None, "Storylet location tags".into(), ReferenceVia::Tag);
        }
        for parameter in &storylet.parameters {
            if role_query(&parameter.query, &target.name) {
                record(&mut out, "storylet", &storylet.id, &storylet.title, None,
                    format!("Role query {}", parameter.param_name), ReferenceVia::Tag);
            }
        }
        for (graph_value, preconditions) in [(graph(storylet), false)]
            .into_iter().chain(storylet.precondition_graph.clone().into_iter().map(|g| (g, true)))
        {
            let prefix = if preconditions { "Preconditions " } else { "" };
            for node in &graph_value.nodes {
                match &node.data {
                    NarrativeNodeData::AddEntityTag { tag, .. }
                    | NarrativeNodeData::RemoveEntityTag { tag, .. }
                    | NarrativeNodeData::HasEntityTag { tag, .. } if tag == &target.name =>
                        record(&mut out, "storylet", &storylet.id, &storylet.title, Some(node),
                            format!("{prefix}Entity tag"), ReferenceVia::Tag),
                    NarrativeNodeData::Dialog { sections, .. } => {
                        for section in sections {
                            for item in &section.items {
                                let hit = match item {
                                    DialogueItem::Line { speaker_role, target_role, emotion_tag, action_tag, .. } =>
                                        [Some(speaker_role), Some(target_role), emotion_tag.as_ref(), action_tag.as_ref()]
                                            .into_iter().flatten().any(|value| value == &target.name),
                                    DialogueItem::IdleAction { actor_role, target: target_role, emote, .. } =>
                                        [Some(actor_role), Some(target_role), emote.as_ref()].into_iter().flatten()
                                            .any(|value| value == &target.name),
                                };
                                if hit {
                                    record(&mut out, "storylet", &storylet.id, &storylet.title, Some(node),
                                        format!("{prefix}Dialogue tag/role"), ReferenceVia::Tag);
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    sorted(out)
}
