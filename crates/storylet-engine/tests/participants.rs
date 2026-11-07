use storylet_engine::chrono_participants::{
    Controller, KnowKind, KnowledgeRow, KnowledgeTable, ParticipantTable,
};
use storylet_engine::chrono_state::{
    BijectiveDelta, GraphLayer, MutationOp, TagBitset, WorldStateTable, WorldVertex,
    apply_mutation, revert_delta,
};

fn row(holder: u16, fact: u16, kind: KnowKind, source_event: u32) -> KnowledgeRow {
    KnowledgeRow {
        holder,
        fact,
        kind,
        source_event,
    }
}

#[test]
fn learn_keeps_first_source() {
    let mut table = KnowledgeTable::default();
    assert!(table.learn(row(0, 5, KnowKind::Rumor, 3)));
    assert!(!table.learn(row(0, 5, KnowKind::Evidence, 9)));
    assert_eq!(table.knows(0, 5).unwrap().source_event, 3);
    assert_eq!(table.knows(0, 5).unwrap().kind, KnowKind::Rumor);
}

#[test]
fn colocated_excludes_self_and_none() {
    let mut table = ParticipantTable::default();
    table.push("a", 0, Controller::Human { seat: 0 }, Some(1));
    table.push("b", 1, Controller::Rule, Some(1));
    table.push("c", 2, Controller::Rule, None);
    assert_eq!(table.colocated(0).collect::<Vec<_>>(), vec![1]);
    assert_eq!(table.colocated(2).count(), 0);
}

#[test]
fn implicit_player_is_single_human() {
    let table = ParticipantTable::implicit_player(Some(4));
    assert_eq!(table.len(), 1);
    assert_eq!(table.humans().collect::<Vec<_>>(), vec![0]);
    assert_eq!(table.keys[0], "player");
    assert_eq!(table.entity[0], u16::MAX);
    assert_eq!(table.location[0], Some(4));
    assert_eq!(table.index_of("player"), Some(0));
}

#[test]
fn rumor_and_testimony_are_unconfirmed() {
    assert!(!KnowKind::Rumor.is_confirmed());
    assert!(!KnowKind::Testimony.is_confirmed());
    assert!(KnowKind::Evidence.is_confirmed());
}

#[test]
fn humans_are_ordered_by_seat_then_index() {
    let mut table = ParticipantTable::default();
    table.push("npc", 0, Controller::Rule, None);
    table.push("b", 1, Controller::Human { seat: 1 }, None);
    table.push("a", 2, Controller::Human { seat: 0 }, None);
    assert_eq!(table.humans().collect::<Vec<_>>(), vec![2, 1]);
}

#[test]
fn set_location_returns_previous() {
    let mut table = ParticipantTable::implicit_player(None);
    assert_eq!(table.set_location(0, Some(2)), None);
    assert_eq!(table.set_location(0, Some(3)), Some(2));
}

#[test]
fn rows_stay_sorted_and_slice_by_holder() {
    let mut table = KnowledgeTable::default();
    table.learn(row(2, 1, KnowKind::Told, 1));
    table.learn(row(0, 9, KnowKind::Witnessed, 2));
    table.learn(row(2, 0, KnowKind::Evidence, 3));
    table.learn(row(0, 4, KnowKind::Rumor, 4));
    let keys: Vec<_> = table.rows.iter().map(|r| (r.holder, r.fact)).collect();
    assert_eq!(keys, vec![(0, 4), (0, 9), (2, 0), (2, 1)]);
    assert_eq!(table.rows_for(2).len(), 2);
    assert!(table.rows_for(1).is_empty());
    assert_eq!(table.forget(0, 4).unwrap().source_event, 4);
    assert!(table.knows(0, 4).is_none());
    assert!(table.forget(0, 4).is_none());
}

#[test]
fn tables_round_trip_through_serde() {
    let mut table = ParticipantTable::implicit_player(Some(1));
    table.push("npc", 3, Controller::Rule, None);
    let json = serde_json::to_string(&table).unwrap();
    assert!(json.contains("\"kind\":\"human\""));
    assert_eq!(
        serde_json::from_str::<ParticipantTable>(&json).unwrap(),
        table
    );
}

