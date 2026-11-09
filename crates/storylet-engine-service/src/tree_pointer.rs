//! Runtime projection for TreeGraph position nodes.
use serde_json::{json, Value};
use crate::contracts::{EntityProperty, EntityRecord, EntityTemplateRecord, PropertyType, TraitTemplateRecord};

pub const POSITION: &str = "__tree_position";
pub const NODES: &str = "__tree_nodes";
pub fn is_reserved_key(key: &str) -> bool { matches!(key, POSITION | NODES) }

pub fn uses_pointer(graph: &crate::narrative_graph::NarrativeGraph) -> bool {
    graph.nodes.iter().any(|node| match &node.data {
        crate::narrative_graph::NarrativeNodeData::ApplyCommand { property_key, .. }
        | crate::narrative_graph::NarrativeNodeData::GetEntityProperty { property_key, .. } => property_key == POSITION,
        _ => false,
    })
}

pub fn validate_pointer_graph(graph: &crate::narrative_graph::NarrativeGraph, entities: &[EntityTemplateRecord], templates: &[TraitTemplateRecord]) -> Result<(), String> {
    use crate::narrative_graph::{NarrativeEdgeKind, NarrativeNodeData};
    for node in &graph.nodes {
        let (value_type, command) = match &node.data {
            NarrativeNodeData::ApplyCommand { property_key, value_type, operation, value } if property_key == POSITION => {
                if operation != "set" { return Err(format!("node '{}' must set the tree position", node.id)); }
                (value_type, Some(value))
            }
            NarrativeNodeData::GetEntityProperty { property_key, value_type } if property_key == POSITION => (value_type, None),
            _ => continue,
        };
        if value_type != "string" { return Err(format!("node '{}' has an invalid tree position type", node.id)); }
        let mut inputs = graph.edges.iter().filter(|edge| edge.kind == NarrativeEdgeKind::Data && edge.to_node == node.id && edge.to_port == "entity");
        let input = inputs.next().filter(|edge| edge.from_port == "output")
            .filter(|_| inputs.next().is_none())
            .ok_or_else(|| format!("node '{}' needs one configured tree entity input", node.id))?;
        let owner = graph.nodes.iter().find(|source| source.id == input.from_node)
            .and_then(|source| match &source.data { NarrativeNodeData::GetEntity { entity_path } => entities.iter().find(|entity| entity.id == entity_path.trim()), _ => None })
            .ok_or_else(|| format!("node '{}' needs a configured Get Entity tree input", node.id))?;
        let is_tree = templates.iter().any(|template| template.key == crate::tree_graph::TREE_GRAPH_KEY
            && owner.traits.iter().any(|instance| instance.template_id == template.id));
        if !is_tree { return Err(format!("entity '{}' must have TreeGraph", owner.id)); }
        let members = owner.properties.iter().find(|property| property.key == NODES).and_then(|property| property.value.as_array())
            .ok_or_else(|| format!("entity '{}' must have TreeGraph", owner.id))?;
        if command.is_some_and(|value| !members.contains(value)) {
            return Err(format!("node '{}' selects a position outside tree '{}'", node.id, owner.id));
        }
        if command.is_none() {
            for edge in graph.edges.iter().filter(|edge| edge.kind == NarrativeEdgeKind::Data && edge.from_node == node.id) {
                if let Some(NarrativeNodeData::Compare { value, .. }) = graph.nodes.iter().find(|target| target.id == edge.to_node).map(|target| &target.data) {
                    if !members.contains(value) { return Err(format!("node '{}' selects a position outside tree '{}'", node.id, owner.id)); }
                }
            }
        }
    }
    Ok(())
}

