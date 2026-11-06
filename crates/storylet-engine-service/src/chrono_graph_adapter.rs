//! Stable compilation of authored narrative graphs into the v5 token runtime.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use storylet_engine::chrono_runtime::PredicateOperator;
use storylet_engine::chrono_runtime::{
    DataEdgeConnection, DataPinDefinition, EdgeConnection, FlowGraphAsset, FlowNodeDef,
    NodeCategory, NodeKind, PinDefinition, TimelineActionType, TimelineTrackEvent,
    DEFAULT_INPUT_PIN, DEFAULT_OUTPUT_PIN, DataValue, PropertyOperation, HolderRef,
};
use storylet_engine::chrono_dispatch::WaypointTopicGraph;
use storylet_engine::chrono_state::{
    AttributeAssignment as RuntimeAttributeAssignment, AttributeOp as RuntimeAttributeOp,
    AttributePredicate as RuntimeAttributePredicate, AttributeValue as RuntimeAttributeValue,
    EdgePattern as RuntimeEdgePattern, EdgeTemplate as RuntimeEdgeTemplate,
    GraphBinding as RuntimeGraphBinding, GraphLayer as RuntimeGraphLayer,
    GraphMutation as RuntimeGraphMutation, GraphPattern as RuntimeGraphPattern,
    GraphTransformRule as RuntimeGraphTransformRule, MutationOp,
    PassionMutation as RuntimePassionMutation, TagBitset, VertexPattern as RuntimeVertexPattern,
    VertexTemplate as RuntimeVertexTemplate, EntityTagMutation,
};

use crate::chrono_adapter::CompiledChronoDispatch;
use crate::contracts::{EntityTemplateRecord, PropertyType, StoryletRecord};
use crate::narrative_graph::{
    AttributeAssignment as AuthoredAttributeAssignment, AttributeOp as AuthoredAttributeOp,
    AttributePredicate as AuthoredAttributePredicate, AttributeValue as AuthoredAttributeValue,
    EdgePattern as AuthoredEdgePattern, EdgeTemplate as AuthoredEdgeTemplate,
    GraphBinding as AuthoredGraphBinding, GraphLayer as AuthoredGraphLayer,
    GraphTransformRule as AuthoredGraphTransformRule, NarrativeGraph, NarrativeNode,
    NarrativeNodeData,
    VertexTemplate as AuthoredVertexTemplate,
};

#[derive(Clone, Debug)]
pub struct CompiledNarrativeGraphs {
    pub asset: FlowGraphAsset,
    pub storylet_start_nodes: BTreeMap<String, u32>,
    pub mission_assets: BTreeMap<String, FlowGraphAsset>,
}

/// Compile every storylet graph into one immutable project topology. The start
/// node of each storylet remains a stable entry point for QBN and End jumps.
pub fn compile_narrative_graphs(
    project_id: &str,
    storylets: &[StoryletRecord],
    compiled: &CompiledChronoDispatch,
    entity_indices: &BTreeMap<String, u16>,
) -> Result<CompiledNarrativeGraphs, String> {
    if storylets.is_empty() {
        return Err("ChronoFlow cannot compile a project without storylets".into());
    }
    let mut ordered: Vec<&StoryletRecord> = storylets.iter().collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));

    let mut sources = Vec::with_capacity(ordered.len());
    let mut guid_owners = BTreeMap::<u32, String>::new();
    let mut storylet_start_nodes = BTreeMap::new();
    for storylet in &ordered {
        let graph: NarrativeGraph = match &storylet.graph {
            Some(value) => value.clone(),
            None => NarrativeGraph::from_storylet(storylet),
        };
        graph.validate().map_err(|issues| {
            format!(
                "storylet {} graph is invalid: {}",
                storylet.id,
                issues.join("; ")
            )
        })?;
        if let Some(precondition_graph) = &storylet.precondition_graph {
            precondition_graph.validate_precondition().map_err(|issues| {
                format!(
                    "storylet {} precondition graph is invalid: {}",
                    storylet.id,
                    issues.join("; ")
                )
            })?;
        }
        let mut node_guids = BTreeMap::new();
        for node in &graph.nodes {
            let guid = stable_node_guid(&storylet.id, &node.id);
            let identity = format!("{}/{}", storylet.id, node.id);
            if let Some(previous) = guid_owners.insert(guid, identity.clone()) {
                return Err(format!(
                    "stable node GUID collision between {previous} and {identity}"
                ));
            }
            node_guids.insert(node.id.clone(), guid);
        }
        let start = graph
            .nodes
            .iter()
            .find(|node| matches!(node.data, NarrativeNodeData::Start { .. }))
            .ok_or_else(|| format!("storylet {} has no Start node", storylet.id))?;
        storylet_start_nodes.insert(storylet.id.clone(), node_guids[&start.id]);
        sources.push((storylet, graph, node_guids));
    }

    let first_storylet = ordered[0];
    let start_node_guid = storylet_start_nodes[&first_storylet.id];
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut mission_assets = BTreeMap::new();
    let mut data_edges = Vec::new();
    for (storylet, graph, node_guids) in &sources {
        let storylet_index = compiled
            .storylet_index(&storylet.id)
            .ok_or_else(|| format!("storylet {} was not compiled into dispatch", storylet.id))?;
        let asset = compile_storylet_asset(
            storylet,
            graph,
            node_guids,
            storylet_index,
            compiled,
            entity_indices,
            &storylet_start_nodes,
        )?;
        nodes.extend(asset.nodes.iter().cloned());
        edges.extend(asset.edges.iter().cloned());
        data_edges.extend(asset.data_edges.iter().cloned());
        mission_assets.insert(asset.asset_id.clone(), asset);
    }
    for node in &nodes {
        if let NodeKind::MissionSubGraph { sub_graph_asset_id } = &node.kind
            && !mission_assets.contains_key(sub_graph_asset_id)
        {
            return Err(format!(
                "Mission node {} references unknown storylet graph asset {}",
                node.node_guid, sub_graph_asset_id
            ));
        }
    }
    nodes.sort_by_key(|node| node.node_guid);
    edges.sort_by(|a, b| {
        a.source_node_guid
            .cmp(&b.source_node_guid)
            .then_with(|| a.source_pin_name.cmp(&b.source_pin_name))
            .then_with(|| a.target_node_guid.cmp(&b.target_node_guid))
            .then_with(|| a.target_pin_name.cmp(&b.target_pin_name))
    });
    let asset = FlowGraphAsset {
        asset_id: format!("project:{project_id}"),
        start_node_guid,
        nodes,
        edges,
        data_edges,
    };
    asset
        .validate()
        .map_err(|error| format!("compiled ChronoFlow graph is invalid: {error}"))?;
    Ok(CompiledNarrativeGraphs {
        asset,
        storylet_start_nodes,
        mission_assets,
    })
}

fn compile_storylet_asset(
    storylet: &StoryletRecord,
    graph: &NarrativeGraph,
    node_guids: &BTreeMap<String, u32>,
    storylet_index: u16,
    compiled: &CompiledChronoDispatch,
    entity_indices: &BTreeMap<String, u16>,
    storylet_start_nodes: &BTreeMap<String, u32>,
) -> Result<FlowGraphAsset, String> {
    let mut nodes = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        let guid = node_guids[&node.id];
        let (kind, category) = compile_node_kind(
            node,
            storylet_index,
            compiled,
            entity_indices,
            storylet_start_nodes,
        )?;
        nodes.push(FlowNodeDef {
            node_guid: guid,
            node_type_id: node_type_name(&node.data).to_owned(),
            kind,
            category,
            entity_path: node_entity_path(&storylet.id, &node.data),
            input_pins: compile_input_pins(node, graph),
            output_pins: compile_output_pins(node),
            data_input_pins: compile_data_input_pins(node),
            data_output_pins: compile_data_output_pins(node),
            cost_ms: 0,
            mission_scope: None,
            payload: json!({
                "storyletId": storylet.id,
                "nodeId": node.id,
                "title": node.title,
                "x": node.x,
                "y": node.y,
                "nodeData": node.data,
            }),
        });
    }
    let mut edges = Vec::with_capacity(graph.edges.len());
    let mut data_edges = Vec::new();
    for edge in &graph.edges {
        let source = *node_guids.get(&edge.from_node).ok_or_else(|| {
            format!("edge {} has unknown source node {}", edge.id, edge.from_node)
        })?;
        let target = *node_guids.get(&edge.to_node).ok_or_else(|| {
            format!("edge {} has unknown target node {}", edge.id, edge.to_node)
        })?;
        let (source_pin, target_pin) = if edge.kind == crate::narrative_graph::NarrativeEdgeKind::Data {
            (edge.from_port.clone(), edge.to_port.clone())
        } else {
            (map_output_port(&edge.from_port), incoming_input_pin(graph, &edge.to_node, &edge.id))
        };
        if edge.kind == crate::narrative_graph::NarrativeEdgeKind::Data {
            data_edges.push(DataEdgeConnection { source_node_guid: source, source_pin_name: source_pin, target_node_guid: target, target_pin_name: target_pin });
        } else {
            edges.push(EdgeConnection { source_node_guid: source, source_pin_name: source_pin, target_node_guid: target, target_pin_name: target_pin });
        }
    }
    nodes.sort_by_key(|node| node.node_guid);
    edges.sort_by(|a, b| {
        a.source_node_guid.cmp(&b.source_node_guid)
            .then_with(|| a.source_pin_name.cmp(&b.source_pin_name))
            .then_with(|| a.target_node_guid.cmp(&b.target_node_guid))
            .then_with(|| a.target_pin_name.cmp(&b.target_pin_name))
    });
    data_edges.sort_by(|a, b| {
        a.source_node_guid.cmp(&b.source_node_guid)
            .then_with(|| a.source_pin_name.cmp(&b.source_pin_name))
            .then_with(|| a.target_node_guid.cmp(&b.target_node_guid))
            .then_with(|| a.target_pin_name.cmp(&b.target_pin_name))
    });
    let start_node_guid = storylet_start_nodes[&storylet.id];
    let asset = FlowGraphAsset {
        asset_id: storylet.id.clone(),
        start_node_guid,
        nodes,
        edges,
        data_edges,
    };
    asset.validate().map_err(|error| {
        format!(
            "compiled mission storylet {} is invalid: {error}",
            storylet.id
        )
    })?;
    Ok(asset)
}
/// Compile an authored pure precondition graph into the runtime asset used by
/// candidate evaluation. It intentionally reuses the normal node/edge compiler
/// so graph errors stay explicit and precondition graphs remain the sole eligibility contract.
pub fn compile_precondition_graph(
    storylet: &StoryletRecord,
    graph: &NarrativeGraph,
    compiled: &CompiledChronoDispatch,
    entity_indices: &BTreeMap<String, u16>,
) -> Result<FlowGraphAsset, String> {
    graph.validate_precondition().map_err(|issues| {
        format!(
            "storylet {} precondition graph is invalid: {}",
            storylet.id,
            issues.join("; ")
        )
    })?;
    let mut node_guids = BTreeMap::new();
    for node in &graph.nodes {
        node_guids.insert(node.id.clone(), stable_node_guid(&storylet.id, &node.id));
    }
    let start = graph
        .nodes
        .iter()
        .find(|node| matches!(node.data, NarrativeNodeData::Start { .. }))
        .ok_or_else(|| format!("storylet {} precondition graph has no Start node", storylet.id))?;
    let mut starts = BTreeMap::new();
    starts.insert(storylet.id.clone(), node_guids[&start.id]);
    compile_storylet_asset(
        storylet,
        graph,
        &node_guids,
        compiled.storylet_index(&storylet.id)
            .ok_or_else(|| format!("storylet {} was not compiled into dispatch", storylet.id))?,
        compiled,
        entity_indices,
        &starts,
    )
    .map_err(|error| format!("storylet {} precondition graph compile failed: {error}", storylet.id))
}
/// Attach compiled authored precondition assets after world/entity indices exist.
pub fn bind_precondition_graphs(
    compiled: &mut CompiledChronoDispatch,
    storylets: &[StoryletRecord],
    entity_indices: &BTreeMap<String, u16>,
) -> Result<(), String> {
    for storylet in storylets {
        let Some(graph) = &storylet.precondition_graph else {
            continue;
        };
        let asset = compile_precondition_graph(storylet, graph, compiled, entity_indices)?;
        let index = compiled
            .storylet_index(&storylet.id)
            .ok_or_else(|| format!("storylet {} was not compiled into dispatch", storylet.id))?;
        compiled.dispatcher.storylets[index as usize].precondition_graph = Some(asset);
    }
    Ok(())
}


