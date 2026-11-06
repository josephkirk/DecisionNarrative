use std::collections::BTreeMap;
use storylet_engine::chrono_dispatch::{StoryletDef, UnifiedDispatcher};
use storylet_engine::chrono_participants::{KnowKind, KnowledgeRow};
use storylet_engine::chrono_runtime::{
    EdgeConnection, FlowGraphAsset, FlowNodeDef, NodeCategory, NodeKind, PinDefinition,
};
use storylet_engine::chrono_space::{
    advance_slot, moves_for, Clock, ClockState, Connection, MoveBlock, OnClose,
};
use storylet_engine::chrono_state::{BijectiveDelta, GameplayTagRegistry, PassionRegistry, WorldStateTable};
use storylet_engine::run::{RunAction, RunContent, RunKernel, RunStart};

const LIGHTHOUSE: u16 = 0;
const ROCK: u16 = 1;
const HARBOR: u16 = 2;

fn pin(name: &str, input: bool) -> PinDefinition {
    PinDefinition { pin_name: name.into(), is_input: input }
}

/// A choice node with two exits to one end node, set in a three-place world with a five-slot clock.
fn content() -> RunContent {
    let choice = FlowNodeDef {
        node_guid: 1,
        node_type_id: "choice".into(),
        category: NodeCategory::LogicGate,
        kind: NodeKind::Choice { default_output_pin: String::new() },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("left", false), pin("right", false)],
        ..Default::default()
    };
    let end = FlowNodeDef {
        node_guid: 2,
        node_type_id: "end".into(),
        category: NodeCategory::Mutator,
        kind: NodeKind::End { jump_to_storylet: None, precondition_result: true },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let edge = |pin: &str| EdgeConnection {
        source_node_guid: 1,
        source_pin_name: pin.into(),
        target_node_guid: 2,
        target_pin_name: "In".into(),
    };
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    world.seed_current_location(LIGHTHOUSE).unwrap();
    RunContent {
        registry: GameplayTagRegistry::compile(["fact".to_string(), "fact.tide_table".to_string()]).unwrap(),
        dispatcher: UnifiedDispatcher::new(vec![StoryletDef { storylet_id: 0, payload_node_guid: 1, ..Default::default() }]),
        passion_registry: PassionRegistry::empty(),
        storylet_titles: BTreeMap::from([(0, "Start".into())]),
        entity_names: BTreeMap::from([(LIGHTHOUSE, "Lighthouse".into()), (ROCK, "Rock".into()), (HARBOR, "Harbor".into())]),
        initial_world: world,
        root_asset: FlowGraphAsset {
            asset_id: "space".into(),
            start_node_guid: 1,
            nodes: vec![choice, end],
            edges: vec![edge("left"), edge("right")],
            data_edges: vec![],
        },
        clock: Some(Clock {
            slot_labels: ["D1 evening", "D2 morning", "D2 evening", "D3 morning", "D3 evening"].map(String::from).to_vec(),
            actions_per_slot: 2,
        }),
        connections: vec![
            Connection { from: LIGHTHOUSE, to: HARBOR, open_slots: None, requires_fact: None, on_close: None },
            // The rock is only reachable in slots 1 and 3 (D2 morning, D3 morning).
            Connection { from: LIGHTHOUSE, to: ROCK, open_slots: Some(0b01010), requires_fact: None, on_close: Some(OnClose::Return { to: LIGHTHOUSE }) },
            Connection { from: ROCK, to: LIGHTHOUSE, open_slots: None, requires_fact: None, on_close: None },
        ],
        ..Default::default()
    }
}

fn started(content: &RunContent) -> RunKernel {
    RunKernel::start(content, RunStart::Beginning, 1).unwrap()
}
fn set_slot(run: &mut RunKernel, slot: u32) {
    run.world.clock.as_mut().unwrap().slot = slot;
}
fn choose_left() -> RunAction {
    RunAction::Choose { node_guid: 1, output_pin: 0 }
}

