use storylet_engine::chrono_chronicle::{ChronicleEntry, ChronicleTable, TellingRow, TellingTable};
use storylet_engine::chrono_dispatch::UnifiedDispatcher;
use storylet_engine::chrono_participants::{Controller, KnowKind};
use storylet_engine::chrono_runtime::{
    EdgeConnection, FlowGraphAsset, FlowGraphInstance, FlowNodeDef, NodeCategory,
    NodeExecutionState, NodeKind, PinDefinition, RuntimeContext, Visibility,
};
use storylet_engine::chrono_state::{revert_delta, BijectiveDelta, WorldStateTable};

fn entry(key: &str, story_time: i64, facts: Vec<u16>) -> ChronicleEntry {
    ChronicleEntry {
        key: key.into(),
        story_time,
        label: key.to_uppercase(),
        location: None,
        facts,
        text: String::new(),
    }
}

/// P1..P4 in story order; P2 carries facts 10 and 11.
fn canon() -> ChronicleTable {
    ChronicleTable::new(vec![
        entry("p4", 40, vec![]),
        entry("p1", 10, vec![]),
        entry("p3", 30, vec![]),
        entry("p2", 20, vec![10, 11]),
    ])
}

fn world() -> WorldStateTable {
    let mut world = WorldStateTable::new();
    world.participants.push("a", 0, Controller::Human { seat: 0 }, Some(3));
    world.participants.push("b", 1, Controller::Rule, Some(3));
    world.participants.push("c", 2, Controller::Rule, Some(4));
    world
}

#[test]
fn chronicle_is_sorted_by_story_time_then_key() {
    let table = ChronicleTable::new(vec![entry("b", 5, vec![]), entry("a", 5, vec![]), entry("z", 1, vec![])]);
    let keys: Vec<_> = table.entries.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(keys, ["z", "a", "b"]);
    assert_eq!(table.index_of("a"), Some(1));
}

