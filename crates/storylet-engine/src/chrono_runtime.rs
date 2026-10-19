//! Token-driven flow graph, scene timeline, and call-history runtime for the v5
//! contract (CFN-SPEC-001 Modules 3, 4, 5, and 6).
//!
//! `FlowGraphAsset` is the immutable topology: nodes, pins, and edges with
//! persistent globally unique node GUIDs. `FlowGraphInstance` is one active
//! execution of that asset: node states, the flat `SignalToken` vector, the
//! fixed-point integer-millisecond clock, and the call history. World state and
//! storylet policy come from `chrono_state` and `chrono_dispatch`; this module
//! owns execution order, node call data, and the reversible records that make
//! rollback exact.
//!
//! Determinism: tokens are consumed in ascending token id order, node lookups are
//! ordered by node GUID or name, action parts are ordered by priority key then
//! combiner entity, and every tie is broken by a stable key. Nothing here reads a
//! wall clock or a random source; the caller supplies the seed.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::chrono_dispatch::{DispatchError, UnifiedDispatcher, WaypointStep, WaypointTopicGraph};
use crate::chrono_state::{
    apply_mutation, apply_mutation_with_registry, apply_passion_mutation, revert_delta,
    BijectiveDelta, ChronoStateError, GameplayTagRegistry, GraphTransformRule, MutationOp,
    PassionMutation, PassionRegistry, TagBitset, WorldStateTable, MAX_TAGS,
};

/// Input pin name used when a call record is synthesized by replay or invoke.
pub const DEFAULT_INPUT_PIN: &str = "In";

/// Output pin name used when a node completes without an authored choice.
pub const DEFAULT_OUTPUT_PIN: &str = "Out";

/// Hard per-tick execution cap. Zero-cost cycles are rejected by
/// `FlowGraphAsset::validate`; this cap also stops assets that bypassed
/// validation from hanging the frame loop.
pub const MAX_TICK_EXECUTIONS: usize = 4096;

/// Node categories from REQ-FG-04.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCategory {
    #[default]
    Mutator,
    Waiter,
    LogicGate,
    DynamicDispatcher,
    ScopedContainer,
}

/// Node execution states from REQ-FG-05.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeExecutionState {
    #[default]
    Idle,
    ActiveLatent,
    Completed,
    Aborted,
}

/// Composer activity for one action part inside a scene time window (REQ-SC-02).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventOverlapActivity {
    #[default]
    StartActivity,
    RegularActivity,
    EndActivity,
}

/// Timeline action classes, ordered by the composer priority key used for
/// dispatch order and conflict arbitration (REQ-SC-03). Higher values win.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimelineActionType {
    #[default]
    WorkspotPose,
    TransitionAnim,
    DialogueVo,
    CameraCut,
    VfxPoint,
}

impl TimelineActionType {
    /// Priority key for a timeline action. Authors may add a positive offset on
    /// the event itself; the table value is the base.
    pub const fn action_key_priority(&self) -> i32 {
        match self {
            TimelineActionType::WorkspotPose => 0,
            TimelineActionType::TransitionAnim => 1,
            TimelineActionType::DialogueVo => 2,
            TimelineActionType::CameraCut => 3,
            TimelineActionType::VfxPoint => 4,
        }
    }
}

/// One pin on a node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinDefinition {
    pub pin_name: String,
    pub is_input: bool,
}

/// One edge between two pins (REQ-FG-02).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EdgeConnection {
    pub source_node_guid: u32,
    pub source_pin_name: String,
    pub target_node_guid: u32,
    pub target_pin_name: String,
}

/// Flat signal token (REQ-FG-03): 32 bytes, no heap payload. The pin is an
/// index into the target node's `input_pins`; status lives in `flags`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignalToken {
    pub token_id: u64,
    pub graph_instance_id: u64,
    pub target_node_guid: u32,
    /// Position in the target node's `input_pins`; the asset owns the name.
    pub target_pin_index: u16,
    pub flags: u16,
    pub time_budget_ms: i32,
    pub deferred_until_ms: i32,
}

/// Token is waiting to be consumed.
pub const TOKEN_ACTIVE: u16 = 1 << 0;
/// Token was budget-starved and parked until `deferred_until_ms`.
pub const TOKEN_DEFERRED: u16 = 1 << 1;

impl SignalToken {
    pub fn is_active(&self) -> bool {
        self.flags & TOKEN_ACTIVE != 0
    }

    pub fn set_active(&mut self, active: bool) {
        if active {
            self.flags |= TOKEN_ACTIVE;
        } else {
            self.flags &= !TOKEN_ACTIVE;
        }
    }

    pub fn is_deferred(&self) -> bool {
        self.flags & TOKEN_DEFERRED != 0
    }

    pub fn set_deferred(&mut self, deferred: bool) {
        if deferred {
            self.flags |= TOKEN_DEFERRED;
        } else {
            self.flags &= !TOKEN_DEFERRED;
        }
    }
}

/// Clamp a millisecond clock into the token's packed i32 range.
fn clamp_token_ms(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
/// Host-consumable edge emitted when a passion crosses a configured threshold.
/// Rollback emits the inverse edge; callers can activate or cancel the linked asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassionThresholdTriggerEvent {
    pub node_guid: u32,
    pub npc_id: u16,
    pub threshold_tag_bit: u16,
    pub linked_graph_asset_id: String,
    pub activated: bool,
}

/// One authored node: identity, category, behavior, and pins (REQ-FG-01).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowNodeDef {
    pub node_guid: u32,
    pub node_type_id: String,
    pub category: NodeCategory,
    pub kind: NodeKind,
    #[serde(default)]
    pub entity_path: String,
    #[serde(default)]
    pub input_pins: Vec<PinDefinition>,
    #[serde(default)]
    pub output_pins: Vec<PinDefinition>,
    /// Integer milliseconds charged against the frame budget when this node runs.
    #[serde(default)]
    pub cost_ms: i64,
    /// Mission node that owns this node's invocations, when scoped (REQ-CH-04).
    #[serde(default)]
    pub mission_scope: Option<u32>,
    /// Authored labels and content preserved for UI and MCP consumers.
    #[serde(default)]
    pub payload: serde_json::Value,
}

/// Node behavior. Every kind is executable through the same three endpoints
/// (`ExecuteNormal`, `ForceComplete`, `CancelAndRevert`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NodeKind {
    SetMetric {
        metric_id: u16,
        delta: f32,
    },
    /// Absolute metric assignment: sets a metric slot to `value` rather than
    /// adding to it. The inverse restores the prior value through the recorded
    /// metric undo pair.
    SetMetricValue {
        metric_id: u16,
        value: f32,
    },
    Mutation {
        op: MutationOp,
    },
    /// Apply an attributed world-graph transformation and cache its exact inverse.
    GraphTransform {
        rule: GraphTransformRule,
    },
    /// Reversible Star-NPC passion mutation with threshold graph events.
    PassionMutation {
        op: PassionMutation,
    },
    Waiter {
        duration_ms: i64,
        #[serde(default)]
        required_tags: TagBitset,
        /// Named wait event from the authored graph. A waiter also wakes when
        /// `signal_event` delivers this name, independently of its timer.
        #[serde(default)]
        wait_event: String,
    },
    #[default]
    LogicAll,
    LogicAny {
        #[serde(default)]
        default_output_pin: String,
    },
    Choice {
        #[serde(default)]
        default_output_pin: String,
    },
    Poi {
        poi_id: String,
    },
    QbnHub {
        deck: Vec<u16>,
        #[serde(default)]
        exit_storylet_id: Option<u16>,
    },
    SalienceSelector {
        pool: Vec<u16>,
        #[serde(default)]
        fallback_storylet_id: Option<u16>,
        #[serde(default)]
        fallback_output_pin: String,
    },
    WaypointConversation {
        graph: WaypointTopicGraph,
        initial_topic_id: String,
        npc_goal_topic_id: String,
    },
    MissionSubGraph {
        sub_graph_asset_id: String,
    },
    SceneSection {
        section_duration_ms: i64,
        events: Vec<TimelineTrackEvent>,
    },
    /// Terminal node, optionally invoking its authored destination storylet.
    End {
        #[serde(default)]
        jump_to_storylet: Option<u16>,
    },
    /// Condition gate with explicit true/false branch pins.
    Conditional {
        condition: Condition,
    },
    /// Typed variable assignments, kept in authored order so rollback is exact.
    VariableBatch {
        assignments: Vec<VariableAssignment>,
    },
    /// Explicit deferred invoke target, preserved from the legacy
    /// `NarrativeNodeData::Teleport { destination_storylet }`.
    Teleport {
        #[serde(default)]
        destination_storylet: Option<u16>,
        #[serde(default)]
        destination_node_guid: Option<u32>,
    },
}

/// One entry on a scene section timeline track (REQ-SC-01).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimelineTrackEvent {
    pub event_id: u64,
    pub action_type: TimelineActionType,
    pub target_entity_path: String,
    pub start_time_ms: i64,
    pub duration_ms: i64,
    pub payload_asset_id: String,
    /// Pose captured before this event overwrote it, used by reverse scrubbing.
    #[serde(default)]
    pub cached_previous_pose_id: String,
    /// Author offset added to the action-type priority key.
    #[serde(default)]
    pub priority_offset: i32,
}

/// One resolved timeline action inside a time window (REQ-SC-02).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionPart {
    pub action_key_priority: i32,
    pub combiner_entity_path: String,
    pub activity: EventOverlapActivity,
    pub local_cursor_time_ms: i64,
    pub event_ref: TimelineTrackEvent,
}

/// A command the composer dispatches after arbitration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposerCommand {
    pub node_guid: u32,
    pub combiner_entity_path: String,
    pub activity: EventOverlapActivity,
    pub action_key_priority: i32,
    pub event_id: u64,
    pub payload_asset_id: String,
    pub cached_previous_pose_id: String,
}

/// A recorded composer conflict: the winner and the parts it displaced.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposerConflict {
    pub combiner_entity_path: String,
    pub winner: ComposerCommand,
    pub rejected: Vec<ComposerCommand>,
}

/// Per-entity pose arbitration for one scene section (REQ-SC-03).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SceneComposer {
    pub actor_pose: BTreeMap<String, String>,
    pub applied: Vec<ComposerCommand>,
    pub conflicts: Vec<ComposerConflict>,
}

/// One call-history record (REQ-CH-01).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallHistoryEntry {
    pub node_guid: u32,
    pub executed_input_pin: String,
    pub chosen_output_pin: String,
    #[serde(default)]
    pub chosen_storylet_id: Option<u16>,
}

/// One save-replay record: a call-history entry plus the frame it was played on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveReplayEntry {
    pub node_guid: u32,
    pub executed_input_pin: String,
    pub chosen_output_pin: String,
    #[serde(default)]
    pub chosen_storylet_id: Option<u16>,
    pub frame_ms: i64,
}

/// Per-node inverse cache. `CancelAndRevert` pops these in LIFO order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NodeCallData {
    Delta {
        delta: BijectiveDelta,
    },
    Storylet {
        storylet_id: u16,
        delta: BijectiveDelta,
        /// Completion clock before this call, restored on rollback so a
        /// rolled-back storylet is not left on cooldown.
        #[serde(default)]
        previous_completed_ms: Option<i64>,
    },
    WaypointStep {
        previous_topic_id: String,
        edge_id: String,
        #[serde(default)]
        revealed_quality_tag: Option<u16>,
        /// Authored line/asset the host should speak on this transition.
        #[serde(default)]
        dialogue_transition_asset_id: Option<String>,
        delta: BijectiveDelta,
    },
    SceneSection {
        previous_clock_ms: i64,
        previous_poses: Vec<ScenePoseUndo>,
        /// History commit created when the section completed; reopening is
        /// only safe when this is the current head.
        #[serde(default)]
        completion_commit: Option<usize>,
    },
    Route {
        output_pin: String,
    },
    Mission {
        sub_graph_asset_id: String,
    },
    Waiter {
        wake_ms: i64,
    },
    Poi {
        poi_id: String,
    },
    VariableBatch {
        entries: Vec<VariableRollbackEntry>,
    },
    Teleport {
        destination_storylet: Option<u16>,
        destination_node_guid: Option<u32>,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenePoseUndo {
    pub combiner_entity_path: String,
    pub previous_pose_id: Option<String>,
}

impl NodeCallData {
    /// The recorded delta, when the record carries one.
    pub fn delta(&self) -> Option<&BijectiveDelta> {
        match self {
            NodeCallData::Delta { delta }
            | NodeCallData::Storylet { delta, .. }
            | NodeCallData::WaypointStep { delta, .. } => Some(delta),
            _ => None,
        }
    }
}

/// Runtime state for one node (REQ-FG-05).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowNodeInstance {
    pub node_guid: u32,
    pub state: NodeExecutionState,
    pub entity_path: String,
    pub executed_input_pin: Option<String>,
    pub chosen_output_pin: Option<String>,
    pub chosen_storylet_id: Option<u16>,
    /// LIFO inverse cache for `CancelAndRevert`.
    pub call_data: Vec<NodeCallData>,
    /// Input pins already arrived, for AND-gate fan-in.
    pub arrived_input_pins: BTreeSet<String>,
    /// Completion lock keyed by storylet id, matching the dispatcher contract.
    pub completed_storylets: BTreeSet<u16>,
    pub current_topic_id: String,
    pub edge_use_counts: BTreeMap<String, u64>,
    /// Mission-scoped invocations (REQ-CH-04).
    pub local_call_history: Vec<CallHistoryEntry>,
    /// The nested instance a MissionSubGraph node owns (REQ-CH-04, spec §8.2).
    pub sub_graph: Option<Box<FlowGraphInstance>>,
    pub section_clock_ms: i64,
    pub waiter_wake_ms: i64,
}

/// One committed call-history record. The engine is a commit DAG: each commit
/// points at its parent, so two clients that diverged can be reconciled by their
/// lowest common ancestor (REQ-CH-05).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HistoryCommit {
    pub index: usize,
    pub parent: Option<usize>,
    pub entry: CallHistoryEntry,
    pub delta: BijectiveDelta,
}

/// Persistent call-history engine (REQ-CH-05).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PersistentHistoryEngine {
    /// Commit 0 is the root: an empty entry with no parent.
    pub commits: Vec<HistoryCommit>,
    pub head: usize,
}

impl PersistentHistoryEngine {
    pub fn new() -> Self {
        Self {
            commits: vec![HistoryCommit::default()],
            head: 0,
        }
    }

    /// Append a commit under `parent` and return its index.
    pub fn commit(
        &mut self,
        entry: &CallHistoryEntry,
        delta: &BijectiveDelta,
        parent: Option<usize>,
    ) -> usize {
        let index = self.commits.len();
        self.commits.push(HistoryCommit {
            index,
            parent,
            entry: entry.clone(),
            delta: delta.clone(),
        });
        index
    }

    /// Append a commit under the current head.
    pub fn commit_for(&mut self, entry: &CallHistoryEntry, delta: &BijectiveDelta) -> usize {
        let index = self.commit(entry, delta, Some(self.head));
        self.head = index;
        index
    }

    pub fn commit_count(&self) -> usize {
        self.commits.len()
    }

    pub fn commit_at(&self, index: usize) -> Option<&HistoryCommit> {
        self.commits.get(index)
    }

    /// Root-to-`commit` commit indices, in forward order.
    pub fn path_to(&self, commit: usize) -> Result<Vec<usize>, GraphError> {
        let mut path = Vec::new();
        let mut cursor = commit;
        loop {
            let record = self.commits.get(cursor).ok_or(GraphError::CommitNotFound {
                commit_index: cursor,
            })?;
            path.push(cursor);
            match record.parent {
                Some(parent) => cursor = parent,
                None => break,
            }
        }
        path.reverse();
        Ok(path)
    }

    /// Entries on the path from the root to `commit`, root excluded.
    pub fn path_entries(&self, commit: usize) -> Result<Vec<CallHistoryEntry>, GraphError> {
        let mut entries = Vec::new();
        for &index in &self.path_to(commit)? {
            if index == 0 {
                continue;
            }
            entries.push(self.commits[index].entry.clone());
        }
        Ok(entries)
    }

    /// Entries on the path to the current head.
    pub fn call_history(&self) -> Result<Vec<CallHistoryEntry>, GraphError> {
        self.path_entries(self.head)
    }

    pub fn depth(&self, commit: usize) -> Result<usize, GraphError> {
        Ok(self.path_to(commit)?.len() - 1)
    }

    /// Lowest common ancestor of two commits (REQ-CH-05).
    pub fn lca(&self, left: usize, right: usize) -> Result<usize, GraphError> {
        let left_path = self.path_to(left)?;
        let right_path = self.path_to(right)?;
        let mut answer = 0;
        for (a, b) in left_path.iter().zip(right_path.iter()) {
            if a != b {
                break;
            }
            answer = *a;
        }
        Ok(answer)
    }
}

/// Immutable graph topology (REQ-FG-01).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowGraphAsset {
    pub asset_id: String,
    pub start_node_guid: u32,
    pub nodes: Vec<FlowNodeDef>,
    pub edges: Vec<EdgeConnection>,
}

impl FlowGraphAsset {
    /// Structural validation: unique GUIDs, at least one pin per direction,
    /// resolvable edge endpoints, non-negative costs, and resolvable mission
    /// scopes.
    pub fn validate(&self) -> Result<(), GraphError> {
        if self.asset_id.is_empty() {
            return Err(GraphError::EmptyAssetId);
        }
        if self.nodes.is_empty() {
            return Err(GraphError::EmptyAsset);
        }
        let mut seen = BTreeSet::new();
        for node in &self.nodes {
            if !seen.insert(node.node_guid) {
                return Err(GraphError::DuplicateNodeGuid {
                    node_guid: node.node_guid,
                });
            }
            if node.node_type_id.is_empty() {
                return Err(GraphError::EmptyNodeTypeId {
                    node_guid: node.node_guid,
                });
            }
            if node.input_pins.is_empty() || node.output_pins.is_empty() {
                return Err(GraphError::NodeHasNoPins {
                    node_guid: node.node_guid,
                });
            }
            for (pins, is_input) in [(&node.input_pins, true), (&node.output_pins, false)] {
                let mut names = BTreeSet::new();
                for pin in pins {
                    if !names.insert(pin.pin_name.as_str()) {
                        return Err(GraphError::DuplicatePinName {
                            node_guid: node.node_guid,
                            pin_name: pin.pin_name.clone(),
                            is_input,
                        });
                    }
                }
            }
            if node.cost_ms < 0 {
                return Err(GraphError::NegativeNodeCost {
                    node_guid: node.node_guid,
                    cost_ms: node.cost_ms,
                });
            }
            if node.cost_ms > i64::from(i32::MAX) {
                return Err(GraphError::NodeCostTooLarge {
                    node_guid: node.node_guid,
                    cost_ms: node.cost_ms,
                });
            }
            if let NodeKind::SetMetric { delta, .. } = &node.kind {
                if !delta.is_finite() {
                    return Err(GraphError::NonFiniteMetricDelta {
                        node_guid: node.node_guid,
                        delta: *delta,
                    });
                }
            }
            if let NodeKind::SetMetricValue { value, .. } = &node.kind {
                if !value.is_finite() {
                    return Err(GraphError::NonFiniteMetricValue {
                        node_guid: node.node_guid,
                        value: *value,
                    });
                }
            }
            if let NodeKind::Conditional { condition } = &node.kind {
                if condition.rules.is_empty()
                    || find_pin(node, &condition.true_pin, false).is_none()
                    || find_pin(node, &condition.false_pin, false).is_none()
                {
                    return Err(GraphError::ConditionalBranchPinMissing {
                        node_guid: node.node_guid,
                    });
                }
            }
            if let NodeKind::VariableBatch { assignments } = &node.kind {
                if assignments.is_empty() {
                    return Err(GraphError::EmptyVariableAssignment {
                        node_guid: node.node_guid,
                    });
                }
                if assignments
                    .iter()
                    .any(|assignment| assignment.name.is_empty())
                {
                    return Err(GraphError::EmptyVariableName {
                        node_guid: node.node_guid,
                    });
                }
            }
            if let Some(scope) = node.mission_scope {
                if scope == node.node_guid {
                    return Err(GraphError::MissionScopeSelfReference {
                        node_guid: node.node_guid,
                    });
                }
            }
            if let NodeKind::SceneSection {
                section_duration_ms,
                events,
            } = &node.kind
            {
                if *section_duration_ms < 0 {
                    return Err(GraphError::NegativeSectionDuration {
                        node_guid: node.node_guid,
                        duration_ms: *section_duration_ms,
                    });
                }
                if let Some(event) = events
                    .iter()
                    .find(|event| event.start_time_ms < 0 || event.duration_ms < 0)
                {
                    return Err(GraphError::NegativeEventTime {
                        node_guid: node.node_guid,
                        event_id: event.event_id,
                    });
                }
            }
        }
        for node in &self.nodes {
            if let Some(scope) = node.mission_scope {
                if !self.has_node(scope) {
                    return Err(GraphError::MissionScopeNotFound {
                        node_guid: node.node_guid,
                        scope,
                    });
                }
            }
        }
        if !self.has_node(self.start_node_guid) {
            return Err(GraphError::StartNodeNotFound {
                node_guid: self.start_node_guid,
            });
        }
        for edge in &self.edges {
            let source =
                self.node(edge.source_node_guid)
                    .ok_or(GraphError::EdgeReferencesMissingNode {
                        node_guid: edge.source_node_guid,
                    })?;
            let target =
                self.node(edge.target_node_guid)
                    .ok_or(GraphError::EdgeReferencesMissingNode {
                        node_guid: edge.target_node_guid,
                    })?;
            find_pin(source, &edge.source_pin_name, false).ok_or_else(|| {
                GraphError::EdgeReferencesUnknownPin {
                    node_guid: edge.source_node_guid,
                    pin_name: edge.source_pin_name.clone(),
                }
            })?;
            find_pin(target, &edge.target_pin_name, true).ok_or_else(|| {
                GraphError::EdgeReferencesUnknownPin {
                    node_guid: edge.target_node_guid,
                    pin_name: edge.target_pin_name.clone(),
                }
            })?;
        }
        self.validate_acyclic_zero_cost()
    }