#[test]
fn closed_edge_blocks_with_until_label() {
    let content = content();
    let mut run = started(&content);
    set_slot(&mut run, 2);
    let moves = moves_for(&run.world, &content, 0);
    let rock = moves.iter().find(|m| m.to == ROCK).unwrap();
    assert!(!rock.enabled);
    assert_eq!(rock.name, "Rock");
    assert_eq!(rock.block, Some(MoveBlock::ClosedUntil { slot_label: "D3 morning".into() }));
    let harbor = moves.iter().find(|m| m.to == HARBOR).unwrap();
    assert!(harbor.enabled && harbor.block.is_none());
    set_slot(&mut run, 3);
    assert!(moves_for(&run.world, &content, 0).iter().find(|m| m.to == ROCK).unwrap().enabled);
}

#[test]
fn edge_requires_fact() {
    let mut content = content();
    content.connections[0].requires_fact = Some(1);
    let mut run = started(&content);
    let harbor = |run: &RunKernel| moves_for(&run.world, &content, 0).into_iter().find(|m| m.to == HARBOR).unwrap();
    assert_eq!(harbor(&run).block, Some(MoveBlock::NeedsFact { fact: "fact.tide_table".into() }));
    assert!(!harbor(&run).enabled);
    run.world.knowledge.learn(KnowledgeRow { holder: 0, fact: 1, kind: KnowKind::Evidence, source_event: 0 });
    assert!(harbor(&run).enabled);
}

#[test]
fn occupant_returned_when_edge_closes() {
    let content = content();
    let mut run = started(&content);
    set_slot(&mut run, 1);
    let mut delta = BijectiveDelta::default();
    run.world.apply_move(0, Some(ROCK), &mut delta);
    let mut delta = BijectiveDelta::default();
    let events = advance_slot(&mut run.world, &content, &mut delta);
    assert_eq!(events.len(), 1);
    assert_eq!((events[0].participant, events[0].from, events[0].to), (0, ROCK, Some(LIGHTHOUSE)));
    assert_eq!(run.world.active_location(0), Some(LIGHTHOUSE));
    assert_eq!(run.world.clock.as_ref().unwrap().slot, 2);
}

#[test]
fn strand_leaves_the_occupant_in_place_and_reports_it() {
    let mut content = content();
    content.connections[1].on_close = Some(OnClose::Strand);
    let mut run = started(&content);
    set_slot(&mut run, 1);
    let mut delta = BijectiveDelta::default();
    run.world.apply_move(0, Some(ROCK), &mut delta);
    let events = advance_slot(&mut run.world, &content, &mut BijectiveDelta::default());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].to, None);
    assert_eq!(run.world.active_location(0), Some(ROCK));
}

#[test]
fn wait_spends_all_actions_and_advances_when_all_humans_done() {
    let content = content();
    let mut run = started(&content);
    assert_eq!(run.view(&content, 0).place.unwrap().actions_left, Some(2));
    run.act(&content, 0, choose_left()).unwrap();
    let place = run.view(&content, 0).place.unwrap();
    assert_eq!(place.actions_left, Some(1));
    assert_eq!(place.clock_label.as_deref(), Some("D1 evening"));
    run.act(&content, 0, RunAction::Wait).unwrap();
    let place = run.view(&content, 0).place.unwrap();
    assert_eq!(place.clock_label.as_deref(), Some("D2 morning"));
    assert_eq!(place.actions_left, Some(2));
}

#[test]
fn occupancy_event_is_recorded_on_the_step_that_closed_the_edge() {
    let content = content();
    let mut run = started(&content);
    set_slot(&mut run, 1);
    let mut delta = BijectiveDelta::default();
    run.world.apply_move(0, Some(ROCK), &mut delta);
    run.act(&content, 0, RunAction::Wait).unwrap();
    let view = run.view(&content, 0);
    let step = view.steps.last().unwrap();
    assert_eq!(step.notes.len(), 1);
    assert!(step.notes[0].contains("Lighthouse"), "{:?}", step.notes);
    assert_eq!(view.place.unwrap().name, "Lighthouse");
}

#[test]
fn acting_without_actions_left_is_rejected_and_changes_nothing() {
    let content = content();
    let mut run = started(&content);
    run.world.clock = Some(ClockState { slot: 4, actions_left: vec![0] });
    let before = serde_json::to_value(run.view(&content, 0)).unwrap();
    assert!(run.act(&content, 0, choose_left()).is_err());
    assert_eq!(serde_json::to_value(run.view(&content, 0)).unwrap(), before);
}

