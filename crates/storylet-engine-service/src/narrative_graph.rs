//! Non-linear Narrative Node Graph model for Storylet content authoring.
//! Reference: https://github.com/shrimbly/node-banana and NarrativeFlow.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::contracts::StoryletRecord;

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
    GetEntity {
        entity_path: String,
    },
    GetEntityProperty {
        property_key: String,
        #[serde(default = "default_property_type")]
        value_type: String,
    },
    ApplyCommand {
        property_key: String,
        #[serde(default = "default_property_operation")]
        operation: String,
        #[serde(default = "default_property_type")]
        value_type: String,
        #[serde(default)]
        value: Value,
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
    Compare {
        operator: String,
        #[serde(default = "default_property_type")]
        value_type: String,
        #[serde(default)]
        value: Value,
    },
    And {},
    Or {},
    Condition {},
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
    Learn { fact_tag: String, kind: storylet_engine::chrono_participants::KnowKind, visibility: storylet_engine::chrono_runtime::Visibility },
    Knows { holder: String, fact_tag: String, #[serde(default)] confirmed_only: bool },
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
    AddEntityTag {
        #[serde(default = "default_world_target")]
        target_entity_id: String,
        #[serde(default)]
        tag: String,
    },
    RemoveEntityTag {
        #[serde(default = "default_world_target")]
        target_entity_id: String,
        #[serde(default)]
        tag: String,
    },
    HasEntityTag {
        #[serde(default = "default_world_target")]
        target_entity_id: String,
        #[serde(default)]
        tag: String,
        #[serde(default = "default_match_all")]
        match_mode: String,
    },
    End {
        #[serde(default)]
        is_terminal: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        jump_to_storylet: Option<String>,
        /// Candidate eligibility result; omitted means valid.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        precondition_result: Option<bool>,
    },
}

fn default_world_target() -> String {
    "world".to_string()
}
fn default_match_all() -> String {
    "all".to_string()
}
fn default_property_type() -> String {
    "string".into()
}
fn typed_literal_matches(value_type: &str, value: &Value) -> bool {
    match value_type.trim().to_ascii_lowercase().as_str() {
        "bool" | "boolean" => value.is_boolean(),
        "int" | "integer" => value.as_i64().is_some(),
        "float" | "number" => value.as_f64().is_some(),
        "string" | "asset" | "entity" | "entity_ref" => value.is_string(),
        _ => false,
    }
}

fn default_property_operation() -> String {
    "set".into()
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NarrativeEdgeKind {
    #[default]
    Flow,
    Data,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NarrativeEdge {
    pub id: String,
    pub from_node: String,
    pub from_port: String,
    pub to_node: String,
    pub to_port: String,
    #[serde(default)]
    pub kind: NarrativeEdgeKind,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct NarrativeGraph {
    pub nodes: Vec<NarrativeNode>,
    pub edges: Vec<NarrativeEdge>,
}

fn node_type_name(data: &NarrativeNodeData) -> &'static str {
    match data {
        NarrativeNodeData::Start { .. } => "Start",
        NarrativeNodeData::Dialog { .. } => "Dialog",
        NarrativeNodeData::Choice { .. } => "Choice",
        NarrativeNodeData::QbnStoryletHub { .. } => "QBN Storylet Hub",
        NarrativeNodeData::SalienceSelector { .. } => "Salience Selector",
        NarrativeNodeData::WaypointConversation { .. } => "Waypoint Conversation",
        NarrativeNodeData::Waiter { .. } => "Waiter",
        NarrativeNodeData::Compare { .. } => "Compare",
        NarrativeNodeData::And {} => "AND",
        NarrativeNodeData::Or {} => "OR",
        NarrativeNodeData::Condition {} => "Condition",
        NarrativeNodeData::SceneSection { .. } => "Scene Section",
        NarrativeNodeData::MissionSubGraph { .. } => "Mission Subgraph",
        NarrativeNodeData::Poi { .. } => "POI",
        NarrativeNodeData::Image { .. } => "Image",
        NarrativeNodeData::Teleport { .. } => "Teleport",
        NarrativeNodeData::Learn { .. } => "Learn",
        NarrativeNodeData::Knows { .. } => "Knows",
        NarrativeNodeData::GoToLocation { .. } => "Go To Location",
        NarrativeNodeData::GraphTransform { .. } => "Graph Transform",
        NarrativeNodeData::PassionMutation { .. } => "Passion Mutation",
        NarrativeNodeData::AddEntityTag { .. } => "Add Entity Tag",
        NarrativeNodeData::RemoveEntityTag { .. } => "Remove Entity Tag",
        NarrativeNodeData::HasEntityTag { .. } => "Has Entity Tag",
        NarrativeNodeData::GetEntity { .. } => "Get Entity",
        NarrativeNodeData::GetEntityProperty { .. } => "Get Entity Property",
        NarrativeNodeData::ApplyCommand { .. } => "Apply Command",
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
        self.validate_mode()
    }

    fn validate_mode(&self) -> Result<(), Vec<String>> {
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
        if ends.is_empty() {
            issues.push("Graph must have at least one End node.".to_string());
        }
        if ids.len() != self.nodes.len() {
            issues.push("Node IDs must be unique.".to_string());
        }
        if starts.len() != 1 || ends.is_empty() || ids.len() != self.nodes.len() {
            return Err(issues);
        }
        let start = starts[0].id.as_str();
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
            let source = self.nodes.iter().find(|node| node.id == edge.from_node).unwrap();
            let target = self.nodes.iter().find(|node| node.id == edge.to_node).unwrap();
            match edge.kind {
                NarrativeEdgeKind::Flow => {
                    let valid_target = !matches!(
                        &target.data,
                        NarrativeNodeData::GetEntity { .. }
                            | NarrativeNodeData::GetEntityProperty { .. }
                            | NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. }
                            | NarrativeNodeData::And {}
                            | NarrativeNodeData::Or {}
                            | NarrativeNodeData::Compare { .. }
                    ) && edge.to_port == "input";
                    let valid_source = match &source.data {
                        NarrativeNodeData::GetEntity { .. }
                        | NarrativeNodeData::GetEntityProperty { .. }
                        | NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. }
                        | NarrativeNodeData::And {}
                        | NarrativeNodeData::Or {}
                        | NarrativeNodeData::Compare { .. } => false,
                        NarrativeNodeData::Choice { choices } => choices
                            .iter()
                            .enumerate()
                            .any(|(index, _)| edge.from_port == format!("choice_{index}")),
                        NarrativeNodeData::Condition {} => {
                            matches!(edge.from_port.as_str(), "true" | "false")
                        }
                        NarrativeNodeData::WaypointConversation { topic_nodes, .. } => {
                            topic_nodes.iter().any(|topic| {
                                topic.get("triggers_scene_exit").and_then(Value::as_bool) == Some(true)
                                    && topic.get("exit_output_pin").and_then(Value::as_str) == Some(edge.from_port.as_str())
                            })
                        }
                        _ => edge.from_port == "output",
                    };
                    if !valid_target || !valid_source {
                        issues.push(format!("Flow edge {} uses an invalid source or target port.", edge.id));
                    }
                    outgoing.entry(edge.from_node.clone()).or_default().push(edge.to_node.clone());
                    incoming.entry(edge.to_node.clone()).or_default().push(edge.from_node.clone());
                }
                NarrativeEdgeKind::Data => {
                    let source_type = match &source.data {
                        NarrativeNodeData::GetEntity { .. } => Some(("output", "entity_ref")),
                        NarrativeNodeData::GetEntityProperty { value_type, .. } => Some((
                            "output",
                            if value_type == "entity" { "entity_ref" } else { value_type.as_str() },
                        )),
                        NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. }
                        | NarrativeNodeData::And {}
                        | NarrativeNodeData::Or {}
                        | NarrativeNodeData::Compare { .. } => Some(("output", "bool")),
                        _ => None,
                    };
                    let target_type = match &target.data {
                        NarrativeNodeData::GetEntityProperty { .. } => Some(("entity", "entity_ref")),
                        NarrativeNodeData::ApplyCommand { .. } => match edge.to_port.as_str() {
                            "entity" => Some(("entity", "entity_ref")),
                            _ => None,
                        },
                        NarrativeNodeData::AddEntityTag { .. }
                        | NarrativeNodeData::RemoveEntityTag { .. }
                        | NarrativeNodeData::HasEntityTag { .. } => match edge.to_port.as_str() {
                            "target" => Some(("target", "entity_ref")),
                            _ => None,
                        },
                        NarrativeNodeData::Condition {} | NarrativeNodeData::And {} | NarrativeNodeData::Or {} => Some(("input", "bool")),
                        NarrativeNodeData::Compare { .. } => Some(("input", "any")),
                        _ => None,
                    };
                    if source_type.is_none_or(|(port, _)| edge.from_port != port)
                        || target_type.is_none_or(|(port, _)| edge.to_port != port)
                        || source_type.zip(target_type).is_some_and(|((_, actual), (_, expected))| {
                            expected != "any" && actual != expected
                        })
                    {
                        issues.push(format!("Data edge {} uses an invalid port or incompatible type.", edge.id));
                    }
                    if let (
                        NarrativeNodeData::GetEntityProperty { value_type: source_type, .. },
                        NarrativeNodeData::Compare { operator, value_type, value },
                    ) = (&source.data, &target.data)
                    {
                        let compatible = match value_type.as_str() {
                            "bool" | "boolean" => source_type == "bool" || source_type == "boolean",
                            "int" | "integer" => matches!(source_type.as_str(), "int" | "integer"),
                            "float" => matches!(source_type.as_str(), "float" | "number"),
                            "string" | "asset" | "entity" | "entity_ref" => source_type == value_type,
                            _ => true,
                        };
                        let ordered = matches!(operator.as_str(), "gt" | "gte" | "lt" | "lte");
                        if !compatible
                            || (ordered
                                && !matches!(value_type.as_str(), "int" | "integer" | "float"))
                        {
                            issues.push(format!(
                                "Data edge {} has an incompatible comparison source type.",
                                edge.id
                            ));
                        }
                        if !typed_literal_matches(value_type, value) {
                            issues.push(format!(
                                "Data edge {} has an incompatible comparison literal.",
                                edge.id
                            ));
                        }
                    }
                }
            }
        }
        if incoming.get(start).is_some_and(|nodes| !nodes.is_empty())
            || outgoing.get(start).map_or(0, Vec::len) != 1
        {
            issues.push("Start must have no incoming edges and exactly one outgoing edge.".into());
        }
        for end in &ends {
            if outgoing.get(&end.id).is_some_and(|nodes| !nodes.is_empty()) {
                issues.push("End cannot have outgoing edges.".into());
            }
        }
        let reachable = {
            let mut seen = BTreeSet::new();
            let mut queue = VecDeque::from([start.to_string()]);
            while let Some(id) = queue.pop_front() {
                if !seen.insert(id.clone()) {
                    continue;
                }
                if let Some(next) = outgoing.get(&id) {
                    queue.extend(next.iter().cloned());
                }
                let consumes_data = self.nodes.iter().find(|node| node.id == id).is_some_and(|node| {
                    matches!(
                        &node.data,
                        NarrativeNodeData::Condition {}
                            | NarrativeNodeData::GetEntityProperty { .. }
                            | NarrativeNodeData::ApplyCommand { .. }
                            | NarrativeNodeData::And {}
                            | NarrativeNodeData::Or {}
                            | NarrativeNodeData::Compare { .. }
                            | NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. }
                    )
                });
                if consumes_data {
                    queue.extend(self.edges.iter().filter(|edge| {
                        edge.to_node == id && edge.kind == NarrativeEdgeKind::Data
                    }).map(|edge| edge.from_node.clone()));
                }
            }
            seen
        };
        let mut reaches_end = BTreeSet::new();
        let mut end_queue: VecDeque<String> = ends.iter().map(|end| end.id.clone()).collect();
        while let Some(id) = end_queue.pop_front() {
            if !reaches_end.insert(id.clone()) {
                continue;
            }
            if let Some(sources) = incoming.get(&id) {
                end_queue.extend(sources.iter().cloned());
            }
            let consumes_data = self.nodes.iter().find(|node| node.id == id).is_some_and(|node| {
                matches!(
                    &node.data,
                    NarrativeNodeData::Condition {}
                        | NarrativeNodeData::GetEntityProperty { .. }
                        | NarrativeNodeData::ApplyCommand { .. }
                        | NarrativeNodeData::And {}
                        | NarrativeNodeData::Or {}
                        | NarrativeNodeData::Compare { .. }
                        | NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. }
                )
            });
            if consumes_data {
                end_queue.extend(self.edges.iter().filter(|edge| {
                    edge.to_node == id && edge.kind == NarrativeEdgeKind::Data
                }).map(|edge| edge.from_node.clone()));
            }
        }
        if self.nodes.iter().any(|node| !reachable.contains(&node.id)) {
            issues.push("Every node must be reachable from Start.".into());
        }
        if self
            .nodes
            .iter()
            .any(|node| !reaches_end.contains(&node.id))
        {
            issues.push("Every node must have a path to an End node.".into());
        }
        for node in &self.nodes {
            let edges: Vec<_> = self
                .edges
                .iter()
                .filter(|edge| edge.from_node == node.id && edge.kind == NarrativeEdgeKind::Flow)
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
                NarrativeNodeData::Learn { fact_tag, .. } | NarrativeNodeData::Knows { fact_tag, .. } => {
                    if matches!(node.data, NarrativeNodeData::Knows { .. }) {
                        let count = self.edges.iter().filter(|edge| edge.from_node == node.id && edge.kind == NarrativeEdgeKind::Data).count();
                        if count != 1 { issues.push(format!("Knows node {} must have exactly one outgoing boolean data connection.", node.title)); }
                    }
                    if !fact_tag.starts_with("fact.") || fact_tag.len() <= 5 {
                        issues.push(format!("{} node \"{}\": fact tag \"{}\" must be under \"fact.\"", node_type_name(&node.data), node.title, fact_tag));
                    }
                }
                NarrativeNodeData::GoToLocation { location_id } => {
                    if location_id.trim().is_empty() {
                        issues.push("Go To Location nodes must select a location.".into());
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
                NarrativeNodeData::GetEntity { entity_path } => {
                    if entity_path.trim().is_empty() {
                        issues.push(format!("Get Entity node {} must select an Entity.", node.title));
                    }
                    if !edges.is_empty() {
                        issues.push(format!("Get Entity node {} must not have outgoing Flow edges.", node.title));
                    }
                }
                NarrativeNodeData::GetEntityProperty { property_key, value_type, .. } => {
                    if property_key.trim().is_empty() {
                        issues.push(format!("Get Entity Property node {} must specify a property key.", node.title));
                    }
                    if !matches!(value_type.as_str(), "bool" | "int" | "float" | "string" | "asset" | "entity") {
                        issues.push(format!("Get Entity Property node {} has an unsupported value type.", node.title));
                    }
                    let inputs = self.edges.iter().filter(|edge| {
                        edge.to_node == node.id && edge.to_port == "entity" && edge.kind == NarrativeEdgeKind::Data
                    }).count();
                    if inputs != 1 {
                        issues.push(format!("Get Entity Property node {} must have exactly one Entity data input.", node.title));
                    }
                    if !edges.is_empty() {
                        issues.push(format!("Get Entity Property node {} must not have outgoing Flow edges.", node.title));
                    }
                }
                NarrativeNodeData::ApplyCommand { property_key, operation, value_type, value } => {
                    if property_key.trim().is_empty() {
                        issues.push(format!("Apply Command node {} must specify a property key.", node.title));
                    }
                    if !matches!(operation.as_str(), "set" | "add" | "subtract" | "toggle") {
                        issues.push(format!("Apply Command node {} has an unsupported operation.", node.title));
                    }
                    let literal_ok = match value_type.as_str() {
                        "bool" => value.is_boolean(),
                        "int" => value.as_i64().is_some(),
                        "float" => value.as_f64().is_some(),
                        "string" | "asset" | "entity" => value.is_string(),
                        _ => false,
                    };
                    if !literal_ok {
                        issues.push(format!("Apply Command node {} has an invalid literal for its value type.", node.title));
                    }
                    if operation == "toggle" && value_type != "bool" {
                        issues.push(format!("Apply Command node {} can only toggle boolean properties.", node.title));
                    }
                    if matches!(operation.as_str(), "add" | "subtract")
                        && !matches!(value_type.as_str(), "int" | "float")
                    {
                        issues.push(format!("Apply Command node {} can only add or subtract numeric properties.", node.title));
                    }
                    let inputs = self.edges.iter().filter(|edge| {
                        edge.to_node == node.id && edge.to_port == "entity" && edge.kind == NarrativeEdgeKind::Data
                    }).count();
                    if inputs != 1 {
                        issues.push(format!("Apply Command node {} must have exactly one Entity data input.", node.title));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::Compare {
                    operator,
                    value_type,
                    value,
                } => {
                    if !edges.is_empty() {
                        issues.push(format!(
                            "Compare node {} must not have outgoing Flow edges.",
                            node.title
                        ));
                    }
                    if !matches!(operator.as_str(), "eq" | "neq" | "gt" | "gte" | "lt" | "lte") {
                        issues.push(format!("Compare node {} has an unsupported operator.", node.title));
                    }
                    if !typed_literal_matches(value_type, value) {
                        issues.push(format!(
                            "Compare node {} has an invalid literal for its value type.",
                            node.title
                        ));
                    }
                    if matches!(operator.as_str(), "gt" | "gte" | "lt" | "lte")
                        && !matches!(value_type.as_str(), "int" | "integer" | "float")
                    {
                        issues.push(format!(
                            "Compare node {} uses an ordered operator with a non-numeric type.",
                            node.title
                        ));
                    }
                }
                NarrativeNodeData::HasEntityTag { tag, .. } => {
                    if tag.trim().is_empty() {
                        issues.push(format!("Has Entity Tag node {} must specify a tag.", node.title));
                    }
                    let outputs = self.edges.iter().filter(|edge| {
                        edge.from_node == node.id && edge.kind == NarrativeEdgeKind::Data
                    }).count();
                    if outputs != 1 {
                        issues.push(format!("Has Entity Tag node {} must have exactly one outgoing boolean data connection.", node.title));
                    }
                    if !edges.is_empty() {
                        issues.push(format!("Has Entity Tag node {} must not have outgoing Flow edges.", node.title));
                    }
                }
                NarrativeNodeData::Choice { choices } => {
                    for index in 0..choices.len() {
                        let port = format!("choice_{index}");
                        if edges.iter().filter(|edge| edge.from_port == port).count() != 1 {
                            issues.push(format!("Choice node {} must have exactly one outgoing edge for branch {}.", node.title, index));
                        }
                    }
                    if edges.iter().any(|edge| {
                        !choices.iter().enumerate().any(|(index, _)| edge.from_port == format!("choice_{index}"))
                    }) {
                        issues.push(format!("Choice node {} has an unknown branch output.", node.title));
                    }
                }
                NarrativeNodeData::Condition {} => {
                    let inputs = self.edges.iter().filter(|edge| {
                        edge.to_node == node.id && edge.kind == NarrativeEdgeKind::Data
                    }).count();
                    if inputs != 1 {
                        issues.push(format!("Condition node {} must have exactly one incoming boolean data input.", node.title));
                    }
                    for port in ["true", "false"] {
                        if edges.iter().filter(|edge| edge.from_port == port).count() != 1 {
                            issues.push(format!("Condition node {} must have exactly one outgoing {} branch.", node.title, port));
                        }
                    }
                    if edges.iter().any(|edge| !matches!(edge.from_port.as_str(), "true" | "false")) {
                        issues.push(format!("Condition node {} has an unknown flow output pin.", node.title));
                    }
                }
                NarrativeNodeData::And {} | NarrativeNodeData::Or {} => {
                    let inputs = self
                        .edges
                        .iter()
                        .filter(|edge| edge.to_node == node.id && edge.kind == NarrativeEdgeKind::Data)
                        .count();
                    if inputs < 2 {
                        issues.push(format!(
                            "{} node {} must have at least two incoming boolean data inputs.",
                            node_type_name(&node.data),
                            node.title
                        ));
                    }
                    if !edges.is_empty() {
                        issues.push(format!("{} node {} must not have outgoing Flow edges.", node_type_name(&node.data), node.title));
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
                NarrativeNodeData::AddEntityTag { tag, .. } => {
                    if tag.trim().is_empty() {
                        issues.push(format!("Add Entity Tag node {} must specify a tag.", node.title));
                    }
                    require_default_output(&mut issues, &node.title, &edges);
                }
                NarrativeNodeData::RemoveEntityTag { tag, .. } => {
                    if tag.trim().is_empty() {
                        issues.push(format!("Remove Entity Tag node {} must specify a tag.", node.title));
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

    /// Validate a graph used solely for candidate eligibility. Such graphs may
    /// never contain nodes that can mutate or pause runtime state.
    pub fn validate_precondition(&self) -> Result<(), Vec<String>> {
        let mut issues = Vec::new();
        if self.nodes.len() > 128 || self.edges.len() > 256 {
            issues.push("Precondition graph exceeds the execution limit.".into());
        }
        for node in &self.nodes {
            if !matches!(
                &node.data,
                NarrativeNodeData::Start { .. }
                    | NarrativeNodeData::End { .. }
                    | NarrativeNodeData::GetEntity { .. }
                    | NarrativeNodeData::GetEntityProperty { .. }
                    | NarrativeNodeData::Compare { .. }
                    | NarrativeNodeData::HasEntityTag { .. } | NarrativeNodeData::Knows { .. }
                    | NarrativeNodeData::And {}
                    | NarrativeNodeData::Or {}
                    | NarrativeNodeData::Condition {}
            ) {
                issues.push(format!(
                    "Node {} is not permitted in a precondition graph.",
                    node.title
                ));
            }
        }
        if let Err(mut graph_issues) = self.validate_mode() {
            issues.append(&mut graph_issues);
        }
        if issues.is_empty() { Ok(()) } else { Err(issues) }
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
            .unwrap_or_else(|| "player".into());

        let opening_item = DialogueItem::Line {
            id: "line_0".into(),
            speaker_role: default_speaker.clone(),
            target_role: "player".into(),
            content: prose.to_string(),
            emotion_tag: None,
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
            kind: NarrativeEdgeKind::Flow,
        });


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
                precondition_result: None,
            },
        });
        edges.push(NarrativeEdge {
            id: "edge_dialog_end".into(),
            from_node: "node_dialog_1".into(),
            from_port: "output".into(),
            to_node: "node_end".into(),
            to_port: "input".into(),
            kind: NarrativeEdgeKind::Flow,
        });

        Self {
            nodes,
            edges,
        }
    }

    /// A storylet is a dedicated ending only when it has at least one End and
    /// every End is terminal. A graph with a jump exit remains in the normal
    /// path; taking a terminal branch can still end that branch at runtime.
    pub fn ending_from_graph(&self) -> bool {
        let mut has_end = false;
        for node in &self.nodes {
            if let NarrativeNodeData::End {
                is_terminal,
                jump_to_storylet,
                ..
            } = &node.data
            {
                has_end = true;
                if !*is_terminal || jump_to_storylet.is_some() {
                    return false;
                }
            }
        }
        has_end
    }

    /// Compile this node graph back into storylet prose.
    pub fn compile_to_storylet_content(&self, st: &mut StoryletRecord) {
        let mut prose_parts = Vec::new();

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
                
                NarrativeNodeData::End { .. } => {}
                _ => {}
            }
        }

        if !prose_parts.is_empty() {
            let full_prose = prose_parts.join("\n\n");
            st.content.scene_opening = Some(full_prose.clone());
            st.content.text_template = full_prose;
        }

        
    }
}


#[cfg(test)]
mod knowledge_authoring_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn learn_node_requires_fact_root() {
        let graph: NarrativeGraph = serde_json::from_value(json!({"nodes":[
            {"id":"s","type":"start","title":"Start","text":"Start","x":0,"y":0,"width":100,"height":100},
            {"id":"l","type":"learn","title":"Clue","fact_tag":"clue.x","kind":"evidence","visibility":"private","x":0,"y":0,"width":100,"height":100},
            {"id":"e","type":"end","title":"End","x":0,"y":0,"width":100,"height":100}
        ],"edges":[{"id":"a","from_node":"s","from_port":"output","to_node":"l","to_port":"input"},{"id":"b","from_node":"l","from_port":"output","to_node":"e","to_port":"input"}]})).unwrap();
        assert!(graph.validate().unwrap_err().iter().any(|issue| issue.contains("must be under \"fact.\"")));
    }
}