    /// A cycle made only of zero-cost nodes never consumes frame budget, so a
    /// token can loop forever inside one tick. Every node on the cycle must be
    /// rejected at load time (REQ-FG-03).
    fn validate_acyclic_zero_cost(&self) -> Result<(), GraphError> {
        // A Choice or QBN hub stops the token until the host supplies input, so
        // a loop through one cannot spin inside a single tick even when every
        // node costs zero.
        let zero_cost: BTreeSet<u32> = self
            .nodes
            .iter()
            .filter(|node| node.cost_ms == 0)
            .filter(|node| {
                !matches!(
                    node.kind,
                    NodeKind::Choice { .. } | NodeKind::QbnHub { .. }
                )
            })
            .map(|node| node.node_guid)
            .collect();
        let mut adjacency: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
        for edge in &self.edges {
            if zero_cost.contains(&edge.source_node_guid) && zero_cost.contains(&edge.target_node_guid)
            {
                adjacency
                    .entry(edge.source_node_guid)
                    .or_default()
                    .push(edge.target_node_guid);
            }
        }
        for targets in adjacency.values_mut() {
            targets.sort_unstable();
            targets.dedup();
        }
        // 0 = unvisited, 1 = on the current stack, 2 = finished.
        let mut state: BTreeMap<u32, u8> = BTreeMap::new();
        for &start in zero_cost.iter() {
            if state.get(&start).copied().unwrap_or(0) != 0 {
                continue;
            }
            let mut stack: Vec<(u32, usize)> = vec![(start, 0)];
            state.insert(start, 1);
            while let Some((node, position)) = stack.pop() {
                let targets = adjacency.get(&node).map(Vec::as_slice).unwrap_or(&[]);
                if position < targets.len() {
                    stack.push((node, position + 1));
                    let next = targets[position];
                    match state.get(&next).copied().unwrap_or(0) {
                        0 => {
                            state.insert(next, 1);
                            stack.push((next, 0));
                        }
                        1 => return Err(GraphError::ZeroCostCycle { node_guid: next }),
                        _ => {}
                    }
                } else {
                    state.insert(node, 2);
                }
            }
        }
        Ok(())
    }

    pub fn has_node(&self, node_guid: u32) -> bool {
        self.nodes.iter().any(|node| node.node_guid == node_guid)
    }

    pub fn node(&self, node_guid: u32) -> Option<&FlowNodeDef> {
        self.nodes.iter().find(|node| node.node_guid == node_guid)
    }

    pub fn input_pin_index(&self, node_guid: u32, name: &str) -> Option<u16> {
        let node = self.node(node_guid)?;
        node.input_pins
            .iter()
            .position(|pin| pin.pin_name == name)
            .and_then(|index| u16::try_from(index).ok())
    }

    pub fn input_pin_name(&self, node_guid: u32, index: u16) -> Option<&str> {
        self.node(node_guid)?
            .input_pins
            .get(index as usize)
            .map(|pin| pin.pin_name.as_str())
    }

    /// Incoming edges in deterministic order: source GUID, then source pin.
    pub fn incoming_edges(&self, node_guid: u32) -> Vec<&EdgeConnection> {
        let mut edges: Vec<&EdgeConnection> = self
            .edges
            .iter()
            .filter(|edge| edge.target_node_guid == node_guid)
            .collect();
        edges.sort_by(|a, b| {
            a.source_node_guid
                .cmp(&b.source_node_guid)
                .then_with(|| a.source_pin_name.cmp(&b.source_pin_name))
        });
        edges
    }

    /// Outgoing edges in deterministic order: target GUID, then target pin.
    pub fn outgoing_edges(&self, node_guid: u32, pin_name: &str) -> Vec<&EdgeConnection> {
        let mut edges: Vec<&EdgeConnection> = self
            .edges
            .iter()
            .filter(|edge| edge.source_node_guid == node_guid && edge.source_pin_name == pin_name)
            .collect();
        edges.sort_by(|a, b| {
            a.target_node_guid
                .cmp(&b.target_node_guid)
                .then_with(|| a.target_pin_name.cmp(&b.target_pin_name))
        });
        edges
    }

    pub fn has_direct_edge(&self, from: u32, to: u32) -> bool {
        self.edges
            .iter()
            .any(|edge| edge.source_node_guid == from && edge.target_node_guid == to)
    }

    /// Shortest edge path from `from` to `to`, as `(node_guid, input_pin,
    /// output_pin)` steps for the intermediate nodes only. Adjacency is ordered
    /// by target GUID then target pin, so the result is reproducible.
    pub fn shortest_bridge_path(
        &self,
        from: u32,
        to: u32,
    ) -> Result<Vec<(u32, String, String)>, GraphError> {
        if from == to {
            return Ok(Vec::new());
        }
        let mut adjacency: Vec<(u32, u32, String, String)> = self
            .edges
            .iter()
            .map(|edge| {
                (
                    edge.source_node_guid,
                    edge.target_node_guid,
                    edge.source_pin_name.clone(),
                    edge.target_pin_name.clone(),
                )
            })
            .collect();
        // BFS over the whole edge set. Sorting by source node, target node and
        // target pin makes the chosen path reproducible.
        adjacency.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.3.cmp(&b.3))
        });
        // parent[node] = (previous node, pin entered on `node`, pin left on
        // `previous`).
        let mut parent: BTreeMap<u32, (u32, String, String)> = BTreeMap::new();
        let mut queue = vec![from];
        let mut visited = BTreeSet::from([from]);
        let mut cursor = 0;
        while cursor < queue.len() {
            let current = queue[cursor];
            cursor += 1;
            for edge in adjacency.iter().filter(|edge| edge.0 == current) {
                let next = edge.1;
                if !visited.insert(next) {
                    continue;
                }
                parent.insert(next, (edge.0, edge.3.clone(), edge.2.clone()));
                if next == to {
                    return Ok(self.path_steps(&parent, from, to));
                }
                queue.push(next);
            }
        }
        Err(GraphError::NoBridgePath {
            from_node_guid: from,
            to_node_guid: to,
        })
    }

    fn path_steps(
        &self,
        parent: &BTreeMap<u32, (u32, String, String)>,
        from: u32,
        to: u32,
    ) -> Vec<(u32, String, String)> {
        let mut chain = vec![to];
        let mut cursor = to;
        while let Some((previous, _, _)) = parent.get(&cursor) {
            if *previous == from {
                break;
            }
            cursor = *previous;
            chain.push(cursor);
        }
        chain.reverse();
        let mut steps = Vec::new();
        for pair in chain.windows(2) {
            let node = pair[0];
            let next = pair[1];
            if node == from || node == to {
                continue;
            }
            let (_, input_pin, _) = parent
                .get(&node)
                .expect("bridge step has an entering edge");
            let (_, _, output_pin) = parent
                .get(&next)
                .expect("bridge step has a leaving edge");
            steps.push((node, input_pin.clone(), output_pin.clone()));
        }
        steps
    }
}

fn find_pin<'a>(
    node: &'a FlowNodeDef,
    pin_name: &str,
    is_input: bool,
) -> Option<&'a PinDefinition> {
    let pins = if is_input {
        &node.input_pins
    } else {
        &node.output_pins
    };
    pins.iter().find(|pin| pin.pin_name == pin_name)
}

/// Errors raised by the flow-graph runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum GraphError {
    EmptyAssetId,
    EmptyAsset,
    DuplicateNodeGuid {
        node_guid: u32,
    },
    EmptyNodeTypeId {
        node_guid: u32,
    },
    NodeHasNoPins {
        node_guid: u32,
    },
    NegativeNodeCost {
        node_guid: u32,
        cost_ms: i64,
    },
    NodeCostTooLarge {
        node_guid: u32,
        cost_ms: i64,
    },
    DuplicatePinName {
        node_guid: u32,
        pin_name: String,
        is_input: bool,
    },
    NonFiniteMetricDelta {
        node_guid: u32,
        delta: f32,
    },
    MissionScopeSelfReference {
        node_guid: u32,
    },
    MissionScopeNotFound {
        node_guid: u32,
        scope: u32,
    },
    /// A mission node has no child instance attached (REQ-CH-04).
    MissionAssetNotAttached {
        node_guid: u32,
        sub_graph_asset_id: String,
    },
    /// The supplied child asset does not match the node's `sub_graph_asset_id`.
    MissionAssetIdMismatch {
        node_guid: u32,
        asset_id: String,
    },
    /// The child asset already appears in this instance's ancestor chain.
    MissionAssetCycle {
        node_guid: u32,
        asset_id: String,
    },
    MissionAssetAlreadyAttached {
        node_guid: u32,
    },
    MissionReentry {
        node_guid: u32,
    },
    SceneReopenNotTail {
        node_guid: u32,
    },
    StartNodeNotFound {
        node_guid: u32,
    },
    EdgeReferencesMissingNode {
        node_guid: u32,
    },
    EdgeReferencesUnknownPin {
        node_guid: u32,
        pin_name: String,
    },
    NodeNotFound {
        node_guid: u32,
    },
    PinNotFound {
        node_guid: u32,
        pin_name: String,
    },
    PinWrongDirection {
        node_guid: u32,
        pin_name: String,
        expected_input: bool,
    },
    JumpTargetNotFound {
        node_guid: u32,
        storylet_id: u16,
    },
    UnresolvedStorylet {
        storylet_id: u16,
    },
    /// A direct storylet selection named an id outside the node's authored pool.
    StoryletNotInDeck {
        node_guid: u32,
        storylet_id: u16,
    },
    /// A direct storylet selection named a storylet that is not currently
    /// playable (completion lock, preconditions, arc phase, or cooldown).
    StoryletNotPlayable {
        node_guid: u32,
        storylet_id: u16,
    },
    NoSalienceMatch {
        node_guid: u32,
        pool_len: usize,
    },
    NoDeclaredRoute {
        node_guid: u32,
    },
    NoIncomingEdge {
        node_guid: u32,
    },
    NoBridgePath {
        from_node_guid: u32,
        to_node_guid: u32,
    },
    UnresolvedPrerequisiteChain {
        node_guid: u32,
        missing_tags: Vec<u16>,
    },
    EmptyTimeWindow {
        node_guid: u32,
        cursor_ms: i64,
    },
    NegativeFrameBudget(i64),
    /// A cycle of zero-cost nodes would never consume frame budget.
    ZeroCostCycle {
        node_guid: u32,
    },
    /// A frame exceeded the per-tick execution cap, which also stops
    /// zero-cost tokens that were authored after validation.
    ExecutionLimitExceeded {
        limit: usize,
    },
    NegativeSectionDuration {
        node_guid: u32,
        duration_ms: i64,
    },
    NegativeEventTime {
        node_guid: u32,
        event_id: u64,
    },
    /// A signal token minted by another instance must not drive this graph.
    TokenInstanceMismatch {
        token_instance_id: u64,
        instance_id: u64,
    },
    CallDataMismatch {
        node_guid: u32,
    },
    RevertStackEmpty {
        node_guid: u32,
    },
    CommitNotFound {
        commit_index: usize,
    },
    /// A turn-based cooldown cannot be honored because the runtime clock is
    /// integer milliseconds and no turn-to-millisecond conversion was supplied.
    TurnCooldownNotConvertible {
        storylet_id: u16,
        cooldown_turns: u64,
    },
    /// A named tag effect cannot be resolved without a compiled registry.
    RegistryRequired {
        node_guid: u32,
        tag_name: String,
    },
    PassionRegistryRequired {
        node_guid: u32,
    },
    NonFiniteMetricValue {
        node_guid: u32,
        value: f32,
    },
    ConditionalBranchPinMissing {
        node_guid: u32,
    },
    EmptyVariableName {
        node_guid: u32,
    },
    VariableTypeMismatch {
        node_guid: u32,
        name: String,
    },
    MetricIndexOutOfRange {
        node_guid: u32,
        metric_index: u16,
    },
    EntityIndexOutOfRange {
        node_guid: u32,
        entity_index: u16,
    },
    EmptyCondition {
        node_guid: u32,
    },
    EmptyVariableAssignment {
        node_guid: u32,
    },
    TeleportDestinationNotFound {
        node_guid: u32,
        destination: u32,
    },
    InsufficientBudget {
        node_guid: u32,
        cost_ms: i64,
        budget_ms: i64,
    },
    TopicEdgeNotFound {
        node_guid: u32,
        edge_id: String,
    },
    TopicEdgeNotAdjacent {
        node_guid: u32,
        edge_id: String,
        current_topic_id: String,
    },
    TopicNodeNotFound {
        node_guid: u32,
        topic_id: String,
    },
    UnsupportedNode {
        node_guid: u32,
        category: NodeCategory,
    },
    State(ChronoStateError),
    Dispatch(DispatchError),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::EmptyAssetId => write!(f, "asset id is empty"),
            GraphError::EmptyAsset => write!(f, "asset has no nodes"),
            GraphError::DuplicateNodeGuid { node_guid } => {
                write!(f, "duplicate node guid {node_guid}")
            }
            GraphError::EmptyNodeTypeId { node_guid } => {
                write!(f, "node {node_guid} has an empty node type id")
            }
            GraphError::NodeHasNoPins { node_guid } => {
                write!(f, "node {node_guid} has no pins in one direction")
            }
            GraphError::NegativeNodeCost { node_guid, cost_ms } => {
                write!(f, "node {node_guid} has negative cost {cost_ms} ms")
            }
            GraphError::NodeCostTooLarge { node_guid, cost_ms } => write!(
                f,
                "node {node_guid} cost {cost_ms} ms exceeds the packed token range"
            ),
            GraphError::DuplicatePinName {
                node_guid,
                pin_name,
                is_input,
            } => write!(
                f,
                "node {node_guid} declares duplicate {} pin '{pin_name}'",
                if *is_input { "input" } else { "output" }
            ),
            GraphError::NonFiniteMetricDelta { node_guid, delta } => {
                write!(f, "node {node_guid} has a non-finite metric delta {delta}")
            }
            GraphError::MissionScopeSelfReference { node_guid } => {
                write!(f, "node {node_guid} scopes its own mission")
            }
            GraphError::MissionScopeNotFound { node_guid, scope } => write!(
                f,
                "node {node_guid} scopes mission {scope} which is not in the asset"
            ),
            GraphError::MissionAssetNotAttached {
                node_guid,
                sub_graph_asset_id,
            } => write!(
                f,
                "mission node {node_guid} has no instance attached for asset {sub_graph_asset_id}"
            ),
            GraphError::MissionAssetIdMismatch {
                node_guid,
                asset_id,
            } => write!(
                f,
                "mission node {node_guid} does not reference the supplied asset {asset_id}"
            ),
            GraphError::MissionAssetCycle {
                node_guid,
                asset_id,
            } => write!(
                f,
                "mission node {node_guid} attaches asset {asset_id}, which is already in its ancestor chain"
            ),
            GraphError::MissionAssetAlreadyAttached { node_guid } => write!(
                f,
                "mission node {node_guid} already has an instance attached"
            ),
            GraphError::MissionReentry { node_guid } => write!(
                f,
                "mission node {node_guid} cannot be re-entered after completion"
            ),
            GraphError::SceneReopenNotTail { node_guid } => write!(
                f,
                "scene section {node_guid} can only be reopened while its completion is the history head"
            ),
            GraphError::StartNodeNotFound { node_guid } => {
                write!(f, "start node {node_guid} is not in the asset")
            }
            GraphError::EdgeReferencesMissingNode { node_guid } => {
                write!(f, "edge references missing node {node_guid}")
            }
            GraphError::EdgeReferencesUnknownPin {
                node_guid,
                pin_name,
            } => write!(
                f,
                "edge references unknown pin {pin_name} on node {node_guid}"
            ),
            GraphError::NodeNotFound { node_guid } => {
                write!(f, "node {node_guid} is not in the instance")
            }
            GraphError::PinNotFound {
                node_guid,
                pin_name,
            } => write!(f, "pin {pin_name} is not defined on node {node_guid}"),
            GraphError::PinWrongDirection {
                node_guid,
                pin_name,
                expected_input,
            } => write!(
                f,
                "pin {pin_name} on node {node_guid} is not a {} pin",
                if *expected_input { "input" } else { "output" }
            ),
            GraphError::UnresolvedStorylet { storylet_id } => {
                write!(f, "storylet {storylet_id} is not in the dispatcher")
            }
            GraphError::StoryletNotInDeck {
                node_guid,
                storylet_id,
            } => write!(
                f,
                "storylet {storylet_id} is not in the pool authored on node {node_guid}"
            ),
            GraphError::StoryletNotPlayable {
                node_guid,
                storylet_id,
            } => write!(
                f,
                "storylet {storylet_id} is not playable when selected from node {node_guid}"
            ),
            GraphError::NoSalienceMatch {
                node_guid,
                pool_len,
            } => write!(
                f,
                "salience node {node_guid} matched nothing in a pool of {pool_len}"
            ),
            GraphError::NoDeclaredRoute { node_guid } => write!(
                f,
                "node {node_guid} has no declared route and no default output pin"
            ),
            GraphError::NoIncomingEdge { node_guid } => {
                write!(f, "node {node_guid} has no incoming edge to traverse")
            }
            GraphError::NoBridgePath {
                from_node_guid,
                to_node_guid,
            } => write!(f, "no bridge path from {from_node_guid} to {to_node_guid}"),
            GraphError::UnresolvedPrerequisiteChain {
                node_guid,
                missing_tags,
            } => write!(
                f,
                "node {node_guid} cannot satisfy its prerequisite chain, missing tags {missing_tags:?}"
            ),
            GraphError::EmptyTimeWindow {
                node_guid,
                cursor_ms,
            } => write!(
                f,
                "empty time window on node {node_guid} at cursor {cursor_ms} ms"
            ),
            GraphError::NegativeFrameBudget(value) => write!(f, "negative frame budget {value} ms"),
            GraphError::ZeroCostCycle { node_guid } => write!(
                f,
                "node {node_guid} is part of a cycle whose nodes all cost zero milliseconds"
            ),
            GraphError::ExecutionLimitExceeded { limit } => {
                write!(f, "frame exceeded the {limit} execution limit")
            }
            GraphError::NegativeSectionDuration {
                node_guid,
                duration_ms,
            } => write!(
                f,
                "scene section {node_guid} has a negative duration of {duration_ms} ms"
            ),
            GraphError::NegativeEventTime { node_guid, event_id } => write!(
                f,
                "scene section {node_guid} event {event_id} has a negative time"
            ),
            GraphError::TokenInstanceMismatch {
                token_instance_id,
                instance_id,
            } => write!(
                f,
                "token from instance {token_instance_id} cannot drive instance {instance_id}"
            ),
            GraphError::CallDataMismatch { node_guid } => write!(
                f,
                "call data for node {node_guid} does not match the recorded delta"
            ),
            GraphError::RevertStackEmpty { node_guid } => {
                write!(f, "call data for node {node_guid} is empty")
            }
            GraphError::CommitNotFound { commit_index } => {
                write!(f, "commit {commit_index} is not in the history engine")
            }
            GraphError::TurnCooldownNotConvertible {
                storylet_id,
                cooldown_turns,
            } => write!(
                f,
                "storylet {storylet_id} has a turn-based cooldown of {cooldown_turns} turns and no turn-to-millisecond conversion; keep the legacy turn path for this storylet"
            ),
            GraphError::UnsupportedNode {
                node_guid,
                category,
            } => write!(
                f,
                "node {node_guid} of category {category:?} does not support this endpoint"
            ),
            GraphError::RegistryRequired {
                node_guid,
                tag_name,
            } => write!(
                f,
                "node {node_guid} needs the tag registry to resolve {tag_name}"
            ),
            GraphError::PassionRegistryRequired { node_guid } => write!(
                f,
                "passion mutation at node {node_guid} needs the passion registry"
            ),
            GraphError::InsufficientBudget {
                node_guid,
                cost_ms,
                budget_ms,
            } => write!(
                f,
                "node {node_guid} costs {cost_ms} ms but the token carries {budget_ms} ms"
            ),
            GraphError::TopicEdgeNotFound { node_guid, edge_id } => {
                write!(f, "node {node_guid} has no topic edge {edge_id}")
            }
            GraphError::TopicEdgeNotAdjacent {
                node_guid,
                edge_id,
                current_topic_id,
            } => write!(
                f,
                "topic edge {edge_id} on node {node_guid} does not leave current topic {current_topic_id}"
            ),
            GraphError::TopicNodeNotFound {
                node_guid,
                topic_id,
            } => write!(f, "node {node_guid} has no topic node {topic_id}"),
            GraphError::NonFiniteMetricValue { node_guid, value } => write!(
                f,
                "node {node_guid} assigns non-finite metric value {value}"
            ),
            GraphError::ConditionalBranchPinMissing { node_guid } => write!(
                f,
                "node {node_guid} has a condition branch that is not a declared output pin"
            ),
            GraphError::EmptyVariableName { node_guid } => {
                write!(f, "node {node_guid} mutates an unnamed variable")
            }
            GraphError::VariableTypeMismatch { node_guid, name } => write!(
                f,
                "node {node_guid} applies an operator that does not fit variable {name}"
            ),
            GraphError::MetricIndexOutOfRange {
                node_guid,
                metric_index,
            } => write!(
                f,
                "node {node_guid} reads metric {metric_index} outside the metric index space"
            ),
            GraphError::EntityIndexOutOfRange {
                node_guid,
                entity_index,
            } => write!(
                f,
                "node {node_guid} reads entity {entity_index} outside the entity index space"
            ),
            GraphError::EmptyCondition { node_guid } => {
                write!(f, "node {node_guid} has a condition with nothing to test")
            }
            GraphError::EmptyVariableAssignment { node_guid } => {
                write!(f, "node {node_guid} has no variable assignments")
            }
            GraphError::JumpTargetNotFound {
                node_guid,
                storylet_id,
            } => write!(
                f,
                "node {node_guid} jumps to storylet {storylet_id} without a graph target"
            ),
            GraphError::TeleportDestinationNotFound {
                node_guid,
                destination,
            } => write!(
                f,
                "teleport at node {node_guid} targets unknown node {destination}"
            ),
            GraphError::State(error) => write!(f, "{error}"),
            GraphError::Dispatch(error) => write!(f, "{error}"),
        }
    }
}

impl Error for GraphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            GraphError::State(error) => Some(error),
            GraphError::Dispatch(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ChronoStateError> for GraphError {
    fn from(error: ChronoStateError) -> Self {
        GraphError::State(error)
    }
}

impl From<DispatchError> for GraphError {
    fn from(error: DispatchError) -> Self {
        GraphError::Dispatch(error)
    }
}

/// Everything a node execution needs from outside the graph: world state,
/// storylet policy, and the optional hierarchical registry that makes tag
/// predicates and effects resolve through ancestor masks.
pub struct RuntimeContext<'a> {
    pub state: &'a mut WorldStateTable,
    pub dispatcher: &'a UnifiedDispatcher,
    pub registry: Option<&'a GameplayTagRegistry>,
    pub passion_registry: Option<&'a PassionRegistry>,
}

impl<'a> RuntimeContext<'a> {
    pub fn new(state: &'a mut WorldStateTable, dispatcher: &'a UnifiedDispatcher) -> Self {
        Self {
            state,
            dispatcher,
            registry: None,
            passion_registry: None,
        }
    }

    pub fn with_registry(
        state: &'a mut WorldStateTable,
        dispatcher: &'a UnifiedDispatcher,
        registry: &'a GameplayTagRegistry,
    ) -> Self {
        Self {
            state,
            dispatcher,
            registry: Some(registry),
            passion_registry: None,
        }
    }
    pub fn with_registries(
        state: &'a mut WorldStateTable,
        dispatcher: &'a UnifiedDispatcher,
        registry: &'a GameplayTagRegistry,
        passion_registry: &'a PassionRegistry,
    ) -> Self {
        Self {
            state,
            dispatcher,
            registry: Some(registry),
            passion_registry: Some(passion_registry),
        }
    }