#[test]
fn move_follows_connections_and_spends_an_action() {
    let content = content();
    let mut run = started(&content);
    run.act(&content, 0, RunAction::Move { to: HARBOR }).unwrap();
    let place = run.view(&content, 0).place.unwrap();
    assert_eq!(place.name, "Harbor");
    assert_eq!(place.actions_left, Some(1));
    // Harbor has no outgoing connection, and the rock is closed in slot 0.
    assert!(run.act(&content, 0, RunAction::Move { to: ROCK }).is_err());
}

#[test]
fn move_through_a_closed_connection_is_rejected() {
    let content = content();
    let mut run = started(&content);
    assert!(run.act(&content, 0, RunAction::Move { to: ROCK }).is_err());
    assert_eq!(run.view(&content, 0).place.unwrap().name, "Lighthouse");
}

#[test]
fn timeless_project_spends_nothing() {
    let mut content = content();
    content.clock = None;
    let mut run = started(&content);
    assert!(run.world.clock.is_none());
    run.act(&content, 0, choose_left()).unwrap();
    let place = run.view(&content, 0).place.unwrap();
    assert_eq!(place.actions_left, None);
    assert_eq!(place.clock_label, None);
    // Scheduled connections are always open without a clock.
    assert!(moves_for(&run.world, &content, 0).iter().all(|m| m.enabled));
}

#[test]
fn clock_state_survives_rewind() {
    let content = content();
    let mut run = started(&content);
    run.act(&content, 0, choose_left()).unwrap();
    run.rewind(0).unwrap();
    assert_eq!(run.view(&content, 0).place.unwrap().actions_left, Some(2));
}

// --- Rule participants -------------------------------------------------------

use storylet_engine::chrono_participants::Controller;
use storylet_engine::chrono_runtime::Visibility;

const ROSA: u16 = 1;
const BENEDEK: u16 = 2;

fn learn_node(guid: u32, fact: u16, kind: KnowKind, visibility: Visibility) -> FlowNodeDef {
    FlowNodeDef {
        node_guid: guid,
        node_type_id: "learn".into(),
        category: NodeCategory::Mutator,
        kind: NodeKind::Learn { fact, kind, visibility },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    }
}
fn end_node(guid: u32) -> FlowNodeDef {
    FlowNodeDef {
        node_guid: guid,
        node_type_id: "end".into(),
        category: NodeCategory::Mutator,
        kind: NodeKind::End { jump_to_storylet: None, precondition_result: true },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    }
}
fn flow(from: u32, to: u32) -> EdgeConnection {
    EdgeConnection { source_node_guid: from, source_pin_name: "Out".into(), target_node_guid: to, target_pin_name: "In".into() }
}

/// Adds Rosa and Benedek (rule participants at the harbor) and one gossip
/// storylet per entry of `gossip`: `(storylet id, fact it teaches, display priority)`.
fn with_rules(gossip: &[(u16, &str, i32)]) -> RunContent {
    let mut content = content();
    content.registry = GameplayTagRegistry::compile(
        ["fact", "fact.tide_table", "fact.a", "fact.b"].map(String::from),
    )
    .unwrap();
    content.initial_world.participants.push("rosa", 10, Controller::Rule, Some(HARBOR));
    content.initial_world.participants.push("benedek", 11, Controller::Rule, Some(HARBOR));
    let mut defs = vec![StoryletDef { storylet_id: 0, payload_node_guid: 1, ..Default::default() }];
    for &(id, fact_name, priority) in gossip {
        let fact = content.registry.id(fact_name).unwrap();
        let learn = 100 + id as u32 * 10;
        content.root_asset.nodes.push(learn_node(learn, fact, KnowKind::Told, Visibility::CoLocated));
        content.root_asset.nodes.push(end_node(learn + 1));
        content.root_asset.edges.push(flow(learn, learn + 1));
        content.storylet_titles.insert(id, format!("Gossip {id}"));
        defs.push(StoryletDef {
            storylet_id: id,
            payload_node_guid: learn,
            display_priority: priority,
            played_by: vec![ROSA],
            ..Default::default()
        });
    }
    content.dispatcher = UnifiedDispatcher::new(defs);
    content
}
fn registry_fact(content: &RunContent, name: &str) -> u16 {
    content.registry.id(name).unwrap()
}