#[test]
fn learn_then_revert_restores_world() {
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    let before = world.clone();
    let mut delta = BijectiveDelta::default();
    world.apply_learn(row(0, 7, KnowKind::Witnessed, 3), &mut delta);
    assert!(world.knowledge.knows(0, 7).is_some());
    assert_eq!(delta.knowledge_added, vec![(0, 7)]);
    // A duplicate learn is a no-op and must not be recorded for revert.
    world.apply_learn(row(0, 7, KnowKind::Rumor, 9), &mut delta);
    assert_eq!(delta.knowledge_added.len(), 1);
    revert_delta(&mut world, &delta).unwrap();
    assert_eq!(world, before);
}

fn world_with_locations() -> WorldStateTable {
    let mut world = WorldStateTable::new();
    let mut tag = TagBitset::new();
    tag.set_bit(5).unwrap();
    world.entity_tags[1] = tag;
    let mut tag = TagBitset::new();
    tag.set_bit(6).unwrap();
    world.entity_tags[2] = tag;
    world.seed_current_location(1).unwrap();
    world.ensure_participants();
    world
}

#[test]
fn move_then_revert_restores_location_projection() {
    let mut world = world_with_locations();
    let before = world.clone();
    assert_eq!(world.active_location(0), Some(1));
    let mut delta = BijectiveDelta::default();
    world.apply_move(0, Some(2), &mut delta);
    assert_eq!(world.current_location_entity, Some(2));
    assert_eq!(world.active_location(0), Some(2));
    assert_eq!(world.current_location_tag_mask, world.entity_tags[2]);
    assert!(world.global_tags.contains_all(&world.entity_tags[2]));
    assert!(!world.global_tags.intersects(&world.entity_tags[1]));
    revert_delta(&mut world, &delta).unwrap();
    assert_eq!(world.current_location_entity, Some(1));
    assert_eq!(world, before);
}

#[test]
fn move_of_other_participant_leaves_projection_alone() {
    let mut world = world_with_locations();
    world.participants.push("npc", 3, Controller::Rule, Some(1));
    let before = world.clone();
    let mut delta = BijectiveDelta::default();
    world.apply_move(1, Some(2), &mut delta);
    assert_eq!(world.active_location(1), Some(2));
    assert_eq!(world.current_location_entity, Some(1));
    assert_eq!(delta.participant_moves, vec![(1, Some(1), Some(2))]);
    revert_delta(&mut world, &delta).unwrap();
    assert_eq!(world, before);
}

#[test]
fn chained_moves_revert_in_reverse_order() {
    let mut world = world_with_locations();
    let before = world.clone();
    let mut delta = BijectiveDelta::default();
    world.apply_move(0, Some(2), &mut delta);
    world.apply_move(0, None, &mut delta);
    world.apply_move(0, Some(1), &mut delta);
    revert_delta(&mut world, &delta).unwrap();
    assert_eq!(world, before);
}

#[test]
fn v5_world_json_loads_with_implicit_player() {
    let mut value = serde_json::to_value(WorldStateTable::new()).unwrap();
    let object = value.as_object_mut().unwrap();
    assert!(object.remove("participants").is_some());
    assert!(object.remove("knowledge").is_some());
    let mut world: WorldStateTable = serde_json::from_value(value).unwrap();
    assert!(world.participants.is_empty());
    world.ensure_participants();
    assert_eq!(world.participants.len(), 1);
    assert_eq!(
        world.participants.location[0],
        world.current_location_entity
    );
}

#[test]
fn ensure_participants_normalizes_uneven_columns() {
    let mut world = WorldStateTable::new();
    world
        .participants
        .push("a", 0, Controller::Human { seat: 0 }, None);
    world.participants.push("b", 1, Controller::Rule, None);
    world.participants.location.pop();
    world.knowledge.rows = vec![
        row(1, 2, KnowKind::Told, 4),
        row(0, 9, KnowKind::Rumor, 1),
        row(0, 9, KnowKind::Evidence, 2),
    ];
    world.ensure_participants();
    assert_eq!(world.participants.keys.len(), 1);
    assert_eq!(world.participants.entity.len(), 1);
    assert_eq!(world.participants.controller.len(), 1);
    assert_eq!(world.participants.location.len(), 1);
    let keys: Vec<_> = world
        .knowledge
        .rows
        .iter()
        .map(|r| (r.holder, r.fact))
        .collect();
    assert_eq!(keys, vec![(0, 9), (1, 2)]);
    assert_eq!(world.knowledge.knows(0, 9).unwrap().kind, KnowKind::Rumor);
}