    fn apply_op(&mut self, op: &MutationOp) -> Result<BijectiveDelta, GraphError> {
        match self.registry {
            Some(registry) => {
                apply_mutation_with_registry(self.state, op, registry).map_err(GraphError::State)
            }
            None => apply_mutation(self.state, op).map_err(GraphError::State),
        }
    }
}

/// Salience coverage for one dynamic dispatcher node, so a caller can prove the
/// pool is not starved before the node runs (REQ-SD-03).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SalienceCoverage {
    pub node_guid: u32,
    pub eligible: Vec<u16>,
    pub has_fallback: bool,
}

impl SceneComposer {
    /// Sort the parts by priority key then combiner entity, arbitrate one winner
    /// per entity, and dispatch the winners in that order (REQ-SC-02, REQ-SC-03).
    /// Ties are broken by node GUID then event id, so the winner is reproducible.
    pub fn execute_sorted_action_parts(
        &mut self,
        node_guid: u32,
        parts: &[ActionPart],
        is_reverse: bool,
    ) -> Result<Vec<ComposerCommand>, GraphError> {
        self.execute_sorted_action_parts_multi(&[(node_guid, parts)], is_reverse)
    }

    /// Arbitrate commands from several concurrent scene sections in one pass so
    /// conflicts span nodes (REQ-SC-06).
    pub fn execute_sorted_action_parts_multi(
        &mut self,
        owners: &[(u32, &[ActionPart])],
        is_reverse: bool,
    ) -> Result<Vec<ComposerCommand>, GraphError> {
        if owners.iter().all(|(_, parts)| parts.is_empty()) {
            return Ok(Vec::new());
        }
        let mut commands: Vec<ComposerCommand> = Vec::new();
        for (node_guid, parts) in owners {
            for part in *parts {
                commands.push(ComposerCommand {
                    node_guid: *node_guid,
                    combiner_entity_path: part.combiner_entity_path.clone(),
                    activity: part.activity,
                    action_key_priority: part.action_key_priority,
                    event_id: part.event_ref.event_id,
                    payload_asset_id: part.event_ref.payload_asset_id.clone(),
                    cached_previous_pose_id: part.event_ref.cached_previous_pose_id.clone(),
                });
            }
        }
        commands.sort_by(|a, b| {
            a.action_key_priority
                .cmp(&b.action_key_priority)
                .then_with(|| a.combiner_entity_path.cmp(&b.combiner_entity_path))
                .then_with(|| a.node_guid.cmp(&b.node_guid))
                .then_with(|| a.event_id.cmp(&b.event_id))
        });

        let mut winner: BTreeMap<String, usize> = BTreeMap::new();
        for (index, command) in commands.iter().enumerate() {
            let entity = command.combiner_entity_path.clone();
            match winner.get(&entity).copied() {
                None => {
                    winner.insert(entity, index);
                }
                Some(current) if beats(&commands[current], command) => {
                    winner.insert(entity, index);
                }
                Some(_) => {}
            }
        }

        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, command) in commands.iter().enumerate() {
            groups
                .entry(command.combiner_entity_path.clone())
                .or_default()
                .push(index);
        }
        for (entity, indices) in &groups {
            let chosen = *winner.get(entity).expect("group has a winner");
            if indices.len() < 2 {
                continue;
            }
            let mut rejected: Vec<ComposerCommand> = indices
                .iter()
                .filter(|&&index| index != chosen)
                .map(|&index| commands[index].clone())
                .collect();
            rejected.sort_by(|a, b| {
                b.action_key_priority
                    .cmp(&a.action_key_priority)
                    .then_with(|| a.node_guid.cmp(&b.node_guid))
                    .then_with(|| a.event_id.cmp(&b.event_id))
            });
            self.conflicts.push(ComposerConflict {
                combiner_entity_path: entity.clone(),
                winner: commands[chosen].clone(),
                rejected,
            });
        }

        let mut applied = Vec::new();
        for (index, command) in commands.iter().enumerate() {
            if winner.get(&command.combiner_entity_path).copied() != Some(index) {
                continue;
            }
            let pose = if is_reverse {
                command.cached_previous_pose_id.clone()
            } else {
                command.payload_asset_id.clone()
            };
            self.actor_pose
                .insert(command.combiner_entity_path.clone(), pose);
            self.applied.push(command.clone());
            applied.push(command.clone());
        }
        Ok(applied)
    }
}

/// Composer arbitration order: higher priority key wins, then the smaller node
/// GUID, then the smaller event id.
fn beats(winner: &ComposerCommand, challenger: &ComposerCommand) -> bool {
    challenger.action_key_priority > winner.action_key_priority
        || (challenger.action_key_priority == winner.action_key_priority
            && (challenger.node_guid < winner.node_guid
                || (challenger.node_guid == winner.node_guid
                    && challenger.event_id < winner.event_id)))
}

/// One active execution of an asset (REQ-FG-05).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FlowGraphInstance {
    pub instance_id: u64,
    pub asset: FlowGraphAsset,
    /// Node instances, ordered by node GUID.
    pub nodes: Vec<FlowNodeInstance>,
    /// Flat token vector, the runtime ring buffer from REQ-FG-03.
    pub tokens: Vec<SignalToken>,
    /// Linear call history for the branch that reaches the current head.
    pub global_call_history: Vec<CallHistoryEntry>,
    pub history: PersistentHistoryEngine,
    pub composer: SceneComposer,
    /// Fixed-point integer-millisecond clock.
    pub clock_ms: i64,
    pub seed: u64,
    pub next_token_id: u64,
    /// Turn counter. The runtime clock is milliseconds, so a turn-based cooldown
    /// is only honored when `ms_per_turn` is supplied; otherwise the caller keeps
    /// the legacy turn path.
    pub turn: u64,
    pub ms_per_turn: Option<i64>,
    /// Frame at which each storylet last completed, for cooldown checks.
    pub last_completed_ms: BTreeMap<u16, i64>,
    /// Once-only lock across every dispatcher node in this instance, not just
    /// the node that completed the storylet (REQ-BB-02).
    #[serde(default)]
    pub completed_storylets: BTreeSet<u16>,
    /// Local variable store for typed assignments (REQ-VAR).
    pub variables: VariableStore,
    /// Asset ids from the root instance down to this instance, used to reject
    /// mission cycles deterministically (REQ-CH-04).
    pub mission_asset_path: Vec<String>,
    /// Transient host events from newly committed passion threshold crossings.
    /// Timeline reconstruction suppresses replayed historical events.
    #[serde(skip)]
    pub pending_passion_events: Vec<PassionThresholdTriggerEvent>,
}

impl FlowGraphInstance {
    pub fn new(instance_id: u64, asset: FlowGraphAsset, seed: u64) -> Result<Self, GraphError> {
        let asset_id = asset.asset_id.clone();
        Self::new_at_path(instance_id, asset, seed, vec![asset_id])
    }

    /// Build an instance with a known mission ancestor chain, so attaching a
    /// child asset can reject a cycle at attach time (REQ-CH-04).
    fn new_at_path(
        instance_id: u64,
        asset: FlowGraphAsset,
        seed: u64,
        mission_asset_path: Vec<String>,
    ) -> Result<Self, GraphError> {
        asset.validate()?;
        let mut nodes: Vec<FlowNodeInstance> = asset
            .nodes
            .iter()
            .map(|node| FlowNodeInstance {
                node_guid: node.node_guid,
                entity_path: node.entity_path.clone(),
                ..FlowNodeInstance::default()
            })
            .collect();
        nodes.sort_by_key(|node| node.node_guid);
        Ok(Self {
            instance_id,
            asset,
            nodes,
            tokens: Vec::new(),
            global_call_history: Vec::new(),
            history: PersistentHistoryEngine::new(),
            composer: SceneComposer::default(),
            clock_ms: 0,
            seed,
            next_token_id: 0,
            turn: 0,
            ms_per_turn: None,
            last_completed_ms: BTreeMap::new(),
            completed_storylets: BTreeSet::new(),
            variables: VariableStore::default(),
            mission_asset_path,
            pending_passion_events: Vec::new(),
        })
    }

    /// Clear every runtime record without touching the asset.
    pub fn reset(&mut self) {
        for node in &mut self.nodes {
            let node_guid = node.node_guid;
            let entity_path = self
                .asset
                .node(node_guid)
                .map(|def| def.entity_path.clone())
                .unwrap_or_default();
            // Attachments survive a reset: the child instance is reset in place,
            // so a load rebuilds mission state from its scoped array.
            let mut sub_graph = node.sub_graph.take();
            if let Some(child) = sub_graph.as_mut() {
                child.reset();
            }
            *node = FlowNodeInstance {
                node_guid,
                entity_path,
                sub_graph,
                ..FlowNodeInstance::default()
            };
        }
        self.variables.clear();
        self.tokens.clear();
        self.global_call_history.clear();
        self.history = PersistentHistoryEngine::new();
        self.composer = SceneComposer::default();
        self.clock_ms = 0;
        self.next_token_id = 0;
        self.turn = 0;
        self.last_completed_ms.clear();
        self.completed_storylets.clear();
        self.pending_passion_events.clear();
    }
    /// Non-empty scoped mission arrays anywhere in this instance tree, keyed by
    /// mission node GUID. This is the replay source for nested missions.
    pub fn nested_mission_histories(&self) -> BTreeMap<u32, Vec<CallHistoryEntry>> {
        fn collect(instance: &FlowGraphInstance, out: &mut BTreeMap<u32, Vec<CallHistoryEntry>>) {
            for node in &instance.nodes {
                if !node.local_call_history.is_empty() {
                    out.insert(node.node_guid, node.local_call_history.clone());
                }
                if let Some(child) = node.sub_graph.as_deref() {
                    collect(child, out);
                }
            }
        }
        let mut out = BTreeMap::new();
        collect(self, &mut out);
        out
    }

    /// Write scoped mission arrays back into this instance tree. Used after a
    /// reset that must clear runtime state but keep the replay source alive.
    fn restore_mission_mirrors(
        &mut self,
        mirrors: &BTreeMap<u32, Vec<CallHistoryEntry>>,
    ) {
        for node in &mut self.nodes {
            if let Some(entries) = mirrors.get(&node.node_guid) {
                node.local_call_history.clone_from(entries);
            }
            if let Some(child) = node.sub_graph.as_mut() {
                child.restore_mission_mirrors(mirrors);
            }
        }
    }

    /// Take committed passion threshold events, including events from mission children.
    pub fn drain_passion_threshold_events(&mut self) -> Vec<PassionThresholdTriggerEvent> {
        let mut events = std::mem::take(&mut self.pending_passion_events);
        for node in &mut self.nodes {
            if let Some(child) = node.sub_graph.as_mut() {
                events.extend(child.drain_passion_threshold_events());
            }
        }
        events
    }

    fn clear_pending_passion_threshold_events(&mut self) {
        self.pending_passion_events.clear();
        for node in &mut self.nodes {
            if let Some(child) = node.sub_graph.as_mut() {
                child.clear_pending_passion_threshold_events();
            }
        }
    }

    pub fn node_index(&self, node_guid: u32) -> Option<usize> {
        let index = self
            .nodes
            .partition_point(|node| node.node_guid < node_guid);
        if index < self.nodes.len() && self.nodes[index].node_guid == node_guid {
            Some(index)
        } else {
            None
        }
    }

    pub fn node(&self, node_guid: u32) -> Option<&FlowNodeInstance> {
        self.nodes.get(self.node_index(node_guid)?)
    }

    pub fn node_mut(&mut self, node_guid: u32) -> Option<&mut FlowNodeInstance> {
        let index = self.node_index(node_guid)?;
        self.nodes.get_mut(index)
    }

    pub fn call_history(&self) -> &[CallHistoryEntry] {
        &self.global_call_history
    }
    /// Deliver an external gameplay verb and immediately release every matching
    /// latent waiter, regardless of its timeout or required-tag condition.
    pub fn signal_event(
        &mut self,
        ctx: &mut RuntimeContext,
        event_name: &str,
    ) -> Result<Vec<SignalToken>, GraphError> {
        if event_name.is_empty() {
            return Ok(Vec::new());
        }
        let waiting: Vec<(u32, String)> = self
            .nodes
            .iter()
            .filter(|node| node.state == NodeExecutionState::ActiveLatent)
            .filter_map(|node| {
                let def = self.asset.node(node.node_guid)?;
                let NodeKind::Waiter { wait_event, .. } = &def.kind else {
                    return None;
                };
                (wait_event == event_name).then(|| {
                    (
                        node.node_guid,
                        node.executed_input_pin
                            .clone()
                            .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_string()),
                    )
                })
            })
            .collect();
        let mut emitted = Vec::new();
        for (node_guid, input_pin) in waiting {
            let plan = self.plan(ctx, node_guid, &input_pin, None, None, 0, true, true)?;
            emitted.extend(self.commit_plan(&plan)?);
        }
        Ok(emitted)
    }

    /// Deterministic per-node seed, derived from the instance seed and the node
    /// GUID. Same mixing as the dispatcher's deterministic index.
    pub fn next_deterministic_seed(&mut self, node_guid: u32) -> u64 {
        self.seed = mix64(self.seed ^ (node_guid as u64).wrapping_add(0x9e37_79b9_7f4a_7755));
        self.seed
    }
}

fn mix64(value: u64) -> u64 {
    let mut x = value;
    x = (x ^ (x >> 30)).wrapping_mul(0xbf03_9999_7171_5f15);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// A node execution decided against current state. `plan` performs the world
/// change; `commit_plan` applies the record side effects (history, call data,
/// node state). Splitting them keeps the borrow discipline simple: no borrow into
/// the instance is held across a record call.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct NodePlan {
    node_guid: u32,
    input_pin: String,
    output_pin: Option<String>,
    storylet_id: Option<u16>,
    delta: BijectiveDelta,
    call_data: Option<NodeCallData>,
    completed: bool,
    arrived_input_pins: BTreeSet<String>,
    completed_storylets: Option<BTreeSet<u16>>,
    current_topic_id: Option<String>,
    edge_use_count: Option<(String, i64)>,
    section_clock_ms: Option<i64>,
    waiter_wake_ms: Option<i64>,
    remaining_budget_ms: i64,
    emit: bool,
    record: bool,
    variables: Vec<(VariableScope, String, VariableValue)>,
    deferred_target: Option<u32>,
    commit: bool,
}

/// One scene section's planned slice of a frame, before arbitration.
#[derive(Clone, Debug)]
struct SectionFrame {
    node_guid: u32,
    is_reverse: bool,
    parts: Vec<ActionPart>,
    t_start: i64,
    t_end: i64,
    just_completed: bool,
    reopened: bool,
    previous_poses: BTreeMap<(u64, String), Option<String>>,
}

impl FlowGraphInstance {
    /// Decide one node execution for any kind through the same path.
    fn plan(
        &self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        input_pin: &str,
        storylet_id: Option<u16>,
        chosen_pin: Option<&str>,
        budget_ms: i64,
        emit: bool,
        force: bool,
    ) -> Result<NodePlan, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        find_pin(def, input_pin, true).ok_or(GraphError::PinNotFound {
            node_guid,
            pin_name: input_pin.to_string(),
        })?;
        let node = &self.nodes[index];
        let mut plan = NodePlan {
            node_guid,
            input_pin: input_pin.to_string(),
            remaining_budget_ms: budget_ms,
            emit,
            commit: true,
            ..NodePlan::default()
        };
        let chosen = chosen_pin.map(|pin| pin.to_string());
        let arrived = {
            let mut arrived = node.arrived_input_pins.clone();
            arrived.insert(plan.input_pin.clone());
            arrived
        };
        plan.arrived_input_pins = arrived;
        plan.completed_storylets = Some(node.completed_storylets.clone());