/// Binds QBN/Teleport dispatch results to the stable Start GUIDs compiled above.
pub fn bind_storylet_start_nodes(
    compiled: &mut CompiledChronoDispatch,
    graphs: &CompiledNarrativeGraphs,
) -> Result<(), String> {
    let keys: Vec<Option<String>> = compiled
        .dispatcher
        .storylets
        .iter()
        .map(|storylet| compiled.storylet_key(storylet.storylet_id).map(str::to_owned))
        .collect();
    for (storylet, key) in compiled.dispatcher.storylets.iter_mut().zip(keys) {
        let id = key.ok_or_else(|| {
            format!("storylet index {} has no source id", storylet.storylet_id)
        })?;
        storylet.payload_node_guid = *graphs
            .storylet_start_nodes
            .get(&id)
            .ok_or_else(|| format!("storylet {id} has no compiled graph entry"))?;
    }
    Ok(())
}
pub fn resolve_entity_tag(
    _target: &str,
    tag: &str,
    _entity_game_ids: Option<&BTreeMap<String, String>>,
) -> String {
    // Gameplay tags are catalog-canonical names. Entity identity selects the
    // graph target; it must never become part of the tag namespace.
    tag.trim().to_owned()
}


fn convert_typed_value(value_type: &str, value: &Value) -> Result<DataValue, String> {
    match value_type.trim().to_ascii_lowercase().as_str() {
        "bool" | "boolean" => value.as_bool().map(DataValue::Bool),
        "int" | "integer" => value.as_i64().map(DataValue::Int),
        "float" | "number" => value.as_f64().map(DataValue::Float),
        "string" => value.as_str().map(|v| DataValue::String(v.to_owned())),
        "asset" => value.as_str().map(|v| DataValue::Asset(v.to_owned())),
        "entity" | "entity_ref" => value.as_str().map(|v| DataValue::EntityRef(v.to_owned())),
        other => return Err(format!("unsupported typed value type {other}")),
    }
    .ok_or_else(|| format!("typed literal does not match value type {value_type}"))
}

fn compile_node_kind(
    node: &NarrativeNode,
    _storylet_index: u16,
    compiled: &CompiledChronoDispatch,
    entity_indices: &BTreeMap<String, u16>,
    storylet_start_nodes: &BTreeMap<String, u32>,
) -> Result<(NodeKind, NodeCategory), String> {
    let mapped = match &node.data {
        NarrativeNodeData::GetEntity { entity_path } => {
            if entity_path.trim().is_empty() {
                return Err("Get Entity node must select an entity.".into());
            }
            let entity_id = resolve_entity_path(entity_path, entity_indices)?;
            (NodeKind::GetEntity { entity_id }, NodeCategory::LogicGate)
        }
        NarrativeNodeData::GetEntityProperty { property_key, value_type, .. } => (NodeKind::GetEntityProperty { property_key: property_key.clone(), value_type: value_type.clone() }, NodeCategory::LogicGate),
        NarrativeNodeData::ApplyCommand { property_key, operation, value_type, value } => (
            NodeKind::ApplyCommand {
                property_key: property_key.clone(),
                operation: match operation.as_str() {
                    "add" => PropertyOperation::Add,
                    "subtract" => PropertyOperation::Subtract,
                    "toggle" => PropertyOperation::Toggle,
                    _ => PropertyOperation::Set,
                },
                value: convert_typed_value(value_type, value)?,
            },
            NodeCategory::Mutator,
        ),
        NarrativeNodeData::And {} => (NodeKind::And, NodeCategory::LogicGate),
        NarrativeNodeData::Or {} => (NodeKind::Or, NodeCategory::LogicGate),
        NarrativeNodeData::Condition {} => (NodeKind::Condition { operator: PredicateOperator::Eq, literal: None }, NodeCategory::LogicGate),
        NarrativeNodeData::QbnStoryletHub { deck_storylet_ids, hub_exit_storylet_id } => (
            NodeKind::QbnHub {
                deck: deck_storylet_ids.iter().map(|id| storylet_index_for(compiled, id)).collect::<Result<Vec<_>, _>>()?,
                exit_storylet_id: if hub_exit_storylet_id.trim().is_empty() { None } else { Some(storylet_index_for(compiled, hub_exit_storylet_id)?) },
            },
            NodeCategory::LogicGate,
        ),
        NarrativeNodeData::SalienceSelector { candidate_pool_ids, fallback_storylet_id } => (
            NodeKind::SalienceSelector {
                pool: candidate_pool_ids.iter().map(|id| storylet_index_for(compiled, id)).collect::<Result<Vec<_>, _>>()?,
                fallback_storylet_id: if fallback_storylet_id.trim().is_empty() { None } else { Some(storylet_index_for(compiled, fallback_storylet_id)?) },
                fallback_output_pin: DEFAULT_OUTPUT_PIN.into(),
            },
            NodeCategory::LogicGate,
        ),
        NarrativeNodeData::WaypointConversation { topic_nodes, topic_edges, initial_topic_id, npc_goal_topic_id } => {
            let graph: WaypointTopicGraph = serde_json::from_value(json!({"nodes": topic_nodes, "edges": topic_edges}))
                .map_err(|error| format!("invalid Waypoint Conversation topic graph: {error}"))?;
            (NodeKind::WaypointConversation { graph, initial_topic_id: initial_topic_id.clone(), npc_goal_topic_id: npc_goal_topic_id.clone() }, NodeCategory::ScopedContainer)
        }
        NarrativeNodeData::HasEntityTag { target_entity_id, tag, match_mode } => (NodeKind::HasEntityTag { entity_id: resolve_entity_path(target_entity_id, entity_indices)?, tag: resolve_entity_tag(target_entity_id, tag, Some(&compiled.entity_game_ids)), match_mode: match_mode.clone() }, NodeCategory::LogicGate),
        NarrativeNodeData::Start { .. } | NarrativeNodeData::Dialog { .. } | NarrativeNodeData::Image { .. } => (NodeKind::LogicAny { default_output_pin:String::new() }, NodeCategory::LogicGate),
        NarrativeNodeData::Choice { .. } => (NodeKind::Choice { default_output_pin:String::new() }, NodeCategory::LogicGate),
        NarrativeNodeData::Waiter { wait_event } => (NodeKind::Waiter { duration_ms:i64::MAX, required_tags:TagBitset::new(), wait_event:wait_event.clone() }, NodeCategory::Waiter),
        NarrativeNodeData::Compare { operator, value_type, value } => (NodeKind::Compare { operator: predicate_operator(operator)?, literal: convert_typed_value(value_type, value)? }, NodeCategory::LogicGate),
        NarrativeNodeData::End { jump_to_storylet, precondition_result, .. } => (NodeKind::End { jump_to_storylet:jump_to_storylet.as_deref().map(|id|storylet_index_for(compiled,id)).transpose()?, precondition_result:precondition_result.unwrap_or(true) }, NodeCategory::LogicGate),
        NarrativeNodeData::Teleport { destination_storylet } => (NodeKind::Teleport { destination_storylet:Some(storylet_index_for(compiled,destination_storylet)?), destination_node_guid:storylet_start_nodes.get(destination_storylet).copied() }, NodeCategory::Mutator),
        NarrativeNodeData::Learn { fact_tag, kind, visibility } => (NodeKind::Learn { fact: compiled.registry.id(fact_tag).ok_or_else(|| format!("unknown fact {fact_tag}"))?, kind: *kind, visibility: *visibility }, NodeCategory::Mutator),
        NarrativeNodeData::Knows { holder, fact_tag, confirmed_only } => (NodeKind::Knows { holder: if holder == "acting" { HolderRef::Acting } else { HolderRef::Participant(*compiled.participant_indices.get(holder).ok_or_else(|| format!("unknown participant {holder}"))?) }, fact: compiled.registry.id(fact_tag).ok_or_else(|| format!("unknown fact {fact_tag}"))?, confirmed_only: *confirmed_only }, NodeCategory::LogicGate),
        NarrativeNodeData::Reveal { event_id, kind, visibility } => {
            let entry = compiled.chronicle.index_of(event_id).ok_or_else(|| format!("Reveal node: event \"{event_id}\" is not a canon event with a story time"))?;
            let facts = compiled.chronicle.entries[entry as usize].facts.clone();
            (NodeKind::Reveal { entry, kind: *kind, visibility: *visibility, facts }, NodeCategory::Mutator)
        }
        NarrativeNodeData::GoToLocation { location_id } => {
            let index = *entity_indices.get(location_id).ok_or_else(|| format!("unknown location {location_id}"))?;
            (NodeKind::MoveParticipant { location_entity: index }, NodeCategory::Mutator)
        }
        NarrativeNodeData::GraphTransform { rule } => (NodeKind::GraphTransform { rule: convert_graph_transform_rule(rule) }, NodeCategory::Mutator),
        NarrativeNodeData::PassionMutation { op } => (NodeKind::PassionMutation { op: RuntimePassionMutation { micro_deltas: op.micro_deltas.clone(), active_ops: op.active_ops.clone() } }, NodeCategory::Mutator),
        NarrativeNodeData::AddEntityTag { target_entity_id, tag } => {
            let resolved = resolve_entity_tag(target_entity_id, tag, Some(&compiled.entity_game_ids));
            let id = compiled.registry.id(&resolved).ok_or_else(|| format!("tag '{resolved}' is not registered"))?;
            let mut mask = TagBitset::new(); if let Some(m)=compiled.registry.ancestors_mask(id){mask=*m;} else {let _=mask.set_bit(id);}
            let mut op = MutationOp::default();
            if target_entity_id.trim().is_empty() || target_entity_id.eq_ignore_ascii_case("world") { op.add_global_tags.merge(&mask); }
            else { op.entity_tag_ops.push(EntityTagMutation { entity_id: resolve_entity_index(target_entity_id, entity_indices)?, add_tags: mask, remove_tags: TagBitset::new() }); }
            (NodeKind::TagMutation { op, adding: true }, NodeCategory::Mutator)
        }
        NarrativeNodeData::RemoveEntityTag { target_entity_id, tag } => {
            let resolved = resolve_entity_tag(target_entity_id, tag, Some(&compiled.entity_game_ids));
            let id = compiled.registry.id(&resolved).ok_or_else(|| format!("tag '{resolved}' is not registered"))?;
            let mut mask = TagBitset::new(); if let Some(m)=compiled.registry.descendants_mask(id){mask=*m;} else {let _=mask.set_bit(id);}
            let mut op = MutationOp::default();
            if target_entity_id.trim().is_empty() || target_entity_id.eq_ignore_ascii_case("world") { op.remove_global_tags.merge(&mask); }
            else { op.entity_tag_ops.push(EntityTagMutation { entity_id: resolve_entity_index(target_entity_id, entity_indices)?, add_tags: TagBitset::new(), remove_tags: mask }); }
            (NodeKind::TagMutation { op, adding: false }, NodeCategory::Mutator)
        }
        NarrativeNodeData::MissionSubGraph { sub_graph_asset_id } => (NodeKind::MissionSubGraph { sub_graph_asset_id: sub_graph_asset_id.clone() }, NodeCategory::ScopedContainer),
        NarrativeNodeData::SceneSection { duration_ms, events } => (NodeKind::SceneSection { section_duration_ms:i64::try_from(*duration_ms).map_err(|_|"duration out of range".to_string())?, events:events.iter().enumerate().map(|(i,e)|convert_timeline_event(e,i)).collect::<Result<Vec<_>,_>>()? }, NodeCategory::ScopedContainer),
        NarrativeNodeData::Poi { entity_path } => (NodeKind::Poi { poi_id:entity_path.clone() }, NodeCategory::LogicGate),
    };
    Ok(mapped)
}
/// Validate authored property nodes against the entity selected by their data input.
pub fn validate_entity_property_graph(graph: &NarrativeGraph, entities: &[EntityTemplateRecord]) -> Result<(), String> {
    let node_by_id: BTreeMap<&str, &NarrativeNode> =
        graph.nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    for node in &graph.nodes {
        let (key, value_type, operation, command_value) = match &node.data {
            NarrativeNodeData::GetEntityProperty { property_key, value_type } =>
                (property_key.as_str(), value_type.as_str(), None, None),
            NarrativeNodeData::ApplyCommand { property_key, operation, value_type, value } =>
                (property_key.as_str(), value_type.as_str(), Some(operation.as_str()), Some(value)),
            _ => continue,
        };
        let source = graph.edges.iter()
            .find(|edge| edge.kind == crate::narrative_graph::NarrativeEdgeKind::Data
                && edge.to_node == node.id && edge.to_port == "entity")
            .and_then(|edge| node_by_id.get(edge.from_node.as_str()).copied())
            .ok_or_else(|| format!("node {} has no resolvable entity data source", node.id))?;
        let candidates: Vec<&EntityTemplateRecord> = match &source.data {
            NarrativeNodeData::GetEntity { entity_path } => {
                let target = entity_path.trim();
                let found = entities.iter().find(|entity| entity.id == target);
                vec![found.ok_or_else(|| format!("node {} references unresolved entity '{}'", node.id, entity_path))?]
            }
            NarrativeNodeData::GetEntityProperty { value_type: source_type, .. }
                if matches!(source_type.to_ascii_lowercase().as_str(), "entity" | "entity_ref") =>
                entities.iter().collect(),
            _ => return Err(format!("node {} has invalid entity data source", node.id)),
        };
        if candidates.is_empty() {
            return Err(format!("node {} has no project entity schemas to validate against", node.id));
        }
        let expected = match value_type.to_ascii_lowercase().as_str() {
            "bool" | "boolean" => PropertyType::Bool, "string" => PropertyType::String,
            "int" | "integer" => PropertyType::Int, "float" | "number" => PropertyType::Float,
            "asset" => PropertyType::Asset, "entity" | "entity_ref" => PropertyType::Entity,
            other => return Err(format!("node {} uses unsupported property type '{}'", node.id, other)),
        };
        if let Some(value) = command_value {
            convert_typed_value(value_type, value)
                .map_err(|error| format!("node {} has invalid ApplyCommand value: {}", node.id, error))?;
        }
        for entity in candidates {
            let property = entity.properties.iter().find(|property| property.key == key)
                .ok_or_else(|| format!("node {} references undeclared entity property '{}' on '{}'", node.id, key, entity.id))?;
            if property.is_array {
                return Err(format!("node {} references array property '{}' unsupported by scalar graph nodes", node.id, key));
            }
            if property.property_type != expected {
                return Err(format!("node {} property '{}' type mismatch on '{}'", node.id, key, entity.id));
            }
            if let Some(operation) = operation {
                let valid = match operation.to_ascii_lowercase().as_str() {
                    "set" => true,
                    "toggle" => expected == PropertyType::Bool,
                    "add" | "subtract" => matches!(expected, PropertyType::Int | PropertyType::Float),
                    _ => false,
                };
                if !valid {
                    return Err(format!("node {} operation '{}' is invalid for property '{}'", node.id, operation, key));
                }
            }
        }
    }
    Ok(())
}
fn resolve_entity_index(target: &str, indices: &BTreeMap<String, u16>) -> Result<u16, String> {
    resolve_entity_path(target, indices)
        .and_then(|path| indices.get(&path).copied().ok_or_else(|| format!("unknown entity target {target}")))
}
fn resolve_entity_path(target: &str, indices: &BTreeMap<String, u16>) -> Result<String, String> {
    let target = target.trim();
    if target.is_empty() || target.eq_ignore_ascii_case("world") {
        return Ok("world".into());
    }
    if indices.contains_key(target) {
        return Ok(target.to_owned());
    }
    Err(format!("unknown entity target {target}"))
}
fn predicate_operator(op:&str)->Result<PredicateOperator,String>{match op.trim().to_ascii_lowercase().as_str(){"eq"|"=="=>Ok(PredicateOperator::Eq),"neq"|"!="=>Ok(PredicateOperator::Neq),"gt"|">"=>Ok(PredicateOperator::Gt),"gte"|">="=>Ok(PredicateOperator::Gte),"lt"|"<"=>Ok(PredicateOperator::Lt),"lte"|"<="=>Ok(PredicateOperator::Lte),other=>Err(format!("unknown condition operator {other}"))}}
fn convert_graph_transform_rule(rule: &AuthoredGraphTransformRule) -> RuntimeGraphTransformRule {
    RuntimeGraphTransformRule {
        lhs: RuntimeGraphPattern {
            vertices: rule
                .lhs
                .vertices
                .iter()
                .map(|vertex| RuntimeVertexPattern {
                    layer: convert_graph_layer(vertex.layer),
                    vertex_id: vertex.vertex_id.clone(),
                    attributes: vertex
                        .attributes
                        .iter()
                        .map(convert_attribute_predicate)
                        .collect(),
                })
                .collect(),
            edges: rule.lhs.edges.iter().map(convert_edge_pattern).collect(),
        },
        rhs: RuntimeGraphMutation {
            add_vertices: rule
                .rhs
                .add_vertices
                .iter()
                .map(convert_vertex_template)
                .collect(),
            remove_vertices: rule
                .rhs
                .remove_vertices
                .iter()
                .map(convert_graph_binding)
                .collect(),
            add_edges: rule
                .rhs
                .add_edges
                .iter()
                .map(convert_edge_template)
                .collect(),
            remove_edges: rule
                .rhs
                .remove_edges
                .iter()
                .map(convert_graph_binding)
                .collect(),
            set_attributes: rule
                .rhs
                .set_attributes
                .iter()
                .map(convert_attribute_assignment)
                .collect(),
        },
    }
}