#[test]
fn mutation_move_player_syncs_participant_zero_and_reverts() {
    let mut world = WorldStateTable::new();
    let mut tag = TagBitset::new();
    tag.set_bit(5).unwrap();
    world.entity_tags[1] = tag;
    let mut tag = TagBitset::new();
    tag.set_bit(6).unwrap();
    world.entity_tags[2] = tag;
    for (id, entity) in [("loc.a", 1), ("loc.b", 2)] {
        world.vertices.push(WorldVertex {
            vertex_id: id.to_string(),
            layers: GraphLayer::Location.into(),
            entity_index: Some(entity),
            attributes: Default::default(),
        });
    }
    world.seed_current_location(1).unwrap();
    world.ensure_participants();
    let before = world.clone();
    let op = MutationOp {
        move_player: Some(2),
        ..MutationOp::default()
    };
    let delta = apply_mutation(&mut world, &op).unwrap();
    assert_eq!(world.current_location_entity, Some(2));
    assert_eq!(world.active_location(0), world.current_location_entity);
    assert_eq!(delta.participant_moves, vec![(0, Some(1), Some(2))]);
    revert_delta(&mut world, &delta).unwrap();
    assert_eq!(world.active_location(0), Some(1));
    assert_eq!(world, before);
}

#[test]
fn seed_current_location_after_ensure_participants_moves_participant_zero() {
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    assert_eq!(world.active_location(0), None);
    world.seed_current_location(3).unwrap();
    assert_eq!(world.current_location_entity, Some(3));
    assert_eq!(world.active_location(0), Some(3));
}

// --- Graph nodes: Learn, MoveParticipant, Knows ---------------------------

use storylet_engine::chrono_dispatch::UnifiedDispatcher;
use storylet_engine::chrono_runtime::{
    DataEdgeConnection, DataPinDefinition, EdgeConnection, FlowGraphAsset, FlowGraphInstance,
    FlowNodeDef, GraphError, HolderRef, NodeCategory, NodeExecutionState, NodeKind, PinDefinition,
    RuntimeContext, Visibility,
};

fn pin(name: &str, is_input: bool) -> PinDefinition {
    PinDefinition {
        pin_name: name.to_owned(),
        is_input,
    }
}

fn flow_node(guid: u32, kind: NodeKind, category: NodeCategory) -> FlowNodeDef {
    FlowNodeDef {
        node_guid: guid,
        node_type_id: "participants_test".to_owned(),
        category,
        kind,
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..FlowNodeDef::default()
    }
}

fn flow_edge(from: u32, from_pin: &str, to: u32) -> EdgeConnection {
    EdgeConnection {
        source_node_guid: from,
        source_pin_name: from_pin.to_owned(),
        target_node_guid: to,
        target_pin_name: "In".to_owned(),
    }
}

fn end_node(guid: u32) -> FlowNodeDef {
    flow_node(
        guid,
        NodeKind::End {
            jump_to_storylet: None,
            precondition_result: true,
        },
        NodeCategory::Mutator,
    )
}

const START: u32 = 1;
const MUTATOR: u32 = 2;
const END: u32 = 3;

/// `Start -> mutator -> End`, started and ready to tick.
fn linear_instance(mutator: NodeKind, acting: u16) -> FlowGraphInstance {
    let asset = FlowGraphAsset {
        asset_id: "participants".to_owned(),
        start_node_guid: START,
        nodes: vec![
            flow_node(START, NodeKind::LogicAll, NodeCategory::LogicGate),
            flow_node(MUTATOR, mutator, NodeCategory::Mutator),
            end_node(END),
        ],
        edges: vec![
            flow_edge(START, "Out", MUTATOR),
            flow_edge(MUTATOR, "Out", END),
        ],
        data_edges: vec![],
    };
    let mut instance = FlowGraphInstance::new(1, asset, 7).unwrap();
    instance.acting_participant = acting;
    instance.start().unwrap();
    instance
}

fn learn(kind: KnowKind, visibility: Visibility) -> NodeKind {
    NodeKind::Learn {
        fact: 7,
        kind,
        visibility,
    }
}

/// Participants 0 and 1 at location 3, participant 2 at location 4.
fn three_participant_world() -> WorldStateTable {
    let mut world = WorldStateTable::new();
    world
        .participants
        .push("a", 0, Controller::Human { seat: 0 }, Some(3));
    world.participants.push("b", 1, Controller::Rule, Some(3));
    world.participants.push("c", 2, Controller::Rule, Some(4));
    world
}