        match &def.kind {
            NodeKind::SetMetric { metric_id, delta } => {
                let mut op = MutationOp::default();
                op.metric_deltas.push((*metric_id, *delta));
                plan.delta = ctx.apply_op(&op)?;
                plan.call_data = Some(NodeCallData::Delta {
                    delta: plan.delta.clone(),
                });
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
            }
            NodeKind::SetMetricValue { metric_id, value } => {
                if ctx.state.metrics.len() <= *metric_id as usize {
                    return Err(GraphError::MetricIndexOutOfRange {
                        node_guid,
                        metric_index: *metric_id,
                    });
                }
                let current = ctx.state.metrics[*metric_id as usize];
                let delta = value - current;
                if !value.is_finite() || !delta.is_finite() {
                    return Err(GraphError::NonFiniteMetricValue {
                        node_guid,
                        value: *value,
                    });
                }
                let mut op = MutationOp::default();
                op.metric_deltas.push((*metric_id, delta));
                plan.delta = ctx.apply_op(&op)?;
                plan.call_data = Some(NodeCallData::Delta {
                    delta: plan.delta.clone(),
                });
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
            }
            NodeKind::Mutation { op } => {
                plan.delta = ctx.apply_op(op)?;
                plan.call_data = Some(NodeCallData::Delta {
                    delta: plan.delta.clone(),
                });
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
            }
            NodeKind::GraphTransform { rule } => {
                let diff = ctx
                    .state
                    .apply_graph_transform(rule)
                    .map_err(GraphError::State)?;
                plan.delta.graph_diffs.push(diff);
                plan.call_data = Some(NodeCallData::Delta {
                    delta: plan.delta.clone(),
                });
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
            }
            NodeKind::PassionMutation { op } => {
                let registry = ctx
                    .passion_registry
                    .ok_or(GraphError::PassionRegistryRequired { node_guid })?;
                let passion_delta = apply_passion_mutation(
                    &mut ctx.state.passions,
                    op,
                    registry,
                    &mut ctx.state.global_tags,
                )
                .map_err(GraphError::State)?;
                plan.delta.passion_delta = Some(passion_delta);
                plan.call_data = Some(NodeCallData::Delta {
                    delta: plan.delta.clone(),
                });
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
            }
            NodeKind::Waiter {
                duration_ms,
                required_tags,
                ..
            } => {
                let wake = self.clock_ms.saturating_add(*duration_ms);
                plan.waiter_wake_ms = Some(wake);
                // A wake (timer or signal) re-enters a node that already owns
                // its inverse record; pushing another would break rollback.
                if node.state != NodeExecutionState::ActiveLatent {
                    plan.call_data = Some(NodeCallData::Waiter { wake_ms: wake });
                }
                if force || (self.clock_ms >= wake && self.tags_satisfied(ctx, required_tags)) {
                    plan.completed = true;
                    plan.record = true;
                    plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                }
            }
            NodeKind::LogicAll => {
                let all_arrived = def
                    .input_pins
                    .iter()
                    .all(|pin| plan.arrived_input_pins.contains(&pin.pin_name));
                if force || all_arrived {
                    plan.completed = true;
                    plan.record = true;
                    plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                    plan.call_data = Some(NodeCallData::Route {
                        output_pin: plan.output_pin.clone().unwrap_or_default(),
                    });
                }
            }
            NodeKind::LogicAny { default_output_pin } => {
                let pin = chosen.unwrap_or_else(|| {
                    if default_output_pin.is_empty() {
                        default_output(def)
                    } else {
                        default_output_pin.clone()
                    }
                });
                if pin.is_empty() {
                    return Err(GraphError::NoDeclaredRoute { node_guid });
                }
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(pin.clone());
                plan.call_data = Some(NodeCallData::Route {
                    output_pin: pin.clone(),
                });
            }
            NodeKind::Choice { default_output_pin } => {
                // A choice pauses until the player selects an output; only an
                // explicit choice (or a force) records a route.
                let pin = match chosen {
                    Some(pin) if !pin.is_empty() => pin.clone(),
                    _ if force && !default_output_pin.is_empty() => default_output_pin.clone(),
                    _ if force && !default_output(def).is_empty() => default_output(def),
                    _ => return Ok(plan),
                };
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(pin.clone());
                plan.call_data = Some(NodeCallData::Route {
                    output_pin: pin.clone(),
                });
            }
            NodeKind::Poi { poi_id } => {
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                plan.call_data = Some(NodeCallData::Poi {
                    poi_id: poi_id.clone(),
                });
            }
            NodeKind::End { jump_to_storylet } => {
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                if let Some(storylet_id) = jump_to_storylet {
                    let destination = ctx
                        .dispatcher
                        .storylet(*storylet_id)
                        .ok_or(GraphError::UnresolvedStorylet {
                            storylet_id: *storylet_id,
                        })?
                        .payload_node_guid;
                    if self.asset.node(destination).is_none() {
                        return Err(GraphError::JumpTargetNotFound {
                            node_guid,
                            storylet_id: *storylet_id,
                        });
                    }
                    plan.deferred_target = Some(destination);
                    plan.call_data = Some(NodeCallData::Teleport {
                        destination_storylet: Some(*storylet_id),
                        destination_node_guid: Some(destination),
                    });
                } else {
                    plan.emit = false;
                    // Every recorded node must be revertible (REQ-CH-06), so a
                    // plain end keeps its route on the inverse cache.
                    plan.call_data = Some(NodeCallData::Route {
                        output_pin: plan.output_pin.clone().unwrap_or_default(),
                    });
                }
            }
            NodeKind::Conditional { condition } => {
                let branch = self.evaluate_condition(ctx, node_guid, condition)?;
                let pin = if branch {
                    &condition.true_pin
                } else {
                    &condition.false_pin
                };
                if pin.is_empty() {
                    return Err(GraphError::ConditionalBranchPinMissing { node_guid });
                }
                let route = chosen.unwrap_or_else(|| pin.clone());
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(route.clone());
                plan.call_data = Some(NodeCallData::Route { output_pin: route });
            }
            NodeKind::VariableBatch { assignments } => {
                if assignments.is_empty() {
                    return Err(GraphError::EmptyVariableAssignment { node_guid });
                }
                let mut rollback = Vec::new();
                for assignment in assignments {
                    if assignment.name.is_empty() {
                        return Err(GraphError::EmptyVariableName { node_guid });
                    }
                    let previous = self
                        .variables
                        .get(&assignment.scope, &assignment.name)
                        .cloned();
                    let next = apply_variable_operator(node_guid, assignment, &previous)?;
                    rollback.push(VariableRollbackEntry {
                        scope: assignment.scope,
                        name: assignment.name.clone(),
                        previous,
                    });
                    plan.variables
                        .push((assignment.scope, assignment.name.clone(), next));
                }
                plan.completed = true;
                plan.record = true;
                plan.call_data = Some(NodeCallData::VariableBatch { entries: rollback });
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
            }
            NodeKind::Teleport {
                destination_storylet,
                destination_node_guid,
            } => {
                let destination = if let Some(destination) = destination_node_guid {
                    Some(*destination)
                } else if let Some(storylet_id) = destination_storylet {
                    Some(
                        ctx.dispatcher
                            .storylet(*storylet_id)
                            .ok_or(GraphError::UnresolvedStorylet {
                                storylet_id: *storylet_id,
                            })?
                            .payload_node_guid,
                    )
                } else {
                    None
                };
                if let Some(destination) = destination {
                    if self.asset.node(destination).is_none() {
                        return Err(GraphError::TeleportDestinationNotFound {
                            node_guid,
                            destination,
                        });
                    }
                }
                plan.completed = true;
                plan.record = true;
                plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                plan.deferred_target = destination;
                plan.call_data = Some(NodeCallData::Teleport {
                    destination_storylet: *destination_storylet,
                    destination_node_guid: destination,
                });
            }
            NodeKind::QbnHub {
                exit_storylet_id, ..
            } => {
                match storylet_id {
                    Some(id) => {
                        self.plan_storylet(ctx, &mut plan, id)?;
                        if force || Some(id) == *exit_storylet_id {
                            plan.completed = true;
                            plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                        }
                    }
                    None => {
                        // ExecuteNormal presents the deck; the node stays latent
                        // until a storylet is chosen or the exit is reached.
                    }
                }
            }
            NodeKind::SalienceSelector {
                pool,
                fallback_storylet_id,
                ..
            } => {
                let selected = match storylet_id {
                    Some(id) => Some(id),
                    None => {
                        let seed = mix64(self.seed ^ (node_guid as u64));
                        // Both dispatch paths must honor cooldowns. Forced runs
                        // (replay/invoke) skip the filter.
                        let candidates: Vec<u16> = if force {
                            pool.clone()
                        } else {
                            pool.iter()
                                .copied()
                                .filter(|id| self.off_cooldown(ctx, *id))
                                .collect()
                        };
                        ctx.dispatcher
                            .try_match_most_salient(
                                &candidates,
                                ctx.state,
                                &self.completed_storylets,
                                seed,
                            )
                            .map_err(GraphError::Dispatch)?
                            .or(*fallback_storylet_id)
                    }
                };
                match selected {
                    Some(id) => {
                        self.plan_storylet(ctx, &mut plan, id)?;
                        plan.completed = true;
                        plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                    }
                    None => {
                        return Err(GraphError::NoSalienceMatch {
                            node_guid,
                            pool_len: pool.len(),
                        });
                    }
                }
            }
            NodeKind::WaypointConversation {
                graph,
                initial_topic_id,
                npc_goal_topic_id,
            } => {
                let current = if node.current_topic_id.is_empty() {
                    initial_topic_id.clone()
                } else {
                    node.current_topic_id.clone()
                };
                plan.current_topic_id = Some(current.clone());
                let step = ctx
                    .dispatcher
                    .compute_next_npc_topic_step(
                        graph,
                        &current,
                        npc_goal_topic_id,
                        &node.edge_use_counts,
                    )
                    .map_err(GraphError::Dispatch)?;
                let target = graph
                    .nodes
                    .iter()
                    .find(|topic| topic.topic_id == step.to_topic_id);
                let mut revealed_quality_tag = None;
                if let Some(name) = target.and_then(|topic| topic.revealed_clue_quality_id.as_ref())
                {
                    let registry = ctx.registry.ok_or(GraphError::RegistryRequired {
                        node_guid,
                        tag_name: name.clone(),
                    })?;
                    let id = registry.id(name).ok_or(GraphError::RegistryRequired {
                        node_guid,
                        tag_name: name.clone(),
                    })?;
                    let mut op = MutationOp::default();
                    op.add_global_tags.set_bit(id).map_err(GraphError::State)?;
                    plan.delta = ctx.apply_op(&op)?;
                    revealed_quality_tag = Some(id);
                }
                plan.call_data = Some(NodeCallData::WaypointStep {
                    previous_topic_id: current.clone(),
                    edge_id: step.edge_id.clone(),
                    revealed_quality_tag,
                    dialogue_transition_asset_id: (!step.dialogue_transition_asset_id.is_empty())
                        .then(|| step.dialogue_transition_asset_id.clone()),
                    delta: plan.delta.clone(),
                });
                plan.record = true;
                plan.current_topic_id = Some(step.to_topic_id.clone());
                plan.edge_use_count = Some((step.edge_id.clone(), 1));
                let exit_pin = target
                    .map(|topic| topic.exit_output_pin.clone())
                    .unwrap_or_default();
                if force
                    || target
                        .map(|topic| topic.triggers_scene_exit)
                        .unwrap_or(false)
                {
                    plan.completed = true;
                    if plan.record {
                        plan.output_pin = Some(chosen.unwrap_or(if exit_pin.is_empty() {
                            default_output(def)
                        } else {
                            exit_pin
                        }));
                    }
                }
            }
            NodeKind::MissionSubGraph { sub_graph_asset_id } => {
                // The mission boundary completes when its child instance has
                // nothing left to run. `plan` is reached without `force` only
                // through `execute_mission`, which always starts the child
                // first, so a never-started child is not mistaken for a
                // finished one.
                let child_finished = self.nodes[index]
                    .sub_graph
                    .as_ref()
                    .is_some_and(|child| !child.has_pending_work());
                if force || child_finished {
                    plan.completed = true;
                    plan.record = true;
                    plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                    plan.call_data = Some(NodeCallData::Mission {
                        sub_graph_asset_id: sub_graph_asset_id.clone(),
                    });
                }
            }
            NodeKind::SceneSection {
                section_duration_ms,
                ..
            } => {
                // Re-entering a completed section restarts its timeline from
                // zero instead of reporting a phantom completion at the end.
                let cursor = if node.state == NodeExecutionState::Completed {
                    0
                } else {
                    node.section_clock_ms
                };
                plan.section_clock_ms = Some(if force {
                    *section_duration_ms
                } else {
                    cursor
                });
                plan.call_data = Some(NodeCallData::SceneSection {
                    // Undo data must describe the real prior state, even when
                    // the new plan restarts a completed section from zero.
                    previous_clock_ms: node.section_clock_ms,
                    previous_poses: Vec::new(),
                    completion_commit: None,
                });
                if force {
                    plan.completed = true;
                    plan.record = true;
                    plan.output_pin = Some(chosen.unwrap_or_else(|| default_output(def)));
                }
            }
        }
        Ok(plan)
    }

    fn plan_storylet(
        &self,
        ctx: &mut RuntimeContext,
        plan: &mut NodePlan,
        storylet_id: u16,
    ) -> Result<(), GraphError> {
        let def = ctx
            .dispatcher
            .storylet(storylet_id)
            .ok_or(GraphError::UnresolvedStorylet { storylet_id })?;
        let op = ctx
            .dispatcher
            .compile_effects(def, ctx.state)
            .map_err(GraphError::Dispatch)?;
        plan.delta = ctx.apply_op(&op)?;
        plan.storylet_id = Some(storylet_id);
        plan.call_data = Some(NodeCallData::Storylet {
            storylet_id,
            delta: plan.delta.clone(),
            previous_completed_ms: self.last_completed_ms.get(&storylet_id).copied(),
        });
        plan.record = true;
        let mut completed = plan.completed_storylets.clone().unwrap_or_default();
        completed.insert(storylet_id);
        plan.completed_storylets = Some(completed);
        Ok(())
    }

    fn commit_plan(&mut self, plan: &NodePlan) -> Result<Vec<SignalToken>, GraphError> {
        let index = self
            .node_index(plan.node_guid)
            .ok_or(GraphError::NodeNotFound {
                node_guid: plan.node_guid,
            })?;
        if plan.record {
            let entry = CallHistoryEntry {
                node_guid: plan.node_guid,
                executed_input_pin: plan.input_pin.clone(),
                chosen_output_pin: plan.output_pin.clone().unwrap_or_default(),
                chosen_storylet_id: plan.storylet_id,
            };
            if plan.commit {
                self.record_call(&entry, &plan.delta)?;
            } else {
                // A replay branch must not append commits to the history DAG,
                // but mission-scoped nodes still record into their scope.
                match self
                    .asset
                    .node(entry.node_guid)
                    .and_then(|def| def.mission_scope)
                {
                    Some(mission) => {
                        let index =
                            self.node_index(mission)
                                .ok_or(GraphError::MissionScopeNotFound {
                                    node_guid: entry.node_guid,
                                    scope: mission,
                                })?;
                        self.nodes[index].local_call_history.push(entry.clone());
                    }
                    None => self.global_call_history.push(entry.clone()),
                }
            }
        }
        let passion_events = plan
            .call_data
            .as_ref()
            .and_then(|data| data.delta())
            .and_then(|delta| delta.passion_delta.as_ref())
            .map(|delta| {
                delta
                    .threshold_events
                    .iter()
                    .map(|event| PassionThresholdTriggerEvent {
                        node_guid: plan.node_guid,
                        npc_id: event.npc_id,
                        threshold_tag_bit: event.threshold_tag_bit,
                        linked_graph_asset_id: event.linked_graph_asset_id.clone(),
                        activated: event.activated,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        {
            let node = &mut self.nodes[index];
            if let Some(completed) = &plan.completed_storylets {
                node.completed_storylets.clone_from(completed);
            }
            if let Some(topic) = &plan.current_topic_id {
                node.current_topic_id = topic.clone();
            }
            if let Some((edge_id, delta)) = &plan.edge_use_count {
                let current =
                    node.edge_use_counts.get(edge_id).copied().unwrap_or(0) as i64 + delta;
                node.edge_use_counts
                    .insert(edge_id.clone(), current.max(0) as u64);
            }
            if let Some(clock) = plan.section_clock_ms {
                node.section_clock_ms = clock;
            }
            if let Some(wake) = plan.waiter_wake_ms {
                node.waiter_wake_ms = wake;
            }
            if let Some(data) = &plan.call_data {
                node.call_data.push(data.clone());
            }
            node.executed_input_pin = Some(plan.input_pin.clone());
            node.chosen_output_pin = plan.output_pin.clone();
            node.chosen_storylet_id = plan.storylet_id;
            node.state = if plan.completed {
                NodeExecutionState::Completed
            } else {
                NodeExecutionState::ActiveLatent
            };
            // AND-gate fan-in reads this set on the next arrival. Completion
            // consumes the set so a later re-entry starts from zero pins.
            if plan.completed {
                node.arrived_input_pins.clear();
            } else {
                node.arrived_input_pins
                    .clone_from(&plan.arrived_input_pins);
            }
        }
        self.pending_passion_events.extend(passion_events);
        for (scope, name, next) in &plan.variables {
            self.variables.set(scope, name, next);
        }
        if let Some(storylet_id) = plan.storylet_id {
            self.last_completed_ms.insert(storylet_id, self.clock_ms);
            self.completed_storylets.insert(storylet_id);
        }
        let mut emitted = Vec::new();
        if plan.emit && plan.completed {
            if let Some(destination) = plan.deferred_target {
                if self
                    .asset
                    .node(destination)
                    .is_some_and(|def| !def.input_pins.is_empty())
                {
                    emitted.push(self.enqueue_token(
                        destination,
                        0,
                        plan.remaining_budget_ms,
                    ));
                }
            } else {
                emitted.extend(self.emit_tokens(
                    plan.node_guid,
                    &plan.output_pin.clone().unwrap_or_default(),
                    plan.remaining_budget_ms,
                ));
            }
        }
        Ok(emitted)
    }

    /// Append one record. Nodes inside a mission scope write only to that
    /// mission node's local history; the mission node itself writes globally
    /// (REQ-CH-04).
    fn record_call(
        &mut self,
        entry: &CallHistoryEntry,
        delta: &BijectiveDelta,
    ) -> Result<(), GraphError> {
        let scope = self
            .asset
            .node(entry.node_guid)
            .and_then(|def| def.mission_scope);
        match scope {
            Some(mission) => {
                let index = self
                    .node_index(mission)
                    .ok_or(GraphError::MissionScopeNotFound {
                        node_guid: entry.node_guid,
                        scope: mission,
                    })?;
                self.nodes[index].local_call_history.push(entry.clone());
                // Scoped calls were always committed to the DAG; keep that so
                // checkout/reconcile can undo and replay them.
                self.history.commit_for(entry, delta);
            }
            None => {
                self.global_call_history.push(entry.clone());
                self.history.commit_for(entry, delta);
            }
        }
        Ok(())
    }

    /// Remove the last record for a node, used by revert paths.
    fn remove_record(
        &mut self,
        node_guid: u32,
        storylet_id: Option<u16>,
    ) -> Result<(), GraphError> {
        let scope = self.asset.node(node_guid).and_then(|def| def.mission_scope);
        if let Some(mission) = scope {
            let index = self
                .node_index(mission)
                .ok_or(GraphError::MissionScopeNotFound {
                    node_guid,
                    scope: mission,
                })?;
            let position = self.nodes[index]
                .local_call_history
                .iter()
                .rposition(|entry| {
                    entry.node_guid == node_guid && entry.chosen_storylet_id == storylet_id
                });
            if let Some(position) = position {
                self.nodes[index].local_call_history.remove(position);
            }
            return Ok(());
        }
        let position = self.global_call_history.iter().rposition(|entry| {
            entry.node_guid == node_guid && entry.chosen_storylet_id == storylet_id
        });
        if let Some(position) = position {
            self.global_call_history.remove(position);
        }
        Ok(())
    }

    fn emit_tokens(&mut self, node_guid: u32, pin_name: &str, budget_ms: i64) -> Vec<SignalToken> {
        let targets: Vec<(u32, u16)> = self
            .asset
            .outgoing_edges(node_guid, pin_name)
            .iter()
            .filter_map(|edge| {
                self.asset
                    .input_pin_index(edge.target_node_guid, &edge.target_pin_name)
                    .map(|index| (edge.target_node_guid, index))
            })
            .collect();
        targets
            .into_iter()
            .map(|(target_node_guid, target_pin_index)| {
                self.enqueue_token(target_node_guid, target_pin_index, budget_ms)
            })
            .collect()
    }

    fn enqueue_token(
        &mut self,
        node_guid: u32,
        pin_index: u16,
        budget_ms: i64,
    ) -> SignalToken {
        let token = SignalToken {
            token_id: self.next_token_id,
            graph_instance_id: self.instance_id,
            target_node_guid: node_guid,
            target_pin_index: pin_index,
            flags: TOKEN_ACTIVE,
            time_budget_ms: clamp_token_ms(budget_ms),
            deferred_until_ms: 0,
        };
        self.next_token_id += 1;
        self.tokens.push(token.clone());
        token
    }

    /// True when the storylet has no millisecond cooldown outstanding at the
    /// instance clock. Unknown ids are treated as ready; callers validate those.
    fn off_cooldown(&self, ctx: &RuntimeContext, storylet_id: u16) -> bool {
        ctx.dispatcher
            .storylet(storylet_id)
            .is_none_or(|storylet| match self.last_completed_ms.get(&storylet_id) {
                Some(last) => {
                    UnifiedDispatcher::cooldown_remaining_ms(storylet, *last, self.clock_ms) == 0
                }
                None => true,
            })
    }

    fn node_cost(&self, node_guid: u32) -> Result<i64, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        Ok(def.cost_ms)
    }

    /// Smallest active token id, so consumption order is reproducible.
    fn next_active_token_index(&self) -> Option<usize> {
        let mut best: Option<(u64, usize)> = None;
        for (index, token) in self.tokens.iter().enumerate() {
            if !token.is_active() {
                continue;
            }
            if best.is_none() || token.token_id < best.unwrap_or((0, 0)).0 {
                best = Some((token.token_id, index));
            }
        }
        best.map(|(_, index)| index)
    }

    fn revive_deferred(&mut self) {
        for token in &mut self.tokens {
            if token.is_deferred() && i64::from(token.deferred_until_ms) <= self.clock_ms {
                token.set_active(true);
                token.set_deferred(false);
            }
        }
    }

    fn tags_satisfied(&self, ctx: &RuntimeContext, mask: &TagBitset) -> bool {
        match ctx.registry {
            Some(registry) => (0..MAX_TAGS).all(|index| {
                !mask.bit(index as u16)
                    || registry.has_hierarchical(&ctx.state.global_tags, index as u16)
            }),
            None => ctx.state.global_tags.contains_all(mask),
        }
    }
}

fn default_output(def: &FlowNodeDef) -> String {
    let pin = match &def.kind {
        NodeKind::LogicAny { default_output_pin } | NodeKind::Choice { default_output_pin } => {
            default_output_pin
        }
        NodeKind::SalienceSelector {
            fallback_output_pin,
            ..
        } => fallback_output_pin,
        _ => &String::new(),
    };
    if pin.is_empty() {
        DEFAULT_OUTPUT_PIN.to_string()
    } else {
        pin.clone()
    }
}

impl FlowGraphInstance {
    /// Reset the instance and seed the start node's token. Execution happens on
    /// the next `tick`, which owns the frame budget the first token is charged
    /// against.
    pub fn start(&mut self) -> Result<Vec<SignalToken>, GraphError> {
        self.start_at_node(self.asset.start_node_guid)
    }

    /// Reset the instance and seed a token at a stable authored entry point.
    /// Dispatchers use this to enter a selected storylet without rebuilding
    /// the aggregate project graph.
    pub fn start_at_node(&mut self, node_guid: u32) -> Result<Vec<SignalToken>, GraphError> {
        if self
            .asset
            .node(node_guid)
            .ok_or(GraphError::StartNodeNotFound { node_guid })?
            .input_pins
            .is_empty()
        {
            return Err(GraphError::NodeHasNoPins { node_guid });
        }
        self.reset();
        let token = self.enqueue_token(node_guid, 0, 0);
        Ok(vec![token])
    }

    /// Advance one frame. The clock moves first, latent waiters that are ready
    /// wake in node GUID order, then active tokens are consumed in ascending
    /// token id order against the frame budget. A token whose node costs more
    /// than the remaining budget is deferred to a later frame instead of being
    /// dropped.
    pub fn tick(
        &mut self,
        ctx: &mut RuntimeContext,
        frame_budget_ms: i64,
    ) -> Result<Vec<SignalToken>, GraphError> {
        if frame_budget_ms < 0 {
            return Err(GraphError::NegativeFrameBudget(frame_budget_ms));
        }
        self.turn = self.turn.saturating_add(1);
        self.clock_ms = self.clock_ms.saturating_add(frame_budget_ms);
        self.revive_deferred();
        let mut executed = Vec::new();
        self.wake_ready_waiters(ctx, &mut executed)?;
        if frame_budget_ms > 0 {
            let sections: Vec<u32> = self
                .nodes
                .iter()
                .filter(|node| node.state == NodeExecutionState::ActiveLatent)
                .filter(|node| {
                    self.asset
                        .node(node.node_guid)
                        .is_some_and(|def| matches!(&def.kind, NodeKind::SceneSection { .. }))
                })
                .map(|node| node.node_guid)
                .collect();
            if !sections.is_empty() {
                let mut frames = Vec::with_capacity(sections.len());
                for node_guid in sections {
                    frames.push(self.plan_section_frame(node_guid, frame_budget_ms)?);
                }
                let owned: Vec<(u32, Vec<ActionPart>)> = frames
                    .iter()
                    .map(|frame| (frame.node_guid, frame.parts.clone()))
                    .collect();
                let slices: Vec<(u32, &[ActionPart])> = owned
                    .iter()
                    .map(|(node_guid, parts)| (*node_guid, parts.as_slice()))
                    .collect();
                let applied = self
                    .composer
                    .execute_sorted_action_parts_multi(&slices, false)?;
                let mut by_node: BTreeMap<u32, Vec<ComposerCommand>> = BTreeMap::new();
                for command in applied {
                    by_node.entry(command.node_guid).or_default().push(command);
                }
                for frame in frames {
                    let commands = by_node.remove(&frame.node_guid).unwrap_or_default();
                    executed.extend(self.apply_section_frame(
                        frame,
                        &commands,
                        true,
                        true,
                        frame_budget_ms,
                    )?);
                }
            }
        }
        // A mission keeps running across frames: the token loop below only
        // consumes the parent's tokens, so a child instance that is still
        // working is continued here, in node GUID order. Nodes with a token
        // pending this frame are handled by that loop instead.
        let running: Vec<u32> = self
            .nodes
            .iter()
            .filter(|node| node.state == NodeExecutionState::ActiveLatent)
            .filter(|node| {
                self.asset
                    .node(node.node_guid)
                    .is_some_and(|def| matches!(&def.kind, NodeKind::MissionSubGraph { .. }))
            })
            .filter(|node| {
                !self
                    .tokens
                    .iter()
                    .any(|token| token.is_active() && token.target_node_guid == node.node_guid)
            })
            .map(|node| node.node_guid)
            .collect();
        for node_guid in running {
            let input_pin = self
                .node(node_guid)
                .and_then(|node| node.executed_input_pin.clone())
                .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_string());
            executed.extend(self.execute_mission(
                ctx,
                node_guid,
                &input_pin,
                frame_budget_ms,
                false,
            )?);
        }
        let mut remaining = frame_budget_ms;
        let mut executions = 0usize;
        loop {
            if executions >= MAX_TICK_EXECUTIONS {
                return Err(GraphError::ExecutionLimitExceeded {
                    limit: MAX_TICK_EXECUTIONS,
                });
            }
            let index = match self.next_active_token_index() {
                Some(index) => index,
                None => break,
            };
            if self.tokens[index].graph_instance_id != self.instance_id {
                return Err(GraphError::TokenInstanceMismatch {
                    token_instance_id: self.tokens[index].graph_instance_id,
                    instance_id: self.instance_id,
                });
            }
            let mut token = self.tokens[index].clone();
            let cost = self.node_cost(token.target_node_guid)?;
            if cost > remaining && (executions > 0 || frame_budget_ms == 0) {
                // Defer exactly one frame and keep the unused budget; deferring
                // by `cost` would livelock a node that costs more than a frame.
                let deferred = &mut self.tokens[index];
                deferred.set_active(false);
                deferred.set_deferred(true);
                deferred.deferred_until_ms = clamp_token_ms(
                    self.clock_ms.saturating_add(frame_budget_ms.max(1)),
                );
                continue;
            }
            // The token carries this token's share of the frame budget. The
            // first execution may overrun the frame so a node costing more than
            // one frame still makes progress.
            token.time_budget_ms = clamp_token_ms(remaining.max(cost));
            remaining = (remaining - cost).max(0);
            executions += 1;
            executed.push(token);
            let emitted = self.execute_input_at_index(
                ctx,
                token.target_node_guid,
                token.target_pin_index,
                &token,
            )?;
            // New tokens appended by this node remain active in the same frame.
            self.tokens[index].set_active(false);
            self.tokens[index].set_deferred(false);
            executed.extend(emitted);
        }
        // Consumed tokens are never reused; keep the ring buffer bounded.
        self.tokens.retain(|token| token.is_active() || token.is_deferred());
        Ok(executed)
    }

    /// ExecuteNormal: run a node when a token arrives at a named input pin.
    pub fn execute_input(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        input_pin: &str,
        token: &SignalToken,
    ) -> Result<Vec<SignalToken>, GraphError> {
        let pin_index = self
            .asset
            .input_pin_index(node_guid, input_pin)
            .ok_or_else(|| GraphError::PinNotFound {
                node_guid,
                pin_name: input_pin.to_string(),
            })?;
        self.execute_input_at_index(ctx, node_guid, pin_index, token)
    }

    /// ExecuteNormal with the pin already resolved to an index.
    pub fn execute_input_at_index(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        pin_index: u16,
        token: &SignalToken,
    ) -> Result<Vec<SignalToken>, GraphError> {
        if token.graph_instance_id != self.instance_id {
            return Err(GraphError::TokenInstanceMismatch {
                token_instance_id: token.graph_instance_id,
                instance_id: self.instance_id,
            });
        }
        let input_pin = self
            .asset
            .input_pin_name(node_guid, pin_index)
            .ok_or_else(|| GraphError::PinNotFound {
                node_guid,
                pin_name: pin_index.to_string(),
            })?
            .to_string();
        let cost = self.node_cost(node_guid)?;
        let budget_ms = i64::from(token.time_budget_ms);
        if cost > budget_ms {
            return Err(GraphError::InsufficientBudget {
                node_guid,
                cost_ms: cost,
                budget_ms,
            });
        }
        if self
            .asset
            .node(node_guid)
            .is_some_and(|def| matches!(&def.kind, NodeKind::MissionSubGraph { .. }))
        {
            return self.execute_mission(
                ctx,
                node_guid,
                &input_pin,
                budget_ms - cost,
                false,
            );
        }
        let plan = self.plan(
            ctx,
            node_guid,
            &input_pin,
            None,
            None,
            budget_ms - cost,
            true,
            false,
        )?;
        self.commit_plan(&plan)
    }

    /// Resolve a dynamic branch by choosing an output pin.
    pub fn choose(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        output_pin: &str,
    ) -> Result<Vec<SignalToken>, GraphError> {
        self.check_pin(node_guid, output_pin, false)?;
        let plan = self.plan(
            ctx,
            node_guid,
            DEFAULT_INPUT_PIN,
            None,
            Some(output_pin),
            0,
            true,
            true,
        )?;
        self.commit_plan(&plan)
    }

    /// Run one storylet on a QBN hub or salience selector.
    pub fn select_storylet(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        storylet_id: u16,
    ) -> Result<Vec<SignalToken>, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let pool: Option<&[u16]> = match &def.kind {
            NodeKind::QbnHub { deck, .. } => Some(deck),
            NodeKind::SalienceSelector { pool, .. } => Some(pool),
            _ => None,
        };
        // A dispatcher node with an empty pool rejects every id; testing
        // `!pool.is_empty()` would accept anything.
        if let Some(pool) = pool {
            if !pool.contains(&storylet_id) {
                return Err(GraphError::StoryletNotInDeck {
                    node_guid,
                    storylet_id,
                });
            }
            let completed = self.completed_storylets.clone();
            let eligible = match ctx.dispatcher.storylet(storylet_id) {
                Some(storylet) => {
                    ctx.dispatcher
                        .try_is_playable(storylet, ctx.state, &completed)
                        .map_err(GraphError::Dispatch)?
                        && self.off_cooldown(ctx, storylet_id)
                }
                None => false,
            };
            if !eligible {
                return Err(GraphError::StoryletNotPlayable {
                    node_guid,
                    storylet_id,
                });
            }
        }
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let input_pin = self.nodes[index]
            .executed_input_pin
            .clone()
            .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_string());
        let plan = self.plan(
            ctx,
            node_guid,
            &input_pin,
            Some(storylet_id),
            None,
            0,
            true,
            false,
        )?;
        self.commit_plan(&plan)
    }

    /// The deck a hub would present right now, in dispatcher order.
    pub fn present_deck(
        &self,
        ctx: &RuntimeContext,
        node_guid: u32,
    ) -> Result<Vec<u16>, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let NodeKind::QbnHub { deck, .. } = &def.kind else {
            return Err(GraphError::UnsupportedNode {
                node_guid,
                category: def.category,
            });
        };
        Ok(ctx
            .dispatcher
            .try_query_qbn_deck(deck, ctx.state, &self.completed_storylets)
            .map_err(GraphError::Dispatch)?
            .into_iter()
            .filter(|storylet_id| self.off_cooldown(ctx, *storylet_id))
            .collect())
    }

    /// Deck filtered by turn-based cooldowns. A storylet whose cooldown cannot be
    /// converted to milliseconds is reported as a typed incompatibility so the
    /// caller keeps the legacy turn path for it rather than the runtime guessing.
    pub fn present_deck_with_turn_cooldowns(
        &self,
        ctx: &RuntimeContext,
        node_guid: u32,
        cooldown_turns: &BTreeMap<u16, u64>,
    ) -> Result<Vec<u16>, GraphError> {
        let mut ready = Vec::new();
        for storylet_id in self.present_deck(ctx, node_guid)? {
            let turns = cooldown_turns.get(&storylet_id).copied().unwrap_or(0);
            if self.turn_cooldown_remaining_ms(storylet_id, turns)? == 0 {
                ready.push(storylet_id);
            }
        }
        Ok(ready)
    }

    pub fn turn_cooldown_remaining_ms(
        &self,
        storylet_id: u16,
        cooldown_turns: u64,
    ) -> Result<i64, GraphError> {
        if cooldown_turns == 0 {
            return Ok(0);
        }
        let ms_per_turn = match self.ms_per_turn {
            Some(ms) => ms,
            None => {
                return Err(GraphError::TurnCooldownNotConvertible {
                    storylet_id,
                    cooldown_turns,
                });
            }
        };
        let last = self
            .last_completed_ms
            .get(&storylet_id)
            .copied()
            .unwrap_or(0);
        let expires = last.saturating_add(ms_per_turn.saturating_mul(cooldown_turns as i64));
        Ok((expires - self.clock_ms).max(0))
    }

    /// Salience coverage for a selector, so a caller can prove the pool is not
    /// starved before the node runs (REQ-SD-03).
    pub fn salience_coverage(
        &self,
        ctx: &RuntimeContext,
        node_guid: u32,
    ) -> Result<SalienceCoverage, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let NodeKind::SalienceSelector {
            pool,
            fallback_storylet_id,
            ..
        } = &def.kind
        else {
            return Err(GraphError::UnsupportedNode {
                node_guid,
                category: def.category,
            });
        };
        let completed = &self.completed_storylets;
        let mut eligible = Vec::new();
        for storylet_id in pool.iter().copied() {
            let Some(def) = ctx.dispatcher.storylet(storylet_id) else {
                continue;
            };
            // Same fail-closed arc-phase rule as `match_most_salient`.
            if def.required_arc_phases.is_empty() {
                continue;
            }
            if ctx
                .dispatcher
                .try_is_playable(def, ctx.state, completed)
                .map_err(GraphError::Dispatch)?
            {
                eligible.push(storylet_id);
            }
        }
        Ok(SalienceCoverage {
            node_guid,
            eligible,
            has_fallback: fallback_storylet_id.is_some(),
        })
    }

    fn check_pin(&self, node_guid: u32, pin_name: &str, is_input: bool) -> Result<(), GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        if find_pin(def, pin_name, is_input).is_none() {
            return Err(GraphError::PinNotFound {
                node_guid,
                pin_name: pin_name.to_string(),
            });
        }
        Ok(())
    }

    fn wake_ready_waiters(
        &mut self,
        ctx: &mut RuntimeContext,
        executed: &mut Vec<SignalToken>,
    ) -> Result<(), GraphError> {
        let ready: Vec<usize> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.state == NodeExecutionState::ActiveLatent)
            .filter_map(|(index, node)| {
                let def = self.asset.node(node.node_guid)?;
                let NodeKind::Waiter { required_tags, .. } = &def.kind else {
                    return None;
                };
                (self.clock_ms >= node.waiter_wake_ms && self.tags_satisfied(ctx, required_tags))
                    .then_some(index)
            })
            .collect();
        for index in ready {
            let node_guid = self.nodes[index].node_guid;
            let input_pin = self.nodes[index]
                .executed_input_pin
                .clone()
                .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_string());
            let entry = CallHistoryEntry {
                node_guid,
                executed_input_pin: input_pin,
                chosen_output_pin: DEFAULT_OUTPUT_PIN.to_string(),
                chosen_storylet_id: None,
            };
            self.record_call(&entry, &BijectiveDelta::default())?;
            let node = &mut self.nodes[index];
            node.state = NodeExecutionState::Completed;
            node.chosen_output_pin = Some(DEFAULT_OUTPUT_PIN.to_string());
            // The inverse record was pushed when the waiter became latent; a
            // second one here would leave an unmatched entry for rollback.
            executed.extend(self.emit_tokens(node_guid, DEFAULT_OUTPUT_PIN, 0));
        }
        Ok(())
    }
}

