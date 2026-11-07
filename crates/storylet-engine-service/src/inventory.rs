//! Inventory trait projection and graph input validation for deterministic playtests.
use std::collections::BTreeSet;

use serde_json::{json, Value};

use crate::contracts::{EntityProperty, EntityRecord, EntityTemplateRecord, PropertyType, TraitTemplateRecord};
use crate::narrative_graph::{NarrativeEdgeKind, NarrativeGraph, NarrativeNodeData};

const ITEMS: &str = "__inventory_items";
const ROWS: &str = "__inventory_rows";
const COLUMNS: &str = "__inventory_columns";
const ITEM: &str = "__inventory_item";

fn runtime_property(key: &str, property_type: PropertyType, is_array: bool, value: Value) -> EntityProperty {
    EntityProperty { id: key.into(), key: key.into(), name: key.into(), property_type, is_array, multiline: false, value }
}

/// Project authored trait values into runtime-only typed properties. No project
/// entity is written: replay reconstructs the same projection from the revision.
pub fn project_inventory_entities(
    entities: &mut Vec<EntityTemplateRecord>,
    instances: &[EntityRecord],
    templates: &[TraitTemplateRecord],
) -> Result<(), String> {
    let owner_id = templates.iter().find(|t| t.key == "inventory_owner").map(|t| t.id.as_str());
    let item_id = templates.iter().find(|t| t.key == "item").map(|t| t.id.as_str());
    let has_trait = |traits: &[crate::contracts::TraitInstance], id: Option<&str>| id.is_some_and(|id| traits.iter().any(|t| t.template_id == id));
    // The engine normally projects templates. Include concrete inventory entities
    // too so an explicit Get Entity input can refer to either authoring kind.
    for entity in instances.iter().filter(|entity| entity.id != "world" && (has_trait(&entity.traits, owner_id) || has_trait(&entity.traits, item_id))) {
        if entities.iter().any(|other| other.id == entity.id) { return Err(format!("inventory entity '{}' has a duplicate ID", entity.id)); }
        entities.push(EntityTemplateRecord {
            id: entity.id.clone(), object_id: entity.object_id.clone().unwrap_or_default(), game_id: entity.game_id.clone().unwrap_or_default(), game_id_auto: false,
            name: entity.name.clone(), description: entity.description.clone(), tags: entity.tags.clone(),
            thumbnail: entity.thumbnail.clone(), folder_id: entity.folder_id.clone(), properties: entity.properties.clone(), traits: entity.traits.clone(), created_at: entity.created_at,
        });
    }
    let item_ids: BTreeSet<String> = entities.iter().filter(|entity| has_trait(&entity.traits, item_id)).map(|entity| entity.id.clone()).collect();
    for entity in entities.iter_mut() {
        let owner = owner_id.and_then(|id| entity.traits.iter().find(|instance| instance.template_id == id));
        let is_item = item_ids.contains(&entity.id);
        if owner.is_none() && !is_item { continue; }
        if entity.properties.iter().any(|property| [ITEMS, ROWS, COLUMNS, ITEM].contains(&property.key.as_str())) {
            return Err(format!("entity '{}' uses a reserved inventory property", entity.id));
        }
        if is_item { entity.properties.push(runtime_property(ITEM, PropertyType::Bool, false, json!(true))); }
        if let Some(owner) = owner {
            let columns = owner.values.get("columns").map_or(Some(4), Value::as_u64).filter(|v| (1..=20).contains(v)).ok_or_else(|| format!("inventory '{}' has invalid columns", entity.id))?;
            let rows = owner.values.get("rows").map_or(Some(4), Value::as_u64).filter(|v| (1..=20).contains(v)).ok_or_else(|| format!("inventory '{}' has invalid rows", entity.id))?;
            let items = owner.values.get("items").cloned().unwrap_or_else(|| json!([]));
            let values = items.as_array().ok_or_else(|| format!("inventory '{}' has invalid items", entity.id))?;
            if values.len() > (columns * rows) as usize { return Err(format!("inventory '{}' exceeds its capacity", entity.id)); }
            let mut unique = BTreeSet::new();
            for value in values {
                let id = value.as_str().filter(|id| !id.is_empty()).ok_or_else(|| format!("inventory '{}' has an invalid item reference", entity.id))?;
                if !unique.insert(id) { return Err(format!("inventory '{}' contains duplicate item '{}'", entity.id, id)); }
                if !item_ids.contains(id) { return Err(format!("inventory '{}' references '{}' without the Item trait", entity.id, id)); }
            }
            entity.properties.push(runtime_property(COLUMNS, PropertyType::Int, false, json!(columns)));
            entity.properties.push(runtime_property(ROWS, PropertyType::Int, false, json!(rows)));
            entity.properties.push(runtime_property(ITEMS, PropertyType::Entity, true, items));
        }
    }
    Ok(())
}