#[test]
fn learn_records_call_history_source() {
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(learn(KnowKind::Evidence, Visibility::Private), 0);
    instance.tick(&mut ctx, 16).unwrap();
    assert_eq!(
        instance.node(END).unwrap().state,
        NodeExecutionState::Completed
    );
    let learn_index = instance
        .call_history()
        .iter()
        .position(|entry| entry.node_guid == MUTATOR)
        .unwrap();
    // The start node's entry comes first, so the index is not trivially 0.
    assert_eq!(learn_index, 1);
    let row = ctx.state.knowledge.knows(0, 7).unwrap();
    assert_eq!(row.source_event, learn_index as u32);
    assert_eq!(row.kind, KnowKind::Evidence);
}

#[test]
fn colocated_learn_creates_witness_rows() {
    let mut world = three_participant_world();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(learn(KnowKind::Evidence, Visibility::CoLocated), 0);
    instance.tick(&mut ctx, 16).unwrap();
    let knowledge = &ctx.state.knowledge;
    let actor = knowledge.knows(0, 7).unwrap();
    assert_eq!(actor.kind, KnowKind::Evidence);
    let witness = knowledge.knows(1, 7).unwrap();
    assert_eq!(witness.kind, KnowKind::Witnessed);
    assert_eq!(witness.source_event, actor.source_event);
    assert!(knowledge.knows(2, 7).is_none());
}

#[test]
fn private_learn_creates_no_witness_rows() {
    let mut world = three_participant_world();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(learn(KnowKind::Told, Visibility::Private), 1);
    instance.tick(&mut ctx, 16).unwrap();
    assert_eq!(
        ctx.state.knowledge.knows(1, 7).unwrap().kind,
        KnowKind::Told
    );
    assert!(ctx.state.knowledge.knows(0, 7).is_none());
}

/// `Condition(input <- Knows)`; returns whether the `true` branch ran.
fn knows_branch(world: &mut WorldStateTable, holder: HolderRef, confirmed_only: bool) -> bool {
    const CONDITION: u32 = 1;
    const KNOWS: u32 = 2;
    const ON_TRUE: u32 = 3;
    const ON_FALSE: u32 = 4;
    let condition = FlowNodeDef {
        output_pins: vec![pin("true", false), pin("false", false)],
        data_input_pins: vec![DataPinDefinition {
            pin_name: "input".to_owned(),
            value_type: "bool".to_owned(),
            is_input: true,
        }],
        ..flow_node(
            CONDITION,
            NodeKind::Condition {
                operator: Default::default(),
                literal: None,
            },
            NodeCategory::LogicGate,
        )
    };
    let knows = FlowNodeDef {
        input_pins: vec![],
        output_pins: vec![],
        data_output_pins: vec![DataPinDefinition {
            pin_name: "value".to_owned(),
            value_type: "bool".to_owned(),
            is_input: false,
        }],
        ..flow_node(
            KNOWS,
            NodeKind::Knows {
                holder,
                fact: 7,
                confirmed_only,
            },
            NodeCategory::LogicGate,
        )
    };
    let asset = FlowGraphAsset {
        asset_id: "knows".to_owned(),
        start_node_guid: CONDITION,
        nodes: vec![condition, knows, end_node(ON_TRUE), end_node(ON_FALSE)],
        edges: vec![
            flow_edge(CONDITION, "true", ON_TRUE),
            flow_edge(CONDITION, "false", ON_FALSE),
        ],
        data_edges: vec![DataEdgeConnection {
            source_node_guid: KNOWS,
            source_pin_name: "value".to_owned(),
            target_node_guid: CONDITION,
            target_pin_name: "input".to_owned(),
        }],
    };
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(world, &dispatcher);
    let mut instance = FlowGraphInstance::new(1, asset, 7).unwrap();
    instance.start().unwrap();
    instance.tick(&mut ctx, 16).unwrap();
    let ran = |guid| instance.node(guid).unwrap().state == NodeExecutionState::Completed;
    assert_ne!(ran(ON_TRUE), ran(ON_FALSE));
    ran(ON_TRUE)
}