impl FlowGraphInstance {
    /// ForceComplete: run a node instantly without a token, as the replay and
    /// invoke paths do.
    pub fn force_complete(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        output_pin: &str,
        storylet_id: Option<u16>,
    ) -> Result<(), GraphError> {
        self.force_complete_with_input(ctx, node_guid, DEFAULT_INPUT_PIN, output_pin, storylet_id)
            .map(|_| ())
    }

    fn force_complete_with_input(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        input_pin: &str,
        output_pin: &str,
        storylet_id: Option<u16>,
    ) -> Result<BijectiveDelta, GraphError> {
        // Older persisted histories can carry an empty input pin; replay used
        // to synthesize the default, so keep that behavior.
        let input_pin = if input_pin.is_empty() {
            DEFAULT_INPUT_PIN
        } else {
            input_pin
        };
        self.force_complete_with_input_commit(
            ctx,
            node_guid,
            input_pin,
            output_pin,
            storylet_id,
            true,
        )
    }

    fn force_complete_with_input_commit(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        input_pin: &str,
        output_pin: &str,
        storylet_id: Option<u16>,
        commit: bool,
    ) -> Result<BijectiveDelta, GraphError> {
        if !input_pin.is_empty() {
            self.check_pin(node_guid, input_pin, true)?;
        }
        if !output_pin.is_empty() {
            self.check_pin(node_guid, output_pin, false)?;
        }
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let mission = self
            .asset
            .node(node_guid)
            .is_some_and(|def| matches!(&def.kind, NodeKind::MissionSubGraph { .. }));
        if mission {
            if self.nodes[index].state == NodeExecutionState::Completed {
                return Ok(BijectiveDelta::default());
            }
            // ForceComplete completes the mission boundary. When the child is
            // fresh and the adapter supplied its scoped array, that array is
            // replayed into the child so the world state is rebuilt without
            // serializing it (REQ-CH-03); otherwise the child graph is driven
            // by frame ticks, not by a force.
            let scoped = self.nodes[index].local_call_history.clone();
            let nested = self.nodes[index]
                .sub_graph
                .as_deref()
                .map(FlowGraphInstance::nested_mission_histories)
                .unwrap_or_default();
            if let Some(child) = self.nodes[index].sub_graph.as_mut() {
                if !scoped.is_empty()
                    && child.global_call_history.is_empty()
                    && !child.has_pending_work()
                {
                    child.replay_history_with_missions(ctx, &scoped, &nested)?;
                }
            }
        }
        let scene_duration = self.asset.node(node_guid).and_then(|def| match &def.kind {
            NodeKind::SceneSection {
                section_duration_ms,
                ..
            } => Some(*section_duration_ms),
            _ => None,
        });
        if let Some(duration) = scene_duration {
            if self.nodes[index].state == NodeExecutionState::Completed {
                return Ok(BijectiveDelta::default());
            }
            let previous_clock_ms = self.nodes[index].section_clock_ms;
            if previous_clock_ms < duration {
                self.process_time_window_inner(
                    node_guid,
                    duration - previous_clock_ms,
                    false,
                    false,
                    0,
                )?;
            } else {
                self.nodes[index]
                    .call_data
                    .push(NodeCallData::SceneSection {
                        previous_clock_ms,
                        previous_poses: Vec::new(),
                        completion_commit: None,
                    });
            }
            let mut plan = self.plan(
                ctx,
                node_guid,
                input_pin,
                storylet_id,
                Some(output_pin),
                0,
                false,
                true,
            )?;
            plan.call_data = None;
            plan.commit = commit;
            let delta = plan.delta.clone();
            self.commit_plan(&plan)?;
            return Ok(delta);
        }
        let mut plan = self.plan(
            ctx,
            node_guid,
            input_pin,
            storylet_id,
            Some(output_pin),
            0,
            false,
            true,
        )?;
        plan.commit = commit;
        let delta = plan.delta.clone();
        self.commit_plan(&plan)?;
        if mission {
            self.sync_mission_history(node_guid)?;
        }
        Ok(delta)
    }

    /// CancelAndRevert: pop this node's inverse cache and undo exactly,
    /// including waypoint edge use counts and revealed qualities.
    pub fn cancel_and_revert(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
    ) -> Result<(), GraphError> {
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let record = match self.nodes[index].call_data.pop() {
            Some(record) => record,
            None => return Err(GraphError::RevertStackEmpty { node_guid }),
        };
        let inverse_passion_events = record
            .delta()
            .and_then(|delta| delta.passion_delta.as_ref())
            .map(|delta| {
                delta
                    .threshold_events
                    .iter()
                    .map(|event| PassionThresholdTriggerEvent {
                        node_guid,
                        npc_id: event.npc_id,
                        threshold_tag_bit: event.threshold_tag_bit,
                        linked_graph_asset_id: event.linked_graph_asset_id.clone(),
                        activated: !event.activated,
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(delta) = record.delta() {
            revert_delta(ctx.state, delta).map_err(GraphError::State)?;
        }
        match &record {
            NodeCallData::Storylet {
                storylet_id,
                previous_completed_ms,
                ..
            } => {
                self.nodes[index].completed_storylets.remove(storylet_id);
                if !self
                    .nodes
                    .iter()
                    .any(|node| node.completed_storylets.contains(storylet_id))
                {
                    self.completed_storylets.remove(storylet_id);
                }
                match previous_completed_ms {
                    Some(clock) => {
                        self.last_completed_ms.insert(*storylet_id, *clock);
                    }
                    None => {
                        self.last_completed_ms.remove(storylet_id);
                    }
                }
            }
            NodeCallData::WaypointStep {
                previous_topic_id,
                edge_id,
                ..
            } => {
                let count = self.nodes[index]
                    .edge_use_counts
                    .get(edge_id)
                    .copied()
                    .unwrap_or(0)
                    .saturating_sub(1);
                self.nodes[index]
                    .edge_use_counts
                    .insert(edge_id.clone(), count);
                self.nodes[index].current_topic_id = previous_topic_id.clone();
            }
            NodeCallData::SceneSection {
                previous_clock_ms,
                previous_poses,
                ..
            } => {
                for undo in previous_poses.iter().rev() {
                    match &undo.previous_pose_id {
                        Some(pose) => {
                            self.composer
                                .actor_pose
                                .insert(undo.combiner_entity_path.clone(), pose.clone());
                        }
                        None => {
                            self.composer.actor_pose.remove(&undo.combiner_entity_path);
                        }
                    }
                }
                self.nodes[index].section_clock_ms = *previous_clock_ms;
            }
            NodeCallData::Waiter { .. } => self.nodes[index].waiter_wake_ms = 0,
            NodeCallData::Route { .. } => self.nodes[index].chosen_output_pin = None,
            NodeCallData::VariableBatch { entries } => {
                for entry in entries.iter().rev() {
                    match &entry.previous {
                        Some(value) => {
                            self.variables.set(&entry.scope, &entry.name, value);
                        }
                        None => {
                            self.variables.remove(&entry.scope, &entry.name);
                        }
                    }
                }
            }
            NodeCallData::Mission { .. } => {
                // Snapshot the child's scoped mirrors before unwinding: they
                // are the replay source for the forward reconcile pass, but a
                // plain reset would discard them.
                let mirrors = self.nodes[index]
                    .sub_graph
                    .as_deref()
                    .map(FlowGraphInstance::nested_mission_histories)
                    .unwrap_or_default();
                self.unwind_mission_history(ctx, node_guid)?;
                if let Some(child) = self.nodes[index].sub_graph.as_mut() {
                    child.reset();
                    child.restore_mission_mirrors(&mirrors);
                }
                // Do not sync: that would erase the mirror we just restored.
            }
            NodeCallData::Delta { .. }
            | NodeCallData::Poi { .. }
            | NodeCallData::Teleport { .. } => {}
        }
        let cancelled_storylet = record_storylet(&record);
        self.remove_record(node_guid, cancelled_storylet)?;
        // A direct revert of the current head must move the head to its parent;
        // otherwise a later checkout/reconcile cancels the same call again.
        let head = self.history.head;
        if head != 0 {
            let commit = &self.history.commits[head];
            if commit.entry.node_guid == node_guid
                && commit.entry.chosen_storylet_id == cancelled_storylet
            {
                self.history.head = commit.parent.unwrap_or(0);
            }
        }
        self.pending_passion_events.extend(inverse_passion_events);
        let node = &mut self.nodes[index];
        node.state = NodeExecutionState::Idle;
        node.executed_input_pin = None;
        node.chosen_output_pin = None;
        node.chosen_storylet_id = None;
        Ok(())
    }

    /// Process a scene time window. Forward windows are half-open `[t0, t1)`;
    /// backward scrubbing uses `(t1, t0]` and inverts the activity mapping.
    /// Parts are sorted by priority key then combiner entity, arbitrated per
    /// entity, and dispatched in that order (REQ-SC-02, REQ-SC-03).
    pub fn process_time_window(
        &mut self,
        node_guid: u32,
        delta_ms: i64,
    ) -> Result<Vec<ActionPart>, GraphError> {
        self.process_time_window_inner(node_guid, delta_ms, true, true, 0)
    }

    /// Scrub a scene window without touching call history or emitting tokens.
    /// Used by preview clients that apply the result to a throwaway instance.
    pub fn preview_time_window(
        &mut self,
        node_guid: u32,
        delta_ms: i64,
    ) -> Result<Vec<ActionPart>, GraphError> {
        self.process_time_window_inner(node_guid, delta_ms, false, false, 0)
    }

    fn process_time_window_inner(
        &mut self,
        node_guid: u32,
        delta_ms: i64,
        record_completion: bool,
        emit_completion: bool,
        completion_budget_ms: i64,
    ) -> Result<Vec<ActionPart>, GraphError> {
        let frame = self.plan_section_frame(node_guid, delta_ms)?;
        let parts = frame.parts.clone();
        let applied = self.composer.execute_sorted_action_parts(
            node_guid,
            &frame.parts,
            frame.is_reverse,
        )?;
        self.apply_section_frame(
            frame,
            &applied,
            record_completion,
            emit_completion,
            completion_budget_ms,
        )?;
        Ok(parts)
    }

    /// First half of a scene window: resolve the time slice and its parts
    /// without mutating the clock, composer, call_data, or history.
    fn plan_section_frame(
        &self,
        node_guid: u32,
        delta_ms: i64,
    ) -> Result<SectionFrame, GraphError> {
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let t_start = self.nodes[index].section_clock_ms;
        if delta_ms == 0 {
            return Err(GraphError::EmptyTimeWindow {
                node_guid,
                cursor_ms: t_start,
            });
        }
        let (duration_ms, authored) = {
            let def = self
                .asset
                .node(node_guid)
                .ok_or(GraphError::NodeNotFound { node_guid })?;
            let NodeKind::SceneSection {
                section_duration_ms,
                events,
            } = &def.kind
            else {
                return Err(GraphError::UnsupportedNode {
                    node_guid,
                    category: def.category,
                });
            };
            (*section_duration_ms, events.clone())
        };
        let upper = duration_ms.max(0);
        let t_end = t_start.saturating_add(delta_ms).clamp(0, upper);
        let events = sorted_events(&authored);
        let mut parts = Vec::new();
        if delta_ms > 0 {
            for event in &events {
                let end = event.start_time_ms + event.duration_ms;
                if event.duration_ms == 0 {
                    if event.start_time_ms >= t_start && event.start_time_ms < t_end {
                        parts.push(make_part(event, EventOverlapActivity::StartActivity, 0));
                    }
                    continue;
                }
                if end <= t_start || event.start_time_ms >= t_end {
                    continue;
                }
                let activity = if event.start_time_ms >= t_start {
                    EventOverlapActivity::StartActivity
                } else if end <= t_end {
                    EventOverlapActivity::EndActivity
                } else {
                    EventOverlapActivity::RegularActivity
                };
                parts.push(make_part(event, activity, t_end - event.start_time_ms));
            }
        } else {
            for event in events.iter().rev() {
                let end = event.start_time_ms + event.duration_ms;
                if event.duration_ms == 0 {
                    if event.start_time_ms > t_end && event.start_time_ms <= t_start {
                        parts.push(make_part(event, EventOverlapActivity::EndActivity, 0));
                    }
                    continue;
                }
                if end < t_end || event.start_time_ms > t_start {
                    continue;
                }
                let activity = if event.start_time_ms > t_end {
                    EventOverlapActivity::EndActivity
                } else if end <= t_start {
                    EventOverlapActivity::StartActivity
                } else {
                    EventOverlapActivity::RegularActivity
                };
                parts.push(make_part(event, activity, t_end - event.start_time_ms));
            }
        }
        let previous_poses: BTreeMap<(u64, String), Option<String>> = parts
            .iter()
            .map(|part| {
                let entity = part.combiner_entity_path.clone();
                let pose = self.composer.actor_pose.get(&entity).cloned();
                ((part.event_ref.event_id, entity), pose)
            })
            .collect();
        let was_completed = self.nodes[index].state == NodeExecutionState::Completed;
        Ok(SectionFrame {
            node_guid,
            is_reverse: delta_ms < 0,
            parts,
            t_start,
            t_end,
            just_completed: t_end == duration_ms && !was_completed,
            reopened: delta_ms < 0 && t_end < duration_ms && was_completed,
            previous_poses,
        })
    }
    /// Second half: apply a frame's arbitrated commands and update its clock,
    /// inverse cache, completion record, and emitted tokens.
    fn apply_section_frame(
        &mut self,
        frame: SectionFrame,
        applied: &[ComposerCommand],
        record_completion: bool,
        emit_completion: bool,
        completion_budget_ms: i64,
    ) -> Result<Vec<SignalToken>, GraphError> {
        let node_guid = frame.node_guid;
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let pose_undos: Vec<ScenePoseUndo> = applied
            .iter()
            .map(|command| ScenePoseUndo {
                combiner_entity_path: command.combiner_entity_path.clone(),
                previous_pose_id: frame
                    .previous_poses
                    .get(&(command.event_id, command.combiner_entity_path.clone()))
                    .cloned()
                    .flatten(),
            })
            .collect();
        {
            let node = &mut self.nodes[index];
            if let Some(NodeCallData::SceneSection { previous_poses, .. }) =
                node.call_data.last_mut()
            {
                previous_poses.extend(pose_undos);
            } else {
                node.call_data.push(NodeCallData::SceneSection {
                    previous_clock_ms: frame.t_start,
                    previous_poses: pose_undos,
                    completion_commit: None,
                });
            }
            node.section_clock_ms = frame.t_end;
            if frame.just_completed {
                node.state = NodeExecutionState::Completed;
                node.chosen_output_pin = Some(DEFAULT_OUTPUT_PIN.to_string());
            } else if frame.reopened {
                // Scrubbing back below the end reopens the section; otherwise
                // the clock disagrees with `Completed` and forward play can
                // never emit completion again.
                node.state = NodeExecutionState::ActiveLatent;
                node.chosen_output_pin = None;
            }
        }
        let mut emitted = Vec::new();
        if frame.just_completed {
            let entry = CallHistoryEntry {
                node_guid,
                executed_input_pin: self.nodes[index]
                    .executed_input_pin
                    .clone()
                    .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_string()),
                chosen_output_pin: DEFAULT_OUTPUT_PIN.to_string(),
                chosen_storylet_id: self.nodes[index].chosen_storylet_id,
            };
            if record_completion {
                self.record_call(&entry, &BijectiveDelta::default())?;
                let commit = self.history.head;
                if let Some(NodeCallData::SceneSection {
                    completion_commit,
                    ..
                }) = self.nodes[index].call_data.last_mut()
                {
                    *completion_commit = Some(commit);
                }
            }
            if emit_completion {
                emitted = self.emit_tokens(node_guid, DEFAULT_OUTPUT_PIN, completion_budget_ms);
            }
        } else if frame.reopened && record_completion {
            // Drop the completion record so a later forward pass records it
            // once. Only a tail completion can be removed without rewriting
            // the DAG; anything else must go through checkout first.
            let recorded = self.nodes[index]
                .call_data
                .iter()
                .rev()
                .find_map(|record| match record {
                    NodeCallData::SceneSection {
                        completion_commit, ..
                    } => *completion_commit,
                    _ => None,
                });
            let head = self.history.head;
            // A non-tail completion cannot be removed without reverting its
            // descendants; the section still reopens so previews remain
            // usable, and `checkout` is the path that reconciles the DAG.
            let removable = recorded.map_or_else(
                || {
                    head != 0
                        && self.history.commits[head].entry.node_guid == node_guid
                },
                |commit| commit == head,
            );
            if removable && head != 0 {
                let commit = self.history.commits[head].clone();
                self.remove_record(node_guid, commit.entry.chosen_storylet_id)?;
                self.history.head = commit.parent.unwrap_or(0);
            }
        }
        Ok(emitted)
    }

    /// The step the NPC would take from the current topic, without changing
    /// anything.
    pub fn next_topic_step(
        &self,
        ctx: &RuntimeContext,
        node_guid: u32,
    ) -> Result<WaypointStep, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let NodeKind::WaypointConversation {
            graph,
            npc_goal_topic_id,
            ..
        } = &def.kind
        else {
            return Err(GraphError::UnsupportedNode {
                node_guid,
                category: def.category,
            });
        };
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let current = self.nodes[index].current_topic_id.clone();
        ctx.dispatcher
            .compute_next_npc_topic_step(
                graph,
                &current,
                npc_goal_topic_id,
                &self.nodes[index].edge_use_counts,
            )
            .map_err(GraphError::Dispatch)
    }

    /// Player diverts the conversation onto a specific topic edge. The step is
    /// recorded so the exact inverse restores the prior topic, the edge use
    /// count, and any quality revealed by it.
    pub fn divert_topic(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        edge_id: &str,
    ) -> Result<Vec<SignalToken>, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let NodeKind::WaypointConversation { graph, .. } = &def.kind else {
            return Err(GraphError::UnsupportedNode {
                node_guid,
                category: def.category,
            });
        };
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let current = self.nodes[index].current_topic_id.clone();
        let input_pin = self.nodes[index]
            .executed_input_pin
            .clone()
            .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_string());
        let edge = graph
            .edges
            .iter()
            .find(|edge| edge.edge_id == edge_id)
            .ok_or(GraphError::TopicEdgeNotFound {
                node_guid,
                edge_id: edge_id.to_string(),
            })?;
        if edge.from_topic_id != current {
            return Err(GraphError::TopicEdgeNotAdjacent {
                node_guid,
                edge_id: edge_id.to_string(),
                current_topic_id: current,
            });
        }
        let target = graph
            .nodes
            .iter()
            .find(|topic| topic.topic_id == edge.to_topic_id)
            .ok_or(GraphError::TopicNodeNotFound {
                node_guid,
                topic_id: edge.to_topic_id.clone(),
            })?;
        let quality = target.revealed_clue_quality_id.as_ref();
        let exits = target.triggers_scene_exit;
        let exit_pin = if target.exit_output_pin.is_empty() {
            DEFAULT_OUTPUT_PIN.to_string()
        } else {
            target.exit_output_pin.clone()
        };
        let to_topic = edge.to_topic_id.clone();
        let mut plan = NodePlan {
            node_guid,
            input_pin,
            current_topic_id: Some(to_topic),
            edge_use_count: Some((edge_id.to_string(), 1)),
            commit: true,
            ..NodePlan::default()
        };
        let mut revealed_quality_tag = None;
        if let Some(name) = quality {
            let registry = ctx.registry.ok_or(GraphError::RegistryRequired {
                node_guid,
                tag_name: name.clone(),
            })?;
            let id = registry.id(name).ok_or(GraphError::RegistryRequired {
                node_guid,
                tag_name: name.clone(),
            })?;
            let mut op = MutationOp::default();
            op.add_global_tags.set_bit(id).map_err(GraphError::State)?;
            plan.delta = ctx.apply_op(&op)?;
            revealed_quality_tag = Some(id);
        }
        plan.call_data = Some(NodeCallData::WaypointStep {
            previous_topic_id: current,
            edge_id: edge_id.to_string(),
            revealed_quality_tag,
            dialogue_transition_asset_id: None,
            delta: plan.delta.clone(),
        });
        plan.record = true;
        plan.completed = exits;
        if exits {
            plan.output_pin = Some(exit_pin);
        }
        self.commit_plan(&plan)
    }