#[test]
fn rule_storylet_plays_at_slot_start() {
    let content = with_rules(&[(1, "fact.a", 0)]);
    let fact_a = registry_fact(&content, "fact.a");
    let mut run = started(&content);
    assert!(run.world.knowledge.knows(BENEDEK, fact_a).is_none());
    // The player ends the turn; the slot advances and Rosa gossips to Benedek, who shares her place.
    run.act(&content, 0, RunAction::Wait).unwrap();
    let row = run.world.knowledge.knows(BENEDEK, fact_a).expect("Benedek overheard the gossip");
    assert_eq!(row.kind, KnowKind::Witnessed);
    assert_eq!(run.world.knowledge.knows(ROSA, fact_a).unwrap().kind, KnowKind::Told);
    // The player was not at the harbor.
    assert!(run.world.knowledge.knows(0, fact_a).is_none());
}

#[test]
fn rule_pass_one_play_per_participant() {
    // Both storylets belong to Rosa; the higher display priority goes first, one per pass.
    let content = with_rules(&[(1, "fact.a", 0), (2, "fact.b", 5)]);
    let (fact_a, fact_b) = (registry_fact(&content, "fact.a"), registry_fact(&content, "fact.b"));
    let mut run = started(&content);
    run.act(&content, 0, RunAction::Wait).unwrap();
    assert!(run.world.knowledge.knows(ROSA, fact_b).is_some(), "priority 5 plays first");
    assert!(run.world.knowledge.knows(ROSA, fact_a).is_none(), "only one play per participant per pass");
    run.act(&content, 0, RunAction::Wait).unwrap();
    assert!(run.world.knowledge.knows(ROSA, fact_a).is_some(), "the other plays at the next slot");
}

#[test]
fn rule_storylet_does_not_repeat_unless_repeatable() {
    let content = with_rules(&[(1, "fact.a", 0)]);
    let mut run = started(&content);
    run.act(&content, 0, RunAction::Wait).unwrap();
    run.act(&content, 0, RunAction::Wait).unwrap();
    let plays: usize = run.view(&content, 0).steps.iter().map(|s| s.rule_plays.len()).sum();
    assert_eq!(plays, 1);
}

#[test]
fn rule_play_is_recorded_on_the_triggering_step() {
    let content = with_rules(&[(1, "fact.a", 0)]);
    let mut run = started(&content);
    run.act(&content, 0, RunAction::Wait).unwrap();
    let view = run.view(&content, 0);
    let step = view.steps.last().unwrap();
    assert_eq!(step.participant, 0, "the step is the human's act");
    assert_eq!(step.rule_plays.len(), 1);
    assert_eq!(step.rule_plays[0].participant, ROSA);
    assert_eq!(step.rule_plays[0].storylet.title, "Gossip 1");
}

#[test]
fn rule_pass_without_a_clock_runs_after_every_human_act() {
    let mut content = with_rules(&[(1, "fact.a", 0)]);
    content.clock = None;
    let fact_a = registry_fact(&content, "fact.a");
    let mut run = started(&content);
    run.act(&content, 0, choose_left()).unwrap();
    assert!(run.world.knowledge.knows(ROSA, fact_a).is_some());
}

#[test]
fn rule_play_is_undone_by_rewind() {
    let content = with_rules(&[(1, "fact.a", 0)]);
    let fact_a = registry_fact(&content, "fact.a");
    let mut run = started(&content);
    run.act(&content, 0, RunAction::Wait).unwrap();
    run.rewind(0).unwrap();
    assert!(run.world.knowledge.knows(ROSA, fact_a).is_none());
    run.act(&content, 0, RunAction::Wait).unwrap();
    assert!(run.world.knowledge.knows(ROSA, fact_a).is_some(), "the replayed slot start plays it again");
}

#[test]
fn storylets_played_by_a_rule_participant_are_not_offered_to_humans() {
    let content = with_rules(&[(1, "fact.a", 0)]);
    let run = started(&content);
    assert!(run.why_not(&content, 0, 10).iter().all(|m| m.storylet.id != 1));
}

#[test]
fn participant_views_say_who_is_human() {
    let content = with_rules(&[]);
    let run = started(&content);
    let participants = run.view(&content, 0).participants;
    let humans: Vec<_> = participants.iter().map(|p| p.human).collect();
    assert_eq!(humans, [true, false, false]);
}