fn convert_graph_layer(layer: AuthoredGraphLayer) -> RuntimeGraphLayer {
    match layer {
        AuthoredGraphLayer::Location => RuntimeGraphLayer::Location,
        AuthoredGraphLayer::Character => RuntimeGraphLayer::Character,
        AuthoredGraphLayer::Item => RuntimeGraphLayer::Item,
        AuthoredGraphLayer::Fact => RuntimeGraphLayer::Fact,
    }
}

fn convert_attribute_value(value: &AuthoredAttributeValue) -> RuntimeAttributeValue {
    match value {
        AuthoredAttributeValue::Int { value } => RuntimeAttributeValue::Int(*value),
        AuthoredAttributeValue::Float { value } => RuntimeAttributeValue::Float(*value),
        AuthoredAttributeValue::Bool { value } => RuntimeAttributeValue::Bool(*value),
        AuthoredAttributeValue::String { value } => RuntimeAttributeValue::String(value.clone()),
        AuthoredAttributeValue::Tag { value } => RuntimeAttributeValue::Tag(*value),
    }
}

fn convert_attribute_op(op: AuthoredAttributeOp) -> RuntimeAttributeOp {
    match op {
        AuthoredAttributeOp::Eq => RuntimeAttributeOp::Eq,
        AuthoredAttributeOp::Neq => RuntimeAttributeOp::Neq,
        AuthoredAttributeOp::Gt => RuntimeAttributeOp::Gt,
        AuthoredAttributeOp::Gte => RuntimeAttributeOp::Gte,
        AuthoredAttributeOp::Lt => RuntimeAttributeOp::Lt,
        AuthoredAttributeOp::Lte => RuntimeAttributeOp::Lte,
    }
}

fn convert_graph_binding(binding: &AuthoredGraphBinding) -> RuntimeGraphBinding {
    match binding {
        AuthoredGraphBinding::Literal { id } => RuntimeGraphBinding::Literal(id.clone()),
        AuthoredGraphBinding::VertexPattern { index } => RuntimeGraphBinding::VertexPattern(*index),
    }
}

fn convert_attribute_predicate(
    predicate: &AuthoredAttributePredicate,
) -> RuntimeAttributePredicate {
    RuntimeAttributePredicate {
        key: predicate.key.clone(),
        op: convert_attribute_op(predicate.op),
        value: convert_attribute_value(&predicate.value),
    }
}

fn convert_edge_pattern(pattern: &AuthoredEdgePattern) -> RuntimeEdgePattern {
    RuntimeEdgePattern {
        edge_id: pattern.edge_id.clone(),
        relation_type: pattern.relation_type.clone(),
        source: pattern.source.as_ref().map(convert_graph_binding),
        target: pattern.target.as_ref().map(convert_graph_binding),
    }
}

fn convert_vertex_template(template: &AuthoredVertexTemplate) -> RuntimeVertexTemplate {
    RuntimeVertexTemplate {
        id: convert_graph_binding(&template.id),
        layer: convert_graph_layer(template.layer),
        attributes: template
            .attributes
            .iter()
            .map(|(key, value)| (key.clone(), convert_attribute_value(value)))
            .collect(),
    }
}

fn convert_edge_template(template: &AuthoredEdgeTemplate) -> RuntimeEdgeTemplate {
    RuntimeEdgeTemplate {
        id: convert_graph_binding(&template.id),
        source: convert_graph_binding(&template.source),
        target: convert_graph_binding(&template.target),
        relation_type: template.relation_type.clone(),
        weight: template.weight,
    }
}

fn convert_attribute_assignment(
    assignment: &AuthoredAttributeAssignment,
) -> RuntimeAttributeAssignment {
    RuntimeAttributeAssignment {
        vertex: convert_graph_binding(&assignment.vertex),
        key: assignment.key.clone(),
        value: convert_attribute_value(&assignment.value),
    }
}