    /// Attach the child asset a mission node owns (REQ-CH-04, spec §8.2). The
    /// adapter builds instances top-down, one call per mission node. The child
    /// asset must validate, must match the node's `sub_graph_asset_id`, and must
    /// not already appear in this instance's ancestor chain — that is what makes
    /// a mission cycle impossible.
    pub fn attach_mission_asset(
        &mut self,
        node_guid: u32,
        child_asset: &FlowGraphAsset,
        seed: u64,
    ) -> Result<(), GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let NodeKind::MissionSubGraph { sub_graph_asset_id } = &def.kind else {
            return Err(GraphError::UnsupportedNode {
                node_guid,
                category: def.category,
            });
        };
        if child_asset.asset_id != *sub_graph_asset_id {
            return Err(GraphError::MissionAssetIdMismatch {
                node_guid,
                asset_id: child_asset.asset_id.clone(),
            });
        }
        if self.mission_asset_path.contains(&child_asset.asset_id) {
            return Err(GraphError::MissionAssetCycle {
                node_guid,
                asset_id: child_asset.asset_id.clone(),
            });
        }
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        if self.nodes[index].sub_graph.is_some() {
            return Err(GraphError::MissionAssetAlreadyAttached { node_guid });
        }
        let mut path = self.mission_asset_path.clone();
        path.push(child_asset.asset_id.clone());
        let instance_id =
            mix64(self.instance_id ^ (node_guid as u64).wrapping_add(0x9e37_79b9_7f4a_7755));
        let child = Self::new_at_path(instance_id, child_asset.clone(), seed, path)?;
        self.nodes[index].sub_graph = Some(Box::new(child));
        Ok(())
    }

    /// Attach every authored mission asset and recursively attach its children.
    /// Asset identity in a MissionSubGraph node is the target storylet ID.
    pub fn attach_mission_assets(
        &mut self,
        mission_assets: &BTreeMap<String, FlowGraphAsset>,
        seed: u64,
    ) -> Result<(), GraphError> {
        let missions = self
            .asset
            .nodes
            .iter()
            .filter_map(|node| match &node.kind {
                NodeKind::MissionSubGraph { sub_graph_asset_id } => {
                    Some((node.node_guid, sub_graph_asset_id.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (node_guid, asset_id) in missions {
            let asset = mission_assets.get(&asset_id).ok_or_else(|| {
                GraphError::MissionAssetNotAttached {
                    node_guid,
                    sub_graph_asset_id: asset_id.clone(),
                }
            })?;
            self.attach_mission_asset(node_guid, asset, mix64(seed ^ u64::from(node_guid)))?;
            let index = self
                .node_index(node_guid)
                .ok_or(GraphError::NodeNotFound { node_guid })?;
            let child = self.nodes[index].sub_graph.as_mut().ok_or_else(|| {
                GraphError::MissionAssetNotAttached {
                    node_guid,
                    sub_graph_asset_id: asset_id,
                }
            })?;
            child.attach_mission_assets(mission_assets, seed)?;
        }
        Ok(())
    }

    /// The nested instance a mission node owns, when one is attached.
    pub fn mission_instance(&self, node_guid: u32) -> Option<&FlowGraphInstance> {
        self.node(node_guid)
            .and_then(|node| node.sub_graph.as_deref())
    }

    /// The scoped array the adapter serializes for a mission node (REQ-CH-02,
    /// REQ-CH-04). The engine keeps it in sync with the attached child's own
    /// history; for legacy same-asset mission scopes it is the scoped list.
    pub fn mission_call_history(&self, node_guid: u32) -> &[CallHistoryEntry] {
        match self.node_index(node_guid) {
            Some(index) => &self.nodes[index].local_call_history,
            None => &[],
        }
    }

    /// True while this instance still has work pending: an active token or a
    /// latent node. A mission boundary completes only when its child reaches
    /// this point.
    fn has_pending_work(&self) -> bool {
        self.tokens
            .iter()
            .any(|token| token.is_active() || token.is_deferred())
            || self
                .nodes
                .iter()
                .any(|node| node.state == NodeExecutionState::ActiveLatent)
    }

    /// Mirror the child instance's history into the mission node's scoped array.
    fn sync_mission_history(&mut self, node_guid: u32) -> Result<(), GraphError> {
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let entries = match self.nodes[index].sub_graph.as_ref() {
            Some(child) => child.call_history().to_vec(),
            None => return Ok(()),
        };
        let mirror = &mut self.nodes[index].local_call_history;
        mirror.clear();
        mirror.extend(entries);
        Ok(())
    }

    /// Unwind a mission's local history in reverse (REQ-CH-04). Only the child
    /// instance's records are touched; the mission node's own record is the
    /// caller's responsibility.
    fn unwind_mission_history(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
    ) -> Result<(), GraphError> {
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let Some(child) = self.nodes[index].sub_graph.as_mut() else {
            return Ok(());
        };
        let local = child.call_history().to_vec();
        for entry in local.iter().rev() {
            child.cancel_and_revert(ctx, entry.node_guid)?;
        }
        Ok(())
    }

    /// Execute one mission node: run the attached child against the shared world
    /// state, then decide the mission boundary itself. The mission node records
    /// globally with an empty delta — a boundary marker — because every real
    /// delta lives in the child instance, so a revert never applies the same
    /// change twice.
    fn execute_mission(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
        input_pin: &str,
        budget_ms: i64,
        force: bool,
    ) -> Result<Vec<SignalToken>, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let NodeKind::MissionSubGraph { sub_graph_asset_id } = &def.kind else {
            return Err(GraphError::UnsupportedNode {
                node_guid,
                category: def.category,
            });
        };
        let asset_id = sub_graph_asset_id.clone();
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        if self.nodes[index].state == NodeExecutionState::Completed {
            // Re-entering would double-apply the child while replay cannot
            // distinguish invocations; reject until scoped histories carry an
            // invocation ordinal.
            return Err(GraphError::MissionReentry { node_guid });
        }
        let first_activation = self.nodes[index].state == NodeExecutionState::Idle;
        let mut executed = {
            let child = self.nodes[index].sub_graph.as_mut().ok_or(
                GraphError::MissionAssetNotAttached {
                    node_guid,
                    sub_graph_asset_id: asset_id,
                },
            )?;
            if first_activation {
                child.start()?;
            }
            child.tick(ctx, budget_ms)?
        };
        let plan = self.plan(
            ctx, node_guid, input_pin, None, None, budget_ms, true, force,
        )?;
        let emitted = self.commit_plan(&plan)?;
        self.sync_mission_history(node_guid)?;
        executed.extend(emitted);
        Ok(executed)
    }
    /// Unwind a mission to its start. Only the mission's local history is
    /// unwound; the mission node's own global record is left intact (REQ-CH-04).
    pub fn rollback_mission_to_start(
        &mut self,
        ctx: &mut RuntimeContext,
        node_guid: u32,
    ) -> Result<(), GraphError> {
        {
            let def = self
                .asset
                .node(node_guid)
                .ok_or(GraphError::NodeNotFound { node_guid })?;
            if !matches!(def.kind, NodeKind::MissionSubGraph { .. }) {
                return Err(GraphError::UnsupportedNode {
                    node_guid,
                    category: def.category,
                });
            }
        }
        let index = self
            .node_index(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let attached = self.nodes[index].sub_graph.is_some();
        if attached {
            // Spec §8.2: unwind the child instance's records in reverse, clear
            // the scoped array, then restart the child from its start node.
            {
                let child = self.nodes[index].sub_graph.as_mut().unwrap();
                let local = child.call_history().to_vec();
                for entry in local.iter().rev() {
                    child.cancel_and_revert(ctx, entry.node_guid)?;
                }
                child.start()?;
            }
        } else {
            let local = self.nodes[index].local_call_history.clone();
            for entry in local.iter().rev() {
                self.cancel_and_revert(ctx, entry.node_guid)?;
            }
        }
        self.nodes[index].local_call_history.clear();
        let node = &mut self.nodes[index];
        // An attached mission stays open so the restarted child continues on the
        // next frame; an unattached mission node returns to Idle as before.
        node.state = if attached {
            NodeExecutionState::ActiveLatent
        } else {
            NodeExecutionState::Idle
        };
        node.executed_input_pin = None;
        node.chosen_output_pin = None;
        node.chosen_storylet_id = None;
        Ok(())
    }
}

fn sorted_events(events: &[TimelineTrackEvent]) -> Vec<&TimelineTrackEvent> {
    let mut ordered: Vec<&TimelineTrackEvent> = events.iter().collect();
    ordered.sort_by(|a, b| {
        a.start_time_ms
            .cmp(&b.start_time_ms)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });
    ordered
}

fn make_part(
    event: &TimelineTrackEvent,
    activity: EventOverlapActivity,
    cursor_ms: i64,
) -> ActionPart {
    ActionPart {
        action_key_priority: event.action_type.action_key_priority() + event.priority_offset,
        combiner_entity_path: event.target_entity_path.clone(),
        activity,
        local_cursor_time_ms: cursor_ms,
        event_ref: event.clone(),
    }
}

fn record_storylet(record: &NodeCallData) -> Option<u16> {
    match record {
        NodeCallData::Storylet { storylet_id, .. } => Some(*storylet_id),
        _ => None,
    }
}

impl<'a> RuntimeContext<'a> {
    fn apply_op_on(
        &self,
        table: &mut WorldStateTable,
        op: &MutationOp,
    ) -> Result<BijectiveDelta, GraphError> {
        match self.registry {
            Some(registry) => {
                apply_mutation_with_registry(table, op, registry).map_err(GraphError::State)
            }
            None => apply_mutation(table, op).map_err(GraphError::State),
        }
    }
}

impl FlowGraphInstance {
    /// Replay a call history from a clean instance (REQ-CH-03).
    pub fn replay_history(
        &mut self,
        ctx: &mut RuntimeContext,
        entries: &[CallHistoryEntry],
    ) -> Result<(), GraphError> {
        self.replay_history_with_missions(ctx, entries, &BTreeMap::new())
    }

    /// Replay a global history plus the scoped mission arrays (REQ-CH-03). The
    /// world state is rebuilt by re-running the recorded nodes; nothing about
    /// the world itself is serialized. `mission_histories` is keyed by mission
    /// node GUID and replayed at the point that node is force-completed, so
    /// mission effects land in the same order as the original run. Node GUIDs
    /// are unique across the instance tree, so nested missions resolve against
    /// the same map.
    pub fn replay_history_with_missions(
        &mut self,
        ctx: &mut RuntimeContext,
        entries: &[CallHistoryEntry],
        mission_histories: &BTreeMap<u32, Vec<CallHistoryEntry>>,
    ) -> Result<(), GraphError> {
        self.reset();
        let result = (|| {
            for entry in entries {
                let mission_asset_id = match self.asset.node(entry.node_guid) {
                    Some(def) => match &def.kind {
                        NodeKind::MissionSubGraph { sub_graph_asset_id } => {
                            Some(sub_graph_asset_id.clone())
                        }
                        _ => None,
                    },
                    None => None,
                };
                if let Some(asset_id) = mission_asset_id {
                    if let Some(scoped) = mission_histories.get(&entry.node_guid) {
                        let index =
                            self.node_index(entry.node_guid)
                                .ok_or(GraphError::NodeNotFound {
                                    node_guid: entry.node_guid,
                                })?;
                        if self.nodes[index].state != NodeExecutionState::Completed {
                            let child = self.nodes[index].sub_graph.as_mut().ok_or(
                                GraphError::MissionAssetNotAttached {
                                    node_guid: entry.node_guid,
                                    sub_graph_asset_id: asset_id,
                                },
                            )?;
                            child.replay_history_with_missions(ctx, scoped, mission_histories)?;
                        }
                    }
                }
                let _ = self.force_complete_with_input(
                    ctx,
                    entry.node_guid,
                    &entry.executed_input_pin,
                    &entry.chosen_output_pin,
                    entry.chosen_storylet_id,
                )?;
            }
            Ok(())
        })();
        self.clear_pending_passion_threshold_events();
        result
    }

    /// Replay a save file, validating every record against the asset first.
    pub fn replay_save(
        &mut self,
        ctx: &mut RuntimeContext,
        entries: &[SaveReplayEntry],
    ) -> Result<(), GraphError> {
        self.reset();
        let result = (|| {
            for entry in entries {
                if self.asset.node(entry.node_guid).is_none() {
                    return Err(GraphError::NodeNotFound {
                        node_guid: entry.node_guid,
                    });
                }
                if let Some(storylet_id) = entry.chosen_storylet_id {
                    if ctx.dispatcher.storylet(storylet_id).is_none() {
                        return Err(GraphError::UnresolvedStorylet { storylet_id });
                    }
                }
                self.clock_ms = entry.frame_ms;
                self.turn = self.turn.saturating_add(1);
                let _ = self.force_complete_with_input(
                    ctx,
                    entry.node_guid,
                    &entry.executed_input_pin,
                    &entry.chosen_output_pin,
                    entry.chosen_storylet_id,
                )?;
            }
            Ok(())
        })();
        self.clear_pending_passion_threshold_events();
        result
    }
    /// Reconcile with an external history (REQ-CH-06): find the lowest common
    /// ancestor by prefix comparison, revert to it, then append the divergent
    /// suffix as new commits. Returns the new head.
    pub fn reconcile_history(
        &mut self,
        ctx: &mut RuntimeContext,
        target: &[CallHistoryEntry],
    ) -> Result<usize, GraphError> {
        // `path_to` starts at the root sentinel commit 0, which carries no
        // entry; compare only real commits.
        let current: Vec<usize> = self
            .history
            .path_to(self.history.head)?
            .into_iter()
            .filter(|&commit| commit != 0)
            .collect();
        let mut common = 0;
        let mut lca = 0;
        for (position, entry) in target.iter().enumerate() {
            let Some(&commit) = current.get(position) else {
                break;
            };
            if self.history.commits[commit].entry != *entry {
                break;
            }
            common = position + 1;
            lca = commit;
        }
        self.revert_to_commit(ctx, lca)?;
        for entry in &target[common..] {
            let _ = self.force_complete_with_input(
                ctx,
                entry.node_guid,
                &entry.executed_input_pin,
                &entry.chosen_output_pin,
                entry.chosen_storylet_id,
            )?;
        }
        Ok(self.history.head)
    }

    /// Move the head to an existing commit: revert to the LCA with the stored
    /// deltas, then re-run the forward branch. A recomputed delta that differs
    /// from the stored one means the history and the graph disagree.
    pub fn checkout(
        &mut self,
        ctx: &mut RuntimeContext,
        target_commit: usize,
    ) -> Result<Vec<CallHistoryEntry>, GraphError> {
        let lca = self.history.lca(self.history.head, target_commit)?;
        self.revert_to_commit(ctx, lca)?;
        let path = self.history.path_to(target_commit)?;
        // Ancestors of the LCA are already applied; replay only the suffix.
        let start = path
            .iter()
            .position(|&commit| commit == lca)
            .map_or(0, |position| position + 1);
        let mut applied = Vec::new();
        for &commit in &path[start..] {
            if commit == 0 {
                continue;
            }
            let record = self.history.commits[commit].clone();
            let state_snapshot = ctx.state.clone();
            let instance_snapshot = self.clone();
            // Re-run the recorded call through the same path as a live
            // execution so scene timelines and mission children replay too.
            // `commit: false` keeps the DAG untouched; the target commit is
            // already stored, and appending a copy would orphan it.
            let delta = self.force_complete_with_input_commit(
                ctx,
                record.entry.node_guid,
                &record.entry.executed_input_pin,
                &record.entry.chosen_output_pin,
                record.entry.chosen_storylet_id,
                false,
            )?;
            if delta != record.delta {
                *ctx.state = state_snapshot;
                *self = instance_snapshot;
                return Err(GraphError::CallDataMismatch {
                    node_guid: record.entry.node_guid,
                });
            }
            applied.push(record.entry);
        }
        self.history.head = target_commit;
        Ok(applied)
    }

    fn revert_to_commit(&mut self, ctx: &mut RuntimeContext, lca: usize) -> Result<(), GraphError> {
        let path = self.history.path_to(self.history.head)?;
        for &commit in path.iter().rev() {
            if commit == lca {
                break;
            }
            let entry = self.history.commits[commit].entry.clone();
            self.cancel_and_revert(ctx, entry.node_guid)?;
        }
        self.history.head = lca;
        Ok(())
    }

    /// Invoke the story up to a point of interest (REQ-CH-02): post-order seek
    /// with an explicit stack, resolve each hub's prerequisite chain, then
    /// force-complete the path and the POI.
    pub fn invoke_story_to_poi(
        &mut self,
        ctx: &mut RuntimeContext,
        poi_node_guid: u32,
        declared_routes: &BTreeMap<u32, String>,
    ) -> Result<Vec<CallHistoryEntry>, GraphError> {
        {
            let def = self
                .asset
                .node(poi_node_guid)
                .ok_or(GraphError::NodeNotFound {
                    node_guid: poi_node_guid,
                })?;
            if !matches!(def.kind, NodeKind::Poi { .. }) {
                return Err(GraphError::UnsupportedNode {
                    node_guid: poi_node_guid,
                    category: def.category,
                });
            }
        }
        let raw_path = self.seek_backward(poi_node_guid, declared_routes)?;
        let mut simulated_state = ctx.state.clone();
        let mut simulated_context = RuntimeContext {
            state: &mut simulated_state,
            dispatcher: ctx.dispatcher,
            registry: ctx.registry,
            passion_registry: ctx.passion_registry,
        };
        let mut simulated_instance = self.clone();
        let mut path = Vec::new();
        for entry in raw_path {
            if matches!(
                simulated_instance
                    .asset
                    .node(entry.node_guid)
                    .map(|node| &node.kind),
                Some(NodeKind::QbnHub { .. })
            ) {
                let chain = simulated_instance
                    .qbn_prerequisite_chain(&simulated_context, entry.node_guid)?;
                for storylet_id in chain {
                    let mut selected = entry.clone();
                    selected.chosen_storylet_id = Some(storylet_id);
                    let _ = simulated_instance.force_complete_with_input(
                        &mut simulated_context,
                        selected.node_guid,
                        &selected.executed_input_pin,
                        &selected.chosen_output_pin,
                        selected.chosen_storylet_id,
                    )?;
                    path.push(selected);
                }
            } else {
                let _ = simulated_instance.force_complete_with_input(
                    &mut simulated_context,
                    entry.node_guid,
                    &entry.executed_input_pin,
                    &entry.chosen_output_pin,
                    entry.chosen_storylet_id,
                )?;
                path.push(entry);
            }
        }
        for entry in &path {
            let _ = self.force_complete_with_input(
                ctx,
                entry.node_guid,
                &entry.executed_input_pin,
                &entry.chosen_output_pin,
                entry.chosen_storylet_id,
            )?;
        }
        let poi_input = self
            .asset
            .node(poi_node_guid)
            .and_then(|node| node.input_pins.first())
            .map(|pin| pin.pin_name.clone())
            .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_owned());
        let _ = self.force_complete_with_input(
            ctx,
            poi_node_guid,
            &poi_input,
            DEFAULT_OUTPUT_PIN,
            None,
        )?;
        Ok(path)
    }

    /// Post-order seek from a POI back through the graph. AND gates take every
    /// incoming edge; ANY, choice, and multi-fan-in nodes take the declared
    /// route, falling back to the first incoming edge in deterministic order.
    /// Completed nodes stop that branch.
    fn seek_backward(
        &self,
        target: u32,
        declared_routes: &BTreeMap<u32, String>,
    ) -> Result<Vec<CallHistoryEntry>, GraphError> {
        let mut visited = BTreeSet::new();
        let mut entries: Vec<CallHistoryEntry> = Vec::new();
        let mut stack: Vec<(u32, Option<(String, String)>, bool)> = vec![(target, None, false)];
        while let Some((guid, route, exiting)) = stack.pop() {
            if exiting {
                if guid == target {
                    continue;
                }
                let def = self
                    .asset
                    .node(guid)
                    .ok_or(GraphError::NodeNotFound { node_guid: guid })?;
                if self.node(guid).map(|node| node.state) == Some(NodeExecutionState::Completed) {
                    continue;
                }
                let (_, output_pin) =
                    route.ok_or(GraphError::NoDeclaredRoute { node_guid: guid })?;
                let input_pin = if matches!(def.kind, NodeKind::LogicAll) {
                    // The gate completes on its last arriving branch, which is
                    // the last predecessor in deterministic edge order; replay
                    // compares this recorded pin against live execution.
                    self.asset
                        .incoming_edges(guid)
                        .last()
                        .map(|edge| edge.target_pin_name.clone())
                        .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_owned())
                } else {
                    declared_routes
                        .get(&guid)
                        .cloned()
                        .or_else(|| def.input_pins.first().map(|pin| pin.pin_name.clone()))
                        .unwrap_or_else(|| DEFAULT_INPUT_PIN.to_owned())
                };
                let base = CallHistoryEntry {
                    node_guid: guid,
                    executed_input_pin: input_pin,
                    chosen_output_pin: output_pin,
                    chosen_storylet_id: None,
                };
                entries.push(base);
                continue;
            }
            if guid != target
                && self
                    .node(guid)
                    .is_some_and(|node| node.state == NodeExecutionState::Completed)
            {
                continue;
            }
            if !visited.insert(guid) {
                continue;
            }
            stack.push((guid, route, true));
            let def = self
                .asset
                .node(guid)
                .ok_or(GraphError::NodeNotFound { node_guid: guid })?;
            let incoming = self.asset.incoming_edges(guid);
            if incoming.is_empty() {
                continue;
            }
            let selected: Vec<&EdgeConnection> = if matches!(def.kind, NodeKind::LogicAll) {
                incoming
            } else {
                let pin = declared_routes
                    .get(&guid)
                    .cloned()
                    .unwrap_or_else(|| incoming[0].target_pin_name.clone());
                incoming
                    .into_iter()
                    .filter(|edge| edge.target_pin_name == pin)
                    .take(1)
                    .collect()
            };
            for edge in selected.iter().rev() {
                stack.push((
                    edge.source_node_guid,
                    Some((edge.target_pin_name.clone(), edge.source_pin_name.clone())),
                    false,
                ));
            }
        }
        Ok(entries)
    }

    /// Greedy prerequisite chain for a hub: pick deck members that are eligible
    /// against a simulated copy of the world and contribute a tag the exit
    /// storylet still needs, ordered by display priority then storylet id, then
    /// append the exit.
    fn qbn_prerequisite_chain(
        &self,
        ctx: &RuntimeContext,
        node_guid: u32,
    ) -> Result<Vec<u16>, GraphError> {
        let def = self
            .asset
            .node(node_guid)
            .ok_or(GraphError::NodeNotFound { node_guid })?;
        let NodeKind::QbnHub {
            deck,
            exit_storylet_id,
        } = &def.kind
        else {
            return Err(GraphError::UnsupportedNode {
                node_guid,
                category: def.category,
            });
        };
        let exit_id = match exit_storylet_id {
            Some(id) => *id,
            None => return Err(GraphError::UnresolvedStorylet { storylet_id: 0 }),
        };
        let exit = ctx
            .dispatcher
            .storylet(exit_id)
            .ok_or(GraphError::UnresolvedStorylet {
                storylet_id: exit_id,
            })?;
        let needed = exit.required_tags;
        let completed = self
            .node(node_guid)
            .map(|node| node.completed_storylets.clone())
            .unwrap_or_default();
        let mut simulated = ctx.state.clone();
        let mut used = BTreeSet::new();
        let mut chain: Vec<u16> = Vec::new();
        let mut candidates: Vec<u16> = deck
            .iter()
            .copied()
            .filter(|storylet_id| {
                *storylet_id != exit_id
                    && !completed.contains(storylet_id)
                    && !used.contains(storylet_id)
            })
            .collect();
        candidates.sort_by(|a, b| {
            let left = ctx.dispatcher.storylet(*a);
            let right = ctx.dispatcher.storylet(*b);
            right
                .map(|def| def.display_priority)
                .unwrap_or(i32::MIN)
                .cmp(&left.map(|def| def.display_priority).unwrap_or(i32::MIN))
                .then_with(|| a.cmp(b))
        });
        for _ in 0..candidates.len() {
            if simulated.global_tags.contains_all(&needed) {
                break;
            }
            let mut picked = None;
            for storylet_id in &candidates {
                if used.contains(storylet_id) {
                    continue;
                }
                let Some(def) = ctx.dispatcher.storylet(*storylet_id) else {
                    continue;
                };
                if !ctx
                    .dispatcher
                    .try_is_eligible(def, &simulated)
                    .map_err(GraphError::Dispatch)?
                {
                    continue;
                }
                let op = ctx
                    .dispatcher
                    .compile_effects(def, &simulated)
                    .map_err(GraphError::Dispatch)?;
                let contributes = op.add_global_tags.popcount_intersection(&needed) > 0;
                if !contributes {
                    continue;
                }
                picked = Some((*storylet_id, op));
                break;
            }
            match picked {
                Some((storylet_id, op)) => {
                    used.insert(storylet_id);
                    chain.push(storylet_id);
                    ctx.apply_op_on(&mut simulated, &op)?;
                }
                None => break,
            }
        }
        let missing: Vec<u16> = (0..MAX_TAGS)
            .filter(|index| needed.bit(*index as u16) && !simulated.global_tags.bit(*index as u16))
            .map(|index| index as u16)
            .collect();
        if !missing.is_empty() {
            return Err(GraphError::UnresolvedPrerequisiteChain {
                node_guid,
                missing_tags: missing,
            });
        }
        chain.push(exit_id);
        Ok(chain)
    }
}

/// Rebase a saved call history onto an updated asset (REQ-CH-07): drop records
/// whose node or storylet no longer exists, then bridge consecutive records that
/// are no longer directly connected.
pub fn rebase_call_history(
    updated: &FlowGraphAsset,
    dispatcher: &UnifiedDispatcher,
    saved: &[CallHistoryEntry],
) -> Result<Vec<CallHistoryEntry>, GraphError> {
    let mut rebased: Vec<CallHistoryEntry> = Vec::new();
    let mut previous: Option<u32> = None;
    for entry in saved {
        if updated.node(entry.node_guid).is_none() {
            continue;
        }
        if let Some(storylet_id) = entry.chosen_storylet_id {
            if dispatcher.storylet(storylet_id).is_none() {
                continue;
            }
        }
        let def = updated.node(entry.node_guid).expect("node checked above");
        if !entry.executed_input_pin.is_empty()
            && updated
                .input_pin_index(entry.node_guid, &entry.executed_input_pin)
                .is_none()
        {
            continue;
        }
        if !entry.chosen_output_pin.is_empty()
            && !def
                .output_pins
                .iter()
                .any(|pin| pin.pin_name == entry.chosen_output_pin)
        {
            continue;
        }
        if let Some(from) = previous {
            if !updated.has_direct_edge(from, entry.node_guid) {
                for (node_guid, input_pin, output_pin) in
                    updated.shortest_bridge_path(from, entry.node_guid)?
                {
                    rebased.push(CallHistoryEntry {
                        node_guid,
                        executed_input_pin: input_pin,
                        chosen_output_pin: output_pin,
                        chosen_storylet_id: None,
                    });
                }
            }
        }
        rebased.push(entry.clone());
        previous = Some(entry.node_guid);
    }
    Ok(rebased)
}

/// Operators used by composite conditional nodes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConditionOperator {
    #[default]
    Tag,
    Metric,
    Visited,
    Met,
    Variable,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOperator {
    #[default]
    GreaterOrEqual,
    Greater,
    Less,
    LessOrEqual,
    Equal,
    NotEqual,
}

/// Scope keys keep storylet-local variables isolated from each other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableScope {
    #[default]
    Global,
    Storylet(u16),
}

/// Typed value supported by local and global graph variables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum VariableValue {
    Number(f32),
    Flag(bool),
    Text(String),
}