#[test]
fn reveal_order_is_free_and_canon_unchanged() {
    let canon = canon();
    let before = canon.clone();
    let (p1, p3) = (canon.index_of("p1").unwrap(), canon.index_of("p3").unwrap());
    let mut telling = TellingTable::default();
    assert!(telling.reveal(TellingRow { holder: 0, entry: p3, source_event: 1 }));
    assert!(telling.reveal(TellingRow { holder: 0, entry: p1, source_event: 2 }));
    assert_eq!(telling.order_for(0), vec![p3, p1]);
    assert_eq!(telling.told_number(0, p3), Some(1));
    assert_eq!(telling.told_number(0, p1), Some(2));
    assert_eq!(telling.told_number(0, canon.index_of("p2").unwrap()), None);
    assert!(telling.order_for(1).is_empty());
    assert_eq!(canon, before);
    let times: Vec<_> = canon.entries.iter().map(|e| e.story_time).collect();
    assert!(times.windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn reveal_twice_is_idempotent() {
    let canon = canon();
    let p2 = canon.index_of("p2").unwrap();
    let mut state = world();
    let mut delta = BijectiveDelta::default();
    assert!(state.apply_reveal(0, p2, &canon.entries[p2 as usize].facts, KnowKind::Recollection, 4, &mut delta));
    let rows = state.knowledge.rows.len();
    let mut second = BijectiveDelta::default();
    assert!(!state.apply_reveal(0, p2, &canon.entries[p2 as usize].facts, KnowKind::Recollection, 9, &mut second));
    assert_eq!(state.knowledge.rows.len(), rows);
    assert_eq!(state.telling.rows.len(), 1);
    assert!(second.is_empty());
}

#[test]
fn reveal_learns_entry_facts_with_kind() {
    let canon = canon();
    let p2 = canon.index_of("p2").unwrap();
    let mut state = world();
    let mut delta = BijectiveDelta::default();
    state.apply_reveal(0, p2, &canon.entries[p2 as usize].facts, KnowKind::Recollection, 4, &mut delta);
    for fact in [10, 11] {
        let row = state.knowledge.knows(0, fact).unwrap();
        assert_eq!(row.kind, KnowKind::Recollection);
        assert_eq!(row.source_event, 4);
    }
    assert!(state.knowledge.knows(1, 10).is_none());
}

#[test]
fn revert_reveal_removes_telling_and_facts() {
    let canon = canon();
    let p2 = canon.index_of("p2").unwrap();
    let mut state = world();
    let before = state.clone();
    let mut delta = BijectiveDelta::default();
    state.apply_reveal(0, p2, &canon.entries[p2 as usize].facts, KnowKind::Recollection, 4, &mut delta);
    assert!(!delta.is_empty());
    revert_delta(&mut state, &delta).unwrap();
    assert_eq!(state, before);
}

// --- Reveal node -----------------------------------------------------------

fn pin(name: &str, is_input: bool) -> PinDefinition {
    PinDefinition { pin_name: name.to_owned(), is_input }
}

fn flow_node(guid: u32, kind: NodeKind, category: NodeCategory) -> FlowNodeDef {
    FlowNodeDef {
        node_guid: guid,
        node_type_id: "chronicle_test".to_owned(),
        category,
        kind,
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..FlowNodeDef::default()
    }
}

fn edge(from: u32, to: u32) -> EdgeConnection {
    EdgeConnection {
        source_node_guid: from,
        source_pin_name: "Out".into(),
        target_node_guid: to,
        target_pin_name: "In".into(),
    }
}

const REVEAL: u32 = 2;

fn reveal_instance(visibility: Visibility, acting: u16) -> FlowGraphInstance {
    let asset = FlowGraphAsset {
        asset_id: "chronicle".into(),
        start_node_guid: 1,
        nodes: vec![
            flow_node(1, NodeKind::LogicAll, NodeCategory::LogicGate),
            flow_node(
                REVEAL,
                NodeKind::Reveal { entry: 1, kind: KnowKind::Recollection, visibility, facts: vec![10, 11] },
                NodeCategory::Mutator,
            ),
            flow_node(
                3,
                NodeKind::End { jump_to_storylet: None, precondition_result: true },
                NodeCategory::Mutator,
            ),
        ],
        edges: vec![edge(1, REVEAL), edge(REVEAL, 3)],
        data_edges: vec![],
    };
    let mut instance = FlowGraphInstance::new(1, asset, 7).unwrap();
    instance.acting_participant = acting;
    instance.start().unwrap();
    instance
}

#[test]
fn reveal_node_tells_the_actor_and_learns_the_facts() {
    let mut world = world();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = reveal_instance(Visibility::Private, 0);
    instance.tick(&mut ctx, 16).unwrap();
    assert_eq!(instance.node(3).unwrap().state, NodeExecutionState::Completed);
    assert_eq!(ctx.state.telling.order_for(0), vec![1]);
    assert_eq!(ctx.state.knowledge.knows(0, 10).unwrap().kind, KnowKind::Recollection);
    assert!(ctx.state.telling.order_for(1).is_empty());
}

#[test]
fn colocated_reveal_node_also_tells_witnesses() {
    let mut world = world();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = reveal_instance(Visibility::CoLocated, 0);
    instance.tick(&mut ctx, 16).unwrap();
    assert_eq!(ctx.state.telling.order_for(1), vec![1]);
    assert_eq!(ctx.state.knowledge.knows(1, 10).unwrap().kind, KnowKind::Witnessed);
    assert!(ctx.state.telling.order_for(2).is_empty());
}

#[test]
fn cancel_and_revert_reveal_node_restores_the_world() {
    let mut world = world();
    let before = world.clone();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = reveal_instance(Visibility::CoLocated, 0);
    instance.tick(&mut ctx, 16).unwrap();
    instance.cancel_and_revert(&mut ctx, REVEAL).unwrap();
    assert_eq!(*ctx.state, before);
}