/// Refuse tree edits that would invalidate a saved storylet's selected position.
pub fn validate_owner_references(
    owner_id: &str,
    traits: Option<&[crate::contracts::TraitInstance]>,
    templates: &[TraitTemplateRecord],
    storylets: &[crate::contracts::StoryletRecord],
) -> Result<(), String> {
    use crate::narrative_graph::{NarrativeEdgeKind, NarrativeNodeData};
    let tree_id = templates.iter().find(|template| template.key == crate::tree_graph::TREE_GRAPH_KEY).map(|template| template.id.as_str());
    let tree = traits.and_then(|traits| traits.iter().find(|instance| Some(instance.template_id.as_str()) == tree_id));
    for storylet in storylets {
        for graph in [storylet.graph.as_ref(), storylet.precondition_graph.as_ref()].into_iter().flatten() {
            for node in &graph.nodes {
                let target = match &node.data {
                    NarrativeNodeData::ApplyCommand { property_key, value, .. } if property_key == POSITION => Some(value),
                    NarrativeNodeData::GetEntityProperty { property_key, .. } if property_key == POSITION => None,
                    _ => continue,
                };
                let targets_owner = graph.edges.iter().any(|edge| {
                    edge.kind == NarrativeEdgeKind::Data && edge.to_node == node.id && edge.to_port == "entity"
                        && graph.nodes.iter().any(|source| source.id == edge.from_node
                            && matches!(&source.data, NarrativeNodeData::GetEntity { entity_path } if entity_path.trim() == owner_id))
                });
                if !targets_owner { continue; }
                let members = tree.and_then(|instance| instance.values.get("nodes")).and_then(Value::as_array)
                    .ok_or_else(|| format!("Cannot change tree '{owner_id}': storylet '{}' uses its tree position.", storylet.title))?;
                if target.is_some_and(|value| !members.contains(value)) {
                    return Err(format!("Cannot change tree '{owner_id}': storylet '{}' selects a removed tree item.", storylet.title));
                }
                if target.is_none() {
                    for edge in graph.edges.iter().filter(|edge| edge.kind == NarrativeEdgeKind::Data && edge.from_node == node.id) {
                        if let Some(NarrativeNodeData::Compare { value, .. }) = graph.nodes.iter().find(|next| next.id == edge.to_node).map(|next| &next.data) {
                            if !members.contains(value) {
                                return Err(format!("Cannot change tree '{owner_id}': storylet '{}' checks a removed tree item.", storylet.title));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn project(properties: &mut Vec<EntityProperty>, values: &std::collections::BTreeMap<String, Value>, name: &str) -> Result<(), String> {
    if properties.iter().any(|property| property.key == POSITION || property.key == NODES) {
        return Err(format!("entity '{name}' uses a reserved tree property"));
    }
    let nodes = values.get("nodes").cloned().unwrap_or_else(|| json!([]));
    if !nodes.is_array() { return Err(format!("tree '{name}' has invalid nodes")); }
    for (key, property_type, is_array, value) in [
        (POSITION, PropertyType::String, false, json!("")),
        (NODES, PropertyType::Entity, true, nodes),
    ] {
        properties.push(EntityProperty { id: key.into(), key: key.into(), name: key.into(), property_type, is_array, multiline: false, value });
    }
    Ok(())
}

pub fn project_tree_entities(entities: &mut Vec<EntityTemplateRecord>, instances: &[EntityRecord], templates: &[TraitTemplateRecord]) -> Result<(), String> {
    let Some(id) = templates.iter().find(|template| template.key == "tree_graph").map(|template| template.id.as_str()) else { return Ok(()) };
    for entity in instances.iter().filter(|entity| entity.id != "world" && entity.traits.iter().any(|instance| instance.template_id == id)) {
        if let Some(existing) = entities.iter().find(|existing| existing.id == entity.id) {
            // Inventory projection may already have added this concrete entity.
            let inventory = templates.iter().any(|template| matches!(template.key.as_str(), "inventory_owner" | "item")
                && entity.traits.iter().any(|instance| instance.template_id == template.id));
            if inventory && existing.traits == entity.traits {
                continue;
            }
            return Err(format!("tree entity '{}' has a duplicate ID", entity.id));
        }
        entities.push(EntityTemplateRecord {
            id: entity.id.clone(), object_id: entity.object_id.clone().unwrap_or_default(), game_id: entity.game_id.clone().unwrap_or_default(), game_id_auto: false,
            name: entity.name.clone(), description: entity.description.clone(), tags: entity.tags.clone(), thumbnail: entity.thumbnail.clone(), folder_id: entity.folder_id.clone(), properties: entity.properties.clone(), traits: entity.traits.clone(), created_at: entity.created_at,
        });
    }
    for entity in entities.iter_mut() {
        if let Some(instance) = entity.traits.iter().find(|instance| instance.template_id == id) {
            project(&mut entity.properties, &instance.values, &entity.id)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrono_adapter::compile_chrono_dispatch;
    use crate::chrono_graph_adapter::{bind_storylet_start_nodes, compile_narrative_graphs};
    use crate::contracts::StoryletRecord;
    use crate::narrative_graph::NarrativeGraph;
    use std::collections::BTreeMap;
    use storylet_engine::chrono_runtime::{FlowGraphInstance, RuntimeContext};
    use storylet_engine::chrono_state::{AttributeValue, GraphLayer, WorldStateTable, WorldVertex};

    fn graph(target: &str) -> NarrativeGraph {
        serde_json::from_value(json!({
            "nodes": [
                {"id":"start","type":"start","title":"start","text":"Start","x":0,"y":0,"width":100,"height":100},
                {"id":"owner","type":"get_entity","title":"owner","entity_path":"tree","x":0,"y":0,"width":100,"height":100},
                {"id":"set","type":"apply_command","title":"set","property_key":POSITION,"operation":"set","value_type":"string","value":target,"x":0,"y":0,"width":100,"height":100},
                {"id":"get","type":"get_entity_property","title":"get","property_key":POSITION,"value_type":"string","x":0,"y":0,"width":100,"height":100},
                {"id":"compare","type":"compare","title":"compare","operator":"eq","value_type":"string","value":target,"x":0,"y":0,"width":100,"height":100},
                {"id":"condition","type":"condition","title":"condition","x":0,"y":0,"width":100,"height":100},
                {"id":"success","type":"end","title":"success","is_terminal":true,"x":0,"y":0,"width":100,"height":100},
                {"id":"failure","type":"end","title":"failure","is_terminal":true,"x":0,"y":0,"width":100,"height":100}
            ],
            "edges": [
                {"id":"a","kind":"flow","from_node":"start","from_port":"output","to_node":"set","to_port":"input"},
                {"id":"b","kind":"flow","from_node":"set","from_port":"output","to_node":"condition","to_port":"input"},
                {"id":"c","kind":"flow","from_node":"condition","from_port":"true","to_node":"success","to_port":"input"},
                {"id":"d","kind":"flow","from_node":"condition","from_port":"false","to_node":"failure","to_port":"input"},
                {"id":"e","kind":"data","from_node":"owner","from_port":"output","to_node":"set","to_port":"entity"},
                {"id":"f","kind":"data","from_node":"owner","from_port":"output","to_node":"get","to_port":"entity"},
                {"id":"g","kind":"data","from_node":"get","from_port":"output","to_node":"compare","to_port":"input"},
                {"id":"h","kind":"data","from_node":"compare","from_port":"output","to_node":"condition","to_port":"input"}
            ]
        })).unwrap()
    }

    #[test]
    fn validates_tree_membership_and_projects_without_mutating_authored_entities() {
        let templates: Vec<TraitTemplateRecord> = serde_json::from_value(json!([
            {"id":"tree-trait","key":"tree_graph","name":"TreeGraph","properties":[]}
        ])).unwrap();
        let original: Vec<EntityTemplateRecord> = serde_json::from_value(json!([
            {"id":"tree","name":"Tree","traits":[{"id":"t","template_id":"tree-trait","values":{"nodes":["a","b"]}}]},
            {"id":"other","name":"Other"}
        ])).unwrap();
        let mut projected = original.clone();
        project_tree_entities(&mut projected, &[], &templates).unwrap();
        assert!(original[0].properties.is_empty());
        assert!(validate_pointer_graph(&graph("a"), &projected, &templates).is_ok());
        assert!(validate_pointer_graph(&graph("missing"), &projected, &templates).unwrap_err().contains("outside tree"));
        assert!(validate_pointer_graph(&graph("a"), &projected[1..], &templates).is_err());
        let mut impostor = projected.clone();
        impostor[0].traits.clear();
        assert!(validate_pointer_graph(&graph("a"), &impostor, &templates).unwrap_err().contains("TreeGraph"));
        let mut spaced = graph("a");
        if let crate::narrative_graph::NarrativeNodeData::GetEntity { entity_path } = &mut spaced.nodes[1].data {
            *entity_path = " tree ".into();
        }
        assert!(validate_pointer_graph(&spaced, &projected, &templates).is_ok());
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id":"test","title":"Referenced story","content":{"text_template":"x"},"created_at":1,"graph":spaced
        })).unwrap();
        assert!(validate_owner_references("tree", Some(&original[0].traits), &templates, std::slice::from_ref(&storylet)).is_ok());
        assert!(validate_owner_references("tree", None, &templates, std::slice::from_ref(&storylet)).is_err());
        let mut changed = original[0].traits.clone();
        changed[0].values.insert("nodes".into(), json!(["b"]));
        assert!(validate_owner_references("tree", Some(&changed), &templates, std::slice::from_ref(&storylet)).is_err());
        assert!(validate_owner_references("other", None, &templates, &[storylet]).is_ok());
    }

    #[test]
    fn concrete_tree_owner_can_also_have_inventory() {
        let templates: Vec<TraitTemplateRecord> = serde_json::from_value(json!([
            {"id":"tree-trait","key":"tree_graph","name":"TreeGraph","properties":[]},
            {"id":"inventory-trait","key":"inventory_owner","name":"InventoryOwner","properties":[]}
        ])).unwrap();
        let instances: Vec<EntityRecord> = serde_json::from_value(json!([
            {"id":"tree","name":"Tree","traits":[
                {"id":"t","template_id":"tree-trait","values":{"nodes":["a"]}},
                {"id":"i","template_id":"inventory-trait","values":{"items":[]}}
            ]}
        ])).unwrap();
        let mut projected = Vec::new();
        crate::inventory::project_inventory_entities(&mut projected, &instances, &templates).unwrap();
        project_tree_entities(&mut projected, &instances, &templates).unwrap();
        assert_eq!(projected.len(), 1);
        assert!(projected[0].properties.iter().any(|property| property.key == POSITION));
        assert!(projected[0].properties.iter().any(|property| property.key == "__inventory_items"));
        let mut conflicting: Vec<EntityTemplateRecord> = serde_json::from_value(json!([{"id":"tree","name":"Duplicate"}])).unwrap();
        assert!(crate::inventory::project_inventory_entities(&mut conflicting, &instances, &templates).is_err());
        let mut tree_only = instances.clone();
        tree_only[0].traits.retain(|instance| instance.template_id == "tree-trait");
        assert!(project_tree_entities(&mut conflicting, &tree_only, &templates).is_err());
    }

    #[test]
    fn set_and_check_position_execute_on_the_selected_owner() {
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id":"pointer-test","title":"Pointer test","content":{"text_template":"x"},"created_at":1,"graph":graph("b")
        })).unwrap();
        storylet.graph.as_ref().unwrap().validate().unwrap();
        let mut dispatch = compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[]).unwrap();
        let ids = BTreeMap::from([("tree".to_owned(), 0)]);
        let graphs = compile_narrative_graphs("project", std::slice::from_ref(&storylet), &dispatch, &ids).unwrap();
        bind_storylet_start_nodes(&mut dispatch, &graphs).unwrap();
        let mut state = WorldStateTable::new();
        for id in ["other", "tree"] {
            state.vertices.push(WorldVertex { vertex_id:id.into(), layers:GraphLayer::Item.into(), entity_index:None,
                attributes:BTreeMap::from([("prop.__tree_position".into(), AttributeValue::String(String::new()))]) });
        }
        let mut instance = FlowGraphInstance::new(1, graphs.asset, 7).unwrap();
        let mut context = RuntimeContext::with_registry(&mut state, &dispatch.dispatcher, &dispatch.registry);
        instance.start().unwrap();
        instance.tick(&mut context, 100).unwrap();
        assert_eq!(context.state.vertex("tree").unwrap().attributes.get("prop.__tree_position"), Some(&AttributeValue::String("b".into())));
        assert_eq!(context.state.vertex("other").unwrap().attributes.get("prop.__tree_position"), Some(&AttributeValue::String(String::new())));
        assert!(instance.call_history().iter().any(|entry| instance.asset.node(entry.node_guid).is_some_and(|node| node.payload["nodeId"] == "success")));
    }
}