impl Default for VariableValue {
    fn default() -> Self {
        Self::Number(0.0)
    }
}

/// One boolean condition in an AND-composite conditional node.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConditionRule {
    pub operator: ConditionOperator,
    #[serde(default)]
    pub metric_index: u16,
    #[serde(default)]
    pub entity_index: u16,
    #[serde(default)]
    pub comparison: ComparisonOperator,
    #[serde(default)]
    pub value: VariableValue,
    #[serde(default)]
    pub tags: TagBitset,
    #[serde(default)]
    pub tag_name: String,
    #[serde(default)]
    pub variable_scope: VariableScope,
    #[serde(default)]
    pub variable_name: String,
}

/// AND-composite condition with explicit branch pins.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    #[serde(default)]
    pub rules: Vec<ConditionRule>,
    pub true_pin: String,
    pub false_pin: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableOperator {
    #[default]
    Set,
    Add,
    Subtract,
    Toggle,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableAssignment {
    pub scope: VariableScope,
    pub name: String,
    pub operator: VariableOperator,
    pub value: VariableValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableRollbackEntry {
    pub scope: VariableScope,
    pub name: String,
    pub previous: Option<VariableValue>,
}

/// Deterministic nested variable table. Nesting keeps lookups allocation-free
/// and serializes cleanly for runtime session persistence.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VariableStore {
    #[serde(with = "variable_scope_map")]
    pub values: BTreeMap<VariableScope, BTreeMap<String, VariableValue>>,
}

mod variable_scope_map {
    use super::{VariableScope, VariableValue};
    use serde::de::Error;
    use serde::ser::SerializeSeq;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S>(
        values: &BTreeMap<VariableScope, BTreeMap<String, VariableValue>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(values.len()))?;
        for (scope, variables) in values {
            sequence.serialize_element(&(scope, variables))?;
        }
        sequence.end()
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<BTreeMap<VariableScope, BTreeMap<String, VariableValue>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries =
            Vec::<(VariableScope, BTreeMap<String, VariableValue>)>::deserialize(deserializer)?;
        let mut values = BTreeMap::new();
        for (scope, variables) in entries {
            if values.insert(scope, variables).is_some() {
                return Err(D::Error::custom("duplicate variable scope"));
            }
        }
        Ok(values)
    }
}

impl VariableStore {
    pub fn get(&self, scope: &VariableScope, name: &str) -> Option<&VariableValue> {
        self.values.get(scope)?.get(name)
    }

    pub fn set(&mut self, scope: &VariableScope, name: &str, value: &VariableValue) {
        self.values
            .entry(*scope)
            .or_default()
            .insert(name.to_string(), value.clone());
    }

    pub fn remove(&mut self, scope: &VariableScope, name: &str) -> Option<VariableValue> {
        let (old, remove_scope) = {
            let values = self.values.get_mut(scope)?;
            let old = values.remove(name);
            (old, values.is_empty())
        };
        if remove_scope {
            self.values.remove(scope);
        }
        old
    }

    pub fn clear(&mut self) {
        self.values.clear();
    }

    pub fn names(&self) -> Vec<String> {
        self.values
            .values()
            .flat_map(|scope| scope.keys().cloned())
            .collect()
    }
}

fn apply_variable_operator(
    node_guid: u32,
    assignment: &VariableAssignment,
    previous: &Option<VariableValue>,
) -> Result<VariableValue, GraphError> {
    let mismatch = || GraphError::VariableTypeMismatch {
        node_guid,
        name: assignment.name.clone(),
    };
    match assignment.operator {
        VariableOperator::Set => match &assignment.value {
            VariableValue::Number(value) if !value.is_finite() => {
                Err(GraphError::NonFiniteMetricValue {
                    node_guid,
                    value: *value,
                })
            }
            value => Ok(value.clone()),
        },
        VariableOperator::Add | VariableOperator::Subtract => {
            let VariableValue::Number(delta) = &assignment.value else {
                return Err(mismatch());
            };
            let delta = *delta;
            if !delta.is_finite() {
                return Err(GraphError::NonFiniteMetricValue {
                    node_guid,
                    value: delta,
                });
            }
            let current = match previous {
                None => 0.0,
                Some(VariableValue::Number(value)) => *value,
                _ => return Err(mismatch()),
            };
            let next = if assignment.operator == VariableOperator::Add {
                current + delta
            } else {
                current - delta
            };
            if !next.is_finite() {
                return Err(GraphError::NonFiniteMetricValue {
                    node_guid,
                    value: next,
                });
            }
            Ok(VariableValue::Number(next))
        }
        VariableOperator::Toggle => {
            if !matches!(&assignment.value, VariableValue::Flag(_)) {
                return Err(mismatch());
            }
            let current = match previous {
                None => false,
                Some(VariableValue::Flag(value)) => *value,
                _ => return Err(mismatch()),
            };
            Ok(VariableValue::Flag(!current))
        }
    }
}

fn compare_f32(left: f32, right: f32, comparison: ComparisonOperator) -> bool {
    match comparison {
        ComparisonOperator::GreaterOrEqual => left >= right,
        ComparisonOperator::Greater => left > right,
        ComparisonOperator::Less => left < right,
        ComparisonOperator::LessOrEqual => left <= right,
        ComparisonOperator::Equal => left == right,
        ComparisonOperator::NotEqual => left != right,
    }
}

fn compare_values(
    left: &VariableValue,
    right: &VariableValue,
    comparison: ComparisonOperator,
) -> bool {
    match (left, right) {
        (VariableValue::Number(left), VariableValue::Number(right)) => {
            compare_f32(*left, *right, comparison)
        }
        (VariableValue::Flag(left), VariableValue::Flag(right)) => match comparison {
            ComparisonOperator::Equal => left == right,
            ComparisonOperator::NotEqual => left != right,
            _ => false,
        },
        (VariableValue::Text(left), VariableValue::Text(right)) => match comparison {
            ComparisonOperator::Equal => left == right,
            ComparisonOperator::NotEqual => left != right,
            _ => false,
        },
        _ => false,
    }
}