fn compile_data_input_pins(node: &NarrativeNode) -> Vec<DataPinDefinition> {
    match &node.data {
        NarrativeNodeData::GetEntityProperty { .. } => vec![DataPinDefinition { pin_name: "entity".into(), value_type: "entity_ref".into(), is_input: true }],
        NarrativeNodeData::ApplyCommand { .. } => vec![
            DataPinDefinition { pin_name: "entity".into(), value_type: "entity_ref".into(), is_input: true },
        ],
        NarrativeNodeData::AddEntityTag { .. } | NarrativeNodeData::RemoveEntityTag { .. } | NarrativeNodeData::HasEntityTag { .. } =>
            vec![DataPinDefinition { pin_name: "target".into(), value_type: "entity_ref".into(), is_input: true }],
        NarrativeNodeData::Condition {} | NarrativeNodeData::And {} | NarrativeNodeData::Or {} =>
            vec![DataPinDefinition { pin_name: "input".into(), value_type: "bool".into(), is_input: true }],
        NarrativeNodeData::Compare { .. } => vec![DataPinDefinition { pin_name: "input".into(), value_type: "any".into(), is_input: true }],
        _ => Vec::new(),
    }
}

fn compile_data_output_pins(node: &NarrativeNode) -> Vec<DataPinDefinition> {
    match &node.data {
        NarrativeNodeData::GetEntity { .. } => vec![DataPinDefinition { pin_name: "output".into(), value_type: "entity_ref".into(), is_input: false }],
        NarrativeNodeData::GetEntityProperty { value_type, .. } => vec![DataPinDefinition {
            pin_name: "output".into(),
            value_type: if value_type == "entity" { "entity_ref".into() } else { value_type.clone() },
            is_input: false,
        }],
        NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. } =>
            vec![DataPinDefinition { pin_name: "output".into(), value_type: "bool".into(), is_input: false }],
        NarrativeNodeData::And {} | NarrativeNodeData::Or {} | NarrativeNodeData::Compare { .. } =>
            vec![DataPinDefinition { pin_name: "output".into(), value_type: "bool".into(), is_input: false }],
        _ => Vec::new(),
    }
}

fn compile_input_pins(node: &NarrativeNode, graph: &NarrativeGraph) -> Vec<PinDefinition> {
    if matches!(node.data, NarrativeNodeData::GetEntity { .. } | NarrativeNodeData::GetEntityProperty { .. } | NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. } | NarrativeNodeData::And {} | NarrativeNodeData::Or {} | NarrativeNodeData::Compare { .. }) {
        return Vec::new();
    }
    let mut edges: Vec<&crate::narrative_graph::NarrativeEdge> = graph.edges.iter()
        .filter(|edge| edge.to_node == node.id && edge.kind == crate::narrative_graph::NarrativeEdgeKind::Flow)
        .collect();
    edges.sort_by(|a,b| a.id.cmp(&b.id));
    if edges.len() > 1 {
        return edges.iter().enumerate().map(|(index,_)| PinDefinition { pin_name: format!("In_{index}"), is_input: true }).collect();
    }
    vec![PinDefinition { pin_name: DEFAULT_INPUT_PIN.into(), is_input: true }]
}

fn compile_output_pins(node: &NarrativeNode) -> Vec<PinDefinition> {
    let names: Vec<String> = match &node.data {
        NarrativeNodeData::Choice { choices } => (0..choices.len())
            .map(|index| format!("choice_{index}"))
            .collect(),
        NarrativeNodeData::Condition {} => vec!["true".into(), "false".into()],
        NarrativeNodeData::GetEntity { .. } | NarrativeNodeData::GetEntityProperty { .. } | NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. } | NarrativeNodeData::And {} | NarrativeNodeData::Or {} | NarrativeNodeData::Compare { .. } => Vec::new(),
        NarrativeNodeData::WaypointConversation { topic_nodes, .. } => {
            let mut pins = BTreeSet::new();
            for topic in topic_nodes {
                if let Some(pin) = topic
                    .get("exit_output_pin")
                    .and_then(Value::as_str)
                    .filter(|pin| !pin.is_empty())
                {
                    pins.insert(pin.to_owned());
                }
            }
            if pins.is_empty() {
                vec![DEFAULT_OUTPUT_PIN.into()]
            } else {
                pins.into_iter().collect()
            }
        }
        _ => vec![DEFAULT_OUTPUT_PIN.into()],
    };
    names
        .into_iter()
        .map(|pin_name| PinDefinition {
            pin_name,
            is_input: false,
        })
        .collect()
}

fn incoming_input_pin(graph: &NarrativeGraph, target_node_id: &str, edge_id: &str) -> String {
    let mut edges: Vec<&crate::narrative_graph::NarrativeEdge> = graph
        .edges
        .iter()
        .filter(|edge| edge.to_node == target_node_id && edge.kind == crate::narrative_graph::NarrativeEdgeKind::Flow)
        .collect();
    if edges.len() <= 1 {
        return DEFAULT_INPUT_PIN.to_owned();
    }
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    let index = edges
        .iter()
        .position(|edge| edge.id == edge_id)
        .unwrap_or(0);
    format!("In_{index}")
}

fn map_output_port(port: &str) -> String {
    if port == "output" {
        DEFAULT_OUTPUT_PIN.to_owned()
    } else {
        port.to_owned()
    }
}

fn storylet_index_for(compiled: &CompiledChronoDispatch, id: &str) -> Result<u16, String> {
    compiled
        .storylet_index(id)
        .ok_or_else(|| format!("graph references unknown storylet {id}"))
}