#[test]
fn knows_confirmed_only_rejects_rumor() {
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    world.knowledge.learn(row(0, 7, KnowKind::Rumor, 0));
    assert!(!knows_branch(&mut world, HolderRef::Acting, true));
    assert!(knows_branch(&mut world, HolderRef::Acting, false));
    // A holder that does not know the fact at all is false either way.
    world.participants.push("npc", 1, Controller::Rule, None);
    assert!(!knows_branch(&mut world, HolderRef::Participant(1), false));
}

#[test]
fn cancel_and_revert_learn_removes_rows() {
    let mut world = three_participant_world();
    let before = world.clone();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(learn(KnowKind::Evidence, Visibility::CoLocated), 0);
    instance.tick(&mut ctx, 16).unwrap();
    assert!(ctx.state.knowledge.knows(1, 7).is_some());
    instance.cancel_and_revert(&mut ctx, MUTATOR).unwrap();
    for holder in 0..3 {
        assert!(ctx.state.knowledge.knows(holder, 7).is_none());
    }
    assert!(
        instance
            .call_history()
            .iter()
            .all(|entry| entry.node_guid != MUTATOR)
    );
    assert_eq!(*ctx.state, before);
}

#[test]
fn force_complete_learn_uses_same_source_rule() {
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(learn(KnowKind::Inference, Visibility::Private), 0);
    instance
        .force_complete(&mut ctx, MUTATOR, "Out", None)
        .unwrap();
    let index = instance
        .call_history()
        .iter()
        .position(|entry| entry.node_guid == MUTATOR)
        .unwrap();
    assert_eq!(
        ctx.state.knowledge.knows(0, 7).unwrap().source_event,
        index as u32
    );
}

#[test]
fn move_participant_moves_acting_and_reverts() {
    let mut world = three_participant_world();
    let before = world.clone();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(NodeKind::MoveParticipant { location_entity: 4 }, 1);
    instance.tick(&mut ctx, 16).unwrap();
    assert_eq!(ctx.state.active_location(1), Some(4));
    assert_eq!(
        ctx.state.participants.colocated(1).collect::<Vec<_>>(),
        vec![2]
    );
    instance.cancel_and_revert(&mut ctx, MUTATOR).unwrap();
    assert_eq!(*ctx.state, before);
}

#[test]
fn nodes_without_a_participant_return_typed_errors() {
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut world = WorldStateTable::new();
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(learn(KnowKind::Evidence, Visibility::Private), 0);
    assert!(matches!(
        instance.tick(&mut ctx, 16),
        Err(GraphError::ParticipantNotFound {
            node_guid: MUTATOR,
            participant: 0
        })
    ));
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(NodeKind::MoveParticipant { location_entity: 2 }, 3);
    assert!(matches!(
        instance.tick(&mut ctx, 16),
        Err(GraphError::ParticipantNotFound {
            node_guid: MUTATOR,
            participant: 3
        })
    ));
    let mut world = WorldStateTable::with_capacity(2);
    world.ensure_participants();
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(NodeKind::MoveParticipant { location_entity: 2 }, 0);
    assert!(matches!(
        instance.tick(&mut ctx, 16),
        Err(GraphError::EntityIndexOutOfRange {
            node_guid: MUTATOR,
            entity_index: 2
        })
    ));
}

#[test]
fn new_node_kinds_round_trip_through_serde() {
    for kind in [
        learn(KnowKind::Rumor, Visibility::CoLocated),
        NodeKind::MoveParticipant { location_entity: 9 },
        NodeKind::Knows {
            holder: HolderRef::Participant(2),
            fact: 7,
            confirmed_only: true,
        },
        NodeKind::Knows {
            holder: HolderRef::Acting,
            fact: 7,
            confirmed_only: false,
        },
    ] {
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(
            serde_json::from_str::<NodeKind>(&json).unwrap(),
            kind,
            "{json}"
        );
    }
    let json = serde_json::to_value(learn(KnowKind::Rumor, Visibility::CoLocated)).unwrap();
    assert_eq!(json["kind"], "learn");
    assert_eq!(json["visibility"], "co_located");
    let instance: FlowGraphInstance = {
        let mut value = serde_json::to_value(linear_instance(NodeKind::LogicAll, 0)).unwrap();
        assert!(
            value
                .as_object_mut()
                .unwrap()
                .remove("acting_participant")
                .is_some()
        );
        serde_json::from_value(value).unwrap()
    };
    assert_eq!(instance.acting_participant, 0);
}

