use std::collections::{BTreeMap, BTreeSet};

use storylet_engine::chrono_dispatch::{
    CooldownUnit, DispatchError, RoleRequirement, StoryletDef, UnifiedDispatcher,
    WaypointTopicEdge, WaypointTopicGraph, WaypointTopicNode,
};
use storylet_engine::chrono_state::{GameplayTagRegistry, WorldStateTable};

fn tag_registry(names: &[&str]) -> GameplayTagRegistry {
    GameplayTagRegistry::compile(names.iter().map(|name| (*name).to_owned())).unwrap()
}


fn node(topic_id: &str) -> WaypointTopicNode {
    WaypointTopicNode {
        topic_id: topic_id.to_owned(),
        ..Default::default()
    }
}

fn edge(edge_id: &str, from: &str, to: &str, base_cost: f64, fallback: bool) -> WaypointTopicEdge {
    WaypointTopicEdge {
        edge_id: edge_id.to_owned(),
        from_topic_id: from.to_owned(),
        to_topic_id: to.to_owned(),
        base_cost,
        used_penalty_cost: 100.0,
        dialogue_transition_asset_id: String::new(),
        is_fallback_default: fallback,
    }
}


// Distinct role bindings, including the adapter-written current-location marker.
#[test]
fn role_binding_never_shares_an_actor_and_honours_the_current_marker() {
    let tags = tag_registry(&["location.current", "role.clerk", "role.courier"]);
    let clerk = tags.id("role.clerk").unwrap();
    let courier = tags.id("role.courier").unwrap();
    let current = tags.id("location.current").unwrap();

    let mut def = StoryletDef::default();
    def.storylet_id = 5;
    def.required_roles = vec![
        RoleRequirement {
            role_tag: "role.clerk".to_owned(),
            location_tag: Some("location.current".to_owned()),
            ..Default::default()
        },
        RoleRequirement {
            role_tag: "role.courier".to_owned(),
            ..Default::default()
        },
    ];

    let mut dispatcher = UnifiedDispatcher::new(vec![def]);
    dispatcher.resolve(&tags).unwrap();
    let def = dispatcher.storylet(5).unwrap();

    let mut table = WorldStateTable::new();
    // Row 1 is the only clerk at the current location; row 2 is a clerk elsewhere.
    for row in [1usize, 2] {
        table.entity_tags[row].set_bit(clerk).unwrap();
    }
    table.entity_tags[1].set_bit(current).unwrap();
    table.entity_tags[3].set_bit(courier).unwrap();

    let bindings = dispatcher.bind_roles(def, &table).unwrap();
    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].entity_index, 1);
    assert_eq!(bindings[1].entity_index, 3);
    assert!(bindings[0].entity_index != bindings[1].entity_index);
    assert!(dispatcher.is_eligible(def, &table));

    // Reserving the only eligible clerk is a typed error, not a shared actor.
    let busy = BTreeSet::from([1usize]);
    assert_eq!(
        dispatcher.bind_roles_excluding(def, &table, &busy),
        Err(DispatchError::NoDistinctActorForRole {
            role_tag: "role.clerk".to_owned()
        })
    );
}

// Turn and millisecond cooldowns stay separate; declaring both is a typed conflict.
#[test]
fn cooldown_units_are_kept_separate_and_conflicts_are_typed() {
    let dispatcher = UnifiedDispatcher::new(Vec::new());

    assert_eq!(
        dispatcher.cooldown_unit(&StoryletDef {
            storylet_id: 6,
            cooldown_ms: 1000,
            cooldown_turns: 2,
            ..Default::default()
        }),
        Err(DispatchError::CooldownUnitConflict {
            storylet_id: 6,
            cooldown_ms: 1000,
            cooldown_turns: 2,
        })
    );
    assert_eq!(
        dispatcher.cooldown_unit(&StoryletDef {
            storylet_id: 7,
            cooldown_ms: 1000,
            ..Default::default()
        }),
        Ok(Some(CooldownUnit::Milliseconds))
    );
    assert_eq!(
        dispatcher.cooldown_unit(&StoryletDef {
            storylet_id: 8,
            cooldown_turns: 3,
            ..Default::default()
        }),
        Ok(Some(CooldownUnit::Turns))
    );
    assert_eq!(dispatcher.cooldown_unit(&StoryletDef::default()), Ok(None));
}

