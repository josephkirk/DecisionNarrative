//! Stable compilation of authored narrative graphs into the v5 token runtime.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use storylet_engine::chrono_dispatch::{WaypointTopicEdge, WaypointTopicGraph, WaypointTopicNode};
use storylet_engine::chrono_runtime::{
    ComparisonOperator, Condition, ConditionOperator, ConditionRule, DEFAULT_INPUT_PIN,
    DEFAULT_OUTPUT_PIN, EdgeConnection, FlowGraphAsset, FlowNodeDef, NodeCategory, NodeKind,
    PinDefinition, TimelineActionType, TimelineTrackEvent,
    VariableAssignment as RuntimeVariableAssignment, VariableOperator, VariableScope,
    VariableValue,
};
use storylet_engine::chrono_state::{
    AttributeAssignment as RuntimeAttributeAssignment, AttributeOp as RuntimeAttributeOp,
    AttributePredicate as RuntimeAttributePredicate, AttributeValue as RuntimeAttributeValue,
    EdgePattern as RuntimeEdgePattern, EdgeTemplate as RuntimeEdgeTemplate,
    GraphBinding as RuntimeGraphBinding, GraphLayer as RuntimeGraphLayer,
    GraphMutation as RuntimeGraphMutation, GraphPattern as RuntimeGraphPattern,
    GraphTransformRule as RuntimeGraphTransformRule, MutationOp,
    PassionMutation as RuntimePassionMutation, TagBitset, VertexPattern as RuntimeVertexPattern,
    VertexTemplate as RuntimeVertexTemplate,
};

