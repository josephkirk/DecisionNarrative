use std::collections::{BTreeMap, BTreeSet};
use serde_json::Value;

pub const TREE_GRAPH_KEY: &str = "tree_graph";
pub const TREE_ITEM_KEY: &str = "tree_item";

const VALID_DIRECTIONS: &[&str] = &["right", "left", "down", "up", "radial"];
/// A save only affects tree integrity when a graph changes or an item marker is removed.
pub fn requires_validation(
    index: &crate::entities::WorkspaceIndex,
    before: &[crate::contracts::TraitInstance],
    after: &[crate::contracts::TraitInstance],
) -> bool {
    let graph = index.templates.iter().find(|template| template.key == TREE_GRAPH_KEY);
    let item = index.templates.iter().find(|template| template.key == TREE_ITEM_KEY);
    let graph_changed = graph.is_some_and(|template| {
        before.iter().find(|trait_| trait_.template_id == template.id)
            != after.iter().find(|trait_| trait_.template_id == template.id)
    });
    let item_removed = item.is_some_and(|template| {
        before.iter().any(|trait_| trait_.template_id == template.id)
            && !after.iter().any(|trait_| trait_.template_id == template.id)
    });
    graph_changed || item_removed
}


/// Validates all `tree_graph` trait instances in the workspace index.
///
/// `replacement` optionally substitutes an existing entity/template by ID or adds a new one.
/// `removed_id` optionally omits an entity/template by ID from the effective workspace.
pub fn validate_tree_graphs(
    index: &crate::entities::WorkspaceIndex,
    replacement: Option<(&str, &str, &[crate::contracts::TraitInstance])>,
    removed_id: Option<&str>,
) -> Vec<String> {
    let Some(tree_graph_template) = index.templates.iter().find(|t| t.key == TREE_GRAPH_KEY) else {
        return Vec::new();
    };
    let tree_graph_template_id = &tree_graph_template.id;

    let tree_item_template_id = index
        .templates
        .iter()
        .find(|t| t.key == TREE_ITEM_KEY)
        .map(|t| &t.id);

    // Build the effective set of all entities and tree items.
    // Map entity id -> (name, traits)
    let mut effective_entities: BTreeMap<&str, (&str, &[crate::contracts::TraitInstance])> = BTreeMap::new();

    for entity in &index.entities {
        if Some(entity.id.as_str()) == removed_id {
            continue;
        }
        effective_entities.insert(entity.id.as_str(), (entity.name.as_str(), entity.traits.as_slice()));
    }

    for template in &index.entity_templates {
        if Some(template.id.as_str()) == removed_id {
            continue;
        }
        effective_entities.insert(template.id.as_str(), (template.name.as_str(), template.traits.as_slice()));
    }

    if let Some((rep_id, rep_name, rep_traits)) = replacement {
        if Some(rep_id) != removed_id {
            effective_entities.insert(rep_id, (rep_name, rep_traits));
        }
    }

    // Determine which entities in the effective workspace possess the TreeItem trait.
    let mut tree_items: BTreeSet<&str> = BTreeSet::new();
    if let Some(item_tid) = tree_item_template_id {
        for (&id, (_, traits)) in &effective_entities {
            if traits.iter().any(|instance| instance.template_id == *item_tid) {
                tree_items.insert(id);
            }
        }
    }

    // Determine all owners with the TreeGraph trait to validate.
    // "Visit all existing owners except IDs replaced/removed, plus replacement."
    let mut owners: Vec<(&str, &str, &[crate::contracts::TraitInstance])> = Vec::new();

    let rep_id_opt = replacement.map(|r| r.0);

    for entity in &index.entities {
        if Some(entity.id.as_str()) == removed_id || rep_id_opt == Some(entity.id.as_str()) {
            continue;
        }
        if entity.traits.iter().any(|t| t.template_id == *tree_graph_template_id) {
            owners.push((entity.id.as_str(), entity.name.as_str(), entity.traits.as_slice()));
        }
    }

    for template in &index.entity_templates {
        if Some(template.id.as_str()) == removed_id || rep_id_opt == Some(template.id.as_str()) {
            continue;
        }
        if template.traits.iter().any(|t| t.template_id == *tree_graph_template_id) {
            owners.push((template.id.as_str(), template.name.as_str(), template.traits.as_slice()));
        }
    }

    if let Some((rep_id, rep_name, rep_traits)) = replacement {
        if Some(rep_id) != removed_id && rep_traits.iter().any(|t| t.template_id == *tree_graph_template_id) {
            owners.push((rep_id, rep_name, rep_traits));
        }
    }

    // Sort owners deterministically by ID to ensure stable error ordering
    owners.sort_by_key(|owner| owner.0);

    let mut issues = Vec::new();

    for (owner_id, owner_name, traits) in owners {
        let display_name = if owner_name.is_empty() { owner_id } else { owner_name };

        for instance in traits.iter().filter(|t| t.template_id == *tree_graph_template_id) {
            let mut valid_nodes: BTreeSet<&str> = BTreeSet::new();

            // 1. Validate `nodes`
            match instance.values.get("nodes") {
                None => {}
                Some(Value::Array(nodes_arr)) => {
                    let mut seen_nodes = BTreeSet::new();
                    for node_val in nodes_arr {
                        let Some(node_str) = node_val.as_str() else {
                            issues.push(format!(
                                "owner '{display_name}': tree_graph node ID must be a non-empty string, got {node_val}"
                            ));
                            continue;
                        };

                        if node_str.trim().is_empty() {
                            issues.push(format!(
                                "owner '{display_name}': tree_graph node ID cannot be empty"
                            ));
                            continue;
                        }

                        if !seen_nodes.insert(node_str) {
                            issues.push(format!(
                                "owner '{display_name}': tree_graph has duplicate node '{node_str}'"
                            ));
                            continue;
                        }

                        if !effective_entities.contains_key(node_str) {
                            issues.push(format!(
                                "owner '{display_name}': tree_graph node '{node_str}' references missing entity"
                            ));
                        } else if !tree_items.contains(node_str) {
                            issues.push(format!(
                                "owner '{display_name}': tree_graph node '{node_str}' does not have the TreeItem trait"
                            ));
                        } else {
                            valid_nodes.insert(node_str);
                        }
                    }
                }
                Some(other) => {
                    issues.push(format!(
                        "owner '{display_name}': tree_graph 'nodes' must be an array, got {other}"
                    ));
                }
            }

            // 2. Validate `edges`
            match instance.values.get("edges") {
                None => {}
                Some(Value::Array(edges_arr)) => {
                    let mut seen_edges: BTreeSet<(&str, &str)> = BTreeSet::new();
                    for edge_val in edges_arr {
                        let Some(edge_str) = edge_val.as_str() else {
                            issues.push(format!(
                                "owner '{display_name}': tree_graph edge must be a JSON string, got {edge_val}"
                            ));
                            continue;
                        };

                        let parsed_pair: Result<[String; 2], _> = serde_json::from_str(edge_str);
                        let edge_endpoints = match parsed_pair {
                            Ok([from, to]) => (from, to),
                            _ => {
                                issues.push(format!(
                                    "owner '{display_name}': tree_graph malformed edge '{edge_str}'"
                                ));
                                continue;
                            }
                        };

                        let from_str = edge_endpoints.0.as_str();
                        let to_str = edge_endpoints.1.as_str();

                        let from_valid = valid_nodes.contains(from_str);
                        let to_valid = valid_nodes.contains(to_str);

                        if !from_valid {
                            issues.push(format!(
                                "owner '{display_name}': tree_graph edge source '{from_str}' is not in nodes"
                            ));
                        }
                        if !to_valid {
                            issues.push(format!(
                                "owner '{display_name}': tree_graph edge target '{to_str}' is not in nodes"
                            ));
                        }

                        if from_valid && to_valid {
                            // Borrow strings from valid_nodes for zero-copy set lookup
                            let from_borrowed = *valid_nodes.get(from_str).unwrap();
                            let to_borrowed = *valid_nodes.get(to_str).unwrap();
                            if !seen_edges.insert((from_borrowed, to_borrowed)) {
                                issues.push(format!(
                                    "owner '{display_name}': tree_graph duplicate edge from '{from_str}' to '{to_str}'"
                                ));
                            }
                        }
                    }
                }
                Some(other) => {
                    issues.push(format!(
                        "owner '{display_name}': tree_graph 'edges' must be an array, got {other}"
                    ));
                }
            }

            // 3. Validate `direction`
            match instance.values.get("direction") {
                None => {}
                Some(Value::String(dir_str)) => {
                    if !VALID_DIRECTIONS.contains(&dir_str.as_str()) {
                        issues.push(format!(
                            "owner '{display_name}': tree_graph invalid direction '{dir_str}', expected one of right, left, down, up, radial"
                        ));
                    }
                }
                Some(other) => {
                    issues.push(format!(
                        "owner '{display_name}': tree_graph direction must be a string, got {other}"
                    ));
                }
            }

            // 4. Validate `positions`
            // Graph trait adds `positions` optional string, containing JSON object mapping direction -> node ID -> {x:number, y:number}
            match instance.values.get("positions") {
                None => {}
                Some(Value::String(pos_str)) => {
                    let trimmed = pos_str.trim();
                    if !trimmed.is_empty() {
                        match serde_json::from_str::<Value>(trimmed) {
                            Ok(Value::Object(map)) => {
                                validate_positions_map(&map, display_name, &valid_nodes, &mut issues);
                            }
                            Ok(_) => {
                                issues.push(format!(
                                    "owner '{display_name}': tree_graph positions must be a JSON object"
                                ));
                            }
                            Err(err) => {
                                issues.push(format!(
                                    "owner '{display_name}': tree_graph positions is malformed JSON: {err}"
                                ));
                            }
                        }
                    }
                }
                Some(other) => {
                    issues.push(format!(
                        "owner '{display_name}': tree_graph positions must be a JSON string, got {other}"
                    ));
                }
            }
        }
    }

    issues
}