/// Check the actual graph inputs and traits again on the service boundary.
pub fn validate_inventory_graph(graph: &NarrativeGraph, entities: &[EntityTemplateRecord]) -> Result<(), String> {
    for node in &graph.nodes {
        if !matches!(node.data, NarrativeNodeData::InventoryAdd {} | NarrativeNodeData::InventoryRemove {} | NarrativeNodeData::InventoryHas {}) { continue; }
        for (port, required, label) in [("entity", ITEMS, "InventoryOwner"), ("item", ITEM, "Item")] {
            let edges: Vec<_> = graph.edges.iter().filter(|edge| edge.kind == NarrativeEdgeKind::Data && edge.to_node == node.id && edge.to_port == port).collect();
            let source = if edges.len() == 1 && edges[0].from_port == "output" { graph.nodes.iter().find(|source| source.id == edges[0].from_node) } else { None };
            let path = match source.map(|source| &source.data) { Some(NarrativeNodeData::GetEntity { entity_path }) => entity_path, _ => return Err(format!("node '{}' requires a Get Entity source for {port}", node.title)) };
            let entity = entities.iter().find(|entity| entity.id == *path).ok_or_else(|| format!("node '{}' references unresolved entity '{}'", node.title, path))?;
            if !entity.properties.iter().any(|property| property.key == required) { return Err(format!("node '{}': entity '{}' must have the {label} trait", node.title, entity.id)); }
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
    use std::collections::BTreeMap;
    use storylet_engine::chrono_runtime::{FlowGraphInstance, NodeKind, RuntimeContext};
    use storylet_engine::chrono_state::{AttributeValue, GraphLayer, WorldStateTable, WorldVertex};

    #[test]
    fn inventory_projection_validates_traits_and_capacity() {
        let templates: Vec<TraitTemplateRecord> = serde_json::from_value(json!([
            { "id": "owner-trait", "key": "inventory_owner", "name": "InventoryOwner", "properties": [] },
            { "id": "item-trait", "key": "item", "name": "Item", "properties": [] }
        ])).unwrap();
        let original: Vec<EntityTemplateRecord> = serde_json::from_value(json!([
            { "id": "bag", "name": "Bag", "traits": [{ "id": "bag-trait", "template_id": "owner-trait", "values": { "rows": 1, "columns": 1, "items": ["key"] } }] },
            { "id": "key", "name": "Key", "traits": [{ "id": "key-trait", "template_id": "item-trait", "values": {} }] }
        ])).unwrap();
        let mut projected = original.clone();
        project_inventory_entities(&mut projected, &[], &templates).unwrap();
        assert_eq!(projected[0].properties.iter().find(|p| p.key == ITEMS).unwrap().value, json!(["key"]));
        assert_eq!(projected[1].properties.iter().find(|p| p.key == ITEM).unwrap().value, json!(true));
        let graph: NarrativeGraph = serde_json::from_value(json!({
            "nodes": [
                { "id":"owner", "type":"get_entity", "title":"owner", "entity_path":"bag", "x":0,"y":0,"width":1,"height":1 },
                { "id":"item", "type":"get_entity", "title":"item", "entity_path":"key", "x":0,"y":0,"width":1,"height":1 },
                { "id":"add", "type":"inventory_add", "title":"add", "x":0,"y":0,"width":1,"height":1 }
            ],
            "edges": [
                { "id":"o", "kind":"data", "from_node":"owner", "from_port":"output", "to_node":"add", "to_port":"entity" },
                { "id":"i", "kind":"data", "from_node":"item", "from_port":"output", "to_node":"add", "to_port":"item" }
            ]
        })).unwrap();
        assert!(validate_inventory_graph(&graph, &projected).is_ok());
        let mut wrong_item = projected.clone();
        wrong_item[1].properties.clear();
        assert!(validate_inventory_graph(&graph, &wrong_item).unwrap_err().contains("Item trait"));
        let mut wrong_owner = projected.clone();
        wrong_owner[0].properties.clear();
        assert!(validate_inventory_graph(&graph, &wrong_owner).unwrap_err().contains("InventoryOwner trait"));
        assert!(original[0].properties.is_empty(), "projection must not mutate persisted records");
        let mut invalid = original.clone();
        invalid[0].traits[0].values.insert("items".into(), json!(["key", "key"]));
        assert!(project_inventory_entities(&mut invalid, &[], &templates).unwrap_err().contains("capacity"));
        let mut invalid = original;
        invalid[1].traits.clear();
        assert!(project_inventory_entities(&mut invalid, &[], &templates).unwrap_err().contains("Item trait"));
    }

    #[test]
    fn inventory_add_has_remove_and_has_execute_in_order() {
        let make_node = |id: &str, typ: &str| json!({ "id": id, "type": typ, "title": id, "x": 0, "y": 0, "width": 100, "height": 100 });
        let mut nodes = vec![json!({ "id":"start", "type":"start", "title":"start", "text":"Start", "x":0,"y":0,"width":100,"height":100 }),
            json!({ "id":"owner", "type":"get_entity", "title":"owner", "entity_path":"bag", "x":0,"y":0,"width":100,"height":100 }),
            json!({ "id":"item", "type":"get_entity", "title":"item", "entity_path":"key", "x":0,"y":0,"width":100,"height":100 }),
            make_node("add", "inventory_add"), make_node("has1", "inventory_has"), make_node("branch1", "condition"),
            make_node("remove", "inventory_remove"), make_node("has2", "inventory_has"), make_node("branch2", "condition")];
        for id in ["wrong1", "wrong2", "success"] { let mut end = make_node(id, "end"); end["is_terminal"] = json!(true); nodes.push(end); }
        let edge = |id: String, from: &str, output: &str, to: &str, input: &str, kind: &str| json!({
            "id":id, "from_node":from, "from_port":output, "to_node":to, "to_port":input, "kind":kind
        });
        let mut edges = vec![
            edge("f0".into(), "start", "output", "add", "input", "flow"),
            edge("f1".into(), "add", "output", "branch1", "input", "flow"),
            edge("f2".into(), "branch1", "true", "remove", "input", "flow"),
            edge("f3".into(), "branch1", "false", "wrong1", "input", "flow"),
            edge("f4".into(), "remove", "output", "branch2", "input", "flow"),
            edge("f5".into(), "branch2", "true", "wrong2", "input", "flow"),
            edge("f6".into(), "branch2", "false", "success", "input", "flow"),
            edge("b1".into(), "has1", "output", "branch1", "input", "data"),
            edge("b2".into(), "has2", "output", "branch2", "input", "data"),
        ];
        for target in ["add", "has1", "remove", "has2"] {
            edges.push(edge(format!("{target}-owner"), "owner", "output", target, "entity", "data"));
            edges.push(edge(format!("{target}-item"), "item", "output", target, "item", "data"));
        }
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id":"inventory-test", "title":"Inventory test", "content":{"text_template":"x"}, "created_at":1,
            "graph":{"nodes":nodes,"edges":edges}
        })).unwrap();
        storylet.graph.as_ref().unwrap().validate().unwrap();
        let mut dispatch = compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[]).unwrap();
        let ids = BTreeMap::from([("bag".to_owned(), 0), ("key".to_owned(), 1)]);
        let graphs = compile_narrative_graphs("project", std::slice::from_ref(&storylet), &dispatch, &ids).unwrap();
        bind_storylet_start_nodes(&mut dispatch, &graphs).unwrap();
        let mut state = WorldStateTable::new();
        state.vertices.push(WorldVertex { vertex_id: "bag".into(), layers: GraphLayer::Item.into(), entity_index: None, attributes: BTreeMap::from([
            ("prop.__inventory_items".into(), AttributeValue::Array(vec![])),
            ("prop.__inventory_rows".into(), AttributeValue::Int(1)),
            ("prop.__inventory_columns".into(), AttributeValue::Int(1)),
        ]) });
        state.vertices.push(WorldVertex { vertex_id: "key".into(), layers: GraphLayer::Item.into(), entity_index: None, attributes: BTreeMap::from([
            ("prop.__inventory_item".into(), AttributeValue::Bool(true)),
        ]) });
        let mut instance = FlowGraphInstance::new(1, graphs.asset, 7).unwrap();
        let mut context = RuntimeContext::with_registry(&mut state, &dispatch.dispatcher, &dispatch.registry);
        instance.start().unwrap();
        instance.tick(&mut context, 100).unwrap();
        assert_eq!(context.state.vertex("bag").unwrap().attributes.get("prop.__inventory_items"), Some(&AttributeValue::Array(vec![])));
        let history = instance.call_history();
        assert!(history.iter().any(|entry| instance.asset.node(entry.node_guid).is_some_and(|node| matches!(node.kind, NodeKind::InventoryRemove))));
        assert!(history.iter().any(|entry| instance.asset.node(entry.node_guid).is_some_and(|node| node.payload["nodeId"] == "success")));
        context.state.vertex_mut("bag").unwrap().attributes.insert("prop.__inventory_items".into(), AttributeValue::Array(vec![AttributeValue::EntityRef("other".into())]));
        let mut full = FlowGraphInstance::new(2, instance.asset.clone(), 7).unwrap();
        full.start().unwrap();
        assert!(full.tick(&mut context, 100).unwrap_err().to_string().contains("inventory is full"));
        assert_eq!(context.state.vertex("bag").unwrap().attributes.get("prop.__inventory_items"), Some(&AttributeValue::Array(vec![AttributeValue::EntityRef("other".into())])));
        context.state.vertex_mut("bag").unwrap().attributes.insert("prop.__inventory_items".into(), AttributeValue::Array(vec![AttributeValue::EntityRef("key".into())]));
        let mut duplicate = FlowGraphInstance::new(3, instance.asset.clone(), 7).unwrap();
        duplicate.start().unwrap();
        duplicate.tick(&mut context, 100).unwrap();
        assert_eq!(context.state.vertex("bag").unwrap().attributes.get("prop.__inventory_items"), Some(&AttributeValue::Array(vec![])));
    }
}