#[test]
fn removing_earlier_call_rebases_later_knowledge_source() {
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(learn(KnowKind::Evidence, Visibility::Private), 0);
    instance.asset.nodes.push(flow_node(
        4,
        NodeKind::Mutation {
            op: MutationOp::default(),
        },
        NodeCategory::Mutator,
    ));
    // Add the corresponding runtime instance by reconstructing from the asset.
    instance = FlowGraphInstance::new(1, instance.asset, 7).unwrap();
    instance.force_complete(&mut ctx, 4, "Out", None).unwrap();
    instance
        .force_complete(&mut ctx, MUTATOR, "Out", None)
        .unwrap();
    instance.cancel_and_revert(&mut ctx, 4).unwrap();
    assert_eq!(instance.call_history()[0].node_guid, MUTATOR);
    assert_eq!(ctx.state.knowledge.knows(0, 7).unwrap().source_event, 0);
    instance.cancel_and_revert(&mut ctx, MUTATOR).unwrap();
}

#[test]
fn precondition_graph_uses_requested_actor() {
    let mut world = three_participant_world();
    world.knowledge.learn(row(1, 7, KnowKind::Evidence, 0));
    let condition = FlowNodeDef {
        output_pins: vec![pin("true", false), pin("false", false)],
        data_input_pins: vec![DataPinDefinition {
            pin_name: "input".into(),
            value_type: "bool".into(),
            is_input: true,
        }],
        ..flow_node(
            1,
            NodeKind::Condition {
                operator: Default::default(),
                literal: None,
            },
            NodeCategory::LogicGate,
        )
    };
    let knows = FlowNodeDef {
        input_pins: vec![],
        output_pins: vec![],
        data_output_pins: vec![DataPinDefinition {
            pin_name: "value".into(),
            value_type: "bool".into(),
            is_input: false,
        }],
        ..flow_node(
            2,
            NodeKind::Knows {
                holder: HolderRef::Acting,
                fact: 7,
                confirmed_only: true,
            },
            NodeCategory::LogicGate,
        )
    };
    let false_end = flow_node(
        4,
        NodeKind::End {
            jump_to_storylet: None,
            precondition_result: false,
        },
        NodeCategory::Mutator,
    );
    let asset = FlowGraphAsset {
        asset_id: "precondition".into(),
        start_node_guid: 1,
        nodes: vec![condition, knows, end_node(3), false_end],
        edges: vec![flow_edge(1, "true", 3), flow_edge(1, "false", 4)],
        data_edges: vec![DataEdgeConnection {
            source_node_guid: 2,
            source_pin_name: "value".into(),
            target_node_guid: 1,
            target_pin_name: "input".into(),
        }],
    };
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let ctx = RuntimeContext::new(&mut world, &dispatcher);
    assert!(!FlowGraphInstance::evaluate_precondition_graph(&asset, &ctx).unwrap());
    assert!(FlowGraphInstance::evaluate_precondition_graph_for(&asset, &ctx, 1).unwrap());
}

#[test]
fn mission_uses_actor_changed_after_attachment() {
    let mut world = three_participant_world();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut child = linear_instance(learn(KnowKind::Evidence, Visibility::Private), 0).asset;
    child.asset_id = "child".into();
    let mut parent = linear_instance(
        NodeKind::MissionSubGraph {
            sub_graph_asset_id: child.asset_id.clone(),
        },
        0,
    );
    parent.attach_mission_asset(MUTATOR, &child, 7).unwrap();
    parent.acting_participant = 1;
    parent.tick(&mut ctx, 16).unwrap();
    assert!(ctx.state.knowledge.knows(1, 7).is_some());
    assert!(ctx.state.knowledge.knows(0, 7).is_none());
    let entries = parent.call_history().to_vec();
    let scoped = parent.nested_mission_histories();
    let mut replay_world = three_participant_world();
    let mut replay_ctx = RuntimeContext::new(&mut replay_world, &dispatcher);
    parent.acting_participant = 2;
    parent
        .replay_history_with_missions(&mut replay_ctx, &entries, &scoped)
        .unwrap();
    assert!(replay_ctx.state.knowledge.knows(2, 7).is_some());
    assert!(replay_ctx.state.knowledge.knows(1, 7).is_none());
}