fn validate_positions_map(
    map: &serde_json::Map<String, Value>,
    display_name: &str,
    valid_nodes: &BTreeSet<&str>,
    issues: &mut Vec<String>,
) {
    for (dir_key, dir_val) in map {
        if !VALID_DIRECTIONS.contains(&dir_key.as_str()) {
            issues.push(format!(
                "owner '{display_name}': tree_graph positions invalid direction key '{dir_key}', expected one of right, left, down, up, radial"
            ));
            continue;
        }

        let Some(nodes_obj) = dir_val.as_object() else {
            issues.push(format!(
                "owner '{display_name}': tree_graph positions for direction '{dir_key}' must be an object"
            ));
            continue;
        };

        for (node_id, pos_val) in nodes_obj {
            if !valid_nodes.contains(node_id.as_str()) {
                issues.push(format!(
                    "owner '{display_name}': tree_graph positions direction '{dir_key}' key '{node_id}' is not in nodes"
                ));
                continue;
            }

            let Some(pos_obj) = pos_val.as_object() else {
                issues.push(format!(
                    "owner '{display_name}': tree_graph position for '{node_id}' in direction '{dir_key}' must be an object"
                ));
                continue;
            };

            for key in pos_obj.keys() {
                if key != "x" && key != "y" {
                    issues.push(format!(
                        "owner '{display_name}': tree_graph position for '{node_id}' in direction '{dir_key}' contains unknown key '{key}'"
                    ));
                }
            }

            let x_valid = match pos_obj.get("x") {
                Some(Value::Number(n)) if n.as_f64().is_some_and(f64::is_finite) => true,
                _ => false,
            };
            if !x_valid {
                issues.push(format!(
                    "owner '{display_name}': tree_graph position for '{node_id}' in direction '{dir_key}' has invalid x coordinate, must be a finite number"
                ));
            }

            let y_valid = match pos_obj.get("y") {
                Some(Value::Number(n)) if n.as_f64().is_some_and(f64::is_finite) => true,
                _ => false,
            };
            if !y_valid {
                issues.push(format!(
                    "owner '{display_name}': tree_graph position for '{node_id}' in direction '{dir_key}' has invalid y coordinate, must be a finite number"
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{EntityRecord, EntityTemplateRecord, TraitInstance, TraitTemplateRecord};
    use crate::entities::WorkspaceIndex;
    use serde_json::json;

    fn make_test_index(
        templates: Vec<TraitTemplateRecord>,
        entities: Vec<EntityRecord>,
        entity_templates: Vec<EntityTemplateRecord>,
    ) -> WorkspaceIndex {
        WorkspaceIndex {
            templates,
            entities,
            entity_templates,
            gameplay_tags: Vec::new(),
            assets: Vec::new(),
            folders: Vec::new(),
            storylet_ids: BTreeSet::new(),
        }
    }

    fn standard_templates() -> Vec<TraitTemplateRecord> {
        serde_json::from_value(json!([
            {
                "id": "tid-tree-graph",
                "key": "tree_graph",
                "name": "TreeGraph",
                "properties": []
            },
            {
                "id": "tid-tree-item",
                "key": "tree_item",
                "name": "TreeItem",
                "properties": []
            }
        ])).unwrap()
    }

    #[test]
    fn test_missing_tree_graph_template_returns_empty() {
        let index = make_test_index(Vec::new(), Vec::new(), Vec::new());
        let issues = validate_tree_graphs(&index, None, None);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_valid_multi_root_cycle_shared_node_and_defaults() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "root-1",
                "name": "Root 1",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "root-2",
                "name": "Root 2",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "shared-child",
                "name": "Shared Child",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "graph-owner",
                "name": "Graph Owner",
                "traits": [{
                    "template_id": "tid-tree-graph",
                    "values": {
                        "nodes": ["root-1", "root-2", "shared-child"],
                        "edges": [
                            "[\"root-1\",\"shared-child\"]",
                            "[\"root-2\",\"shared-child\"]",
                            "[\"shared-child\",\"root-1\"]",
                            "[\"root-1\",\"root-1\"]"
                        ],
                        "direction": "radial",
                        "positions": "{\"radial\":{\"root-1\":{\"x\":10.5,\"y\":20.0},\"root-2\":{\"x\":0,\"y\":1000000}}}"
                    }
                }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());
        let issues = validate_tree_graphs(&index, None, None);
        assert!(issues.is_empty(), "Expected no issues, got: {issues:?}");
    }

    #[test]
    fn test_defaults_when_fields_absent() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "empty-graph-owner",
                "name": "Empty Graph Owner",
                "traits": [{
                    "template_id": "tid-tree-graph",
                    "values": {}
                }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());
        let issues = validate_tree_graphs(&index, None, None);
        assert!(issues.is_empty(), "Expected defaults to pass, got: {issues:?}");
    }

    #[test]
    fn test_non_item_or_missing_target() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "plain-entity",
                "name": "Plain Entity",
                "traits": []
            },
            {
                "id": "graph-owner",
                "name": "Graph Owner",
                "traits": [{
                    "template_id": "tid-tree-graph",
                    "values": {
                        "nodes": ["plain-entity", "non-existent-id"]
                    }
                }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());
        let issues = validate_tree_graphs(&index, None, None);
        assert_eq!(issues.len(), 2);
        assert!(issues[0].contains("Graph Owner") && issues[0].contains("plain-entity") && issues[0].contains("TreeItem"));
        assert!(issues[1].contains("Graph Owner") && issues[1].contains("non-existent-id") && issues[1].contains("missing"));
    }

    #[test]
    fn test_duplicate_nodes_and_edges() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "item-1",
                "name": "Item 1",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "item-2",
                "name": "Item 2",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "graph-owner",
                "name": "Graph Owner",
                "traits": [{
                    "template_id": "tid-tree-graph",
                    "values": {
                        "nodes": ["item-1", "item-2", "item-1"],
                        "edges": [
                            "[\"item-1\",\"item-2\"]",
                            "[\"item-1\",\"item-2\"]"
                        ]
                    }
                }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());
        let issues = validate_tree_graphs(&index, None, None);
        assert_eq!(issues.len(), 2);
        assert!(issues[0].contains("duplicate node 'item-1'"));
        assert!(issues[1].contains("duplicate edge from 'item-1' to 'item-2'"));
    }

    #[test]
    fn test_malformed_edge() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "item-1",
                "name": "Item 1",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "graph-owner",
                "name": "Graph Owner",
                "traits": [{
                    "template_id": "tid-tree-graph",
                    "values": {
                        "nodes": ["item-1"],
                        "edges": [
                            "not-json",
                            "[\"item-1\"]",
                            "[\"item-1\",\"item-2\"]"
                        ]
                    }
                }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());
        let issues = validate_tree_graphs(&index, None, None);
        assert_eq!(issues.len(), 3);
        assert!(issues[0].contains("malformed edge 'not-json'"));
        assert!(issues[1].contains("malformed edge '[\"item-1\"]'"));
        assert!(issues[2].contains("edge target 'item-2' is not in nodes"));
    }

    #[test]
    fn test_invalid_direction() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "graph-owner",
                "name": "Graph Owner",
                "traits": [{
                    "template_id": "tid-tree-graph",
                    "values": {
                        "direction": "diagonal"
                    }
                }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());
        let issues = validate_tree_graphs(&index, None, None);
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("invalid direction 'diagonal'"));
    }

    #[test]
    fn test_replacement_removes_tree_item_and_removed_id_target() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "item-1",
                "name": "Item 1",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "item-2",
                "name": "Item 2",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "graph-owner",
                "name": "Graph Owner",
                "traits": [{
                    "template_id": "tid-tree-graph",
                    "values": {
                        "nodes": ["item-1", "item-2"]
                    }
                }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());

        // Test 1: removed_id removes item-2
        let issues_removed = validate_tree_graphs(&index, None, Some("item-2"));
        assert_eq!(issues_removed.len(), 1);
        assert!(issues_removed[0].contains("node 'item-2' references missing entity"));

        // Test 2: replacement updates item-1 with no traits (removing TreeItem)
        let replacement_traits: [TraitInstance; 0] = [];
        let issues_replacement = validate_tree_graphs(
            &index,
            Some(("item-1", "Updated Item 1", &replacement_traits)),
            None,
        );
        assert_eq!(issues_replacement.len(), 1);
        assert!(issues_replacement[0].contains("node 'item-1' does not have the TreeItem trait"));
    }

    #[test]
    fn test_replacement_adds_new_tree_graph_owner() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "item-1",
                "name": "Item 1",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());

        let new_traits: Vec<TraitInstance> = serde_json::from_value(json!([
            {
                "template_id": "tid-tree-graph",
                "values": {
                    "nodes": ["item-1", "missing-item"]
                }
            }
        ])).unwrap();

        let issues = validate_tree_graphs(
            &index,
            Some(("new-graph-owner", "New Graph Owner", &new_traits)),
            None,
        );
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("New Graph Owner") && issues[0].contains("missing-item"));
    }

    #[test]
    fn test_positions_validation_malformed_and_unknown_coordinates() {
        let templates = standard_templates();
        let entities: Vec<EntityRecord> = serde_json::from_value(json!([
            {
                "id": "item-1",
                "name": "Item 1",
                "traits": [{ "template_id": "tid-tree-item", "values": {} }]
            },
            {
                "id": "graph-owner",
                "name": "Graph Owner",
                "traits": [{
                    "template_id": "tid-tree-graph",
                    "values": {
                        "nodes": ["item-1"],
                        "positions": "{\"right\":{\"item-1\":{\"x\":\"invalid\",\"y\":20,\"unknown_key\":123},\"missing-node\":{\"x\":10,\"y\":10}},\"diagonal\":{}}"
                    }
                }]
            }
        ])).unwrap();

        let index = make_test_index(templates, entities, Vec::new());
        let issues = validate_tree_graphs(&index, None, None);
        assert_eq!(issues.len(), 4);
        assert!(issues.iter().any(|i| i.contains("invalid direction key 'diagonal'")));
        assert!(issues.iter().any(|i| i.contains("positions direction 'right' key 'missing-node' is not in nodes")));
        assert!(issues.iter().any(|i| i.contains("contains unknown key 'unknown_key'")));
        assert!(issues.iter().any(|i| i.contains("invalid x coordinate")));
    }
}