impl FlowGraphInstance {
    fn evaluate_condition(
        &self,
        ctx: &RuntimeContext,
        node_guid: u32,
        condition: &Condition,
    ) -> Result<bool, GraphError> {
        if condition.rules.is_empty() {
            return Err(GraphError::EmptyCondition { node_guid });
        }
        for rule in &condition.rules {
            let result = match rule.operator {
                ConditionOperator::Metric => {
                    let index = rule.metric_index as usize;
                    let Some(current) = ctx.state.metrics.get(index) else {
                        return Err(GraphError::MetricIndexOutOfRange {
                            node_guid,
                            metric_index: rule.metric_index,
                        });
                    };
                    compare_values(
                        &VariableValue::Number(*current),
                        &rule.value,
                        rule.comparison,
                    )
                }
                ConditionOperator::Tag => {
                    let mask = if rule.tags.is_empty() {
                        let registry = ctx.registry.ok_or(GraphError::RegistryRequired {
                            node_guid,
                            tag_name: rule.tag_name.clone(),
                        })?;
                        let id =
                            registry
                                .id(&rule.tag_name)
                                .ok_or(GraphError::RegistryRequired {
                                    node_guid,
                                    tag_name: rule.tag_name.clone(),
                                })?;
                        *registry
                            .ancestors_mask(id)
                            .ok_or(GraphError::RegistryRequired {
                                node_guid,
                                tag_name: rule.tag_name.clone(),
                            })?
                    } else {
                        rule.tags
                    };
                    if mask.is_empty() {
                        return Err(GraphError::EmptyCondition { node_guid });
                    }
                    let present = ctx.state.global_tags.contains_all(&mask);
                    compare_values(&VariableValue::Flag(present), &rule.value, rule.comparison)
                }
                ConditionOperator::Visited | ConditionOperator::Met => {
                    if rule.entity_index as usize >= ctx.state.entity_tags.len() {
                        return Err(GraphError::EntityIndexOutOfRange {
                            node_guid,
                            entity_index: rule.entity_index,
                        });
                    }
                    let present = if rule.operator == ConditionOperator::Visited {
                        ctx.state
                            .visited_location_tags
                            .entity_mask
                            .bit(rule.entity_index)
                    } else {
                        ctx.state
                            .met_character_tags
                            .entity_mask
                            .bit(rule.entity_index)
                    };
                    compare_values(&VariableValue::Flag(present), &rule.value, rule.comparison)
                }
                ConditionOperator::Variable => {
                    match self
                        .variables
                        .get(&rule.variable_scope, &rule.variable_name)
                    {
                        Some(current) => compare_values(current, &rule.value, rule.comparison),
                        None => false,
                    }
                }
            };
            if !result {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::chrono_dispatch::UnifiedDispatcher;
    use crate::chrono_state::WorldStateTable;

    #[test]
    fn token_clock_clamps_to_the_packed_range() {
        assert_eq!(clamp_token_ms(i64::MAX), i32::MAX);
        assert_eq!(clamp_token_ms(i64::MIN), i32::MIN);
        assert_eq!(clamp_token_ms(42), 42);
    }

    #[test]
    fn deferred_token_counts_as_pending_work() {
        let asset = FlowGraphAsset {
            asset_id: "deferred".into(),
            start_node_guid: 1,
            nodes: vec![node(
                1,
                NodeKind::Mutation {
                    op: MutationOp::default(),
                },
            )],
            edges: Vec::new(),
        };
        let mut instance = FlowGraphInstance::new(1, asset, 0).unwrap();
        assert!(!instance.has_pending_work());
        instance.tokens.push(SignalToken {
            token_id: 0,
            graph_instance_id: 1,
            target_node_guid: 1,
            target_pin_index: 0,
            flags: TOKEN_DEFERRED,
            time_budget_ms: 1,
            deferred_until_ms: 1,
        });
        assert!(instance.has_pending_work());
    }

    fn node(node_guid: u32, kind: NodeKind) -> FlowNodeDef {
        FlowNodeDef {
            node_guid,
            node_type_id: format!("test-{node_guid}"),
            category: NodeCategory::LogicGate,
            kind,
            input_pins: vec![PinDefinition {
                pin_name: DEFAULT_INPUT_PIN.into(),
                is_input: true,
            }],
            output_pins: vec![PinDefinition {
                pin_name: DEFAULT_OUTPUT_PIN.into(),
                is_input: false,
            }],
            ..FlowNodeDef::default()
        }
    }

    fn edge(from: u32, to: u32) -> EdgeConnection {
        EdgeConnection {
            source_node_guid: from,
            source_pin_name: DEFAULT_OUTPUT_PIN.into(),
            target_node_guid: to,
            target_pin_name: DEFAULT_INPUT_PIN.into(),
        }
    }

    fn mission_node(node_guid: u32, asset_id: &str) -> FlowNodeDef {
        FlowNodeDef {
            category: NodeCategory::ScopedContainer,
            ..node(
                node_guid,
                NodeKind::MissionSubGraph {
                    sub_graph_asset_id: asset_id.into(),
                },
            )
        }
    }

    #[test]
    fn external_event_releases_matching_waiter_before_timeout() {
        let asset = FlowGraphAsset {
            asset_id: "wait-test".into(),
            start_node_guid: 1,
            nodes: vec![
                node(1, NodeKind::LogicAll),
                node(
                    2,
                    NodeKind::Waiter {
                        duration_ms: 10_000,
                        required_tags: TagBitset::new(),
                        wait_event: "player.open_door".into(),
                    },
                ),
                node(
                    3,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
            ],
            edges: vec![
                EdgeConnection {
                    source_node_guid: 1,
                    source_pin_name: DEFAULT_OUTPUT_PIN.into(),
                    target_node_guid: 2,
                    target_pin_name: DEFAULT_INPUT_PIN.into(),
                },
                EdgeConnection {
                    source_node_guid: 2,
                    source_pin_name: DEFAULT_OUTPUT_PIN.into(),
                    target_node_guid: 3,
                    target_pin_name: DEFAULT_INPUT_PIN.into(),
                },
            ],
        };
        let mut instance = FlowGraphInstance::new(9, asset, 1).unwrap();
        let dispatcher = UnifiedDispatcher::default();
        let mut state = WorldStateTable::with_capacity(1, 0);
        let mut context = RuntimeContext::new(&mut state, &dispatcher);
        assert!(instance
            .signal_event(&mut context, "player.open_door")
            .unwrap()
            .is_empty());
        instance.start().unwrap();
        instance.tick(&mut context, 0).unwrap();
        assert_eq!(
            instance.node(2).unwrap().state,
            NodeExecutionState::ActiveLatent
        );
        assert_eq!(instance.clock_ms, 0);

        let emitted = instance
            .signal_event(&mut context, "player.open_door")
            .unwrap();
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].target_node_guid, 3);
        assert_eq!(
            instance.node(2).unwrap().state,
            NodeExecutionState::Completed
        );
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 2));
        instance.tick(&mut context, 0).unwrap();
        let end_calls = instance
            .global_call_history
            .iter()
            .filter(|entry| entry.node_guid == 3)
            .count();
        assert_eq!(end_calls, 1);
        instance.tick(&mut context, 0).unwrap();
        assert_eq!(
            instance
                .global_call_history
                .iter()
                .filter(|entry| entry.node_guid == 3)
                .count(),
            end_calls
        );
    }
    #[test]
    fn output_pin_fans_out_to_every_connected_target() {
        let asset = FlowGraphAsset {
            asset_id: "fanout-test".into(),
            start_node_guid: 1,
            nodes: vec![
                node(1, NodeKind::LogicAll),
                node(
                    2,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
                node(
                    3,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
            ],
            edges: vec![
                EdgeConnection {
                    source_node_guid: 1,
                    source_pin_name: DEFAULT_OUTPUT_PIN.into(),
                    target_node_guid: 2,
                    target_pin_name: DEFAULT_INPUT_PIN.into(),
                },
                EdgeConnection {
                    source_node_guid: 1,
                    source_pin_name: DEFAULT_OUTPUT_PIN.into(),
                    target_node_guid: 3,
                    target_pin_name: DEFAULT_INPUT_PIN.into(),
                },
            ],
        };
        let mut instance = FlowGraphInstance::new(3, asset, 1).unwrap();
        let dispatcher = UnifiedDispatcher::default();
        let mut state = WorldStateTable::with_capacity(1, 0);
        let mut context = RuntimeContext::new(&mut state, &dispatcher);

        instance.start().unwrap();
        instance.tick(&mut context, 0).unwrap();

        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 2));
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 3));
    }
    #[test]
    fn graph_transform_node_is_reversible_through_call_history_cache() {
        use crate::chrono_state::{
            AttributeAssignment, AttributeValue, GraphBinding, GraphLayer, GraphMutation,
            GraphPattern, GraphTransformRule, VertexPattern, VertexTemplate, WorldEdge,
            WorldVertex,
        };
        use std::collections::BTreeMap;

        let mut state = WorldStateTable::with_capacity(1, 0);
        state.vertices.push(WorldVertex {
            vertex_id: "fact.old".into(),
            layer: GraphLayer::Fact,
            entity_index: None,
            attributes: BTreeMap::new(),
        });
        state.vertices.push(WorldVertex {
            vertex_id: "fact.obsolete".into(),
            layer: GraphLayer::Fact,
            entity_index: None,
            attributes: BTreeMap::new(),
        });
        state.edges.push(WorldEdge {
            edge_id: "fact.old_obsolete".into(),
            source_id: "fact.old".into(),
            target_id: "fact.obsolete".into(),
            relation_type: "references".into(),
            weight: 1.0,
        });
        state
            .vertices
            .sort_by(|left, right| left.vertex_id.cmp(&right.vertex_id));
        let original = state.clone();
        let rule = GraphTransformRule {
            lhs: GraphPattern {
                vertices: vec![VertexPattern {
                    layer: GraphLayer::Fact,
                    vertex_id: Some("fact.old".into()),
                    attributes: Vec::new(),
                }],
                edges: Vec::new(),
            },
            rhs: GraphMutation {
                remove_vertices: vec![GraphBinding::Literal("fact.obsolete".into())],
                add_vertices: vec![VertexTemplate {
                    id: GraphBinding::Literal("fact.new".into()),
                    layer: GraphLayer::Fact,
                    attributes: BTreeMap::new(),
                }],
                set_attributes: vec![AttributeAssignment {
                    vertex: GraphBinding::Literal("fact.old".into()),
                    key: "revealed".into(),
                    value: AttributeValue::Bool(true),
                }],
                ..GraphMutation::default()
            },
        };
        let asset = FlowGraphAsset {
            asset_id: "graph-transform-test".into(),
            start_node_guid: 1,
            nodes: vec![node(1, NodeKind::GraphTransform { rule })],
            edges: Vec::new(),
        };
        let mut instance = FlowGraphInstance::new(21, asset, 1).unwrap();
        let dispatcher = UnifiedDispatcher::default();
        let mut context = RuntimeContext::new(&mut state, &dispatcher);

        instance
            .force_complete(&mut context, 1, DEFAULT_OUTPUT_PIN, None)
            .unwrap();
        assert!(context.state.vertex("fact.obsolete").is_none());
        assert!(context.state.edge("fact.old_obsolete").is_none());
        assert!(context.state.vertex("fact.new").is_some());
        assert_eq!(
            context.state.vertex("fact.old").unwrap().attributes["revealed"],
            AttributeValue::Bool(true)
        );
        instance.reconcile_history(&mut context, &[]).unwrap();
        assert_eq!(*context.state, original);
    }
    #[test]
    fn invoke_stops_backward_at_a_completed_node() {
        use std::collections::BTreeMap;

        let asset = FlowGraphAsset {
            asset_id: "invoke-boundary-test".into(),
            start_node_guid: 1,
            nodes: vec![
                node(
                    1,
                    NodeKind::LogicAny {
                        default_output_pin: String::new(),
                    },
                ),
                node(
                    2,
                    NodeKind::LogicAny {
                        default_output_pin: String::new(),
                    },
                ),
                node(
                    3,
                    NodeKind::Poi {
                        poi_id: "end".into(),
                    },
                ),
            ],
            edges: vec![
                EdgeConnection {
                    source_node_guid: 1,
                    source_pin_name: DEFAULT_OUTPUT_PIN.into(),
                    target_node_guid: 2,
                    target_pin_name: DEFAULT_INPUT_PIN.into(),
                },
                EdgeConnection {
                    source_node_guid: 2,
                    source_pin_name: DEFAULT_OUTPUT_PIN.into(),
                    target_node_guid: 3,
                    target_pin_name: DEFAULT_INPUT_PIN.into(),
                },
            ],
        };
        let mut instance = FlowGraphInstance::new(22, asset, 1).unwrap();
        let dispatcher = UnifiedDispatcher::default();
        let mut state = WorldStateTable::with_capacity(1, 0);
        let mut context = RuntimeContext::new(&mut state, &dispatcher);

        instance
            .force_complete(&mut context, 2, DEFAULT_OUTPUT_PIN, None)
            .unwrap();
        instance
            .invoke_story_to_poi(&mut context, 3, &BTreeMap::new())
            .unwrap();

        assert!(!instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 1));
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 2));
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 3));
    }
    #[test]
    fn storylet_scoped_variables_round_trip_through_json() {
        let mut variables = VariableStore::default();
        variables.set(
            &VariableScope::Global,
            "rumor_count",
            &VariableValue::Number(2.0),
        );
        variables.set(
            &VariableScope::Storylet(7),
            "met_guide",
            &VariableValue::Flag(true),
        );

        let encoded = serde_json::to_vec(&variables).expect("serialize variable scopes");
        let decoded: VariableStore =
            serde_json::from_slice(&encoded).expect("restore variable scopes");

        assert_eq!(decoded, variables);
        assert_eq!(
            decoded.get(&VariableScope::Storylet(7), "met_guide"),
            Some(&VariableValue::Flag(true))
        );
    }
    #[test]
    fn active_scene_sections_advance_on_tick_and_reconcile_exactly() {
        let asset = FlowGraphAsset {
            asset_id: "scene-playback-test".into(),
            start_node_guid: 1,
            nodes: vec![
                node(
                    1,
                    NodeKind::LogicAny {
                        default_output_pin: String::new(),
                    },
                ),
                node(
                    2,
                    NodeKind::SceneSection {
                        section_duration_ms: 50,
                        events: vec![TimelineTrackEvent {
                            event_id: 1,
                            action_type: TimelineActionType::WorkspotPose,
                            target_entity_path: "actor:guide".into(),
                            start_time_ms: 0,
                            duration_ms: 50,
                            payload_asset_id: "pose:walk".into(),
                            cached_previous_pose_id: String::new(),
                            priority_offset: 0,
                        }],
                    },
                ),
            ],
            edges: vec![EdgeConnection {
                source_node_guid: 1,
                source_pin_name: DEFAULT_OUTPUT_PIN.into(),
                target_node_guid: 2,
                target_pin_name: DEFAULT_INPUT_PIN.into(),
            }],
        };
        let mut instance = FlowGraphInstance::new(23, asset, 1).unwrap();
        let dispatcher = UnifiedDispatcher::default();
        let mut state = WorldStateTable::with_capacity(1, 0);
        let mut context = RuntimeContext::new(&mut state, &dispatcher);

        instance.start().unwrap();
        instance.tick(&mut context, 0).unwrap();
        instance.tick(&mut context, 25).unwrap();
        instance.tick(&mut context, 25).unwrap();

        assert_eq!(
            instance.node(2).unwrap().state,
            NodeExecutionState::Completed
        );
        assert_eq!(instance.node(2).unwrap().section_clock_ms, 50);
        assert_eq!(instance.composer.actor_pose["actor:guide"], "pose:walk");
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 2));
        instance.reconcile_history(&mut context, &[]).unwrap();
        assert!(instance.composer.actor_pose.is_empty());
        assert_eq!(instance.node(2).unwrap().state, NodeExecutionState::Idle);
        assert_eq!(instance.node(2).unwrap().section_clock_ms, 0);
    }
    #[test]
    fn force_complete_applies_scene_events_and_reconciles_them() {
        let asset = FlowGraphAsset {
            asset_id: "scene-force-test".into(),
            start_node_guid: 1,
            nodes: vec![
                node(
                    1,
                    NodeKind::LogicAny {
                        default_output_pin: String::new(),
                    },
                ),
                node(
                    2,
                    NodeKind::SceneSection {
                        section_duration_ms: 50,
                        events: vec![TimelineTrackEvent {
                            event_id: 1,
                            action_type: TimelineActionType::WorkspotPose,
                            target_entity_path: "actor:guide".into(),
                            start_time_ms: 0,
                            duration_ms: 50,
                            payload_asset_id: "pose:walk".into(),
                            cached_previous_pose_id: String::new(),
                            priority_offset: 0,
                        }],
                    },
                ),
            ],
            edges: vec![EdgeConnection {
                source_node_guid: 1,
                source_pin_name: DEFAULT_OUTPUT_PIN.into(),
                target_node_guid: 2,
                target_pin_name: DEFAULT_INPUT_PIN.into(),
            }],
        };
        let mut instance = FlowGraphInstance::new(24, asset, 1).unwrap();
        let dispatcher = UnifiedDispatcher::default();
        let mut state = WorldStateTable::with_capacity(1, 0);
        let mut context = RuntimeContext::new(&mut state, &dispatcher);

        instance
            .force_complete(&mut context, 2, DEFAULT_OUTPUT_PIN, None)
            .unwrap();
        assert_eq!(instance.composer.actor_pose["actor:guide"], "pose:walk");
        assert_eq!(instance.node(2).unwrap().section_clock_ms, 50);
        instance.reconcile_history(&mut context, &[]).unwrap();
        assert!(instance.composer.actor_pose.is_empty());
        assert_eq!(instance.node(2).unwrap().section_clock_ms, 0);
    }
    #[test]
    fn mission_executes_nested_asset_against_shared_world_state() {
        let parent = FlowGraphAsset {
            asset_id: "mission-parent".into(),
            start_node_guid: 1,
            nodes: vec![
                node(1, NodeKind::LogicAll),
                mission_node(2, "mission-child"),
                node(
                    3,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
            ],
            edges: vec![edge(1, 2), edge(2, 3)],
        };
        let child = FlowGraphAsset {
            asset_id: "mission-child".into(),
            start_node_guid: 10,
            nodes: vec![
                node(10, NodeKind::LogicAll),
                node(
                    11,
                    NodeKind::SetMetric {
                        metric_id: 0,
                        delta: 1.0,
                    },
                ),
                node(
                    12,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
            ],
            edges: vec![edge(10, 11), edge(11, 12)],
        };
        let dispatcher = UnifiedDispatcher::default();
        let mut state = WorldStateTable::with_capacity(0, 1);
        let mut instance = FlowGraphInstance::new(10, parent, 1).unwrap();
        instance.attach_mission_asset(2, &child, 2).unwrap();
        {
            let mut context = RuntimeContext::new(&mut state, &dispatcher);
            instance.start().unwrap();
            instance.tick(&mut context, 0).unwrap();
        }
        assert_eq!(state.metrics[0], 1.0);
        // The child instance's nodes never enter the root global history.
        assert_eq!(
            instance
                .global_call_history
                .iter()
                .map(|entry| entry.node_guid)
                .collect::<Vec<u32>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            instance
                .mission_call_history(2)
                .iter()
                .map(|entry| entry.node_guid)
                .collect::<Vec<u32>>(),
            vec![10, 11, 12]
        );
        assert_eq!(
            instance.node(2).unwrap().state,
            NodeExecutionState::Completed
        );
        assert_eq!(
            instance.node(3).unwrap().state,
            NodeExecutionState::Completed
        );
    }

    #[test]
    fn mission_rollback_unwinds_only_child_history_and_keeps_parent_history() {
        let parent = FlowGraphAsset {
            asset_id: "mission-rollback-parent".into(),
            start_node_guid: 1,
            nodes: vec![
                node(
                    1,
                    NodeKind::SetMetric {
                        metric_id: 0,
                        delta: 1.0,
                    },
                ),
                mission_node(2, "mission-rollback-child"),
                node(
                    3,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
            ],
            edges: vec![edge(1, 2), edge(2, 3)],
        };
        let child = FlowGraphAsset {
            asset_id: "mission-rollback-child".into(),
            start_node_guid: 10,
            nodes: vec![
                node(10, NodeKind::LogicAll),
                node(
                    11,
                    NodeKind::SetMetric {
                        metric_id: 1,
                        delta: 2.0,
                    },
                ),
                node(
                    12,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
            ],
            edges: vec![edge(10, 11), edge(11, 12)],
        };
        let dispatcher = UnifiedDispatcher::default();
        let mut state = WorldStateTable::with_capacity(0, 2);
        let mut instance = FlowGraphInstance::new(11, parent, 1).unwrap();
        instance.attach_mission_asset(2, &child, 2).unwrap();
        let mut context = RuntimeContext::new(&mut state, &dispatcher);
        instance.start().unwrap();
        instance.tick(&mut context, 0).unwrap();
        assert_eq!(context.state.metrics, vec![1.0, 2.0]);
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 1));
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 2));

        instance.rollback_mission_to_start(&mut context, 2).unwrap();
        // Only the child's records are undone; the parent's effect survives.
        assert_eq!(context.state.metrics, vec![1.0, 0.0]);
        assert!(instance.mission_call_history(2).is_empty());
        assert!(instance
            .mission_instance(2)
            .unwrap()
            .call_history()
            .is_empty());
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 1));
        assert!(instance
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 2));
        // The restarted child stays open so it continues on the next frame.
        assert_eq!(
            instance.node(2).unwrap().state,
            NodeExecutionState::ActiveLatent
        );

        instance.tick(&mut context, 0).unwrap();
        assert_eq!(state.metrics, vec![1.0, 2.0]);
        assert_eq!(instance.mission_call_history(2).len(), 3);
    }

    #[test]
    fn mission_scoped_replay_rebuilds_world_state_without_serializing_it() {
        let parent = FlowGraphAsset {
            asset_id: "mission-replay-parent".into(),
            start_node_guid: 1,
            nodes: vec![
                node(
                    1,
                    NodeKind::SetMetric {
                        metric_id: 0,
                        delta: 1.0,
                    },
                ),
                mission_node(2, "mission-replay-child"),
                node(
                    3,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
            ],
            edges: vec![edge(1, 2), edge(2, 3)],
        };
        let child = FlowGraphAsset {
            asset_id: "mission-replay-child".into(),
            start_node_guid: 10,
            nodes: vec![
                node(10, NodeKind::LogicAll),
                node(
                    11,
                    NodeKind::SetMetric {
                        metric_id: 1,
                        delta: 2.0,
                    },
                ),
                node(
                    12,
                    NodeKind::End {
                        jump_to_storylet: None,
                    },
                ),
            ],
            edges: vec![edge(10, 11), edge(11, 12)],
        };
        let dispatcher = UnifiedDispatcher::default();

        let mut live = FlowGraphInstance::new(21, parent.clone(), 1).unwrap();
        live.attach_mission_asset(2, &child, 2).unwrap();
        let mut live_state = WorldStateTable::with_capacity(0, 2);
        {
            let mut context = RuntimeContext::new(&mut live_state, &dispatcher);
            live.start().unwrap();
            live.tick(&mut context, 0).unwrap();
        }
        assert_eq!(live_state.metrics, vec![1.0, 2.0]);
        let global = live.call_history().to_vec();
        let scoped = live.mission_call_history(2).to_vec();

        // Load path: the two arrays alone rebuild the world state.
        let mut loaded = FlowGraphInstance::new(21, parent.clone(), 1).unwrap();
        loaded.attach_mission_asset(2, &child, 2).unwrap();
        let mut loaded_state = WorldStateTable::with_capacity(0, 2);
        let mut mission_histories: BTreeMap<u32, Vec<CallHistoryEntry>> = BTreeMap::new();
        mission_histories.insert(2, scoped);
        {
            let mut context = RuntimeContext::new(&mut loaded_state, &dispatcher);
            loaded
                .replay_history_with_missions(&mut context, &global, &mission_histories)
                .unwrap();
        }
        assert_eq!(loaded_state.metrics, live_state.metrics);
        assert_eq!(loaded.call_history(), live.call_history());
        assert_eq!(loaded.mission_call_history(2), live.mission_call_history(2));

        // ForceComplete completes the mission boundary only; it does not run the
        // child graph, which is driven by frame ticks or an explicit replay.
        let mut boundary = FlowGraphInstance::new(22, parent, 1).unwrap();
        boundary.attach_mission_asset(2, &child, 2).unwrap();
        let mut boundary_state = WorldStateTable::with_capacity(0, 2);
        {
            let mut context = RuntimeContext::new(&mut boundary_state, &dispatcher);
            boundary
                .force_complete(&mut context, 2, DEFAULT_OUTPUT_PIN, None)
                .unwrap();
        }
        assert_eq!(boundary_state.metrics, vec![0.0, 0.0]);
        assert!(boundary.mission_call_history(2).is_empty());
        assert_eq!(
            boundary.node(2).unwrap().state,
            NodeExecutionState::Completed
        );
        assert!(boundary
            .global_call_history
            .iter()
            .any(|entry| entry.node_guid == 2));
    }
    #[test]
    fn waypoint_transitions_without_clues_are_recorded_reversible_and_adjacent_only() {
        let graph = WaypointTopicGraph {
            nodes: vec![
                crate::chrono_dispatch::WaypointTopicNode {
                    topic_id: "opening".into(),
                    ..Default::default()
                },
                crate::chrono_dispatch::WaypointTopicNode {
                    topic_id: "middle".into(),
                    ..Default::default()
                },
                crate::chrono_dispatch::WaypointTopicNode {
                    topic_id: "player_trigger".into(),
                    is_player_only_trigger: true,
                    ..Default::default()
                },
                crate::chrono_dispatch::WaypointTopicNode {
                    topic_id: "remote".into(),
                    ..Default::default()
                },
                crate::chrono_dispatch::WaypointTopicNode {
                    topic_id: "goal".into(),
                    triggers_scene_exit: true,
                    ..Default::default()
                },
            ],
            edges: vec![
                crate::chrono_dispatch::WaypointTopicEdge {
                    edge_id: "npc-step".into(),
                    from_topic_id: "opening".into(),
                    to_topic_id: "middle".into(),
                    ..Default::default()
                },
                crate::chrono_dispatch::WaypointTopicEdge {
                    edge_id: "player-step".into(),
                    from_topic_id: "middle".into(),
                    to_topic_id: "player_trigger".into(),
                    ..Default::default()
                },
                crate::chrono_dispatch::WaypointTopicEdge {
                    edge_id: "npc-goal".into(),
                    from_topic_id: "middle".into(),
                    to_topic_id: "goal".into(),
                    ..Default::default()
                },
                crate::chrono_dispatch::WaypointTopicEdge {
                    edge_id: "remote-step".into(),
                    from_topic_id: "remote".into(),
                    to_topic_id: "goal".into(),
                    ..Default::default()
                },
            ],
        };
        let asset = FlowGraphAsset {
            asset_id: "waypoint-adjacency".into(),
            start_node_guid: 1,
            nodes: vec![FlowNodeDef {
                category: NodeCategory::DynamicDispatcher,
                ..node(
                    1,
                    NodeKind::WaypointConversation {
                        graph,
                        initial_topic_id: "opening".into(),
                        npc_goal_topic_id: "goal".into(),
                    },
                )
            }],
            edges: Vec::new(),
        };
        let dispatcher = UnifiedDispatcher::default();
        let mut state = WorldStateTable::default();
        let mut instance = FlowGraphInstance::new(88, asset, 4).unwrap();
        let mut context = RuntimeContext::new(&mut state, &dispatcher);

        instance.start().unwrap();
        instance.tick(&mut context, 1).unwrap();
        assert_eq!(instance.node(1).unwrap().current_topic_id, "middle");
        assert_eq!(instance.call_history().len(), 1);
        assert_eq!(instance.node(1).unwrap().edge_use_counts["npc-step"], 1);

        assert!(matches!(
            instance.divert_topic(&mut context, 1, "remote-step"),
            Err(GraphError::TopicEdgeNotAdjacent { .. })
        ));
        instance
            .divert_topic(&mut context, 1, "player-step")
            .unwrap();
        assert_eq!(instance.node(1).unwrap().current_topic_id, "player_trigger");
        assert_eq!(instance.call_history().len(), 2);

        instance.reconcile_history(&mut context, &[]).unwrap();
        assert!(instance.call_history().is_empty());
        assert_eq!(instance.node(1).unwrap().current_topic_id, "opening");
        assert!(instance
            .node(1)
            .unwrap()
            .edge_use_counts
            .values()
            .all(|count| *count == 0));
        assert!(context.state.global_tags.is_empty());
    }
}