use crate::chrono_adapter::CompiledChronoDispatch;
use crate::contracts::StoryletRecord;
use crate::narrative_graph::{
    AttributeAssignment as AuthoredAttributeAssignment, AttributeOp as AuthoredAttributeOp,
    AttributePredicate as AuthoredAttributePredicate, AttributeValue as AuthoredAttributeValue,
    EdgePattern as AuthoredEdgePattern, EdgeTemplate as AuthoredEdgeTemplate,
    GraphBinding as AuthoredGraphBinding, GraphLayer as AuthoredGraphLayer,
    GraphTransformRule as AuthoredGraphTransformRule, NarrativeGraph, NarrativeNode,
    NarrativeNodeData, VariableAssignment as AuthoredVariableAssignment,
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
            Some(value) => serde_json::from_value(value.clone()).map_err(|error| {
                format!("storylet {} graph is invalid JSON: {error}", storylet.id)
            })?,
            None => NarrativeGraph::from_storylet(storylet),
        };
        graph.validate().map_err(|issues| {
            format!(
                "storylet {} graph is invalid: {}",
                storylet.id,
                issues.join("; ")
            )
        })?;
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
    for (storylet, graph, node_guids) in &sources {
        let storylet_index = *compiled
            .storylet_indices
            .get(&storylet.id)
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
            node_type_id: format!("{}:{}", storylet.id, node_type_name(&node.data)),
            category,
            kind,
            entity_path: node_entity_path(&node.data),
            input_pins: compile_input_pins(node, graph),
            output_pins: compile_output_pins(node),
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
    for edge in &graph.edges {
        let source = *node_guids.get(&edge.from_node).ok_or_else(|| {
            format!(
                "edge {} has unknown source node {}",
                edge.id, edge.from_node
            )
        })?;
        let target = *node_guids
            .get(&edge.to_node)
            .ok_or_else(|| format!("edge {} has unknown target node {}", edge.id, edge.to_node))?;
        edges.push(EdgeConnection {
            source_node_guid: source,
            source_pin_name: map_output_port(&edge.from_port),
            target_node_guid: target,
            target_pin_name: incoming_input_pin(graph, &edge.to_node, &edge.id),
        });
    }
    nodes.sort_by_key(|node| node.node_guid);
    edges.sort_by(|a, b| {
        a.source_node_guid
            .cmp(&b.source_node_guid)
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
    };
    asset.validate().map_err(|error| {
        format!(
            "compiled mission storylet {} is invalid: {error}",
            storylet.id
        )
    })?;
    Ok(asset)
}

/// Binds QBN/Teleport dispatch results to the stable Start GUIDs compiled above.
pub fn bind_storylet_start_nodes(
    compiled: &mut CompiledChronoDispatch,
    graphs: &CompiledNarrativeGraphs,
) -> Result<(), String> {
    for storylet in &mut compiled.dispatcher.storylets {
        let id = compiled
            .storylet_ids
            .get(storylet.storylet_id as usize)
            .ok_or_else(|| format!("storylet index {} has no source id", storylet.storylet_id))?;
        storylet.payload_node_guid = *graphs
            .storylet_start_nodes
            .get(id)
            .ok_or_else(|| format!("storylet {id} has no compiled graph entry"))?;
    }
    Ok(())
}

fn compile_node_kind(
    node: &NarrativeNode,
    storylet_index: u16,
    compiled: &CompiledChronoDispatch,
    entity_indices: &BTreeMap<String, u16>,
    storylet_start_nodes: &BTreeMap<String, u32>,
) -> Result<(NodeKind, NodeCategory), String> {
    let mapped = match &node.data {
        NarrativeNodeData::Start { .. }
        | NarrativeNodeData::Dialog { .. }
        | NarrativeNodeData::Image { .. } => (
            NodeKind::LogicAny {
                default_output_pin: String::new(),
            },
            NodeCategory::LogicGate,
        ),
        NarrativeNodeData::Choice { .. } => (
            NodeKind::Choice {
                default_output_pin: String::new(),
            },
            NodeCategory::LogicGate,
        ),
        NarrativeNodeData::Variable { assignments } => (
            NodeKind::VariableBatch {
                assignments: assignments
                    .iter()
                    .map(|assignment| convert_variable_assignment(assignment, storylet_index))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            NodeCategory::Mutator,
        ),
        NarrativeNodeData::QbnStoryletHub {
            deck_storylet_ids,
            hub_exit_storylet_id,
        } => (
            NodeKind::QbnHub {
                deck: deck_storylet_ids
                    .iter()
                    .map(|id| storylet_index_for(compiled, id))
                    .collect::<Result<Vec<_>, _>>()?,
                exit_storylet_id: if hub_exit_storylet_id.is_empty() {
                    None
                } else {
                    Some(storylet_index_for(compiled, hub_exit_storylet_id)?)
                },
            },
            NodeCategory::DynamicDispatcher,
        ),
        NarrativeNodeData::SalienceSelector {
            candidate_pool_ids,
            fallback_storylet_id,
        } => {
            let mut pool = Vec::with_capacity(candidate_pool_ids.len());
            for id in candidate_pool_ids {
                let index = storylet_index_for(compiled, id)?;
                let def = compiled.dispatcher.storylet(index).ok_or_else(|| {
                    format!(
                        "salience node {} references storylet {id} that is not in the dispatcher",
                        node.id
                    )
                })?;
                if def.required_arc_phases.is_empty() {
                    return Err(format!(
                        "salience node {} candidate {id} has no required_arc_phases (REQ-BB-05)",
                        node.id
                    ));
                }
                pool.push(index);
            }
            (
                NodeKind::SalienceSelector {
                    pool,
                    fallback_storylet_id: if fallback_storylet_id.trim().is_empty() {
                        None
                    } else {
                        Some(storylet_index_for(compiled, fallback_storylet_id)?)
                    },
                    fallback_output_pin: String::new(),
                },
                NodeCategory::DynamicDispatcher,
            )
        }
        NarrativeNodeData::WaypointConversation {
            topic_nodes,
            topic_edges,
            initial_topic_id,
            npc_goal_topic_id,
        } => {
            let nodes =
                serde_json::from_value::<Vec<WaypointTopicNode>>(Value::Array(topic_nodes.clone()))
                    .map_err(|error| {
                        format!("waypoint node data on {} is invalid: {error}", node.id)
                    })?;
            let edges =
                serde_json::from_value::<Vec<WaypointTopicEdge>>(Value::Array(topic_edges.clone()))
                    .map_err(|error| {
                        format!("waypoint edge data on {} is invalid: {error}", node.id)
                    })?;
            (
                NodeKind::WaypointConversation {
                    graph: WaypointTopicGraph { nodes, edges },
                    initial_topic_id: initial_topic_id.clone(),
                    npc_goal_topic_id: npc_goal_topic_id.clone(),
                },
                NodeCategory::DynamicDispatcher,
            )
        }
        NarrativeNodeData::Waiter { wait_event } => (
            NodeKind::Waiter {
                duration_ms: i64::MAX,
                required_tags: TagBitset::new(),
                wait_event: wait_event.clone(),
            },
            NodeCategory::Waiter,
        ),
        NarrativeNodeData::LogicAll { .. } => (NodeKind::LogicAll, NodeCategory::LogicGate),
        NarrativeNodeData::LogicAny { .. } => (
            NodeKind::LogicAny {
                default_output_pin: String::new(),
            },
            NodeCategory::LogicGate,
        ),
        NarrativeNodeData::Conditional { conditions } => (
            NodeKind::Conditional {
                condition: Condition {
                    rules: conditions
                        .iter()
                        .map(|condition| convert_condition(condition, storylet_index))
                        .collect::<Result<Vec<_>, _>>()?,
                    true_pin: "true".into(),
                    false_pin: "false".into(),
                },
            },
            NodeCategory::LogicGate,
        ),
        NarrativeNodeData::SceneSection {
            duration_ms,
            events,
        } => (
            NodeKind::SceneSection {
                section_duration_ms: i64::try_from(*duration_ms)
                    .map_err(|_| format!("scene section {} duration is out of range", node.id))?,
                events: events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| convert_timeline_event(event, index))
                    .collect::<Result<Vec<_>, _>>()?,
            },
            NodeCategory::ScopedContainer,
        ),
        NarrativeNodeData::MissionSubGraph { sub_graph_asset_id } => (
            NodeKind::MissionSubGraph {
                sub_graph_asset_id: sub_graph_asset_id.clone(),
            },
            NodeCategory::ScopedContainer,
        ),
        NarrativeNodeData::Poi { entity_path } => (
            NodeKind::Poi {
                poi_id: entity_path.clone(),
            },
            NodeCategory::LogicGate,
        ),
        NarrativeNodeData::Teleport {
            destination_storylet,
        } => {
            let destination_storylet_id = storylet_index_for(compiled, destination_storylet)?;
            let destination_node_guid = *storylet_start_nodes
                .get(destination_storylet)
                .ok_or_else(|| {
                    format!(
                        "teleport node {} targets missing storylet {}",
                        node.id, destination_storylet
                    )
                })?;
            (
                NodeKind::Teleport {
                    destination_storylet: Some(destination_storylet_id),
                    destination_node_guid: Some(destination_node_guid),
                },
                NodeCategory::Mutator,
            )
        }
        NarrativeNodeData::GoToLocation { location_id } => {
            let entity_index = *entity_indices
                .get(&format!("location:{location_id}"))
                .ok_or_else(|| {
                    format!(
                        "Go To Location node {} targets unknown location {location_id}",
                        node.id
                    )
                })?;
            let mut op = MutationOp::default();
            op.move_player = Some(entity_index);
            (NodeKind::Mutation { op }, NodeCategory::Mutator)
        }
        NarrativeNodeData::GraphTransform { rule } => (
            NodeKind::GraphTransform {
                rule: convert_graph_transform_rule(rule),
            },
            NodeCategory::Mutator,
        ),
        NarrativeNodeData::PassionMutation { op } => (
            NodeKind::PassionMutation {
                op: RuntimePassionMutation {
                    micro_deltas: op.micro_deltas.clone(),
                    active_ops: op.active_ops.clone(),
                },
            },
            NodeCategory::Mutator,
        ),
        NarrativeNodeData::End {
            jump_to_storylet, ..
        } => {
            let destination = jump_to_storylet
                .as_deref()
                .map(|id| storylet_index_for(compiled, id))
                .transpose()?;
            (
                NodeKind::End {
                    jump_to_storylet: destination,
                },
                NodeCategory::LogicGate,
            )
        }
    };
    Ok(mapped)
}
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

fn compile_input_pins(node: &NarrativeNode, graph: &NarrativeGraph) -> Vec<PinDefinition> {
    let mut edges: Vec<&crate::narrative_graph::NarrativeEdge> = graph
        .edges
        .iter()
        .filter(|edge| edge.to_node == node.id)
        .collect();
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    if edges.len() > 1 {
        return edges
            .iter()
            .enumerate()
            .map(|(index, _)| PinDefinition {
                pin_name: format!("In_{index}"),
                is_input: true,
            })
            .collect();
    }
    vec![PinDefinition {
        pin_name: DEFAULT_INPUT_PIN.into(),
        is_input: true,
    }]
}

fn compile_output_pins(node: &NarrativeNode) -> Vec<PinDefinition> {
    let names: Vec<String> = match &node.data {
        NarrativeNodeData::Choice { choices } => (0..choices.len())
            .map(|index| format!("choice_{index}"))
            .collect(),
        NarrativeNodeData::Conditional { .. } => vec!["true".into(), "false".into()],
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
        .filter(|edge| edge.to_node == target_node_id)
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
        .storylet_indices
        .get(id)
        .copied()
        .ok_or_else(|| format!("graph references unknown storylet {id}"))
}

fn convert_variable_assignment(
    assignment: &AuthoredVariableAssignment,
    storylet_id: u16,
) -> Result<RuntimeVariableAssignment, String> {
    let scope = match assignment.scope.trim().to_ascii_lowercase().as_str() {
        "global" => VariableScope::Global,
        "local" => VariableScope::Storylet(storylet_id),
        other => return Err(format!("unknown variable scope {other}")),
    };
    let operator = match assignment.op.trim().to_ascii_lowercase().as_str() {
        "set" => VariableOperator::Set,
        "add" => VariableOperator::Add,
        "subtract" | "sub" => VariableOperator::Subtract,
        "toggle" => VariableOperator::Toggle,
        other => return Err(format!("unknown variable operator {other}")),
    };
    let value = convert_variable_value(&assignment.value)?;
    Ok(RuntimeVariableAssignment {
        scope,
        name: assignment.quality.clone(),
        operator,
        value,
    })
}

fn convert_condition(
    condition: &crate::narrative_graph::ConditionRule,
    storylet_id: u16,
) -> Result<ConditionRule, String> {
    let scope = match condition.scope.trim().to_ascii_lowercase().as_str() {
        "global" => VariableScope::Global,
        "local" => VariableScope::Storylet(storylet_id),
        other => return Err(format!("unknown conditional variable scope {other}")),
    };
    let comparison = match condition.op.trim().to_ascii_lowercase().as_str() {
        "eq" | "==" => ComparisonOperator::Equal,
        "neq" | "!=" => ComparisonOperator::NotEqual,
        "gt" | ">" => ComparisonOperator::Greater,
        "gte" | ">=" => ComparisonOperator::GreaterOrEqual,
        "lt" | "<" => ComparisonOperator::Less,
        "lte" | "<=" => ComparisonOperator::LessOrEqual,
        other => return Err(format!("unknown condition operator {other}")),
    };
    Ok(ConditionRule {
        operator: ConditionOperator::Variable,
        comparison,
        value: convert_variable_value(&condition.value)?,
        variable_scope: scope,
        variable_name: condition.variable.clone(),
        ..ConditionRule::default()
    })
}

fn convert_variable_value(value: &Value) -> Result<VariableValue, String> {
    match value {
        Value::Bool(value) => Ok(VariableValue::Flag(*value)),
        Value::Number(value) => {
            let value = value
                .as_f64()
                .ok_or_else(|| "invalid number variable value".to_string())?
                as f32;
            if !value.is_finite() {
                return Err("numeric variable value exceeds f32 range".into());
            }
            Ok(VariableValue::Number(value))
        }
        Value::String(value) => Ok(VariableValue::Text(value.clone())),
        _ => Err("variables support only number, boolean, or string values".into()),
    }
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

fn node_entity_path(data: &NarrativeNodeData) -> String {
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
        NarrativeNodeData::Variable { .. } => "Variable",
        NarrativeNodeData::QbnStoryletHub { .. } => "QbnStoryletHub",
        NarrativeNodeData::SalienceSelector { .. } => "SalienceSelector",
        NarrativeNodeData::WaypointConversation { .. } => "WaypointConversation",
        NarrativeNodeData::Waiter { .. } => "Waiter",
        NarrativeNodeData::LogicAll { .. } => "LogicAll",
        NarrativeNodeData::LogicAny { .. } => "LogicAny",
        NarrativeNodeData::Conditional { .. } => "Conditional",
        NarrativeNodeData::SceneSection { .. } => "SceneSection",
        NarrativeNodeData::MissionSubGraph { .. } => "MissionSubGraph",
        NarrativeNodeData::Poi { .. } => "Poi",
        NarrativeNodeData::Image { .. } => "Image",
        NarrativeNodeData::Teleport { .. } => "Teleport",
        NarrativeNodeData::GoToLocation { .. } => "GoToLocation",
        NarrativeNodeData::GraphTransform { .. } => "GraphTransform",
        NarrativeNodeData::PassionMutation { .. } => "PassionMutation",
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
    use super::{bind_storylet_start_nodes, compile_narrative_graphs};
    use crate::chrono_adapter::compile_chrono_dispatch;
    use crate::contracts::StoryletRecord;
    use storylet_engine::chrono_runtime::{FlowGraphInstance, RuntimeContext};
    use storylet_engine::chrono_state::WorldStateTable;

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
            compile_chrono_dispatch(&[storylet.clone()], &[], &[], &[]).expect("dispatch compiles");
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
            local_variables: Vec::new(),
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
            }
        }
        let mut storylet: StoryletRecord = serde_json::from_value(serde_json::json!({
            "id": "scene-merge", "title": "Merge", "content": { "text_template": "Scene." },
            "effects": {}, "created_at": 1
        }))
        .unwrap();
        storylet.graph = Some(serde_json::to_value(authored).unwrap());

        let mut dispatch =
            compile_chrono_dispatch(std::slice::from_ref(&storylet), &[], &[], &[]).unwrap();
        let graphs = compile_narrative_graphs(
            "project-merge",
            std::slice::from_ref(&storylet),
            &dispatch,
            &BTreeMap::new(),
        )
        .unwrap();
        bind_storylet_start_nodes(&mut dispatch, &graphs).unwrap();
        let find_node = |kind: &str| {
            graphs
                .asset
                .nodes
                .iter()
                .find(|node| node.node_type_id.ends_with(kind))
                .map(|node| node.node_guid)
                .unwrap()
        };
        let choice_guid = find_node(":Choice");
        let shared_guid = graphs
            .asset
            .nodes
            .iter()
            .find(|node| {
                node.node_type_id.ends_with(":Dialog") && node.payload["nodeId"] == "shared"
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
        let poi_guid = find_node(":Poi");

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
        });
        parent.graph = Some(serde_json::to_value(graph).unwrap());

        let storylets = [parent, child];
        let mut dispatch = compile_chrono_dispatch(&storylets, &[], &[], &[]).unwrap();
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
}
