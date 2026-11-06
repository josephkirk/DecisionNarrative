use crate::chrono_catalog::StoryletCatalog;
use crate::chrono_dispatch::UnifiedDispatcher;
use crate::chrono_participants::PartIx;
use crate::chrono_runtime::{
    FlowGraphAsset, FlowGraphInstance, NodeExecutionState, NodeKind, RuntimeContext,
};
use crate::chrono_state::{BijectiveDelta, GameplayTagRegistry, PassionRegistry, WorldStateTable};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_AUTO_ADVANCE_MS: i64 = 600_000;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunContent {
    pub registry: GameplayTagRegistry,
    pub dispatcher: UnifiedDispatcher,
    pub passion_registry: PassionRegistry,
    pub storylet_titles: BTreeMap<u16, String>,
    #[serde(default)]
    pub storylet_location_tags: BTreeMap<u16, crate::chrono_state::TagBitset>,
    pub entity_names: BTreeMap<u16, String>,
    pub start_storylet: u16,
    pub initial_world: WorldStateTable,
    pub content_revision: u64,
    #[serde(default)]
    pub root_asset: FlowGraphAsset,
    #[serde(default)]
    pub mission_assets: BTreeMap<String, FlowGraphAsset>,
    #[serde(default)]
    pub catalog: StoryletCatalog,
    #[serde(skip)]
    pub passion_indices: BTreeMap<(String, String), u16>,
    #[serde(default)]
    pub entity_game_ids: BTreeMap<String, String>,
    #[serde(default)]
    pub participant_indices: BTreeMap<String, u16>,
    #[serde(default)]
    pub implicit_participant: bool,
    /// Canon events in story-time order, for `Reveal` nodes and Inspect.
    #[serde(default)]
    pub chronicle: crate::chrono_chronicle::ChronicleTable,
    /// Scheduled connections between places. Empty = use the `connected` edges
    /// of the world graph (always open).
    #[serde(default)]
    pub connections: Vec<crate::chrono_space::Connection>,
    /// The world clock; `None` = timeless.
    #[serde(default)]
    pub clock: Option<crate::chrono_space::Clock>,
}
impl RunContent {
    pub fn storylet_index(&self, id: &str) -> Option<u16> {
        self.catalog.resolve(id)
    }
    pub fn storylet_key(&self, index: u16) -> Option<&str> {
        self.catalog.key(index)
    }
    pub fn storylet_catalog(&self) -> &StoryletCatalog {
        &self.catalog
    }
    pub fn storylet_count(&self) -> usize {
        self.catalog.len()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RunStart {
    Beginning,
    Storylet { storylet: u16 },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RunAction {
    Choose { node_guid: u32, output_pin: u16 },
    PlayStorylet { node_guid: u32, storylet: u16 },
    Topic { node_guid: u32, edge_id: u32 },
    Move { to: u16 },
    Wait,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStep {
    pub index: u32,
    pub branch: u32,
    pub participant: PartIx,
    pub action: RunAction,
    pub label: String,
    /// What the world did because of this step (for example someone driven back).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
    /// Storylets rule participants played because of this step.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_plays: Vec<RulePlay>,
}
/// A storylet a rule participant played on its own; its causal parent is the
/// step that carries it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulePlay {
    pub participant: PartIx,
    pub storylet: StoryletRef,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoryletRef {
    pub id: u16,
    pub title: String,
    /// The authored storylet id, for links back to the editor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantView {
    pub id: PartIx,
    pub name: String,
    /// `false` for rule participants, which the UI never offers as a seat.
    #[serde(default = "default_true")]
    pub human: bool,
}
fn default_true() -> bool {
    true
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceView {
    pub id: u16,
    pub name: String,
    /// Current slot label; absent in a timeless project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_label: Option<String>,
    /// Actions this participant has left in the current slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions_left: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub moves: Vec<crate::chrono_space::MoveOption>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineView {
    pub text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoiceView {
    pub action: RunAction,
    pub label: String,
    pub enabled: bool,
    pub requirement: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneView {
    pub storylet: StoryletRef,
    pub lines: Vec<LineView>,
    pub choices: Vec<ChoiceView>,
    pub node_guid: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchView {
    pub id: u32,
    pub steps: Vec<RunStep>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunView {
    pub participant: PartIx,
    pub participants: Vec<ParticipantView>,
    pub place: Option<PlaceView>,
    pub scene: Option<SceneView>,
    pub also_here: Vec<StoryletRef>,
    pub nothing_here: bool,
    pub ending: Option<StoryletRef>,
    pub steps: Vec<RunStep>,
    pub current_step: u32,
    pub branches: Vec<BranchView>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NearMiss {
    pub storylet: StoryletRef,
    pub reason: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum RunError {
    InvalidAction(String),
    Stalled {
        storylet: Option<u16>,
        node_guid: u32,
    },
    Engine(String),
}

#[derive(Clone, Debug)]
struct PendingSelection {
    hub: u32,
    storylet: u16,
    participant: PartIx,
    hub_instance_id: u64,

    invocation: u64,
}
#[derive(Clone, Debug)]
struct Snapshot {
    world: WorldStateTable,
    instance: FlowGraphInstance,
    commit: usize,
    storylet: u16,
    selections: Vec<PendingSelection>,
    stalled: Option<RunError>,
    rule_played: Vec<(PartIx, u16)>,
}
#[derive(Clone, Debug)]
pub struct RunKernel {
    pub world: WorldStateTable,
    pub instance: FlowGraphInstance,
    registry: GameplayTagRegistry,
    passion_registry: PassionRegistry,
    dispatcher: UnifiedDispatcher,
    branches: Vec<BranchView>,
    snapshots: Vec<Vec<Snapshot>>,
    branch: usize,
    current_step: u32,
    rewound: bool,
    storylet: u16,
    selections: Vec<PendingSelection>,
    next_invocation: u64,
    run_seed: u64,
    stalled: Option<RunError>,
    rule_played: Vec<(PartIx, u16)>,
}
impl RunKernel {
    fn reference(content: &RunContent, id: u16) -> StoryletRef {
        StoryletRef {
            id,
            title: content
                .storylet_titles
                .get(&id)
                .cloned()
                .unwrap_or_else(|| id.to_string()),
            key: content.storylet_key(id).map(str::to_string),
        }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            world: self.world.clone(),
            instance: self.instance.clone(),
            commit: self.instance.history.head,
            storylet: self.storylet,
            selections: self.selections.clone(),
            stalled: self.stalled.clone(),
            rule_played: self.rule_played.clone(),
        }
    }
    fn pending_priority(instance: &FlowGraphInstance, hub: bool) -> Option<u32> {
        for node in &instance.nodes {
            if let Some(child) = node.sub_graph.as_deref() {
                if let Some(guid) = Self::pending_priority(child, hub) {
                    return Some(guid);
                }
            }
        }
        instance.nodes.iter().find_map(|node| {
            if node.state != NodeExecutionState::ActiveLatent {
                return None;
            }
            let def = instance.asset.node(node.node_guid)?;
            let matches = if hub {
                matches!(def.kind, NodeKind::QbnHub { .. })
            } else {
                matches!(
                    def.kind,
                    NodeKind::Choice { .. } | NodeKind::WaypointConversation { .. }
                )
            };
            matches.then_some(node.node_guid)
        })
    }
    fn pending_in(instance: &FlowGraphInstance) -> Option<u32> {
        Self::pending_priority(instance, false).or_else(|| Self::pending_priority(instance, true))
    }
    fn instance_by_id_mut(
        instance: &mut FlowGraphInstance,
        id: u64,
    ) -> Option<&mut FlowGraphInstance> {
        if instance.instance_id == id {
            return Some(instance);
        }
        instance
            .nodes
            .iter_mut()
            .filter_map(|n| n.sub_graph.as_deref_mut())
            .find_map(|child| Self::instance_by_id_mut(child, id))
    }
    fn scene_busy(&self, selection: &PendingSelection) -> bool {
        fn owns_work(instance: &FlowGraphInstance, invocation: u64) -> bool {
            instance
                .tokens
                .iter()
                .any(|token| instance.token_invocations.get(&token.token_id) == Some(&invocation))
                || instance.nodes.iter().any(|node| {
                    (node.state == NodeExecutionState::ActiveLatent
                        && instance.node_invocations.get(&node.node_guid) == Some(&invocation))
                        || node
                            .sub_graph
                            .as_deref()
                            .is_some_and(|child| owns_work(child, invocation))
                })
        }
        owns_work(&self.instance, selection.invocation)
    }
    fn pending(&self) -> Option<u32> {
        Self::pending_in(&self.instance)
    }
    fn owner(instance: &FlowGraphInstance, guid: u32) -> Option<&FlowGraphInstance> {
        for child in instance.nodes.iter().filter_map(|n| n.sub_graph.as_deref()) {
            if let Some(owner) = Self::owner(child, guid) {
                return Some(owner);
            }
        }
        if instance
            .node(guid)
            .is_some_and(|node| node.state == NodeExecutionState::ActiveLatent)
        {
            Some(instance)
        } else {
            None
        }
    }
    fn owner_mut(instance: &mut FlowGraphInstance, guid: u32) -> Option<&mut FlowGraphInstance> {
        let child_index = instance.nodes.iter().position(|n| {
            n.sub_graph
                .as_deref()
                .is_some_and(|child| Self::owner(child, guid).is_some())
        });
        if let Some(i) = child_index {
            return Self::owner_mut(instance.nodes[i].sub_graph.as_deref_mut().unwrap(), guid);
        }
        if instance
            .node(guid)
            .is_some_and(|node| node.state == NodeExecutionState::ActiveLatent)
        {
            Some(instance)
        } else {
            None
        }
    }
    fn has_queued_tokens(instance: &FlowGraphInstance) -> bool {
        !instance.tokens.is_empty()
            || instance
                .nodes
                .iter()
                .filter_map(|node| node.sub_graph.as_deref())
                .any(Self::has_queued_tokens)
    }
    fn auto_advance(&mut self) -> Result<(), RunError> {
        let start = self.instance.clock_ms;
        loop {
            if !Self::has_queued_tokens(&self.instance) {
                if let Some(selection) = self.selections.last().cloned() {
                    if self.pending() == Some(selection.hub) && !self.scene_busy(&selection) {
                        let mut ctx = RuntimeContext::with_registries(
                            &mut self.world,
                            &self.dispatcher,
                            &self.registry,
                            &self.passion_registry,
                        );
                        let owner =
                            Self::instance_by_id_mut(&mut self.instance, selection.hub_instance_id)
                                .ok_or_else(|| {
                                    RunError::Engine("selection hub disappeared".into())
                                })?;
                        owner.acting_participant = selection.participant;
                        owner
                            .complete_reserved_storylet(&mut ctx, selection.hub, selection.storylet)
                            .map_err(|e| RunError::Engine(e.to_string()))?;
                        self.selections.pop();
                        continue;
                    }
                }
            }
            let waiting_on_scene = self.selections.last().is_some_and(|selection| {
                self.scene_busy(selection) && self.pending() == Some(selection.hub)
            });
            if self.pending().is_some()
                && !waiting_on_scene
                && !Self::has_queued_tokens(&self.instance)
            {
                return Ok(());
            }
            let active = self
                .instance
                .nodes
                .iter()
                .any(|n| n.state == NodeExecutionState::ActiveLatent);
            if self.instance.tokens.is_empty() && !active {
                return Ok(());
            }
            if self.instance.clock_ms.saturating_sub(start) >= MAX_AUTO_ADVANCE_MS {
                return Err(RunError::Stalled {
                    storylet: Some(self.storylet),
                    node_guid: self
                        .instance
                        .nodes
                        .iter()
                        .find(|n| n.state == NodeExecutionState::ActiveLatent)
                        .map_or(0, |n| n.node_guid),
                });
            }
            let mut ctx = RuntimeContext::with_registries(
                &mut self.world,
                &self.dispatcher,
                &self.registry,
                &self.passion_registry,
            );
            self.instance
                .tick(&mut ctx, 1000)
                .map_err(|e| RunError::Engine(e.to_string()))?;
        }
    }
    pub fn start(content: &RunContent, start: RunStart, seed: u64) -> Result<Self, RunError> {
        let (run, stalled) = Self::start_preserving_stall(content, start, seed)?;
        if let Some(error) = stalled {
            Err(error)
        } else {
            Ok(run)
        }
    }
    pub fn start_preserving_stall(
        content: &RunContent,
        start: RunStart,
        seed: u64,
    ) -> Result<(Self, Option<RunError>), RunError> {
        let mut instance = FlowGraphInstance::new(1, content.root_asset.clone(), seed)
            .map_err(|e| RunError::Engine(e.to_string()))?;
        instance
            .attach_mission_assets(&content.mission_assets, seed)
            .map_err(|e| RunError::Engine(e.to_string()))?;
        let mut world = content.initial_world.clone();
        world.ensure_participants();
        if let (Some(clock), None) = (&content.clock, &world.clock) {
            world.clock = Some(crate::chrono_space::ClockState::new(clock, world.participants.len()));
        }
        let storylet = match start {
            RunStart::Beginning => content.start_storylet,
            RunStart::Storylet { storylet } => storylet,
        };
        if matches!(start, RunStart::Beginning) {
            instance
                .start()
                .map_err(|e| RunError::Engine(e.to_string()))?;
        } else {
            let start_guid = content
                .dispatcher
                .storylet(storylet)
                .ok_or_else(|| RunError::InvalidAction("unknown storylet".into()))?
                .payload_node_guid;
            let key = content.storylet_key(storylet);
            let guid = instance
                .asset
                .nodes
                .iter()
                .find(|node| {
                    matches!(node.kind, NodeKind::Poi { .. })
                        && (node.node_guid == start_guid
                            || key.is_some_and(|key| {
                                node.payload.get("storyletId").and_then(|v| v.as_str()) == Some(key)
                            }))
                })
                .map(|node| node.node_guid)
                .ok_or_else(|| {
                    RunError::InvalidAction("storylet has no point of interest".into())
                })?;
            let mut ctx = RuntimeContext::with_registries(
                &mut world,
                &content.dispatcher,
                &content.registry,
                &content.passion_registry,
            );
            instance
                .invoke_story_to_poi(&mut ctx, guid, &BTreeMap::new())
                .map_err(|e| RunError::Engine(e.to_string()))?;
            instance
                .resume_from_poi(guid)
                .map_err(|e| RunError::Engine(e.to_string()))?;
        }
        let mut run = Self {
            world,
            instance,
            registry: content.registry.clone(),
            passion_registry: content.passion_registry.clone(),
            dispatcher: content.dispatcher.clone(),
            branches: vec![BranchView {
                id: 0,
                steps: vec![],
            }],
            snapshots: vec![vec![]],
            branch: 0,
            current_step: 0,
            rewound: false,
            storylet,
            selections: Vec::new(),
            next_invocation: 1,
            run_seed: seed,
            stalled: None,
            rule_played: Vec::new(),
        };
        let stalled = match run.auto_advance() {
            Ok(()) => None,
            Err(e @ RunError::Stalled { .. }) => Some(e),
            Err(e) => return Err(e),
        };
        run.stalled = stalled.clone();
        let initial = run.snapshot();
        run.snapshots[0].push(initial);
        Ok((run, stalled))
    }
    pub fn view(&self, content: &RunContent, participant: PartIx) -> RunView {
        let participants = self
            .world
            .participants
            .keys
            .iter()
            .enumerate()
            .map(|(i, n)| ParticipantView {
                id: i as u16,
                name: n.clone(),
                human: matches!(
                    self.world.participants.controller.get(i),
                    Some(crate::chrono_participants::Controller::Human { .. })
                ),
            })
            .collect();
        let place = self.world.active_location(participant).map(|id| PlaceView {
            id,
            name: content
                .entity_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| id.to_string()),
            clock_label: content.clock.as_ref().zip(self.world.clock.as_ref()).and_then(
                |(clock, state)| clock.slot_labels.get(state.slot as usize).cloned(),
            ),
            actions_left: self
                .world
                .clock
                .as_ref()
                .and_then(|state| state.actions_left.get(participant as usize).copied()),
            moves: crate::chrono_space::moves_for(&self.world, content, participant),
        });
        let lines: Vec<LineView> = self
            .instance
            .call_history()
            .iter()
            .filter_map(|entry| self.instance.asset.node(entry.node_guid))
            .filter_map(|node| {
                let data = node.payload.get("nodeData")?;
                if data.get("type").and_then(|v| v.as_str()) != Some("dialog") {
                    return None;
                }
                data.get("text")
                    .and_then(|v| v.as_str())
                    .filter(|text| !text.is_empty())
                    .map(|text| LineView {
                        text: text.to_string(),
                    })
            })
            .collect();
        let mut also_here = vec![];
        let mut scene = None;
        if let Some(guid) = self.pending() {
            if let Some(owner) = Self::owner(&self.instance, guid) {
                if let Some(def) = owner.asset.node(guid) {
                    match &def.kind {
                        NodeKind::Choice { .. } => {
                            let choices = def
                                .output_pins
                                .iter()
                                .enumerate()
                                .map(|(i, p)| ChoiceView {
                                    action: RunAction::Choose {
                                        node_guid: guid,
                                        output_pin: i as u16,
                                    },
                                    label: def
                                        .payload
                                        .get("nodeData")
                                        .and_then(|x| x.get("choices"))
                                        .and_then(|x| x.get(i))
                                        .and_then(|x| x.get("text"))
                                        .and_then(|x| x.as_str())
                                        .unwrap_or(&p.pin_name)
                                        .to_string(),
                                    enabled: true,
                                    requirement: None,
                                })
                                .collect();
                            scene = Some(SceneView {
                                storylet: Self::reference(content, self.storylet),
                                lines: lines.clone(),
                                choices,
                                node_guid: guid,
                            });
                        }
                        NodeKind::WaypointConversation { graph, .. } => {
                            let current = owner
                                .node(guid)
                                .map(|n| n.current_topic_id.as_str())
                                .unwrap_or("");
                            let choices = graph
                                .edges
                                .iter()
                                .enumerate()
                                .filter(|(_, edge)| edge.from_topic_id == current)
                                .map(|(i, edge)| ChoiceView {
                                    action: RunAction::Topic {
                                        node_guid: guid,
                                        edge_id: i as u32,
                                    },
                                    label: if edge.to_topic_id.is_empty() {
                                        edge.edge_id.clone()
                                    } else {
                                        edge.to_topic_id.clone()
                                    },
                                    enabled: true,
                                    requirement: None,
                                })
                                .collect();
                            scene = Some(SceneView {
                                storylet: Self::reference(content, self.storylet),
                                lines: lines.clone(),
                                choices,
                                node_guid: guid,
                            });
                        }
                        NodeKind::QbnHub { .. } => {
                            let mut copy = self.world.clone();
                            let ctx = RuntimeContext::with_registries(
                                &mut copy,
                                &self.dispatcher,
                                &self.registry,
                                &self.passion_registry,
                            );
                            let mut participant_instance = owner.clone();
                            participant_instance.acting_participant = participant;
                            also_here = participant_instance
                                .present_deck(&ctx, guid)
                                .unwrap_or_default()
                                .into_iter()
                                .map(|id| Self::reference(content, id))
                                .collect();
                        }
                        _ => {}
                    }
                }
            }
        }
        let ending = if self.pending().is_none()
            && self.instance.tokens.is_empty()
            && self
                .dispatcher
                .storylet(self.storylet)
                .is_some_and(|d| d.is_ending)
        {
            Some(Self::reference(content, self.storylet))
        } else {
            None
        };
        let nothing_here = scene.is_none() && also_here.is_empty() && ending.is_none();
        RunView {
            participant,
            participants,
            place,
            scene,
            also_here,
            nothing_here,
            ending,
            steps: self.branches[self.branch].steps[..self.current_step as usize].to_vec(),
            current_step: self.current_step,
            branches: self.branches.clone(),
        }
    }
    pub fn act(
        &mut self,
        content: &RunContent,
        participant: PartIx,
        action: RunAction,
    ) -> Result<u32, RunError> {
        self.act_internal(content, participant, action, false)
            .map(|(step, _)| step)
    }
    pub fn act_preserving_stall(
        &mut self,
        content: &RunContent,
        participant: PartIx,
        action: RunAction,
    ) -> Result<(u32, Option<RunError>), RunError> {
        self.act_internal(content, participant, action, true)
    }
    fn act_internal(
        &mut self,
        content: &RunContent,
        participant: PartIx,
        action: RunAction,
        preserve_stall: bool,
    ) -> Result<(u32, Option<RunError>), RunError> {
        if participant as usize >= self.world.participants.len() {
            return Err(RunError::InvalidAction("unknown participant".into()));
        }
        if content.clock.is_some()
            && self
                .world
                .clock
                .as_ref()
                .and_then(|state| state.actions_left.get(participant as usize))
                .is_some_and(|left| *left == 0)
        {
            return Err(RunError::InvalidAction("no actions left in this slot".into()));
        }
        let backup = self.clone();
        self.instance.acting_participant = participant;
        let pending = self.pending();
        let result = (|| {
            let mut ctx = RuntimeContext::with_registries(
                &mut self.world,
                &self.dispatcher,
                &self.registry,
                &self.passion_registry,
            );
            match action {
                RunAction::Choose {
                    node_guid,
                    output_pin,
                } => {
                    if pending != Some(node_guid) {
                        return Err(RunError::InvalidAction("choice is not pending".into()));
                    }
                    let owner = Self::owner_mut(&mut self.instance, node_guid)
                        .ok_or_else(|| RunError::InvalidAction("unknown node".into()))?;
                    owner.acting_participant = participant;
                    let pin = owner
                        .asset
                        .node(node_guid)
                        .and_then(|d| d.output_pins.get(output_pin as usize))
                        .map(|p| p.pin_name.clone())
                        .ok_or_else(|| RunError::InvalidAction("unknown output".into()))?;
                    owner
                        .choose(&mut ctx, node_guid, &pin)
                        .map_err(|e| RunError::InvalidAction(e.to_string()))?;
                }
                RunAction::PlayStorylet {
                    node_guid,
                    storylet,
                } => {
                    if pending != Some(node_guid) {
                        return Err(RunError::InvalidAction(
                            "storylet selection is not pending".into(),
                        ));
                    }
                    let owner = Self::owner_mut(&mut self.instance, node_guid)
                        .ok_or_else(|| RunError::InvalidAction("unknown node".into()))?;
                    owner.acting_participant = participant;
                    let available = owner
                        .present_deck(&ctx, node_guid)
                        .map_err(|e| RunError::InvalidAction(e.to_string()))?;
                    if !available.contains(&storylet) {
                        return Err(RunError::InvalidAction("storylet is not eligible".into()));
                    }
                    let entry = ctx
                        .dispatcher
                        .storylet(storylet)
                        .ok_or_else(|| RunError::InvalidAction("unknown storylet".into()))?
                        .payload_node_guid;
                    let hub_instance_id = owner.instance_id;
                    let local = owner.asset.node(entry).is_some();
                    let scene_instance_id = if local {
                        hub_instance_id
                    } else {
                        self.instance.instance_id
                    };
                    let scene_owner =
                        Self::instance_by_id_mut(&mut self.instance, scene_instance_id)
                            .ok_or_else(|| {
                                RunError::Engine("storylet instance disappeared".into())
                            })?;
                    scene_owner.acting_participant = participant;
                    let invocation = self.next_invocation;
                    self.next_invocation = self
                        .next_invocation
                        .checked_add(1)
                        .ok_or_else(|| RunError::Engine("invocation id exhausted".into()))?;
                    scene_owner
                        .enter_storylet_invocation(entry, invocation)
                        .map_err(|e| RunError::Engine(e.to_string()))?;
                    self.selections.push(PendingSelection {
                        hub: node_guid,
                        storylet,
                        participant,
                        hub_instance_id,
                        invocation,
                    });
                    self.storylet = storylet;
                }
                RunAction::Topic { node_guid, edge_id } => {
                    if pending != Some(node_guid) {
                        return Err(RunError::InvalidAction(
                            "conversation is not pending".into(),
                        ));
                    }
                    let owner = Self::owner_mut(&mut self.instance, node_guid)
                        .ok_or_else(|| RunError::InvalidAction("unknown node".into()))?;
                    owner.acting_participant = participant;
                    let edge = owner
                        .asset
                        .node(node_guid)
                        .and_then(|d| match &d.kind {
                            NodeKind::WaypointConversation { graph, .. } => {
                                graph.edges.get(edge_id as usize).map(|e| e.edge_id.clone())
                            }
                            _ => None,
                        })
                        .ok_or_else(|| RunError::InvalidAction("unknown topic".into()))?;
                    owner
                        .divert_topic(&mut ctx, node_guid, &edge)
                        .map_err(|e| RunError::InvalidAction(e.to_string()))?;
                }
                RunAction::Move { to } if !content.connections.is_empty() => {
                    let option = crate::chrono_space::moves_for(ctx.state, content, participant)
                        .into_iter()
                        .find(|m| m.to == to)
                        .ok_or_else(|| {
                            RunError::InvalidAction("destination is not a connected location".into())
                        })?;
                    if !option.enabled {
                        return Err(RunError::InvalidAction(
                            "that connection is closed right now".into(),
                        ));
                    }
                    let mut delta = BijectiveDelta::default();
                    ctx.state.apply_move(participant, Some(to), &mut delta);
                }
                RunAction::Move { to } => {
                    let current = ctx.state.active_location(participant).ok_or_else(|| {
                        RunError::InvalidAction("participant has no location".into())
                    })?;
                    let source = ctx.state.vertices.iter().find(|v| {
                        v.layer == crate::chrono_state::GraphLayer::Location
                            && v.entity_index == Some(current)
                    });
                    let destination = ctx.state.vertices.iter().find(|v| {
                        v.layer == crate::chrono_state::GraphLayer::Location
                            && v.entity_index == Some(to)
                    });
                    let connected = source.zip(destination).is_some_and(|(a, b)| {
                        ctx.state.edges.iter().any(|edge| {
                            edge.relation_type == "connected"
                                && edge.source_id == a.vertex_id
                                && edge.target_id == b.vertex_id
                        })
                    });
                    if !connected {
                        return Err(RunError::InvalidAction(
                            "destination is not a connected location".into(),
                        ));
                    }
                    let mut delta = BijectiveDelta::default();
                    ctx.state.apply_move(participant, Some(to), &mut delta);
                }
                RunAction::Wait => {}
            }
            self.auto_advance()?;
            Ok(())
        })();
        let stalled = match result {
            Ok(()) => None,
            Err(e @ RunError::Stalled { .. }) if preserve_stall => Some(e),
            Err(e) => {
                *self = backup;
                return Err(e);
            }
        };
        self.stalled = stalled.clone();
        let (notes, slot_advanced) = self.spend_action(content, participant, &action);
        let is_human = self.world.participants.humans().any(|h| h == participant);
        let rule_plays = if is_human && (content.clock.is_none() || slot_advanced) {
            self.rule_pass(content)?
        } else {
            Vec::new()
        };
        if self.rewound {
            let steps = self.branches[self.branch].steps[..self.current_step as usize].to_vec();
            let snapshots = self.snapshots[self.branch][..=self.current_step as usize].to_vec();
            let id = self.branches.len() as u32;
            self.branches.push(BranchView { id, steps });
            self.snapshots.push(snapshots);
            self.branch = id as usize;
            self.rewound = false;
        }
        self.current_step += 1;
        let step = RunStep {
            index: self.current_step,
            branch: self.branch as u32,
            participant,
            action: action.clone(),
            label: format!("{action:?}"),
            notes,
            rule_plays,
        };
        self.branches[self.branch].steps.push(step);
        let snapshot = self.snapshot();
        self.snapshots[self.branch].push(snapshot);
        let _ = content;
        Ok((self.current_step, stalled))
    }
    /// Spends the actor's action (all of them for `Wait`) and, once every human
    /// is out of actions, advances the clock. Returns what the world did as a
    /// result, as plain-language notes for the step.
    fn spend_action(&mut self, content: &RunContent, participant: PartIx, action: &RunAction) -> (Vec<String>, bool) {
        let Some(state) = self.world.clock.as_mut() else {
            return (Vec::new(), false);
        };
        if let Some(left) = state.actions_left.get_mut(participant as usize) {
            *left = if matches!(action, RunAction::Wait) { 0 } else { left.saturating_sub(1) };
        }
        let all_done = self
            .world
            .participants
            .humans()
            .all(|h| state.actions_left.get(h as usize).copied().unwrap_or(0) == 0);
        if !all_done {
            return (Vec::new(), false);
        }
        let slot_before = self.world.clock.as_ref().map(|c| c.slot);
        let mut delta = BijectiveDelta::default();
        let name = |id: u16| content.entity_names.get(&id).cloned().unwrap_or_else(|| id.to_string());
        let events = crate::chrono_space::advance_slot(&mut self.world, content, &mut delta);
        let notes = events
            .into_iter()
            .map(|event| {
                let who = self.world.participants.keys.get(event.participant as usize).cloned().unwrap_or_default();
                match event.to {
                    Some(to) => format!("{who} is driven from {} back to {}", name(event.from), name(to)),
                    None => format!("{who} is stranded at {}", name(event.from)),
                }
            })
            .collect();
        let advanced = self.world.clock.as_ref().map(|c| c.slot) != slot_before;
        (notes, advanced)
    }

    /// Each rule participant, in participant order, plays at most one eligible
    /// storylet that lists it in `played_by`: highest display priority first,
    /// then lowest storylet index. Rule storylets run off-stage on a scratch
    /// instance, so they must not need an interaction.
    fn rule_pass(&mut self, content: &RunContent) -> Result<Vec<RulePlay>, RunError> {
        let mut plays = Vec::new();
        let participants = self.world.participants.len() as PartIx;
        for p in 0..participants {
            if !matches!(self.world.participants.controller[p as usize], crate::chrono_participants::Controller::Rule) {
                continue;
            }
            let Some(storylet) = self.next_rule_storylet(content, p) else { continue };
            self.play_rule_storylet(content, p, storylet)?;
            self.rule_played.push((p, storylet));
            plays.push(RulePlay { participant: p, storylet: Self::reference(content, storylet) });
            if plays.len() >= participants as usize {
                break;
            }
        }
        Ok(plays)
    }
    fn next_rule_storylet(&self, content: &RunContent, p: PartIx) -> Option<u16> {
        let location_tags = self
            .world
            .active_location(p)
            .and_then(|id| self.world.entity_tags.get(id as usize));
        let mut best: Option<(i32, u16)> = None;
        for def in &self.dispatcher.storylets {
            if !def.played_by.contains(&p) {
                continue;
            }
            if !def.is_repeatable && self.rule_played.contains(&(p, def.storylet_id)) {
                continue;
            }
            if let Some(mask) = content.storylet_location_tags.get(&def.storylet_id) {
                if !mask.is_empty() && !location_tags.is_some_and(|tags| tags.intersects(mask)) {
                    continue;
                }
            }
            if !self.dispatcher.is_eligible(def, &self.world) {
                continue;
            }
            if let Some(graph) = &def.precondition_graph {
                let mut world = self.world.clone();
                let ctx = RuntimeContext::with_registries(&mut world, &self.dispatcher, &self.registry, &self.passion_registry);
                if !FlowGraphInstance::evaluate_precondition_graph_for(graph, &ctx, p).unwrap_or(false) {
                    continue;
                }
            }
            let better = best.is_none_or(|(priority, id)| {
                def.display_priority > priority || (def.display_priority == priority && def.storylet_id < id)
            });
            if better {
                best = Some((def.display_priority, def.storylet_id));
            }
        }
        best.map(|(_, id)| id)
    }
    fn play_rule_storylet(&mut self, content: &RunContent, p: PartIx, storylet: u16) -> Result<(), RunError> {
        let entry = self
            .dispatcher
            .storylet(storylet)
            .ok_or_else(|| RunError::InvalidAction("unknown storylet".into()))?
            .payload_node_guid;
        let seed = self.run_seed ^ (u64::from(storylet) << 16) ^ (u64::from(p) << 8) ^ self.rule_played.len() as u64;
        let mut scratch = FlowGraphInstance::new(1, content.root_asset.clone(), seed)
            .map_err(|e| RunError::Engine(e.to_string()))?;
        scratch
            .attach_mission_assets(&content.mission_assets, seed)
            .map_err(|e| RunError::Engine(e.to_string()))?;
        scratch.acting_participant = p;
        scratch
            .enter_storylet_invocation(entry, 1)
            .map_err(|e| RunError::Engine(e.to_string()))?;
        let known_before: Vec<(PartIx, u16)> = self.world.knowledge.rows.iter().map(|r| (r.holder, r.fact)).collect();
        let told_before: Vec<(PartIx, u16)> = self.world.telling.rows.iter().map(|r| (r.holder, r.entry)).collect();
        let mut ticks = 0;
        loop {
            let active = scratch.nodes.iter().any(|n| n.state == NodeExecutionState::ActiveLatent);
            if scratch.tokens.is_empty() && !active {
                break;
            }
            if ticks >= 10_000 || (scratch.tokens.is_empty() && active && ticks > 0 && Self::pending_in(&scratch).is_some()) {
                return Err(RunError::InvalidAction(format!(
                    "rule storylet {storylet} needs an interaction; rule storylets must not contain choices"
                )));
            }
            let mut ctx = RuntimeContext::with_registries(&mut self.world, &self.dispatcher, &self.registry, &self.passion_registry);
            scratch.tick(&mut ctx, 1000).map_err(|e| RunError::Engine(e.to_string()))?;
            ticks += 1;
        }
        // The scratch instance has its own call history; point what it taught
        // at the last event of the real run so Inspect links it to this step.
        let source = self.instance.call_history().len().saturating_sub(1) as u32;
        for row in &mut self.world.knowledge.rows {
            if !known_before.contains(&(row.holder, row.fact)) {
                row.source_event = source;
            }
        }
        for row in &mut self.world.telling.rows {
            if !told_before.contains(&(row.holder, row.entry)) {
                row.source_event = source;
            }
        }
        Ok(())
    }
    pub fn stalled(&self) -> Option<RunError> {
        self.stalled.clone()
    }
    pub fn rewind(&mut self, to_step: u32) -> Result<(), RunError> {
        let snap = self.snapshots[self.branch]
            .get(to_step as usize)
            .cloned()
            .ok_or_else(|| RunError::InvalidAction("unknown step".into()))?;
        let mut ctx = RuntimeContext::with_registries(
            &mut self.world,
            &self.dispatcher,
            &self.registry,
            &self.passion_registry,
        );
        self.instance
            .checkout(&mut ctx, snap.commit)
            .map_err(|e| RunError::Engine(e.to_string()))?;
        self.world = snap.world;
        self.instance = snap.instance;
        self.storylet = snap.storylet;
        self.selections = snap.selections;
        self.rule_played = snap.rule_played;
        self.stalled = snap.stalled;
        self.current_step = to_step;
        self.rewound = true;
        Ok(())
    }
    pub fn why_not(
        &self,
        content: &RunContent,
        participant: PartIx,
        limit: usize,
    ) -> Vec<NearMiss> {
        content
            .dispatcher
            .storylets
            .iter()
            .filter(|d| d.played_by.is_empty() || d.played_by.contains(&participant))
            .filter(
                |d| match content.storylet_location_tags.get(&d.storylet_id) {
                    None => true,
                    Some(mask) if mask.is_empty() => true,
                    Some(mask) => self
                        .world
                        .active_location(participant)
                        .and_then(|id| self.world.entity_tags.get(id as usize))
                        .is_some_and(|tags| tags.intersects(mask)),
                },
            )
            .filter_map(|d| {
                let mut reason =
                    content
                        .dispatcher
                        .explain_ineligible(d, &self.world, &content.registry);
                if reason.is_none() {
                    if let Some(graph) = &d.precondition_graph {
                        let mut world = self.world.clone();
                        let ctx = RuntimeContext::with_registries(
                            &mut world,
                            &content.dispatcher,
                            &content.registry,
                            &content.passion_registry,
                        );
                        if !FlowGraphInstance::evaluate_precondition_graph_for(
                            graph,
                            &ctx,
                            participant,
                        )
                        .unwrap_or(false)
                        {
                            reason = graph.nodes.iter().find_map(|node| match node.kind {
                                NodeKind::Knows {
                                    holder,
                                    fact,
                                    confirmed_only,
                                } => {
                                    let holder = match holder {
                                        crate::chrono_runtime::HolderRef::Acting => participant,
                                        crate::chrono_runtime::HolderRef::Participant(p) => p,
                                    };
                                    let known =
                                        self.world.knowledge.knows(holder, fact).is_some_and(|r| {
                                            !confirmed_only || r.kind.is_confirmed()
                                        });
                                    if known {
                                        None
                                    } else {
                                        let name = content
                                            .registry
                                            .tag(fact)
                                            .unwrap_or("unknown")
                                            .strip_prefix("fact.")
                                            .unwrap_or("unknown")
                                            .replace(['_', '.'], " ");
                                        Some(format!("needs fact \"{name}\""))
                                    }
                                }
                                _ => None,
                            });
                            if reason.is_none() {
                                reason = Some("needs another condition".into());
                            }
                        }
                    }
                }
                reason.map(|reason| NearMiss {
                    storylet: Self::reference(content, d.storylet_id),
                    reason: reason.chars().take(120).collect(),
                })
            })
            .take(limit)
            .collect()
    }
    pub fn replay(
        content: &RunContent,
        start: RunStart,
        seed: u64,
        actions: &[(PartIx, RunAction)],
    ) -> (Result<Self, RunError>, Option<u32>) {
        let mut run = match Self::start(content, start, seed) {
            Ok(v) => v,
            Err(e) => return (Err(e), Some(0)),
        };
        for (i, (p, a)) in actions.iter().enumerate() {
            if let Err(e) = run.act(content, *p, a.clone()) {
                return (Err(e), Some((i + 1) as u32));
            }
        }
        (Ok(run), None)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InspectView {
    pub knows: Vec<KnowRowView>,
    pub world_changes: Vec<ChangeView>,
    pub chronicle: Vec<ChronicleRowView>,
    pub advanced: AdvancedView,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowRowView {
    pub fact: String,
    pub kind: crate::chrono_participants::KnowKind,
    pub confirmed: bool,
    pub source_step: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeView {
    pub key: String,
    pub before: String,
    pub after: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChronicleRowView {
    pub key: String,
    pub label: String,
    pub place: Option<String>,
    pub story_time: i64,
    pub told: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdvancedView {
    pub run_seed: u64,
    pub content_revision: u64,
    pub node_guid: Option<u32>,
    pub interaction: Option<String>,
}
impl RunKernel {
    /// The run step during which the call-history event `event` happened: the
    /// first step whose snapshot has recorded more than `event` calls. Step 0
    /// is the automatic advance at the start.
    fn step_of_event(&self, event: u32) -> Option<u32> {
        (0..=self.current_step).find(|&step| {
            self.snapshots[self.branch]
                .get(step as usize)
                .is_some_and(|snapshot| snapshot.instance.call_history().len() > event as usize)
        })
    }
    pub fn inspect(&self, content: &RunContent, participant: PartIx) -> InspectView {
        let knows = self
            .world
            .knowledge
            .rows_for(participant)
            .iter()
            .map(|r| KnowRowView {
                fact: content
                    .registry
                    .tag(r.fact)
                    .unwrap_or("unknown")
                    .to_string(),
                kind: r.kind,
                confirmed: r.kind.is_confirmed(),
                source_step: self.step_of_event(r.source_event),
            })
            .collect();
        let chronicle = content
            .chronicle
            .entries
            .iter()
            .enumerate()
            .map(|(ix, entry)| ChronicleRowView {
                key: entry.key.clone(),
                label: entry.label.clone(),
                place: entry
                    .location
                    .and_then(|id| content.entity_names.get(&id).cloned()),
                story_time: entry.story_time,
                told: self.world.telling.told_number(participant, ix as u16),
            })
            .collect();
        let mut world_changes = Vec::new();
        if self.current_step > 0 {
            if let Some(before) = self.snapshots[self.branch].get(self.current_step as usize - 1) {
                let a = serde_json::to_value(&before.world).unwrap_or_default();
                let b = serde_json::to_value(&self.world).unwrap_or_default();
                if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
                    for (key, after) in b {
                        if let Some(before) = a.get(key) {
                            if before != after {
                                world_changes.push(ChangeView {
                                    key: key.clone(),
                                    before: before.to_string(),
                                    after: after.to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
        let node_guid = self.pending();
        InspectView {
            knows,
            world_changes,
            chronicle,
            advanced: AdvancedView {
                run_seed: self.run_seed,
                content_revision: content.content_revision,
                node_guid,
                interaction: node_guid
                    .and_then(|n| Self::owner(&self.instance, n))
                    .and_then(|i| i.asset.node(node_guid?))
                    .map(|n| format!("{:?}", n.kind)),
            },
        }
    }
}