fn convert_timeline_event(event: &Value, index: usize) -> Result<TimelineTrackEvent, String> {
    let get = |snake: &str, camel: &str| event.get(snake).or_else(|| event.get(camel));
    let action = get("action_type", "actionType")
        .and_then(Value::as_str)
        .unwrap_or("workspot_pose");
    let action_type = match action.trim().to_ascii_lowercase().as_str() {
        "workspot_pose" | "workspotpose" => TimelineActionType::WorkspotPose,
        "transition_anim" | "transitionanim" => TimelineActionType::TransitionAnim,
        "dialogue_vo" | "dialoguevo" => TimelineActionType::DialogueVo,
        "camera_cut" | "cameracut" => TimelineActionType::CameraCut,
        "vfx_point" | "vfxpoint" => TimelineActionType::VfxPoint,
        other => return Err(format!("unknown scene event action type {other}")),
    };
    let int_field = |snake: &str, camel: &str, default: i64| -> Result<i64, String> {
        match get(snake, camel) {
            Some(value) => value
                .as_i64()
                .ok_or_else(|| format!("scene event {snake} must be an integer")),
            None => Ok(default),
        }
    };
    let start_time_ms = int_field("start_time_ms", "startTimeMs", 0)?;
    let duration_ms = int_field("duration_ms", "durationMs", 0)?;
    if start_time_ms < 0 || duration_ms < 0 || start_time_ms.checked_add(duration_ms).is_none() {
        return Err(
            "scene event start and duration must be nonnegative and have a representable end time"
                .into(),
        );
    }
    let priority_offset = i32::try_from(int_field("priority_offset", "priorityOffset", 0)?)
        .map_err(|_| "scene event priority offset exceeds i32 range")?;
    let event_id = get("event_id", "eventId")
        .and_then(Value::as_u64)
        .unwrap_or(index as u64);
    let target_entity_path = get("target_entity_path", "targetEntityPath")
        .or_else(|| get("combiner_entity_path", "combinerEntityPath"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let payload_asset_id = get("payload_asset_id", "payloadAssetId")
        .or_else(|| get("asset_id", "assetId"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok(TimelineTrackEvent {
        event_id,
        action_type,
        target_entity_path,
        start_time_ms,
        duration_ms,
        payload_asset_id,
        cached_previous_pose_id: get("cached_previous_pose_id", "cachedPreviousPoseId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        priority_offset,
    })
}

fn node_entity_path(_storylet_id: &str, data: &NarrativeNodeData) -> String {
    match data {
        NarrativeNodeData::Poi { entity_path } => entity_path.clone(),
        _ => String::new(),
    }
}

fn node_type_name(data: &NarrativeNodeData) -> &'static str {
    match data {
        NarrativeNodeData::Start { .. } => "Start",
        NarrativeNodeData::Dialog { .. } => "Dialog",
        NarrativeNodeData::Choice { .. } => "Choice",
        NarrativeNodeData::GetEntity { .. } => "GetEntity",
        NarrativeNodeData::GetEntityProperty { .. } => "GetEntityProperty",
        NarrativeNodeData::ApplyCommand { .. } => "ApplyCommand",
        NarrativeNodeData::QbnStoryletHub { .. } => "QbnStoryletHub",
        NarrativeNodeData::SalienceSelector { .. } => "SalienceSelector",
        NarrativeNodeData::WaypointConversation { .. } => "WaypointConversation",
        NarrativeNodeData::Waiter { .. } => "Waiter",
        NarrativeNodeData::Compare { .. } => "Compare",
        NarrativeNodeData::And {} => "AND",
        NarrativeNodeData::Or {} => "OR",
        NarrativeNodeData::Condition {} => "Condition",
        NarrativeNodeData::SceneSection { .. } => "SceneSection",
        NarrativeNodeData::MissionSubGraph { .. } => "MissionSubGraph",
        NarrativeNodeData::Poi { .. } => "Poi",
        NarrativeNodeData::Image { .. } => "Image",
        NarrativeNodeData::Teleport { .. } => "Teleport",
        NarrativeNodeData::GoToLocation { .. } => "GoToLocation",
        NarrativeNodeData::GraphTransform { .. } => "GraphTransform",
        NarrativeNodeData::PassionMutation { .. } => "PassionMutation",
        NarrativeNodeData::AddEntityTag { .. } => "AddEntityTag",
        NarrativeNodeData::RemoveEntityTag { .. } => "RemoveEntityTag",
        NarrativeNodeData::HasEntityTag { .. } => "HasEntityTag",
        NarrativeNodeData::Learn { .. } => "Learn",
        NarrativeNodeData::Reveal { .. } => "Reveal",
        NarrativeNodeData::Knows { .. } => "Knows",
        NarrativeNodeData::End { .. } => "End",
    }
}
fn stable_node_guid(storylet_id: &str, node_id: &str) -> u32 {
    let mut hash = 2_166_136_261u32;
    for byte in storylet_id.bytes().chain([0]).chain(node_id.bytes()) {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash.max(1)
}
#[cfg(test)]
mod tests {
    use super::{
        bind_precondition_graphs, bind_storylet_start_nodes, compile_narrative_graphs,
        compile_node_kind, compile_output_pins, validate_entity_property_graph,
    };
    use crate::chrono_adapter::compile_chrono_dispatch;
    use crate::contracts::StoryletRecord;
    use crate::narrative_graph::{NarrativeGraph, NarrativeNode, NarrativeNodeData};
    use serde_json::{json, Value};
    use storylet_engine::chrono_runtime::{FlowGraphInstance, NodeKind, RuntimeContext};
    use storylet_engine::chrono_state::{AttributeValue, GraphLayer, WorldStateTable, WorldVertex};

    fn entity(key: &str, property_type: &str) -> crate::contracts::EntityTemplateRecord {
        serde_json::from_value(json!({
            "id": "world", "name": "World", "root_tag": "world",
            "properties": [{ "key": key, "name": key, "type": property_type, "value": null }]
        })).expect("entity fixture")
    }

    fn property_graph(source: NarrativeNodeData, property: NarrativeNodeData) -> NarrativeGraph {
        NarrativeGraph {
            nodes: vec![
                NarrativeNode { id: "source".into(), title: "Source".into(), x: 0, y: 0, width: 100, height: 80, data: source },
                NarrativeNode { id: "property".into(), title: "Property".into(), x: 120, y: 0, width: 100, height: 80, data: property },
            ],
            edges: vec![crate::narrative_graph::NarrativeEdge {
                id: "source-property".into(), from_node: "source".into(), from_port: "output".into(),
                to_node: "property".into(), to_port: "entity".into(),
                kind: crate::narrative_graph::NarrativeEdgeKind::Data,
            }],
        }
    }
    fn dynamic_property_graph(property: NarrativeNodeData) -> NarrativeGraph {
        let mut graph = property_graph(
            NarrativeNodeData::GetEntity { entity_path: "world".into() },
            NarrativeNodeData::GetEntityProperty { property_key: "ref".into(), value_type: "entity".into() },
        );
        graph.nodes[1].id = "reference".into();
        graph.nodes[1].title = "Reference".into();
        graph.edges[0].to_node = "reference".into();
        graph.nodes.push(NarrativeNode {
            id: "property".into(), title: "Property".into(), x: 240, y: 0,
            width: 100, height: 80, data: property,
        });
        graph.edges.push(crate::narrative_graph::NarrativeEdge {
            id: "reference-property".into(), from_node: "reference".into(), from_port: "output".into(),
            to_node: "property".into(), to_port: "entity".into(),
            kind: crate::narrative_graph::NarrativeEdgeKind::Data,
        });
        graph
    }

    #[test]
    fn entity_property_validation_is_source_aware_and_rejects_invalid_commands() {
        let direct = NarrativeNodeData::GetEntity { entity_path: "world".into() };
        let get = || NarrativeNodeData::GetEntityProperty { property_key: "mood".into(), value_type: "string".into() };
        assert!(validate_entity_property_graph(&property_graph(direct.clone(), get()), &[entity("mood", "string")]).is_ok());
        assert!(validate_entity_property_graph(&property_graph(direct.clone(), NarrativeNodeData::GetEntityProperty { property_key: "missing".into(), value_type: "string".into() }), &[entity("mood", "string")]).is_err());
        assert!(validate_entity_property_graph(&property_graph(direct.clone(), NarrativeNodeData::GetEntityProperty { property_key: "mood".into(), value_type: "int".into() }), &[entity("mood", "string")]).is_err());
        let array_entity: crate::contracts::EntityTemplateRecord = serde_json::from_value(json!({
            "id": "world", "name": "World", "root_tag": "world",
            "properties": [{ "key": "mood", "name": "Mood", "type": "string", "is_array": true, "value": [] }]
        })).expect("array entity fixture");
        assert!(validate_entity_property_graph(&property_graph(direct.clone(), get()), &[array_entity]).is_err());
        let world: crate::contracts::EntityTemplateRecord = serde_json::from_value(json!({
            "id": "world", "name": "World", "root_tag": "world",
            "properties": [
                { "key": "ref", "name": "ref", "type": "entity", "value": null },
                { "key": "mood", "name": "Mood", "type": "string", "value": null }
            ]
        })).expect("world fixture");
        let get_mood = NarrativeNodeData::GetEntityProperty { property_key: "mood".into(), value_type: "string".into() };
        let complete = |property_type: &str| serde_json::from_value(json!({
            "id": "target", "name": "Target", "root_tag": "fact",
            "properties": [
                { "key": "ref", "name": "ref", "type": "entity", "value": null },
                { "key": "mood", "name": "mood", "type": property_type, "value": null }
            ]
        })).expect("dynamic entity fixture");
        assert!(validate_entity_property_graph(&dynamic_property_graph(get_mood.clone()), &[world.clone(), complete("string")]).is_ok());
        assert!(validate_entity_property_graph(&dynamic_property_graph(get_mood.clone()), &[world.clone(), entity("ref", "entity")]).is_err());
        assert!(validate_entity_property_graph(&dynamic_property_graph(get_mood), &[world.clone(), complete("string"), complete("int")]).is_err());
        let command = NarrativeNodeData::ApplyCommand { property_key: "mood".into(), operation: "explode".into(), value_type: "string".into(), value: json!("x") };
        assert!(validate_entity_property_graph(&property_graph(direct, command), &[entity("mood", "string")]).is_err());
        let int_command = |value| NarrativeNodeData::ApplyCommand {
            property_key: "count".into(), operation: "set".into(), value_type: "int".into(), value,
        };
        assert!(validate_entity_property_graph(
            &property_graph(NarrativeNodeData::GetEntity { entity_path: "world".into() }, int_command(json!(3))),
            &[entity("count", "int")],
        ).is_ok());
        assert!(validate_entity_property_graph(
            &property_graph(NarrativeNodeData::GetEntity { entity_path: "world".into() }, int_command(json!("3"))),
            &[entity("count", "int")],
        ).is_err());
        assert!(validate_entity_property_graph(
            &property_graph(NarrativeNodeData::GetEntity { entity_path: "world".into() }, int_command(Value::Null)),
            &[entity("count", "int")],
        ).is_err());
    }

    #[test]
    fn authored_knowledge_nodes_compile_and_resolve_participant() {
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id":"knowledge", "title":"Knowledge", "content":{"text_template":"x"}, "effects":{}, "created_at":1,
            "graph":{"nodes":[{"id":"learn", "title":"Learn", "type":"learn", "fact_tag":"fact.secret", "kind":"evidence", "visibility":"co_located", "x":0,"y":0,"width":100,"height":80}],"edges":[]}
        })).unwrap();
        let mut dispatch = compile_chrono_dispatch(&[storylet], &[], &[]).unwrap();
        dispatch.participant_indices.insert("hero".into(), 1);
        let node: NarrativeNode = serde_json::from_value(json!({"id":"n", "title":"Knows", "type":"knows", "holder":"hero", "fact_tag":"fact.secret", "confirmed_only":true, "x":0,"y":0,"width":100,"height":80})).unwrap();
        let (kind, _) = compile_node_kind(&node, 0, &dispatch, &Default::default(), &Default::default()).unwrap();
        assert!(matches!(kind, NodeKind::Knows { holder: storylet_engine::chrono_runtime::HolderRef::Participant(1), confirmed_only:true, .. }));
        assert_eq!(super::compile_data_output_pins(&node)[0].value_type, "bool");
        assert!(super::compile_data_input_pins(&node).is_empty());
    }

    #[test]
    fn chrono_special_nodes_preserve_runtime_kind_selection_and_output_pins() {
        let storylet: StoryletRecord = serde_json::from_value(json!({
            "id": "hub", "title": "Hub", "content": {"text_template": "x"}, "effects": {}, "created_at": 1
        })).expect("storylet fixture");
        let dispatch = compile_chrono_dispatch(&[storylet], &[], &[]).expect("dispatch");
        let node = |id: &str, data: NarrativeNodeData| NarrativeNode {
            id: id.into(), title: id.into(), x: 0, y: 0, width: 100, height: 80, data
        };
        let hub = node("hub", NarrativeNodeData::QbnStoryletHub { deck_storylet_ids: vec!["hub".into()], hub_exit_storylet_id: "hub".into() });
        let (kind, _) = compile_node_kind(&hub, 0, &dispatch, &std::collections::BTreeMap::new(), &std::collections::BTreeMap::new()).expect("hub");
        assert!(matches!(kind, NodeKind::QbnHub { deck, exit_storylet_id: Some(0) } if deck == vec![0]));
        assert_eq!(super::node_type_name(&hub.data), "QbnStoryletHub");
        let salience = node("salience", NarrativeNodeData::SalienceSelector { candidate_pool_ids: vec!["hub".into()], fallback_storylet_id: "hub".into() });
        let (kind, _) = compile_node_kind(&salience, 0, &dispatch, &std::collections::BTreeMap::new(), &std::collections::BTreeMap::new()).expect("salience");
        assert_eq!(super::node_type_name(&salience.data), "SalienceSelector");
        assert!(matches!(kind, NodeKind::SalienceSelector { pool, fallback_storylet_id: Some(0), fallback_output_pin } if pool == vec![0] && fallback_output_pin == "Out"));
        assert_eq!(compile_output_pins(&salience)[0].pin_name, "Out");
        let waypoint = node("waypoint", NarrativeNodeData::WaypointConversation {
            topic_nodes: vec![json!({"topic_id":"start","exit_output_pin":"leave"})],
            topic_edges: vec![], initial_topic_id: "start".into(), npc_goal_topic_id: "start".into(),
        });
        let (kind, _) = compile_node_kind(&waypoint, 0, &dispatch, &std::collections::BTreeMap::new(), &std::collections::BTreeMap::new()).expect("waypoint");
        assert!(matches!(kind, NodeKind::WaypointConversation { initial_topic_id, npc_goal_topic_id, .. } if initial_topic_id == "start" && npc_goal_topic_id == "start"));
        assert_eq!(super::node_type_name(&waypoint.data), "WaypointConversation");
        assert_eq!(super::node_type_name(&NarrativeNodeData::GetEntity { entity_path: "world".into() }), "GetEntity");
        assert_eq!(super::node_type_name(&NarrativeNodeData::GetEntityProperty { property_key: "awake".into(), value_type: "bool".into() }), "GetEntityProperty");
        assert_eq!(super::node_type_name(&NarrativeNodeData::ApplyCommand { property_key: "awake".into(), operation: "toggle".into(), value_type: "bool".into(), value: json!(false) }), "ApplyCommand");
        assert_eq!(compile_output_pins(&waypoint).iter().map(|pin| pin.pin_name.as_str()).collect::<Vec<_>>(), vec!["leave"]);
    }

    #[test]
    fn authored_precondition_graph_is_bound_to_dispatch_storylet() {
        let storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "gated",
            "title": "Gated",
            "content": { "text_template": "A scene." },
            "effects": {},
            "created_at": 1,
            "precondition_graph": {
                "nodes": [
                    { "id": "start", "title": "Start", "type": "start", "text": "Check", "x": 0, "y": 0, "width": 100, "height": 80 },
                    { "id": "reject", "title": "Reject", "type": "end", "precondition_result": false, "x": 200, "y": 0, "width": 120, "height": 80 }
                ],
                "edges": [
                    { "id": "start-reject", "from_node": "start", "from_port": "output", "to_node": "reject", "to_port": "input" }
                ]
            }
        }))
        .expect("valid storylet");
        let mut dispatch =
            compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[]).expect("dispatch compiles");
        let _graphs = compile_narrative_graphs(
            "project-a",
            std::slice::from_ref(&storylet),
            &dispatch,
            &std::collections::BTreeMap::new(),
        )
        .expect("graphs validate");
        bind_precondition_graphs(
            &mut dispatch,
            std::slice::from_ref(&storylet),
            &std::collections::BTreeMap::new(),
        )
        .expect("precondition graph binds");

        let storylet = dispatch.dispatcher.storylet(0).expect("storylet in dispatch");
        let graph = storylet.precondition_graph.as_ref().expect("authored graph bound");
        assert!(graph.nodes.iter().any(|node| matches!(
            &node.kind,
            NodeKind::End {
                precondition_result,
                ..
            } if !precondition_result
        )));
    }

    #[test]
    fn connected_entity_ref_targets_drive_precondition_tag_checks_for_entities_and_world() {
        use crate::narrative_graph::{NarrativeEdge, NarrativeEdgeKind};

        let entity_indices = std::collections::BTreeMap::from([
            ("world".to_string(), 0_u16),
            ("actor".to_string(), 1_u16),
        ]);
        let node = |id: &str, data: NarrativeNodeData| NarrativeNode {
            id: id.into(), title: id.into(), x: 0, y: 0, width: 160, height: 80, data
        };

        for (storylet_id, dynamic_target) in [
            ("precondition-actor", "actor"),
            ("precondition-world", "world"),
        ] {
            let gameplay_tag = if dynamic_target == "world" {
                "world.garden.precondition"
            } else {
                "character.garden.precondition"
            };
            let mut storylet: StoryletRecord = serde_json::from_value(json!({
                "id": storylet_id,
                "title": storylet_id,
                "content": { "text_template": "Eligible." },
                "created_at": 0
            })).unwrap();
            storylet.precondition_graph = Some(NarrativeGraph {
                nodes: vec![
                    node("start", NarrativeNodeData::Start { label: "Check".into() }),
                    node("target", NarrativeNodeData::GetEntity { entity_path: dynamic_target.into() }),
                    node("has", NarrativeNodeData::HasEntityTag {
                        target_entity_id: "world".into(),
                        tag: gameplay_tag.into(),
                        match_mode: "all".into(),
                    }),
                    node("condition", NarrativeNodeData::Condition {}),
                    node("eligible", NarrativeNodeData::End {
                        is_terminal: true,
                        jump_to_storylet: None,
                        precondition_result: Some(true),
                    }),
                    node("ineligible", NarrativeNodeData::End {
                        is_terminal: true,
                        jump_to_storylet: None,
                        precondition_result: Some(false),
                    }),
                ],
                edges: vec![
                    NarrativeEdge { id: "start-condition".into(), from_node: "start".into(), from_port: "output".into(), to_node: "condition".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "has-condition".into(), from_node: "has".into(), from_port: "output".into(), to_node: "condition".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Data },
                    NarrativeEdge { id: "target-has".into(), from_node: "target".into(), from_port: "output".into(), to_node: "has".into(), to_port: "target".into(), kind: NarrativeEdgeKind::Data },
                    NarrativeEdge { id: "condition-eligible".into(), from_node: "condition".into(), from_port: "true".into(), to_node: "eligible".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "condition-ineligible".into(), from_node: "condition".into(), from_port: "false".into(), to_node: "ineligible".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                ],
            });
            let mut dispatch = compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[])
                .expect("dispatch compiles");
            bind_precondition_graphs(
                &mut dispatch,
                std::slice::from_ref(&storylet),
                &entity_indices,
            )
            .expect("precondition graph compiles");

            let tag_id = dispatch.registry.id(gameplay_tag)
                .expect("precondition tag registered");
            let mut state = WorldStateTable::with_capacity(2);
            state.vertices.push(WorldVertex {
                vertex_id: "actor".into(),
                layer: GraphLayer::Character,
                entity_index: Some(1),
                attributes: std::collections::BTreeMap::new(),
            });
            if dynamic_target == "world" {
                state.global_tags.set_bit(tag_id).unwrap();
            } else {
                state.entity_tags[1].set_bit(tag_id).unwrap();
            }

            let graph = dispatch.dispatcher.storylet(0).unwrap()
                .precondition_graph.as_ref().expect("precondition graph bound");
            let context = RuntimeContext::with_registry(
                &mut state,
                &dispatch.dispatcher,
                &dispatch.registry,
            );
            assert!(
                FlowGraphInstance::evaluate_precondition_graph(graph, &context).unwrap(),
                "{storylet_id}: Has Entity Tag must use the connected target, not its static fallback"
            );
        }
    }


    #[test]
    fn compiled_authored_graph_executes_from_storylet_start_to_end() {
        let storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "scene-a",
            "title": "Scene A",
            "content": { "text_template": "A scene." },
            "effects": {},
            "created_at": 1
        }))
        .expect("valid storylet");
        let mut dispatch =
            compile_chrono_dispatch(&[storylet.clone()], &[], &[]).expect("dispatch compiles");
        let graphs = compile_narrative_graphs(
            "project-a",
            &[storylet],
            &dispatch,
            &std::collections::BTreeMap::new(),
        )
        .expect("graphs compile");
        bind_storylet_start_nodes(&mut dispatch, &graphs).expect("start nodes bind");
        let mut instance = FlowGraphInstance::new(1, graphs.asset, 7).expect("runtime instance");
        let mut state = WorldStateTable::default();
        let mut context =
            RuntimeContext::with_registry(&mut state, &dispatch.dispatcher, &dispatch.registry);

        instance.start().expect("start token");
        instance.tick(&mut context, 100).expect("execute graph");

        let history = instance.call_history();
        assert!(
            history.len() >= 3,
            "expected Start, Dialog, and End in history"
        );
        assert!(
            history
                .iter()
                .any(|entry| entry.node_guid == graphs.storylet_start_nodes["scene-a"])
        );
        assert!(history.iter().any(|entry| {
            instance.asset.node(entry.node_guid).is_some_and(|node| {
                matches!(
                    node.kind,
                    storylet_engine::chrono_runtime::NodeKind::End { .. }
                )
            })
        }));
    }

    #[test]
    fn authored_apply_command_mutates_typed_entity_property() {
        let storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "mutation",
            "title": "Mutation",
            "content": { "text_template": "A scene." },
            "effects": {},
            "created_at": 1,
            "graph": {
                "nodes": [
                    { "id": "start", "type": "start", "title": "Start", "text": "Begin mutation test", "x": 0, "y": 0, "width": 100, "height": 80 },
                    { "id": "world", "type": "get_entity", "title": "World", "entity_path": "world", "x": 100, "y": 100, "width": 140, "height": 80 },
                    { "id": "owner", "type": "get_entity_property", "title": "Owner", "property_key": "owner", "value_type": "entity", "x": 220, "y": 100, "width": 160, "height": 80 },
                    { "id": "command", "type": "apply_command", "title": "Add Score", "property_key": "score", "operation": "add", "value_type": "int", "value": 4, "x": 420, "y": 0, "width": 160, "height": 100 },
                    { "id": "end", "type": "end", "title": "End", "is_terminal": true, "x": 620, "y": 0, "width": 120, "height": 80 }
                ],
                "edges": [
                    { "id": "start-command", "from_node": "start", "from_port": "output", "to_node": "command", "to_port": "input", "kind": "flow" },
                    { "id": "command-end", "from_node": "command", "from_port": "output", "to_node": "end", "to_port": "input", "kind": "flow" },
                    { "id": "world-owner", "from_node": "world", "from_port": "output", "to_node": "owner", "to_port": "entity", "kind": "data" },
                    { "id": "owner-command", "from_node": "owner", "from_port": "output", "to_node": "command", "to_port": "entity", "kind": "data" }
                ]
            }
        })).expect("valid storylet");
        let mut dispatch =
            compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[]).expect("dispatch compiles");
        let graphs = compile_narrative_graphs(
            "project-a",
            std::slice::from_ref(&storylet),
            &dispatch,
            &std::collections::BTreeMap::new(),
        ).expect("graph compiles");
        bind_storylet_start_nodes(&mut dispatch, &graphs).expect("start node binds");

        let mut state = WorldStateTable::new();
        state.vertices.push(WorldVertex {
            vertex_id: "world".into(),
            layer: GraphLayer::Fact,
            entity_index: None,
            attributes: std::collections::BTreeMap::from([
                ("prop.owner".into(), AttributeValue::EntityRef("world".into())),
                ("prop.score".into(), AttributeValue::Int(3)),
            ]),
        });
        let mut instance = FlowGraphInstance::new(1, graphs.asset, 7).expect("runtime instance");
        let mut context =
            RuntimeContext::with_registry(&mut state, &dispatch.dispatcher, &dispatch.registry);
        instance.start().expect("start token");
        instance.tick(&mut context, 100).expect("execute mutation graph");

        let score = state.vertices.iter()
            .find(|vertex| vertex.vertex_id == "world")
            .and_then(|vertex| vertex.attributes.get("prop.score"));
        assert_eq!(score, Some(&AttributeValue::Int(7)));
    }
    #[test]
    fn choice_branches_converge_on_dialog_in_play_and_story_invoke() {
        use crate::narrative_graph::{
            ChoiceOption, NarrativeEdge, NarrativeGraph, NarrativeNode, NarrativeNodeData,
        };
        use std::collections::BTreeMap;
        use storylet_engine::chrono_runtime::NodeKind;

        let node = |id: &str, title: &str, data: NarrativeNodeData| NarrativeNode {
            id: id.into(),
            title: title.into(),
            x: 0,
            y: 0,
            width: 160,
            height: 80,
            data,
        };
        let authored = NarrativeGraph {
            nodes: vec![
                node(
                    "start",
                    "Start",
                    NarrativeNodeData::Start {
                        label: "Begin".into(),
                    },
                ),
                node(
                    "choice",
                    "Choice",
                    NarrativeNodeData::Choice {
                        choices: vec![
                            ChoiceOption {
                                id: "a".into(),
                                text: "A".into(),
                                system_verb: None,
                                effects: None,
                            },
                            ChoiceOption {
                                id: "b".into(),
                                text: "B".into(),
                                system_verb: None,
                                effects: None,
                            },
                        ],
                    },
                ),
                node(
                    "branch_a",
                    "Branch A",
                    NarrativeNodeData::Dialog {
                        speaker_role: None,
                        text: Some("A".into()),
                        quip: None,
                        sections: Vec::new(),
                    },
                ),
                node(
                    "branch_b",
                    "Branch B",
                    NarrativeNodeData::Dialog {
                        speaker_role: None,
                        text: Some("B".into()),
                        quip: None,
                        sections: Vec::new(),
                    },
                ),
                node(
                    "shared",
                    "Shared Dialog",
                    NarrativeNodeData::Dialog {
                        speaker_role: None,
                        text: Some("Shared".into()),
                        quip: None,
                        sections: Vec::new(),
                    },
                ),
                node(
                    "poi",
                    "POI",
                    NarrativeNodeData::Poi {
                        entity_path: "poi:end".into(),
                    },
                ),
                node(
                    "end",
                    "End",
                    NarrativeNodeData::End {
                        is_terminal: true,
                        jump_to_storylet: None,
                        precondition_result: None,
                    },
                ),
            ],
            edges: vec![
                edge("start-choice", "start", "output", "choice", "input"),
                edge("choice-a", "choice", "choice_0", "branch_a", "input"),
                edge("choice-b", "choice", "choice_1", "branch_b", "input"),
                edge("a-shared", "branch_a", "output", "shared", "input"),
                edge("b-shared", "branch_b", "output", "shared", "input"),
                edge("shared-poi", "shared", "output", "poi", "input"),
                edge("poi-end", "poi", "output", "end", "input"),
            ],
        };
        fn edge(id: &str, from: &str, from_port: &str, to: &str, to_port: &str) -> NarrativeEdge {
            NarrativeEdge {
                id: id.into(),
                from_node: from.into(),
                from_port: from_port.into(),
                to_node: to.into(),
                to_port: to_port.into(),
                kind: crate::narrative_graph::NarrativeEdgeKind::Flow,
            }
        }
        let mut storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "scene-merge", "title": "Merge", "content": { "text_template": "Scene." },
            "effects": {}, "created_at": 1
        }))
        .unwrap();
        storylet.graph = Some(authored);

        let mut dispatch =
            compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[]).unwrap();
        let graphs = compile_narrative_graphs(
            "project-merge",
            std::slice::from_ref(&storylet),
            &dispatch,
            &BTreeMap::new(),
        )
        .unwrap();
        bind_storylet_start_nodes(&mut dispatch, &graphs).unwrap();
        let choice_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| node.payload["nodeId"] == "choice")
            .map(|node| node.node_guid)
            .expect("compiled Choice node preserves its authored id");
        let shared_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| {
                node.node_type_id == "Dialog" && node.payload["nodeId"] == "shared"
            })
            .unwrap()
            .node_guid;
        let branch_a_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| node.payload["nodeId"] == "branch_a")
            .unwrap()
            .node_guid;
        let branch_b_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| node.payload["nodeId"] == "branch_b")
            .unwrap()
            .node_guid;
        let poi_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| node.payload["nodeId"] == "poi")
            .map(|node| node.node_guid)
            .expect("compiled Poi node preserves its authored id");

        let mut state = WorldStateTable::default();
        let mut context =
            RuntimeContext::with_registry(&mut state, &dispatch.dispatcher, &dispatch.registry);
        let mut normal = FlowGraphInstance::new(10, graphs.asset.clone(), 1).unwrap();
        normal
            .start_at_node(graphs.storylet_start_nodes["scene-merge"])
            .unwrap();
        normal.tick(&mut context, 100).unwrap();
        assert!(matches!(
            &normal.asset.node(branch_a_guid).unwrap().kind,
            NodeKind::LogicAny { .. }
        ));
        normal
            .choose(&mut context, choice_guid, "choice_0")
            .unwrap();
        normal.tick(&mut context, 100).unwrap();
        assert!(
            normal
                .global_call_history
                .iter()
                .any(|entry| entry.node_guid == shared_guid)
        );
        assert!(
            normal
                .global_call_history
                .iter()
                .any(|entry| entry.node_guid == branch_a_guid)
        );
        assert!(
            !normal
                .global_call_history
                .iter()
                .any(|entry| entry.node_guid == branch_b_guid)
        );

        let mut invoked = FlowGraphInstance::new(11, graphs.asset.clone(), 1).unwrap();
        let mut invoked_state = WorldStateTable::default();
        let mut invoked_context = RuntimeContext::with_registry(
            &mut invoked_state,
            &dispatch.dispatcher,
            &dispatch.registry,
        );
        invoked
            .invoke_story_to_poi(
                &mut invoked_context,
                poi_guid,
                &BTreeMap::from([(shared_guid, "In_0".into())]),
            )
            .unwrap();
        assert!(
            !invoked
                .global_call_history
                .iter()
                .any(|entry| entry.node_guid == branch_b_guid)
        );
        assert!(
            invoked
                .global_call_history
                .iter()
                .any(|entry| entry.node_guid == poi_guid)
        );
    }
    #[test]
    fn jump_wins_over_a_terminal_flag() {
        use crate::narrative_graph::{NarrativeGraph, NarrativeNode, NarrativeNodeData};
        use storylet_engine::chrono_runtime::NodeKind;

        let mut storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "terminal-jump", "title": "Terminal Jump",
            "content": { "text_template": "End." }, "effects": {}, "created_at": 1
        }))
        .unwrap();
        let graph = NarrativeGraph {
            nodes: vec![
                NarrativeNode {
                    id: "start".into(), title: "Start".into(), x: 0, y: 0, width: 100, height: 80,
                    data: NarrativeNodeData::Start { label: "Begin".into() },
                },
                NarrativeNode {
                    id: "end".into(), title: "End".into(), x: 200, y: 0, width: 100, height: 80,
                    data: NarrativeNodeData::End {
                        is_terminal: true,
                        jump_to_storylet: Some("terminal-jump".into()),
                        precondition_result: None,
                    },
                },
            ],
            edges: vec![crate::narrative_graph::NarrativeEdge {
                id: "start-end".into(), from_node: "start".into(), from_port: "output".into(),
                to_node: "end".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow,
            }],
        };
        storylet.graph = Some(graph);
        let compiled = compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[]).unwrap();
        let graphs = compile_narrative_graphs(
            "terminal-jump", std::slice::from_ref(&storylet), &compiled, &std::collections::BTreeMap::new()
        ).unwrap();
        let end = graphs.asset.nodes.iter().find(|node| node.payload["nodeId"] == "end").unwrap();
        assert!(matches!(end.kind, NodeKind::End { jump_to_storylet: Some(_), .. }));
    }

    #[test]
    fn multiple_end_graph_executes_terminal_and_jump_branches() {
        use crate::narrative_graph::{
            ChoiceOption, NarrativeEdge, NarrativeGraph, NarrativeNode, NarrativeNodeData,
        };
        use storylet_engine::chrono_runtime::NodeKind;

        let node = |id: &str, data: NarrativeNodeData| NarrativeNode {
            id: id.into(),
            title: id.into(),
            x: 0,
            y: 0,
            width: 160,
            height: 80,
            data,
        };
        let mut first: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "a-first", "title": "First", "content": { "text_template": "First." },
            "effects": {}, "created_at": 1
        }))
        .unwrap();
        first.graph = Some(NarrativeGraph {
            nodes: vec![
                node("start", NarrativeNodeData::Start { label: "Begin".into() }),
                node("choice", NarrativeNodeData::Choice { choices: vec![
                    ChoiceOption { id: "stay".into(), text: "Stay".into(), system_verb: None, effects: None },
                    ChoiceOption { id: "go".into(), text: "Go".into(), system_verb: None, effects: None },
                ]}),
                node("end-terminal", NarrativeNodeData::End { is_terminal: true, jump_to_storylet: None, precondition_result: None }),
                node("end-jump", NarrativeNodeData::End { is_terminal: false, jump_to_storylet: Some("b-next".into()), precondition_result: None }),
            ],
            edges: vec![
                NarrativeEdge { id: "start-choice".into(), from_node: "start".into(), from_port: "output".into(), to_node: "choice".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "choice-stay".into(), from_node: "choice".into(), from_port: "choice_0".into(), to_node: "end-terminal".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "choice-go".into(), from_node: "choice".into(), from_port: "choice_1".into(), to_node: "end-jump".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
            ],
        });
        let next: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "b-next", "title": "Next", "content": { "text_template": "Next." },
            "effects": {}, "created_at": 1
        }))
        .unwrap();
        let storylets = [first, next];
        let mut dispatch = compile_chrono_dispatch(&storylets, &[], &[]).unwrap();
        let graphs = compile_narrative_graphs(
            "multi-end-project",
            &storylets,
            &dispatch,
            &std::collections::BTreeMap::new(),
        )
        .unwrap();
        bind_storylet_start_nodes(&mut dispatch, &graphs).unwrap();

        let mut state = WorldStateTable::default();
        let mut context =
            RuntimeContext::with_registry(&mut state, &dispatch.dispatcher, &dispatch.registry);
        let choice_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| node.payload["nodeId"] == "choice")
            .unwrap()
            .node_guid;
        let terminal_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| node.payload["nodeId"] == "end-terminal")
            .unwrap()
            .node_guid;
        let jump_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| node.payload["nodeId"] == "end-jump")
            .unwrap()
            .node_guid;
        assert_eq!(
            graphs
                .asset
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, NodeKind::End { .. }))
                .count(),
            3,
            "two authored Ends and the next storylet's End are compiled"
        );

        let mut terminal = FlowGraphInstance::new(11, graphs.asset.clone(), 7).unwrap();
        terminal
            .start_at_node(graphs.storylet_start_nodes["a-first"])
            .unwrap();
        terminal.tick(&mut context, 100).unwrap();
        terminal
            .choose(&mut context, choice_guid, "choice_0")
            .unwrap();
        terminal.tick(&mut context, 100).unwrap();
        assert!(
            terminal
                .global_call_history
                .iter()
                .any(|entry| entry.node_guid == terminal_guid)
        );

        let mut jump = FlowGraphInstance::new(12, graphs.asset.clone(), 7).unwrap();
        jump.start_at_node(graphs.storylet_start_nodes["a-first"])
            .unwrap();
        jump.tick(&mut context, 100).unwrap();
        jump.choose(&mut context, choice_guid, "choice_1").unwrap();
        for _ in 0..6 {
            jump.tick(&mut context, 100).unwrap();
        }
        assert!(
            jump.global_call_history
                .iter()
                .any(|entry| entry.node_guid == jump_guid)
        );
        assert!(
            jump.global_call_history
                .iter()
                .any(|entry| entry.node_guid == graphs.storylet_start_nodes["b-next"])
        );
    }

    #[test]
    fn mission_nodes_run_the_selected_storylet_in_an_isolated_child_instance() {
        use crate::narrative_graph::{NarrativeGraph, NarrativeNode, NarrativeNodeData};
        use storylet_engine::chrono_runtime::NodeKind;

        let mut parent: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "mission-parent",
            "title": "Parent",
            "content": { "text_template": "Parent scene." },
            "created_at": 1
        }))
        .unwrap();
        let child: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "mission-child",
            "title": "Child",
            "content": { "text_template": "Child scene." },
            "created_at": 1
        }))
        .unwrap();

        let mut graph = NarrativeGraph::from_storylet(&parent);
        let end_id = graph
            .nodes
            .iter()
            .find(|node| matches!(node.data, NarrativeNodeData::End { .. }))
            .unwrap()
            .id
            .clone();
        let old_predecessor = graph
            .edges
            .iter_mut()
            .find(|edge| edge.to_node == end_id)
            .map(|edge| {
                let previous = edge.from_node.clone();
                edge.from_node = "mission".into();
                previous
            })
            .unwrap();
        graph.nodes.push(NarrativeNode {
            id: "mission".into(),
            title: "Mission".into(),
            x: 500,
            y: 120,
            width: 220,
            height: 100,
            data: NarrativeNodeData::MissionSubGraph {
                sub_graph_asset_id: child.id.clone(),
            },
        });
        graph.edges.push(crate::narrative_graph::NarrativeEdge {
            id: "dialog-mission".into(),
            from_node: old_predecessor,
            from_port: "output".into(),
            to_node: "mission".into(),
            to_port: "input".into(),
            kind: crate::narrative_graph::NarrativeEdgeKind::Flow,
        });
        parent.graph = Some(graph);

        let storylets = [parent, child];
        let mut dispatch = compile_chrono_dispatch(&storylets, &[], &[]).unwrap();
        let graphs = compile_narrative_graphs(
            "mission-project",
            &storylets,
            &dispatch,
            &std::collections::BTreeMap::new(),
        )
        .unwrap();
        bind_storylet_start_nodes(&mut dispatch, &graphs).unwrap();

        let mut instance = FlowGraphInstance::new(71, graphs.asset.clone(), 7).unwrap();
        instance
            .attach_mission_assets(&graphs.mission_assets, 7)
            .unwrap();
        let mission_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::MissionSubGraph { .. }))
            .unwrap()
            .node_guid;
        assert_eq!(
            instance
                .mission_instance(mission_guid)
                .unwrap()
                .asset
                .asset_id,
            "mission-child"
        );

        let mut state = WorldStateTable::default();
        let mut context = RuntimeContext::with_registries(
            &mut state,
            &dispatch.dispatcher,
            &dispatch.registry,
            &dispatch.passion_registry,
        );
        instance
            .start_at_node(graphs.storylet_start_nodes["mission-parent"])
            .unwrap();
        instance.tick(&mut context, 100).unwrap();
        assert!(!instance.mission_call_history(mission_guid).is_empty());
        assert!(
            instance
                .global_call_history
                .iter()
                .any(|entry| entry.node_guid == mission_guid)
        );

        let global_history = instance.global_call_history.clone();
        instance
            .rollback_mission_to_start(&mut context, mission_guid)
            .unwrap();
        assert!(instance.mission_call_history(mission_guid).is_empty());
        assert_eq!(instance.global_call_history, global_history);
    }

    #[test]
    fn entity_tag_nodes_execute_add_remove_and_conditional_branch() {
        use crate::narrative_graph::{NarrativeEdge, NarrativeGraph, NarrativeNode, NarrativeNodeData};
        let node = |id: &str, data: NarrativeNodeData| NarrativeNode {
            id: id.into(), title: id.into(), x: 0, y: 0, width: 160, height: 80, data
        };
        let authored = NarrativeGraph {
            nodes: vec![
                node("start", NarrativeNodeData::Start { label: "Begin".into() }),
                node("add", NarrativeNodeData::AddEntityTag { target_entity_id: "world".into(), tag: "world.garden.visited".into() }),
                node("has", NarrativeNodeData::HasEntityTag { target_entity_id: "world".into(), tag: "world.garden.visited".into(), match_mode: "all".into() }),
                node("check", NarrativeNodeData::Condition {}),
                node("remove", NarrativeNodeData::RemoveEntityTag { target_entity_id: "world".into(), tag: "world.garden.visited".into() }),
                node("has_after_remove", NarrativeNodeData::HasEntityTag { target_entity_id: "world".into(), tag: "world.garden.visited".into(), match_mode: "all".into() }),
                node("check_after_remove", NarrativeNodeData::Condition {}),
                node("reject", NarrativeNodeData::Dialog { speaker_role: None, text: Some("Unexpected tag result".into()), quip: None, sections: vec![] }),
                node("valid", NarrativeNodeData::Dialog { speaker_role: None, text: Some("Tag removed".into()), quip: None, sections: vec![] }),
                node("end", NarrativeNodeData::End { is_terminal: true, jump_to_storylet: None, precondition_result: None }),
            ],
            edges: vec![
                NarrativeEdge { id: "e1".into(), from_node: "start".into(), from_port: "output".into(), to_node: "add".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "e2".into(), from_node: "add".into(), from_port: "output".into(), to_node: "check".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "e3".into(), from_node: "has".into(), from_port: "output".into(), to_node: "check".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Data },
                NarrativeEdge { id: "e4".into(), from_node: "check".into(), from_port: "true".into(), to_node: "remove".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "e5".into(), from_node: "check".into(), from_port: "false".into(), to_node: "reject".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "e6".into(), from_node: "remove".into(), from_port: "output".into(), to_node: "check_after_remove".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "e7".into(), from_node: "has_after_remove".into(), from_port: "output".into(), to_node: "check_after_remove".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Data },
                NarrativeEdge { id: "e8".into(), from_node: "check_after_remove".into(), from_port: "true".into(), to_node: "reject".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "e9".into(), from_node: "check_after_remove".into(), from_port: "false".into(), to_node: "valid".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "e10".into(), from_node: "reject".into(), from_port: "output".into(), to_node: "end".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
                NarrativeEdge { id: "e11".into(), from_node: "valid".into(), from_port: "output".into(), to_node: "end".into(), to_port: "input".into(), kind: crate::narrative_graph::NarrativeEdgeKind::Flow },
            ],
        };
        let mut storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "tag-test", "title": "Tag Test", "content": { "text_template": "Begin." }, "created_at": 0
        })).unwrap();
        storylet.graph = Some(authored);
        let mut dispatch = compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[]).unwrap();
        let graphs = compile_narrative_graphs("project-tag", std::slice::from_ref(&storylet), &dispatch, &std::collections::BTreeMap::new()).unwrap();
        bind_storylet_start_nodes(&mut dispatch, &graphs).unwrap();
        let mut instance = FlowGraphInstance::new(1, graphs.asset, 7).unwrap();
        let mut state = WorldStateTable::default();
        let mut context = RuntimeContext::with_registry(&mut state, &dispatch.dispatcher, &dispatch.registry);
        instance.start_at_node(graphs.storylet_start_nodes["tag-test"]).unwrap();
        instance.tick(&mut context, 100).unwrap();
        let tag_id = dispatch.registry.id("world.garden.visited").expect("tag was registered");
        assert!(!state.global_tags.bit(tag_id), "Remove Entity Tag clears the tag after the true branch");
        let executed: Vec<_> = instance.call_history().iter()
            .filter_map(|entry| instance.asset.node(entry.node_guid).map(|node| node.payload["nodeId"].as_str().unwrap_or_default()))
            .collect();
        assert!(executed.contains(&"valid"), "Has Entity Tag false result must reach the false branch");
        assert!(!executed.contains(&"reject"), "neither unexpected tag branch should run");
    }
    #[test]
    fn connected_entity_ref_targets_add_remove_and_query_entity_or_world_tags() {
        use crate::narrative_graph::{NarrativeEdge, NarrativeEdgeKind};

        let entity_indices = std::collections::BTreeMap::from([
            ("world".to_string(), 0_u16),
            ("actor".to_string(), 1_u16),
        ]);
        let node = |id: &str, data: NarrativeNodeData| NarrativeNode {
            id: id.into(), title: id.into(), x: 0, y: 0, width: 160, height: 80, data
        };

        for (storylet_id, entity_path) in [("dynamic-world", "world"), ("dynamic-actor", "actor")] {
            let gameplay_tag = if entity_path == "world" {
                "world.garden.dynamic"
            } else {
                "character.garden.dynamic"
            };
            let authored = NarrativeGraph {
                nodes: vec![
                    node("start", NarrativeNodeData::Start { label: "Begin".into() }),
                    node("target", NarrativeNodeData::GetEntity { entity_path: entity_path.into() }),
                    node("add", NarrativeNodeData::AddEntityTag { target_entity_id: "world".into(), tag: gameplay_tag.into() }),
                    node("has", NarrativeNodeData::HasEntityTag { target_entity_id: "world".into(), tag: gameplay_tag.into(), match_mode: "all".into() }),
                    node("check", NarrativeNodeData::Condition {}),
                    node("remove", NarrativeNodeData::RemoveEntityTag { target_entity_id: "world".into(), tag: gameplay_tag.into() }),
                    node("has_after_remove", NarrativeNodeData::HasEntityTag { target_entity_id: "world".into(), tag: gameplay_tag.into(), match_mode: "all".into() }),
                    node("check_after_remove", NarrativeNodeData::Condition {}),
                    node("reject", NarrativeNodeData::Dialog { speaker_role: None, text: Some("Unexpected tag result".into()), quip: None, sections: vec![] }),
                    node("valid", NarrativeNodeData::Dialog { speaker_role: None, text: Some("Tag removed".into()), quip: None, sections: vec![] }),
                    node("end", NarrativeNodeData::End { is_terminal: true, jump_to_storylet: None, precondition_result: None }),
                ],
                edges: vec![
                    NarrativeEdge { id: "e1".into(), from_node: "start".into(), from_port: "output".into(), to_node: "add".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e2".into(), from_node: "add".into(), from_port: "output".into(), to_node: "check".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e3".into(), from_node: "has".into(), from_port: "output".into(), to_node: "check".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Data },
                    NarrativeEdge { id: "e4".into(), from_node: "check".into(), from_port: "true".into(), to_node: "remove".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e5".into(), from_node: "check".into(), from_port: "false".into(), to_node: "reject".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e6".into(), from_node: "remove".into(), from_port: "output".into(), to_node: "check_after_remove".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e7".into(), from_node: "has_after_remove".into(), from_port: "output".into(), to_node: "check_after_remove".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Data },
                    NarrativeEdge { id: "e8".into(), from_node: "check_after_remove".into(), from_port: "true".into(), to_node: "reject".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e9".into(), from_node: "check_after_remove".into(), from_port: "false".into(), to_node: "valid".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e10".into(), from_node: "reject".into(), from_port: "output".into(), to_node: "end".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e11".into(), from_node: "valid".into(), from_port: "output".into(), to_node: "end".into(), to_port: "input".into(), kind: NarrativeEdgeKind::Flow },
                    NarrativeEdge { id: "e12".into(), from_node: "target".into(), from_port: "output".into(), to_node: "add".into(), to_port: "target".into(), kind: NarrativeEdgeKind::Data },
                    NarrativeEdge { id: "e13".into(), from_node: "target".into(), from_port: "output".into(), to_node: "has".into(), to_port: "target".into(), kind: NarrativeEdgeKind::Data },
                    NarrativeEdge { id: "e14".into(), from_node: "target".into(), from_port: "output".into(), to_node: "remove".into(), to_port: "target".into(), kind: NarrativeEdgeKind::Data },
                    NarrativeEdge { id: "e15".into(), from_node: "target".into(), from_port: "output".into(), to_node: "has_after_remove".into(), to_port: "target".into(), kind: NarrativeEdgeKind::Data },
                ],
            };
            let mut storylet: StoryletRecord = serde_json::from_value(json!({
                "id": storylet_id, "title": storylet_id, "content": { "text_template": "Begin." }, "created_at": 0
            })).unwrap();
            storylet.graph = Some(authored);
            let mut dispatch = compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[]).unwrap();
            let graphs = compile_narrative_graphs(
                &format!("project-{storylet_id}"),
                std::slice::from_ref(&storylet),
                &dispatch,
                &entity_indices,
            ).unwrap();
            bind_storylet_start_nodes(&mut dispatch, &graphs).unwrap();
            let tag_id = dispatch.registry.id(gameplay_tag).expect("tag was registered");
            let mut instance = FlowGraphInstance::new(1, graphs.asset, 7).unwrap();
            let mut state = WorldStateTable::with_capacity(2);
            state.vertices.push(WorldVertex {
                vertex_id: "actor".into(),
                layer: GraphLayer::Character,
                entity_index: Some(1),
                attributes: std::collections::BTreeMap::new(),
            });
            {
                let mut context = RuntimeContext::with_registry(&mut state, &dispatch.dispatcher, &dispatch.registry);
                instance.start_at_node(graphs.storylet_start_nodes[storylet_id]).unwrap();
                instance.tick(&mut context, 100).unwrap();
            }
            let executed: Vec<_> = instance.call_history().iter()
                .filter_map(|entry| instance.asset.node(entry.node_guid).map(|node| node.payload["nodeId"].as_str().unwrap_or_default()))
                .collect();
            assert!(executed.contains(&"valid"), "{storylet_id}: Add/Has/Remove/Has must observe the connected target");
            assert!(!executed.contains(&"reject"), "{storylet_id}: unexpected tag branch ran");
            assert!(!state.global_tags.bit(tag_id), "{storylet_id}: tag remains absent from world after removal");
            assert!(!state.entity_tags[1].bit(tag_id), "{storylet_id}: tag remains absent from actor after removal");
        }
    }
    #[test]
    fn typed_boolean_data_nodes_validate() {
        use crate::narrative_graph::NarrativeNodeData;
        for value in [
            serde_json::json!({"type":"has_entity_tag","target_entity_id":"world","tag":"harbor"}),
            serde_json::json!({"type":"and"}),
            serde_json::json!({"type":"or"}),
        ] {
            let data: NarrativeNodeData = serde_json::from_value(value).unwrap();
            assert!(matches!(data, NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. } | NarrativeNodeData::And {} | NarrativeNodeData::Or {}));
        }
    }
}
