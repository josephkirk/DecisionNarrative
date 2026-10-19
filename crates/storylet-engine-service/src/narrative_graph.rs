//! Non-linear Narrative Node Graph model for Storylet content authoring.
//! Reference: https://github.com/shrimbly/node-banana and NarrativeFlow.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

use crate::contracts::{StoryletChoiceRecord, StoryletRecord};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodePort {
    pub id: String,
    pub name: String,
    pub port_type: String, // "input" | "output"
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DialogueItem {
    Line {
        id: String,
        speaker_role: String,
        #[serde(default = "default_target_role")]
        target_role: String,
        content: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        emotion_tag: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action_tag: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action_target: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        audio_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration: Option<f64>,
    },
    IdleAction {
        id: String,
        action_type: String, // "lookat" | "emote"
        actor_role: String,
        target: String, // "player" or role name
        #[serde(default, skip_serializing_if = "Option::is_none")]
        emote: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration: Option<f64>,
    },
}

fn default_target_role() -> String {
    "player".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialogueSection {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub items: Vec<DialogueItem>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NarrativeNodeData {
    Start {
        #[serde(rename = "text", alias = "label")]
        label: String,
    },
    Dialog {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speaker_role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        quip: Option<String>,
        #[serde(default)]
        sections: Vec<DialogueSection>,
    },
    Choice {
        #[serde(default)]
        choices: Vec<ChoiceOption>,
    },
    Variable {
        #[serde(default)]
        assignments: Vec<VariableAssignment>,
    },
    QbnStoryletHub {
        #[serde(default)]
        deck_storylet_ids: Vec<String>,
        #[serde(default)]
        hub_exit_storylet_id: String,
    },
    SalienceSelector {
        #[serde(default)]
        candidate_pool_ids: Vec<String>,
        /// Storylet used when every candidate is gated (spec §11.3).
        #[serde(default)]
        fallback_storylet_id: String,
    },
    WaypointConversation {
        #[serde(default)]
        topic_nodes: Vec<Value>,
        #[serde(default)]
        topic_edges: Vec<Value>,
        #[serde(default)]
        initial_topic_id: String,
        #[serde(default)]
        npc_goal_topic_id: String,
    },
    Waiter {
        #[serde(default)]
        wait_event: String,
    },
    LogicAll {},
    LogicAny {},
    Conditional {
        #[serde(default)]
        conditions: Vec<ConditionRule>,
    },
    SceneSection {
        #[serde(default)]
        duration_ms: u64,
        #[serde(default)]
        events: Vec<Value>,
    },
    MissionSubGraph {
        #[serde(default)]
        sub_graph_asset_id: String,
    },
    Poi {
        #[serde(default)]
        entity_path: String,
    },
    Image {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        image_url: Option<String>,
    },
    Teleport {
        destination_storylet: String,
    },
    GoToLocation {
        location_id: String,
    },
    GraphTransform {
        #[serde(default)]
        rule: GraphTransformRule,
    },
    PassionMutation {
        #[serde(default)]
        op: PassionMutation,
    },
    End {
        #[serde(default)]
        is_terminal: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        jump_to_storylet: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_verb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effects: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableAssignment {
    #[serde(default = "default_scope")]
    pub scope: String, // "global" | "local"
    pub quality: String,
    pub op: String, // "set" | "add"
    pub value: Value,
}

fn default_scope() -> String {
    "global".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConditionRule {
    #[serde(default = "default_scope")]
    pub scope: String, // "global" | "local"
    pub variable: String,
    pub op: String, // "eq" | "neq" | "gt" | "gte" | "lt" | "lte"
    pub value: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalVariableDefinition {
    pub name: String,
    #[serde(rename = "type", alias = "var_type")]
    pub var_type: String, // "boolean" | "number" | "string"
    pub default_value: Value,
}

// ChronoFlow v5 authoring model. Field names mirror `storylet_engine::chrono_state`
// so the graph adapter can compile these records into runtime rules. `GraphBinding`
// and `AttributeValue` use struct variants because internally tagged enums cannot
// round-trip newtype variants through JSON.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphLayer {
    #[default]
    Location,
    Character,
    Item,
    Fact,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AttributeValue {
    Int { value: i64 },
    Float { value: f64 },
    Bool { value: bool },
    String { value: String },
    Tag { value: u16 },
}

impl Default for AttributeValue {
    fn default() -> Self {
        Self::String {
            value: String::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeOp {
    #[default]
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphBinding {
    Literal { id: String },
    VertexPattern { index: usize },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AttributePredicate {
    #[serde(default)]
    pub key: String,
    #[serde(default)]
    pub op: AttributeOp,
    pub value: AttributeValue,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VertexPattern {
    #[serde(default)]
    pub layer: GraphLayer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertex_id: Option<String>,
    #[serde(default)]
    pub attributes: Vec<AttributePredicate>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EdgePattern {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relation_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<GraphBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<GraphBinding>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphPattern {
    #[serde(default)]
    pub vertices: Vec<VertexPattern>,
    #[serde(default)]
    pub edges: Vec<EdgePattern>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VertexTemplate {
    pub id: GraphBinding,
    #[serde(default)]
    pub layer: GraphLayer,
    #[serde(default)]
    pub attributes: BTreeMap<String, AttributeValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeTemplate {
    pub id: GraphBinding,
    pub source: GraphBinding,
    pub target: GraphBinding,
    #[serde(default)]
    pub relation_type: String,
    #[serde(default)]
    pub weight: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttributeAssignment {
    pub vertex: GraphBinding,
    #[serde(default)]
    pub key: String,
    pub value: AttributeValue,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphMutation {
    #[serde(default)]
    pub add_vertices: Vec<VertexTemplate>,
    #[serde(default)]
    pub remove_vertices: Vec<GraphBinding>,
    #[serde(default)]
    pub add_edges: Vec<EdgeTemplate>,
    #[serde(default)]
    pub remove_edges: Vec<GraphBinding>,
    #[serde(default)]
    pub set_attributes: Vec<AttributeAssignment>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GraphTransformRule {
    #[serde(default)]
    pub lhs: GraphPattern,
    #[serde(default)]
    pub rhs: GraphMutation,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PassionMutation {
    #[serde(default)]
    pub micro_deltas: Vec<(u16, f32)>,
    #[serde(default)]
    pub active_ops: Vec<(u16, bool)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NarrativeNode {
    pub id: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(flatten)]
    pub data: NarrativeNodeData,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NarrativeEdge {
    pub id: String,
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NarrativeGraph {
    #[serde(default)]
    pub local_variables: Vec<LocalVariableDefinition>,
    pub nodes: Vec<NarrativeNode>,
    pub edges: Vec<NarrativeEdge>,
}

fn node_type_name(data: &NarrativeNodeData) -> &'static str {
    match data {
        NarrativeNodeData::Start { .. } => "Start",
        NarrativeNodeData::Dialog { .. } => "Dialog",
        NarrativeNodeData::Choice { .. } => "Choice",
        NarrativeNodeData::Variable { .. } => "Variable",
        NarrativeNodeData::QbnStoryletHub { .. } => "QBN Storylet Hub",
        NarrativeNodeData::SalienceSelector { .. } => "Salience Selector",
        NarrativeNodeData::WaypointConversation { .. } => "Waypoint Conversation",
        NarrativeNodeData::Waiter { .. } => "Waiter",
        NarrativeNodeData::LogicAll { .. } => "Logic All",
        NarrativeNodeData::LogicAny { .. } => "Logic Any",
        NarrativeNodeData::SceneSection { .. } => "Scene Section",
        NarrativeNodeData::MissionSubGraph { .. } => "Mission Subgraph",
        NarrativeNodeData::Poi { .. } => "POI",
        NarrativeNodeData::Conditional { .. } => "Conditional",
        NarrativeNodeData::Image { .. } => "Image",
        NarrativeNodeData::Teleport { .. } => "Teleport",
        NarrativeNodeData::GoToLocation { .. } => "Go To Location",
        NarrativeNodeData::GraphTransform { .. } => "Graph Transform",
        NarrativeNodeData::PassionMutation { .. } => "Passion Mutation",
        NarrativeNodeData::End { .. } => "End",
    }
}

fn require_default_output(issues: &mut Vec<String>, title: &str, edges: &[&NarrativeEdge]) {
    if edges.len() != 1 || edges.first().is_some_and(|edge| edge.from_port != "output") {
        issues.push(format!(
            "Node {title} must have exactly one outgoing edge from its output port."
        ));
    }
}

fn is_safe_image_url(image_url: &str) -> bool {
    let value = image_url.trim();
    if value.is_empty()
        || value.starts_with("//")
        || value.contains('\\')
        || value.chars().any(char::is_control)
    {
        return false;
    }
    let first_segment = value.split('/').next().unwrap_or_default();
    match first_segment.split_once(':') {
        Some((scheme, _)) => {
            matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https")
                && value[scheme.len() + 1..].starts_with("//")
        }
        None => true,
    }
}

fn ordered_attribute_op(op: &AttributeOp) -> bool {
    matches!(
        op,
        AttributeOp::Gt | AttributeOp::Gte | AttributeOp::Lt | AttributeOp::Lte
    )
}

fn is_numeric_attribute_value(value: &AttributeValue) -> bool {
    matches!(
        value,
        AttributeValue::Int { .. } | AttributeValue::Float { .. }
    )
}

fn is_finite_attribute_value(value: &AttributeValue) -> bool {
    match value {
        AttributeValue::Float { value } => value.is_finite(),
        _ => true,
    }
}

fn binding_resolves(binding: &GraphBinding, lhs: &GraphPattern) -> bool {
    match binding {
        GraphBinding::VertexPattern { index } => *index < lhs.vertices.len(),
        GraphBinding::Literal { .. } => true,
    }
}

fn graph_mutation_is_empty(mutation: &GraphMutation) -> bool {
    mutation.add_vertices.is_empty()
        && mutation.remove_vertices.is_empty()
        && mutation.add_edges.is_empty()
        && mutation.remove_edges.is_empty()
        && mutation.set_attributes.is_empty()
}

fn rule_bindings(rule: &GraphTransformRule) -> Vec<&GraphBinding> {
    let mut bindings = Vec::new();
    for pattern in &rule.lhs.edges {
        bindings.extend(
            [pattern.source.as_ref(), pattern.target.as_ref()]
                .into_iter()
                .flatten(),
        );
    }
    for template in &rule.rhs.add_vertices {
        bindings.push(&template.id);
    }
    for template in &rule.rhs.add_edges {
        bindings.extend([&template.id, &template.source, &template.target]);
    }
    bindings.extend(
        rule.rhs
            .remove_vertices
            .iter()
            .chain(rule.rhs.remove_edges.iter()),
    );
    for assignment in &rule.rhs.set_attributes {
        bindings.push(&assignment.vertex);
    }
    bindings
}

fn rule_attribute_values(rule: &GraphTransformRule) -> Vec<&AttributeValue> {
    let mut values = Vec::new();
    for vertex in &rule.lhs.vertices {
        values.extend(vertex.attributes.iter().map(|predicate| &predicate.value));
    }
    for template in &rule.rhs.add_vertices {
        values.extend(template.attributes.values());
    }
    for assignment in &rule.rhs.set_attributes {
        values.push(&assignment.value);
    }
    values
}

impl NarrativeGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        use std::collections::{BTreeSet, VecDeque};
        let starts: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| matches!(&node.data, NarrativeNodeData::Start { .. }))
            .collect();
        let ends: Vec<_> = self
            .nodes
            .iter()
            .filter(|node| matches!(&node.data, NarrativeNodeData::End { .. }))
            .collect();
        let ids: BTreeSet<String> = self.nodes.iter().map(|node| node.id.clone()).collect();
        let mut issues = Vec::new();
        if starts.len() != 1 {
            issues.push("Graph must have exactly one Start node.".to_string());
        }
        if ends.len() != 1 {
            issues.push("Graph must have exactly one End node.".to_string());
        }
        if ids.len() != self.nodes.len() {
            issues.push("Node IDs must be unique.".to_string());
        }
        if starts.len() != 1 || ends.len() != 1 || ids.len() != self.nodes.len() {
            return Err(issues);
        }
        let start = starts[0].id.as_str();
        let end = ends[0].id.as_str();
        let mut outgoing: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut incoming: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut edge_ids = BTreeSet::new();
        for edge in &self.edges {
            if !edge_ids.insert(&edge.id) {
                issues.push("Edge IDs must be unique.".into());
            }
            if !ids.contains(&edge.from_node) || !ids.contains(&edge.to_node) {
                issues.push(format!("Edge {} references a missing node.", edge.id));
                continue;
            }
            if edge.to_port != "input" {
                issues.push(format!("Edge {} must target an input port.", edge.id));
            }
            outgoing
                .entry(edge.from_node.clone())
                .or_default()
                .push(edge.to_node.clone());
            incoming
                .entry(edge.to_node.clone())
                .or_default()
                .push(edge.from_node.clone());
        }
        if incoming.get(start).is_some_and(|nodes| !nodes.is_empty())
            || outgoing.get(start).map_or(0, Vec::len) != 1
        {
            issues.push("Start must have no incoming edges and exactly one outgoing edge.".into());
        }
        if outgoing.get(end).is_some_and(|nodes| !nodes.is_empty()) {
            issues.push("End cannot have outgoing edges.".into());
        }
        let walk = |seed: &str, links: &BTreeMap<String, Vec<String>>| {
            let mut seen = BTreeSet::new();
            let mut queue = VecDeque::from([seed.to_string()]);
            while let Some(id) = queue.pop_front() {
                if seen.insert(id.clone()) {
                    if let Some(next) = links.get(&id) {
                        queue.extend(next.iter().cloned());
                    }
                }
            }
            seen
        };
        let reachable = walk(start, &outgoing);
        let reaches_end = walk(end, &incoming);
        if self.nodes.iter().any(|node| !reachable.contains(&node.id)) {
            issues.push("Every node must be reachable from Start.".into());
        }
        if self
            .nodes
            .iter()
            .any(|node| !reaches_end.contains(&node.id))
        {
            issues.push("Every node must have a path to End.".into());
        }
        for node in &self.nodes {
            let edges: Vec<_> = self
                .edges
                .iter()
                .filter(|edge| edge.from_node == node.id)
                .collect();
            if let NarrativeNodeData::Image { image_url } = &node.data {
                if image_url
                    .as_deref()
                    .is_none_or(|url| !is_safe_image_url(url))
                {
                    issues.push(format!(
                        "Image node {} must have a safe HTTP(S) URL or project-relative path.",
                        node.title
                    ));
                }
            }
            match &node.data {
                NarrativeNodeData::GoToLocation { location_id } => {
                    if location_id.trim().is_empty() {
                        issues.push("Go To Location nodes must select a location.".into());
                    }
                    let has_unsupported_incoming_edge = self
                        .edges
                        .iter()
                        .filter(|edge| edge.to_node == node.id)
                        .any(|edge| {
                            !self.nodes.iter().any(|source| {
                                source.id == edge.from_node
                                    && matches!(&source.data, NarrativeNodeData::Choice { .. })
                            })
                        });
                    if has_unsupported_incoming_edge {
                        issues.push(format!("Go To Location node {} may only be targeted directly by a Choice branch.", node.title));
                    }
                    if edges.len() != 1
                        || edges.first().is_some_and(|edge| edge.from_port != "output")
                    {
                        issues.push(format!("Go To Location node {} must have exactly one outgoing edge from its output port.", node.title));
                    }
                }
                NarrativeNodeData::QbnStoryletHub {
                    deck_storylet_ids,
                    hub_exit_storylet_id,
                } => {
                    if deck_storylet_ids.is_empty() {
                        issues.push(format!(
                            "QBN hub {} must have a nonempty storylet deck.",
                            node.title
                        ));
                    }
                    if hub_exit_storylet_id.trim().is_empty()
                        || !deck_storylet_ids.contains(hub_exit_storylet_id)
                    {
                        issues.push(format!(
                            "QBN hub {} must select its exit storylet from its deck.",
                            node.title
                        ));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::SalienceSelector {
                    candidate_pool_ids,
                    fallback_storylet_id,
                } => {
                    if candidate_pool_ids.is_empty() {
                        issues.push(format!(
                            "Salience selector {} must have a nonempty candidate pool.",
                            node.title
                        ));
                    }
                    if !fallback_storylet_id.trim().is_empty()
                        && !candidate_pool_ids
                            .iter()
                            .any(|id| id == fallback_storylet_id)
                    {
                        issues.push(format!(
                            "Salience selector {} fallback {} is not in its candidate pool.",
                            node.title, fallback_storylet_id
                        ));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::WaypointConversation {
                    topic_nodes,
                    topic_edges,
                    initial_topic_id,
                    npc_goal_topic_id,
                } => {
                    let topic_ids: BTreeSet<&str> = topic_nodes
                        .iter()
                        .filter_map(|topic| topic.get("topic_id").and_then(Value::as_str))
                        .collect();
                    let exit_pins: Vec<&str> = topic_nodes
                        .iter()
                        .filter(|topic| {
                            topic.get("triggers_scene_exit").and_then(Value::as_bool) == Some(true)
                        })
                        .filter_map(|topic| topic.get("exit_output_pin").and_then(Value::as_str))
                        .collect();
                    if !topic_ids.contains(initial_topic_id.as_str()) {
                        issues.push(format!(
                            "Waypoint {} must have a valid initial topic.",
                            node.title
                        ));
                    }
                    if !topic_ids.contains(npc_goal_topic_id.as_str()) {
                        issues.push(format!(
                            "Waypoint {} must have a valid NPC goal topic.",
                            node.title
                        ));
                    }
                    let unique_pins: BTreeSet<_> = exit_pins.iter().copied().collect();
                    if exit_pins.is_empty()
                        || unique_pins.len() != exit_pins.len()
                        || topic_nodes.iter().any(|topic| {
                            topic.get("triggers_scene_exit").and_then(Value::as_bool) == Some(true)
                                && topic
                                    .get("exit_output_pin")
                                    .and_then(Value::as_str)
                                    .is_none_or(str::is_empty)
                        })
                    {
                        issues.push(format!(
                            "Waypoint {} must define unique output pins for its scene-exit topics.",
                            node.title
                        ));
                    }
                    for edge in topic_edges {
                        let from = edge.get("from_topic_id").and_then(Value::as_str);
                        let to = edge.get("to_topic_id").and_then(Value::as_str);
                        let cost = edge.get("base_cost").and_then(Value::as_f64);
                        if from.is_none_or(|id| !topic_ids.contains(id))
                            || to.is_none_or(|id| !topic_ids.contains(id))
                            || cost.is_none_or(|value| !value.is_finite() || value < 0.0)
                        {
                            issues.push(format!(
                                "Waypoint {} has an invalid topic transition.",
                                node.title
                            ));
                            break;
                        }
                    }
                    for pin in &unique_pins {
                        if edges.iter().filter(|edge| edge.from_port == *pin).count() != 1 {
                            issues.push(format!(
                                "Waypoint exit {pin} on {} must have exactly one outgoing edge.",
                                node.title
                            ));
                        }
                    }
                    if edges
                        .iter()
                        .any(|edge| !unique_pins.contains(edge.from_port.as_str()))
                    {
                        issues.push(format!(
                            "Waypoint {} has an edge from an unknown exit pin.",
                            node.title
                        ));
                    }
                }
                NarrativeNodeData::Waiter { wait_event } => {
                    if wait_event.trim().is_empty() {
                        issues.push(format!("Waiter {} must name a gameplay event.", node.title));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::LogicAll {} | NarrativeNodeData::LogicAny {} => {
                    if incoming.get(&node.id).map_or(0, Vec::len) < 2 {
                        issues.push(format!(
                            "{} node {} must have at least two incoming branches.",
                            node_type_name(&node.data),
                            node.title
                        ));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::Conditional { conditions } => {
                    if conditions.is_empty() {
                        issues.push(format!(
                            "Conditional node {} must have at least one condition.",
                            node.title
                        ));
                    }
                    let expected_ports = ["true", "false"];
                    for port in expected_ports {
                        if edges.iter().filter(|edge| edge.from_port == port).count() != 1 {
                            issues.push(format!(
                                "Conditional node {} must have exactly one '{port}' branch.",
                                node.title
                            ));
                        }
                    }
                    if edges
                        .iter()
                        .any(|edge| !expected_ports.contains(&edge.from_port.as_str()))
                    {
                        issues.push(format!(
                            "Conditional node {} has an edge from an unknown branch.",
                            node.title
                        ));
                    }
                }
                NarrativeNodeData::SceneSection {
                    duration_ms,
                    events,
                } => {
                    for event in events {
                        let start = event.get("start_time_ms").and_then(Value::as_u64);
                        let duration = event.get("duration_ms").and_then(Value::as_u64);
                        if start.zip(duration).is_none_or(|(start, duration)| {
                            start
                                .checked_add(duration)
                                .is_none_or(|end| end > *duration_ms)
                        }) {
                            issues.push(format!(
                                "Scene Section {} has a timeline event outside its duration.",
                                node.title
                            ));
                            break;
                        }
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::MissionSubGraph { sub_graph_asset_id } => {
                    if sub_graph_asset_id.trim().is_empty() {
                        issues.push(format!(
                            "Mission {} must select a subgraph asset.",
                            node.title
                        ));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::Poi { entity_path } => {
                    if entity_path.trim().is_empty() {
                        issues.push(format!("POI {} must bind an entity path.", node.title));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::Choice { choices } => {
                    let expected_ports: BTreeSet<String> = (0..choices.len())
                        .map(|index| format!("choice_{index}"))
                        .collect();
                    for index in 0..choices.len() {
                        let port = format!("choice_{index}");
                        if edges.iter().filter(|edge| edge.from_port == port).count() != 1 {
                            issues.push(format!(
                                "Choice {} on {} must have exactly one outgoing branch.",
                                index + 1,
                                node.title
                            ));
                        }
                    }
                    if edges
                        .iter()
                        .any(|edge| !expected_ports.contains(&edge.from_port))
                    {
                        issues.push(format!(
                            "Choice node {} has an edge from an unknown output port.",
                            node.title
                        ));
                    }
                }
                NarrativeNodeData::End { .. } => {
                    if !edges.is_empty() {
                        issues.push("End cannot have outgoing edges.".into());
                    }
                }
                NarrativeNodeData::GraphTransform { rule } => {
                    if rule.lhs.vertices.is_empty() && rule.lhs.edges.is_empty() {
                        issues.push(format!(
                            "Graph Transform {} must match at least one graph pattern.",
                            node.title
                        ));
                    }
                    if graph_mutation_is_empty(&rule.rhs) {
                        issues.push(format!(
                            "Graph Transform {} must apply at least one mutation.",
                            node.title
                        ));
                    }
                    if rule_attribute_values(rule)
                        .into_iter()
                        .any(|value| !is_finite_attribute_value(value))
                    {
                        issues.push(format!(
                            "Graph Transform {} has a non-finite attribute value.",
                            node.title
                        ));
                    }
                    if rule
                        .lhs
                        .vertices
                        .iter()
                        .flat_map(|vertex| &vertex.attributes)
                        .any(|predicate| {
                            ordered_attribute_op(&predicate.op)
                                && !is_numeric_attribute_value(&predicate.value)
                        })
                    {
                        issues.push(format!(
                            "Graph Transform {} has an ordered attribute predicate without a numeric value.",
                            node.title
                        ));
                    }
                    if rule_bindings(rule)
                        .into_iter()
                        .any(|binding| !binding_resolves(binding, &rule.lhs))
                    {
                        issues.push(format!(
                            "Graph Transform {} binds a vertex pattern outside its left-hand side.",
                            node.title
                        ));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::PassionMutation { op } => {
                    if op.micro_deltas.is_empty() && op.active_ops.is_empty() {
                        issues.push(format!(
                            "Passion Mutation {} must apply at least one typed delta or active-pool operation.",
                            node.title
                        ));
                    }
                    if op.micro_deltas.iter().any(|(_, delta)| !delta.is_finite()) {
                        issues.push(format!(
                            "Passion Mutation {} has a non-finite passion delta.",
                            node.title
                        ));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                _ => {
                    if edges.len() != 1
                        || edges.first().is_some_and(|edge| edge.from_port != "output")
                    {
                        issues.push(format!(
                            "{} node {} must have exactly one outgoing edge from its output port.",
                            node_type_name(&node.data),
                            node.title
                        ));
                    }
                }
            }
        }
        if issues.is_empty() {
            Ok(())
        } else {
            Err(issues)
        }
    }

    /// Build a default narrative graph from storylet content and choices
    pub fn from_storylet(st: &StoryletRecord) -> Self {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // 1. Start node
        nodes.push(NarrativeNode {
            id: "node_start".into(),
            title: "Start".into(),
            x: 60,
            y: 120,
            width: 160,
            height: 80,
            data: NarrativeNodeData::Start {
                label: "Begin Scene".into(),
            },
        });

        // 2. Dialog node with opening prose
        let prose = st
            .content
            .scene_opening
            .as_deref()
            .unwrap_or(&st.content.text_template);
        let default_speaker = st
            .content
            .speaker_id
            .clone()
            .unwrap_or_else(|| "Narrator".into());

        let opening_item = DialogueItem::Line {
            id: "line_0".into(),
            speaker_role: default_speaker.clone(),
            target_role: "player".into(),
            content: prose.to_string(),
            emotion_tag: Some("neutral".into()),
            action_tag: None,
            action_target: None,
            audio_path: None,
            duration: None,
        };

        let initial_section = DialogueSection {
            id: "sec_main".into(),
            title: "Main Dialogue".into(),
            items: vec![opening_item],
        };

        nodes.push(NarrativeNode {
            id: "node_dialog_1".into(),
            title: "Dialog".into(),
            x: 250,
            y: 120,
            width: 280,
            height: 200,
            data: NarrativeNodeData::Dialog {
                speaker_role: Some(default_speaker),
                text: Some(prose.to_string()),
                quip: None,
                sections: vec![initial_section],
            },
        });

        edges.push(NarrativeEdge {
            id: "edge_start_dialog".into(),
            from_node: "node_start".into(),
            from_port: "output".into(),
            to_node: "node_dialog_1".into(),
            to_port: "input".into(),
        });

        // 3. Choice node if choices exist
        if !st.choices.is_empty() {
            let choice_options = st
                .choices
                .iter()
                .enumerate()
                .map(|(idx, c)| ChoiceOption {
                    id: format!("choice_{idx}"),
                    text: c.label.clone(),
                    system_verb: c.system_verb.clone(),
                    effects: Some(c.effects.clone()),
                })
                .collect();

            nodes.push(NarrativeNode {
                id: "node_choice_1".into(),
                title: "Choice".into(),
                x: 620,
                y: 120,
                width: 240,
                height: (st.choices.len() as i32 * 36 + 80).max(120),
                data: NarrativeNodeData::Choice {
                    choices: choice_options,
                },
            });

            edges.push(NarrativeEdge {
                id: "edge_dialog_choice".into(),
                from_node: "node_dialog_1".into(),
                from_port: "output".into(),
                to_node: "node_choice_1".into(),
                to_port: "input".into(),
            });

            // Route every choice to its location action, teleport destination, or shared End node.
            for (idx, c) in st.choices.iter().enumerate() {
                if let Some(location_id) = c.effects.get("$move_player").and_then(Value::as_str) {
                    let location_node_id = format!("node_location_{}", idx + 1);
                    nodes.push(NarrativeNode {
                        id: location_node_id.clone(),
                        title: "Go To Location".into(),
                        x: 920,
                        y: idx as i32 * 140 + 80,
                        width: 220,
                        height: 100,
                        data: NarrativeNodeData::GoToLocation {
                            location_id: location_id.into(),
                        },
                    });
                    edges.push(NarrativeEdge {
                        id: format!("edge_choice_{idx}_location"),
                        from_node: "node_choice_1".into(),
                        from_port: format!("choice_{idx}"),
                        to_node: location_node_id.clone(),
                        to_port: "input".into(),
                    });
                    edges.push(NarrativeEdge {
                        id: format!("edge_location_{idx}_end"),
                        from_node: location_node_id,
                        from_port: "output".into(),
                        to_node: "node_end".into(),
                        to_port: "input".into(),
                    });
                } else if let Some(ref divert) = c.divert_target {
                    let teleport_id = format!("node_teleport_{idx}");
                    nodes.push(NarrativeNode {
                        id: teleport_id.clone(),
                        title: "Teleport".into(),
                        x: 920,
                        y: idx as i32 * 140 + 80,
                        width: 200,
                        height: 100,
                        data: NarrativeNodeData::Teleport {
                            destination_storylet: divert.clone(),
                        },
                    });
                    edges.push(NarrativeEdge {
                        id: format!("edge_choice_{idx}_teleport"),
                        from_node: "node_choice_1".into(),
                        from_port: format!("choice_{idx}"),
                        to_node: teleport_id.clone(),
                        to_port: "input".into(),
                    });
                    edges.push(NarrativeEdge {
                        id: format!("edge_teleport_{idx}_end"),
                        from_node: teleport_id,
                        from_port: "output".into(),
                        to_node: "node_end".into(),
                        to_port: "input".into(),
                    });
                } else {
                    edges.push(NarrativeEdge {
                        id: format!("edge_choice_{idx}_end"),
                        from_node: "node_choice_1".into(),
                        from_port: format!("choice_{idx}"),
                        to_node: "node_end".into(),
                        to_port: "input".into(),
                    });
                }
            }
        }

        nodes.push(NarrativeNode {
            id: "node_end".into(),
            title: "End".into(),
            x: 560,
            y: 120,
            width: 160,
            height: 90,
            data: NarrativeNodeData::End {
                is_terminal: st.ending,
                jump_to_storylet: None,
            },
        });
        if st.choices.is_empty() {
            edges.push(NarrativeEdge {
                id: "edge_dialog_end".into(),
                from_node: "node_dialog_1".into(),
                from_port: "output".into(),
                to_node: "node_end".into(),
                to_port: "input".into(),
            });
        }

        Self {
            local_variables: Vec::new(),
            nodes,
            edges,
        }
    }

    /// Compile this node graph back into storylet prose, choices, and effects
    pub fn compile_to_storylet_content(&self, st: &mut StoryletRecord) {
        let mut prose_parts = Vec::new();
        let mut choices = Vec::new();
        let mut is_terminal = false;
        let mut compiled_effects = match st.effects.as_object() {
            Some(obj) => obj.clone(),
            None => serde_json::Map::new(),
        };

        for node in &self.nodes {
            match &node.data {
                NarrativeNodeData::Dialog { text, sections, .. } => {
                    // If structured sections exist, compile dialogue lines
                    if !sections.is_empty() {
                        for sec in sections {
                            for item in &sec.items {
                                match item {
                                    DialogueItem::Line {
                                        speaker_role,
                                        content,
                                        emotion_tag,
                                        ..
                                    } => {
                                        if !content.trim().is_empty() {
                                            if let Some(emo) = emotion_tag {
                                                prose_parts.push(format!(
                                                    "{speaker_role} [{emo}]: {content}"
                                                ));
                                            } else {
                                                prose_parts
                                                    .push(format!("{speaker_role}: {content}"));
                                            }
                                        }
                                    }
                                    DialogueItem::IdleAction {
                                        actor_role,
                                        action_type,
                                        target,
                                        emote,
                                        ..
                                    } => {
                                        if action_type == "emote" {
                                            if let Some(em) = emote {
                                                prose_parts.push(format!(
                                                    "({actor_role} {em} at {target})"
                                                ));
                                            }
                                        } else {
                                            prose_parts
                                                .push(format!("({actor_role} looks at {target})"));
                                        }
                                    }
                                }
                            }
                        }
                    } else if let Some(t) = text {
                        if !t.trim().is_empty() {
                            prose_parts.push(t.trim().to_string());
                        }
                    }
                }
                NarrativeNodeData::Choice { choices: opts } => {
                    for (idx, opt) in opts.iter().enumerate() {
                        let port_id = format!("choice_{idx}");
                        let target_node_id = self
                            .edges
                            .iter()
                            .find(|e| {
                                e.from_node == node.id
                                    && (e.from_port == port_id || e.from_port == "output")
                            })
                            .map(|e| e.to_node.clone());

                        let target_node =
                            target_node_id.and_then(|tid| self.nodes.iter().find(|n| n.id == tid));
                        let divert_target = target_node.and_then(|node| match &node.data {
                            NarrativeNodeData::Teleport {
                                destination_storylet,
                            } => Some(destination_storylet.clone()),
                            NarrativeNodeData::End {
                                jump_to_storylet, ..
                            } => jump_to_storylet.clone(),
                            _ => None,
                        });
                        let mut choice_effects = opt
                            .effects
                            .as_ref()
                            .and_then(Value::as_object)
                            .cloned()
                            .map(Value::Object)
                            .unwrap_or_else(|| json!({}));
                        choice_effects
                            .as_object_mut()
                            .expect("choice effects are an object")
                            .remove("$move_player");
                        if let Some(NarrativeNode {
                            data: NarrativeNodeData::GoToLocation { location_id },
                            ..
                        }) = target_node
                        {
                            choice_effects
                                .as_object_mut()
                                .expect("choice effects are an object")
                                .insert("$move_player".into(), Value::String(location_id.clone()));
                        }

                        choices.push(StoryletChoiceRecord {
                            choice_id: format!("choice_{}", idx + 1),
                            label: opt.text.clone(),
                            system_verb: opt.system_verb.clone().or_else(|| Some("ACT".into())),
                            conditions: None,
                            effects: choice_effects,
                            divert_target,
                        });
                    }
                }
                NarrativeNodeData::Variable { assignments } => {
                    for a in assignments {
                        if a.scope == "global" {
                            compiled_effects.insert(a.quality.clone(), a.value.clone());
                        }
                    }
                }
                NarrativeNodeData::End {
                    is_terminal: term,
                    jump_to_storylet,
                } => {
                    if *term {
                        is_terminal = true;
                    }
                    if let Some(dest) = jump_to_storylet {
                        if !dest.trim().is_empty() {
                            is_terminal = true;
                        }
                    }
                }
                _ => {}
            }
        }

        if !prose_parts.is_empty() {
            let full_prose = prose_parts.join(
                "

",
            );
            st.content.scene_opening = Some(full_prose.clone());
            st.content.text_template = full_prose;
        }

        st.choices = choices;
        st.ending = is_terminal;

        st.effects = Value::Object(compiled_effects);
    }
}

#[cfg(test)]
mod local_variable_serde_tests {
    use crate::contracts::StoryletRecord;
    use serde_json::json;

    #[test]
    fn storylet_local_variables_round_trip_typescript_schema() {
        let payload = json!({
            "id": "storylet-local-var",
            "title": "Local variable test",
            "content": { "text_template": "" },
            "created_at": 1,
            "local_variables": [{
                "name": "visited",
                "type": "boolean",
                "default_value": false
            }]
        });

        let storylet: StoryletRecord = serde_json::from_value(payload)
            .expect("TypeScript local-variable records should deserialize");
        assert_eq!(
            storylet.local_variables.as_ref().unwrap()[0].var_type,
            "boolean"
        );

        let serialized = serde_json::to_value(storylet).unwrap();
        assert_eq!(
            serialized["local_variables"][0],
            json!({
                "name": "visited",
                "type": "boolean",
                "default_value": false
            })
        );
    }
    #[test]
    fn storylet_local_variables_accept_legacy_rust_field_name() {
        let payload = json!({
            "id": "legacy-local-var",
            "title": "Legacy local variable",
            "content": { "text_template": "" },
            "created_at": 1,
            "local_variables": [{
                "name": "visited",
                "var_type": "boolean",
                "default_value": false
            }]
        });

        let storylet: StoryletRecord = serde_json::from_value(payload)
            .expect("legacy local-variable records should deserialize");
        let serialized = serde_json::to_value(storylet).unwrap();
        assert_eq!(serialized["local_variables"][0]["type"], "boolean");
        assert!(serialized["local_variables"][0].get("var_type").is_none());
    }
}

#[cfg(test)]
mod graph_wire_contract_tests {
    use super::NarrativeGraph;
    use serde_json::json;

    fn studio_graph() -> serde_json::Value {
        json!({
            "nodes": [
                { "id": "start", "title": "Start", "type": "start", "x": 10, "y": 20, "width": 160, "height": 80, "text": "Begin" },
                { "id": "dialog", "title": "Dialog", "type": "dialog", "x": 200, "y": 20, "width": 280, "height": 160, "text": "The scene", "sections": [] },
                { "id": "end", "title": "End", "type": "end", "x": 520, "y": 20, "width": 160, "height": 90, "is_terminal": false }
            ],
            "edges": [
                { "id": "start-dialog", "from_node": "start", "from_port": "output", "to_node": "dialog", "to_port": "input" },
                { "id": "dialog-end", "from_node": "dialog", "from_port": "output", "to_node": "end", "to_port": "input" }
            ],
            "local_variables": []
        })
    }

    #[test]
    fn studio_graph_wire_format_deserializes_and_validates() {
        let graph: NarrativeGraph = serde_json::from_value(studio_graph())
            .expect("Studio graph wire format must deserialize");
        graph
            .validate()
            .expect("Start-to-End graph should validate");
        let serialized = serde_json::to_value(graph).unwrap();
        assert_eq!(serialized["nodes"][0]["type"], "start");
        assert_eq!(serialized["nodes"][0]["text"], "Begin");
        assert_eq!(serialized["nodes"][0]["x"], 10);
        assert!(serialized["nodes"][0].get("position").is_none());
    }

    #[test]
    fn image_url_validation_matches_typescript() {
        for (url, expected) in [
            ("assets/a:b.png", true),
            ("HTTPS://x/y", true),
            ("https:x", false),
            ("\u{7f}x", false),
            ("javascript:alert(1)", false),
            ("//example.com/image.png", false),
            ("", false),
        ] {
            assert_eq!(super::is_safe_image_url(url), expected, "{url:?}");
        }

        for url in [
            "",
            "javascript:alert(1)",
            "//example.com/image.png",
            "https:x",
            "\u{7f}x",
        ] {
            let mut payload = studio_graph();
            payload["nodes"][1]["type"] = json!("image");
            payload["nodes"][1]["image_url"] = json!(url);
            let graph: NarrativeGraph = serde_json::from_value(payload).unwrap();
            let issues = graph.validate().unwrap_err().join(" ");
            assert!(issues.contains("safe HTTP(S) URL or project-relative path"));
        }

        let mut payload = studio_graph();
        payload["nodes"][1]["type"] = json!("image");
        payload["nodes"][1]["image_url"] = json!("assets/a:b.png");
        let graph: NarrativeGraph = serde_json::from_value(payload).unwrap();
        graph
            .validate()
            .expect("project-relative image paths are allowed");
    }
    #[test]
    fn graph_rejects_multiple_edges_from_one_output_port() {
        let mut payload = studio_graph();
        let duplicate = payload["edges"][1].clone();
        payload["edges"].as_array_mut().unwrap().push(duplicate);
        payload["edges"][2]["id"] = json!("dialog-end-duplicate");
        let graph: NarrativeGraph = serde_json::from_value(payload).unwrap();
        let issues = graph.validate().unwrap_err().join(" ");
        assert!(issues.contains(
            "Dialog node Dialog must have exactly one outgoing edge from its output port."
        ));
    }

    #[test]
    fn go_to_location_choice_compiles_and_round_trips() {
        use super::NarrativeNodeData;
        use crate::contracts::StoryletRecord;
        use serde_json::json;

        let mut payload = studio_graph();
        payload["nodes"].as_array_mut().unwrap().insert(2, json!({
            "id": "choice", "title": "Choice", "type": "choice", "x": 500, "y": 20, "width": 220, "height": 120,
            "choices": [{ "id": "walk", "text": "Walk to market" }]
        }));
        payload["nodes"].as_array_mut().unwrap().insert(3, json!({
            "id": "market", "title": "Go To Location", "type": "go_to_location", "x": 760, "y": 20, "width": 220, "height": 100, "location_id": "market"
        }));
        payload["edges"] = json!([
            { "id": "start-dialog", "from_node": "start", "from_port": "output", "to_node": "dialog", "to_port": "input" },
            { "id": "dialog-choice", "from_node": "dialog", "from_port": "output", "to_node": "choice", "to_port": "input" },
            { "id": "choice-market", "from_node": "choice", "from_port": "choice_0", "to_node": "market", "to_port": "input" },
            { "id": "market-end", "from_node": "market", "from_port": "output", "to_node": "end", "to_port": "input" }
        ]);
        let graph: NarrativeGraph = serde_json::from_value(payload).unwrap();
        graph.validate().expect("movement graph should validate");
        let mut storylet: StoryletRecord = serde_json::from_value(json!({
            "id": "walk", "title": "Walk", "content": { "text_template": "The road opens." }, "created_at": 0
        })).unwrap();
        graph.compile_to_storylet_content(&mut storylet);
        assert_eq!(storylet.choices[0].effects["$move_player"], "market");

        let rebuilt = NarrativeGraph::from_storylet(&storylet);
        assert!(rebuilt.nodes.iter().any(|node| matches!(
            &node.data, NarrativeNodeData::GoToLocation { location_id } if location_id == "market"
        )));
    }

    #[test]
    fn go_to_location_rejects_dialogue_incoming_edge() {
        let mut payload = studio_graph();
        payload["nodes"].as_array_mut().unwrap().insert(2, json!({
            "id": "market", "title": "Market", "type": "go_to_location", "x": 350, "y": 20, "width": 220, "height": 100, "location_id": "market"
        }));
        payload["edges"] = json!([
            { "id": "start-dialog", "from_node": "start", "from_port": "output", "to_node": "dialog", "to_port": "input" },
            { "id": "dialog-market", "from_node": "dialog", "from_port": "output", "to_node": "market", "to_port": "input" },
            { "id": "market-end", "from_node": "market", "from_port": "output", "to_node": "end", "to_port": "input" }
        ]);
        let graph: NarrativeGraph = serde_json::from_value(payload).unwrap();
        let issues = graph.validate().unwrap_err().join(" ");
        assert!(issues.contains(
            "Go To Location node Market may only be targeted directly by a Choice branch."
        ));
    }

    #[test]
    fn studio_graph_without_a_start_to_end_path_is_rejected() {
        let mut payload = studio_graph();
        payload["edges"] = json!([
            { "id": "start-dialog", "from_node": "start", "from_port": "output", "to_node": "dialog", "to_port": "input" }
        ]);
        let graph: NarrativeGraph = serde_json::from_value(payload).unwrap();
        let issues = graph.validate().unwrap_err().join(" ");
        assert!(issues.contains("Every node must have a path to End."));
    }
    #[test]
    fn conditional_requires_authored_conditions_and_exact_true_false_branches() {
        let mut payload = studio_graph();
        payload["nodes"][1]["type"] = json!("conditional");
        payload["nodes"][1]["conditions"] = json!([
            { "scope": "global", "variable": "trust", "op": "gte", "value": 1 }
        ]);
        payload["edges"] = json!([
            { "id": "start-condition", "from_node": "start", "from_port": "output", "to_node": "dialog", "to_port": "input" },
            { "id": "condition-true", "from_node": "dialog", "from_port": "true", "to_node": "end", "to_port": "input" },
            { "id": "condition-false", "from_node": "dialog", "from_port": "false", "to_node": "end", "to_port": "input" }
        ]);
        let graph: NarrativeGraph = serde_json::from_value(payload.clone()).unwrap();
        graph
            .validate()
            .expect("a conditional with both branches should validate");

        payload["edges"].as_array_mut().unwrap().pop();
        let graph: NarrativeGraph = serde_json::from_value(payload).unwrap();
        let issues = graph.validate().unwrap_err().join(" ");
        assert!(issues.contains("must have exactly one 'false' branch"));
    }
}
