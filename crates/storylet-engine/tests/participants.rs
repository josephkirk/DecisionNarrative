use storylet_engine::chrono_participants::{
    Controller, KnowKind, KnowledgeRow, KnowledgeTable, ParticipantTable,
};
use storylet_engine::chrono_state::{revert_delta, BijectiveDelta, TagBitset, WorldStateTable};

fn row(holder: u16, fact: u16, kind: KnowKind, source_event: u32) -> KnowledgeRow {
    KnowledgeRow { holder, fact, kind, source_event }
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
    assert_eq!(serde_json::from_str::<ParticipantTable>(&json).unwrap(), table);
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
    assert_eq!(world.participants.location[0], world.current_location_entity);
}

#[test]
fn ensure_participants_normalizes_uneven_columns() {
    let mut world = WorldStateTable::new();
    world.participants.push("a", 0, Controller::Human { seat: 0 }, None);
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
    let keys: Vec<_> = world.knowledge.rows.iter().map(|r| (r.holder, r.fact)).collect();
    assert_eq!(keys, vec![(0, 9), (1, 2)]);
    assert_eq!(world.knowledge.knows(0, 9).unwrap().kind, KnowKind::Rumor);
}