#[test]
fn unrelated_graph_knowledge_does_not_block_or_rebase_cancel() {
    let mut world = three_participant_world();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut other = linear_instance(learn(KnowKind::Evidence, Visibility::Private), 1);
    // Put the other graph's Learn at index 5.
    for _ in 0..5 {
        other.force_complete(&mut ctx, START, "Out", None).unwrap();
    }
    other
        .force_complete(&mut ctx, MUTATOR, "Out", None)
        .unwrap();
    let other_row = ctx.state.knowledge.knows(1, 7).unwrap();
    assert_eq!(other_row.source_event, 5);
    let mut instance = linear_instance(learn(KnowKind::Evidence, Visibility::Private), 0);
    instance
        .force_complete(&mut ctx, START, "Out", None)
        .unwrap();
    instance
        .force_complete(&mut ctx, MUTATOR, "Out", None)
        .unwrap();
    instance.cancel_and_revert(&mut ctx, MUTATOR).unwrap();
    assert!(ctx.state.knowledge.knows(0, 7).is_none());
    assert_eq!(ctx.state.knowledge.knows(1, 7).unwrap().source_event, 5);
    instance.cancel_and_revert(&mut ctx, START).unwrap();
    assert_eq!(ctx.state.knowledge.knows(1, 7).unwrap().source_event, 5);
}

#[test]
fn removing_earlier_call_rebases_each_repeated_learn_invocation() {
    let mut world = three_participant_world();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut ctx = RuntimeContext::new(&mut world, &dispatcher);
    let mut instance = linear_instance(learn(KnowKind::Evidence, Visibility::Private), 0);
    instance
        .force_complete(&mut ctx, START, "Out", None)
        .unwrap();
    instance
        .force_complete(&mut ctx, MUTATOR, "Out", None)
        .unwrap();
    instance.acting_participant = 1;
    instance
        .force_complete(&mut ctx, MUTATOR, "Out", None)
        .unwrap();
    instance.cancel_and_revert(&mut ctx, START).unwrap();
    assert_eq!(ctx.state.knowledge.knows(0, 7).unwrap().source_event, 0);
    assert_eq!(ctx.state.knowledge.knows(1, 7).unwrap().source_event, 1);
    instance.cancel_and_revert(&mut ctx, MUTATOR).unwrap();
    assert!(ctx.state.knowledge.knows(1, 7).is_none());
    assert!(ctx.state.knowledge.knows(0, 7).is_some());
}

#[test]
fn colocation_precondition_uses_the_acting_participant_and_requires_a_place() {
    let mut world = three_participant_world();
    world.participants.location = vec![Some(1), Some(2), Some(2)];
    let condition = FlowNodeDef {
        output_pins: vec![pin("true", false), pin("false", false)],
        data_input_pins: vec![DataPinDefinition {
            pin_name: "input".into(),
            value_type: "bool".into(),
            is_input: true,
        }],
        ..flow_node(
            1,
            NodeKind::Condition {
                operator: Default::default(),
                literal: None,
            },
            NodeCategory::LogicGate,
        )
    };
    let present = FlowNodeDef {
        input_pins: vec![],
        output_pins: vec![],
        data_output_pins: vec![DataPinDefinition {
            pin_name: "value".into(),
            value_type: "bool".into(),
            is_input: false,
        }],
        ..flow_node(
            2,
            NodeKind::Colocated { participant: 2 },
            NodeCategory::LogicGate,
        )
    };
    let false_end = flow_node(
        4,
        NodeKind::End {
            jump_to_storylet: None,
            precondition_result: false,
        },
        NodeCategory::Mutator,
    );
    let asset = FlowGraphAsset {
        asset_id: "colocation".into(),
        start_node_guid: 1,
        nodes: vec![condition, present, end_node(3), false_end],
        edges: vec![flow_edge(1, "true", 3), flow_edge(1, "false", 4)],
        data_edges: vec![DataEdgeConnection {
            source_node_guid: 2,
            source_pin_name: "value".into(),
            target_node_guid: 1,
            target_pin_name: "input".into(),
        }],
    };
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let ctx = RuntimeContext::new(&mut world, &dispatcher);
    assert!(!FlowGraphInstance::evaluate_precondition_graph_for(&asset, &ctx, 0).unwrap());
    assert!(FlowGraphInstance::evaluate_precondition_graph_for(&asset, &ctx, 1).unwrap());
    ctx.state.participants.location[1] = None;
    ctx.state.participants.location[2] = None;
    assert!(!FlowGraphInstance::evaluate_precondition_graph_for(&asset, &ctx, 1).unwrap());
}