// Deterministic deck order, deterministic tie resolution, and completion locks.
#[test]
fn qbn_deck_order_and_salience_ties_are_deterministic() {
    let mut low = StoryletDef::default();
    low.storylet_id = 1;
    low.display_priority = 3;
    low.is_repeatable = false;
    let mut high = StoryletDef::default();
    high.storylet_id = 2;
    high.display_priority = 5;

    // Salience candidates must declare an arc-phase mask (REQ-BB-05).
    low.required_arc_phases.set_bit(0).unwrap();
    high.required_arc_phases.set_bit(0).unwrap();

    let mut dispatcher = UnifiedDispatcher::new(vec![low, high]);
    dispatcher
        .resolve(&GameplayTagRegistry::empty())
        .unwrap();

    let mut table = WorldStateTable::new();
    table.global_tags.set_bit(0).unwrap();
    let completed = BTreeSet::new();

    // Duplicate IDs appear once; the result is priority order, then ID order.
    assert_eq!(
        dispatcher.query_qbn_deck(&[1, 2, 1], &table, &completed),
        vec![2u16, 1]
    );

    // Both candidates score zero, so the tie is resolved from the seed and repeats.
    let first = dispatcher
        .match_most_salient(&[1, 2], &table, &completed, 0x5eed)
        .unwrap();
    assert!(first == 1 || first == 2);
    assert_eq!(
        dispatcher.match_most_salient(&[1, 2], &table, &completed, 0x5eed),
        Some(first)
    );
    let picks: BTreeSet<u16> = (0..64u64)
        .filter_map(|seed| dispatcher.match_most_salient(&[1, 2], &table, &completed, seed))
        .collect();
    assert_eq!(picks, BTreeSet::from([1u16, 2]));

    // Locking the non-repeatable storylet removes it from both paths.
    let mut locked = BTreeSet::new();
    locked.insert(1);
    assert_eq!(
        dispatcher.query_qbn_deck(&[1, 2], &table, &locked),
        vec![2u16]
    );
    assert_eq!(
        dispatcher.match_most_salient(&[1, 2], &table, &locked, 0x5eed),
        Some(2)
    );
}

// Waypoint pathfinding: equal costs break on edge ID, and the same input replays.
#[test]
fn waypoint_ties_resolve_by_edge_id_and_replay_identically() {
    let dispatcher = UnifiedDispatcher::new(Vec::new());
    let graph = WaypointTopicGraph {
        nodes: vec![node("start"), node("goal")],
        edges: vec![
            edge("b", "start", "goal", 1.0, false),
            edge("a", "start", "goal", 1.0, false),
        ],
    };

    let counts = BTreeMap::new();
    let step = dispatcher
        .compute_next_npc_topic_step(&graph, "start", "goal", &counts)
        .unwrap();
    assert_eq!(step.edge_id, "a");
    assert_eq!(step.cost, 1.0);
    assert!(!step.via_fallback);
    assert_eq!(
        dispatcher.compute_next_npc_topic_step(&graph, "start", "goal", &counts),
        Ok(step.clone())
    );

    // One recorded traversal makes the other equal-cost edge cheaper.
    let counts = BTreeMap::from([("a".to_string(), 1u64)]);
    let step = dispatcher
        .compute_next_npc_topic_step(&graph, "start", "goal", &counts)
        .unwrap();
    assert_eq!(step.edge_id, "b");
    assert_eq!(step.cost, 1.0);

    // Exhausted normal routes fall back to the authored default edge.
    let exhausted = WaypointTopicGraph {
        nodes: vec![node("start"), node("goal"), node("hall")],
        edges: vec![edge("f", "start", "hall", 2.0, true)],
    };
    let step = dispatcher
        .compute_next_npc_topic_step(&exhausted, "start", "goal", &BTreeMap::new())
        .unwrap();
    assert!(step.via_fallback);
    assert_eq!(step.edge_id, "f");

    assert_eq!(
        dispatcher.compute_next_npc_topic_step(&graph, "start", "missing", &counts),
        Err(DispatchError::TopicNotFound {
            topic_id: "missing".to_owned()
        })
    );
    assert_eq!(
        dispatcher.compute_next_npc_topic_step(
            &WaypointTopicGraph {
                nodes: vec![node("start"), node("goal")],
                edges: vec![edge("c", "start", "goal", -1.0, false)],
            },
            "start",
            "goal",
            &counts
        ),
        Err(DispatchError::InvalidEdgeCost {
            edge_id: "c".to_owned(),
            cost: -1.0,
        })
    );
}

