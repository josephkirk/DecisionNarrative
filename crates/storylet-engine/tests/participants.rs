use storylet_engine::chrono_participants::{
    Controller, KnowKind, KnowledgeRow, KnowledgeTable, ParticipantTable,
};

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
