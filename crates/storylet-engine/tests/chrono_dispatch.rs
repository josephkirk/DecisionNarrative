use std::collections::{BTreeMap, BTreeSet};

use storylet_engine::chrono_dispatch::{
    CooldownUnit, DispatchError, PredicateOperator, PredicateSubject, QualityPredicate,
    RoleRequirement, StoryletDef, UnifiedDispatcher, WaypointTopicEdge, WaypointTopicGraph,
    WaypointTopicNode,
};
use storylet_engine::chrono_state::{
    GameplayTagRegistry, MetricRegistry, TagBitset, WorldStateTable,
};

fn tag_registry(names: &[&str]) -> GameplayTagRegistry {
    GameplayTagRegistry::compile(names.iter().map(|name| (*name).to_owned())).unwrap()
}

fn metric_registry(names: &[&str]) -> MetricRegistry {
    MetricRegistry::compile(names.iter().map(|name| (*name).to_owned())).unwrap()
}

fn predicate(
    target_id: &str,
    subject: PredicateSubject,
    operator: PredicateOperator,
    value: f64,
) -> QualityPredicate {
    QualityPredicate {
        target_id: target_id.to_owned(),
        subject,
        operator,
        comparison_value: value,
        specificity_weight: 1.0,
        entity_index: None,
        target_index: None,
        tag_mask: TagBitset::new(),
    }
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

// Independent any-of groups: a match inside one group never satisfies another.
#[test]
fn any_of_groups_stay_independent() {
    let tags = tag_registry(&["mood.curious", "mood.curious.deep", "mood.wary"]);
    let metrics = MetricRegistry::empty();

    let mut def = StoryletDef::default();
    def.storylet_id = 1;
    def.any_of_precondition_groups = vec![
        vec![
            predicate(
                "mood.curious",
                PredicateSubject::Location,
                PredicateOperator::Eq,
                0.0,
            ),
            predicate(
                "mood.curious.deep",
                PredicateSubject::Location,
                PredicateOperator::Eq,
                0.0,
            ),
        ],
        vec![predicate(
            "mood.wary",
            PredicateSubject::Location,
            PredicateOperator::Eq,
            0.0,
        )],
    ];

    let mut dispatcher = UnifiedDispatcher::new(vec![def]);
    dispatcher.resolve(&tags, &metrics).unwrap();
    let def = dispatcher.storylet(1).unwrap();

    let mut table = WorldStateTable::new();
    // The second alternative inside the first group satisfies that group alone.
    tags.add_hierarchical(
        &mut table.global_tags,
        tags.id("mood.curious.deep").unwrap(),
    )
    .unwrap();
    assert!(!dispatcher.is_eligible(def, &table));

    tags.add_hierarchical(&mut table.global_tags, tags.id("mood.wary").unwrap())
        .unwrap();
    assert!(dispatcher.is_eligible(def, &table));
}

// Metric names and tag names are two index spaces; the same name resolves differently.
#[test]
fn metric_and_tag_names_resolve_in_separate_index_spaces() {
    let tags = tag_registry(&["arc.prologue", "quality.curiosity"]);
    let metrics = metric_registry(&["quality.curiosity"]);

    let mut def = StoryletDef::default();
    def.storylet_id = 2;
    def.preconditions = vec![
        predicate(
            "quality.curiosity",
            PredicateSubject::Metric,
            PredicateOperator::Gte,
            2.0,
        ),
        predicate(
            "quality.curiosity",
            PredicateSubject::Location,
            PredicateOperator::Eq,
            0.0,
        ),
    ];

    let mut dispatcher = UnifiedDispatcher::new(vec![def]);
    dispatcher.resolve(&tags, &metrics).unwrap();
    let def = dispatcher.storylet(2).unwrap();

    // Sorted-name order puts `arc.prologue` first, so the tag id is 1 while the
    // metric index for the same name is 0.
    assert_eq!(tags.id("quality.curiosity"), Some(1));
    assert_eq!(metrics.id("quality.curiosity"), Some(0));
    assert_eq!(def.preconditions[0].target_index, Some(0));
    assert!(def.preconditions[0].tag_mask.is_empty());
    assert_eq!(def.preconditions[1].target_index, Some(1));
    assert!(def.preconditions[1].tag_mask.bit(1));

    let mut table = WorldStateTable::new();
    table.metrics[0] = 3.0;
    assert!(
        dispatcher
            .evaluate_predicate(&def.preconditions[0], &table)
            .unwrap()
    );
    assert!(
        !dispatcher
            .evaluate_predicate(&def.preconditions[1], &table)
            .unwrap()
    );

    tags.add_hierarchical(&mut table.global_tags, 1).unwrap();
    assert!(
        dispatcher
            .evaluate_predicate(&def.preconditions[1], &table)
            .unwrap()
    );

    // A metric name that is only in the tag registry is a typed error, never a
    // silent index.
    let mut stray = StoryletDef::default();
    stray.storylet_id = 3;
    stray.preconditions = vec![predicate(
        "arc.prologue",
        PredicateSubject::Metric,
        PredicateOperator::Eq,
        0.0,
    )];
    let mut other = UnifiedDispatcher::new(vec![stray]);
    assert_eq!(
        other.resolve(&tags, &metrics),
        Err(DispatchError::UnresolvedMetric {
            target_id: "arc.prologue".to_owned()
        })
    );
}

// Hierarchical matching for the location, visited, and met-character subjects.
#[test]
fn location_visited_and_met_matching_is_hierarchical() {
    let tags = tag_registry(&[
        "character",
        "character.ledgerkeeper",
        "location",
        "location.archive",
    ]);
    let metrics = MetricRegistry::empty();
    let keeper = tags.id("character.ledgerkeeper").unwrap();
    let archive = tags.id("location.archive").unwrap();

    let mut entity_rule = predicate(
        "location",
        PredicateSubject::Entity,
        PredicateOperator::Eq,
        0.0,
    );
    entity_rule.entity_index = Some(7);
    // Entity-only membership form: no tag name, so no closure.
    let mut visited_entity_rule =
        predicate("", PredicateSubject::Visited, PredicateOperator::Eq, 0.0);
    visited_entity_rule.entity_index = Some(9);

    let mut def = StoryletDef::default();
    def.storylet_id = 4;
    def.preconditions = vec![
        predicate(
            "location",
            PredicateSubject::Location,
            PredicateOperator::Eq,
            0.0,
        ),
        predicate(
            "location",
            PredicateSubject::Visited,
            PredicateOperator::Eq,
            0.0,
        ),
        predicate(
            "character",
            PredicateSubject::MetCharacter,
            PredicateOperator::Eq,
            0.0,
        ),
        entity_rule,
        visited_entity_rule,
    ];

    let mut dispatcher = UnifiedDispatcher::new(vec![def]);
    dispatcher.resolve(&tags, &metrics).unwrap();
    let def = dispatcher.storylet(4).unwrap();

    let mut table = WorldStateTable::new();
    // Absence first: with nothing present, `eq` rejects and `neq` accepts.
    for index in 0..def.preconditions.len() {
        assert!(
            !dispatcher
                .evaluate_predicate(&def.preconditions[index], &table)
                .unwrap()
        );
    }
    let mut not_location = StoryletDef::default();
    not_location.storylet_id = 5;
    not_location.preconditions.push(predicate(
        "location",
        PredicateSubject::Location,
        PredicateOperator::Neq,
        0.0,
    ));
    let mut not_location_dispatcher = UnifiedDispatcher::new(vec![not_location]);
    not_location_dispatcher.resolve(&tags, &metrics).unwrap();
    assert!(
        not_location_dispatcher
            .evaluate_predicate(
                &not_location_dispatcher.storylets[0].preconditions[0],
                &table
            )
            .unwrap()
    );

    // Each rule matches through the ancestor closure of its parent, not the exact bit:
    // tags are added hierarchically (REQ-DOD-03).
    tags.add_hierarchical(&mut table.global_tags, archive)
        .unwrap();
    tags.add_hierarchical(&mut table.entity_tags[7], archive)
        .unwrap();
    tags.add_hierarchical(&mut table.visited_location_tags.tag_mask, archive)
        .unwrap();
    tags.add_hierarchical(&mut table.met_character_tags.tag_mask, keeper)
        .unwrap();
    table.visited_location_tags.entity_mask.set_bit(9).unwrap();

    for index in 0..def.preconditions.len() {
        assert!(
            dispatcher
                .evaluate_predicate(&def.preconditions[index], &table)
                .unwrap(),
            "predicate index {index}: {:?}",
            def.preconditions[index]
        );
    }
    // A range operator on a tag subject is a typed error.
    let mut range_def = StoryletDef::default();
    range_def.storylet_id = 6;
    range_def.preconditions.push(predicate(
        "location",
        PredicateSubject::Location,
        PredicateOperator::Gt,
        0.0,
    ));
    let mut range_dispatcher = UnifiedDispatcher::new(vec![range_def]);
    range_dispatcher.resolve(&tags, &metrics).unwrap();
    assert_eq!(
        range_dispatcher
            .evaluate_predicate(&range_dispatcher.storylets[0].preconditions[0], &table),
        Err(DispatchError::NonNumericSubject {
            target_id: "location".to_owned(),
            subject: PredicateSubject::Location,
            operator: PredicateOperator::Gt,
        })
    );
}

// Distinct role bindings, including the adapter-written current-location marker.
#[test]
fn role_binding_never_shares_an_actor_and_honours_the_current_marker() {
    let tags = tag_registry(&["location.current", "role.clerk", "role.courier"]);
    let metrics = MetricRegistry::empty();
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
    dispatcher.resolve(&tags, &metrics).unwrap();
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
        .resolve(&GameplayTagRegistry::empty(), &MetricRegistry::empty())
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

// TEST_SALIENCE_MONTE_CARLO: 5,000 deterministic trajectories over a 100-storylet
// salience pool. Gates are the compiled masks the adapter would produce; the pool is
// weighted so the highest-weight eligible storylet always wins, which makes the arc
// gate observable instead of vacuous.
const TRAJECTORIES: usize = 5_000;
const POOL: usize = 100;
const FALLBACK_ID: usize = 99;
const PHASE_BITS: [u16; 3] = [200, 201, 202];

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf03_8538_4dc2_d6fc);
    z = (z ^ (z >> 27)).wrapping_mul(0x85eb_ca6b_1b31_10d2);
    z ^ (z >> 31)
}

/// Act 3 premature endings are 60..98; 20..39 are Act 1, 40..59 are Act 2, and 99 is
/// the always-eligible fallback that prevents starvation.
fn phase_group(id: usize) -> usize {
    if (60..FALLBACK_ID).contains(&id) {
        3
    } else if (40..60).contains(&id) {
        2
    } else if (20..40).contains(&id) {
        1
    } else {
        0
    }
}

fn specificity_base(phase: usize) -> f64 {
    match phase {
        1 => 100.0,
        2 => 200.0,
        3 => 1000.0,
        _ => 1.0,
    }
}

fn monte_carlo_pool() -> Vec<StoryletDef> {
    let mut pool = Vec::new();
    for id in 0..POOL {
        let phase = phase_group(id);
        let mut def = StoryletDef::default();
        def.storylet_id = id as u16;
        def.display_priority = id as i32;
        if id == FALLBACK_ID {
            pool.push(def);
            continue;
        }
        let mut gate = TagBitset::new();
        gate.set_bit(id as u16).unwrap();
        def.required_tags = gate;
        if phase > 0 {
            def.required_arc_phases
                .set_bit(PHASE_BITS[phase - 1])
                .unwrap();
        }
        let mut rule = predicate(
            &format!("state.{id}"),
            PredicateSubject::Location,
            PredicateOperator::Eq,
            0.0,
        );
        rule.specificity_weight = specificity_base(phase) + id as f64;
        rule.target_index = Some(id as u16);
        rule.tag_mask = gate;
        def.preconditions = vec![rule];
        pool.push(def);
    }
    pool
}

struct MonteCarloRun {
    activations: Vec<usize>,
    starves: usize,
    act_three_out_of_phase: usize,
    act_three_firings: usize,
}

fn run_monte_carlo(seed: u64) -> MonteCarloRun {
    let dispatcher = UnifiedDispatcher::new(monte_carlo_pool());
    let completed = BTreeSet::new();
    let deck: Vec<u16> = (0..POOL as u16).collect();
    let mut run = MonteCarloRun {
        activations: vec![0; POOL],
        starves: 0,
        act_three_out_of_phase: 0,
        act_three_firings: 0,
    };
    let mut rng = seed;
    for _ in 0..TRAJECTORIES {
        let phase = (splitmix64(&mut rng) % 3) as usize + 1;
        let mut table = WorldStateTable::new();
        table.global_tags.set_bit(PHASE_BITS[phase - 1]).unwrap();
        for _ in 0..4 {
            let bit = (splitmix64(&mut rng) % POOL as u64) as u16;
            table.global_tags.set_bit(bit).unwrap();
        }
        // One gate that matches the current phase, so the arc gate is exercised rather
        // than only rejecting.
        let guaranteed = match phase {
            3 => 60 + (splitmix64(&mut rng) % 39) as usize,
            2 => 40 + (splitmix64(&mut rng) % 20) as usize,
            _ => 20 + (splitmix64(&mut rng) % 20) as usize,
        };
        table.global_tags.set_bit(guaranteed as u16).unwrap();

        let Some(winner) = dispatcher.match_most_salient(&deck, &table, &completed, rng) else {
            run.starves += 1;
            continue;
        };
        run.activations[winner as usize] += 1;
        let winner_phase = phase_group(winner as usize);
        if winner_phase == 3 {
            run.act_three_firings += 1;
            if phase != 3 {
                run.act_three_out_of_phase += 1;
            }
        }
        assert_eq!(
            winner_phase, phase,
            "trajectory picked a storylet outside its phase"
        );
    }
    run
}

#[test]
fn salience_monte_carlo_keeps_act_three_out_of_acts_one_and_two() {
    let run = run_monte_carlo(0xC0FF_EE11_2233_4455);

    assert_eq!(
        run.act_three_out_of_phase, 0,
        "premature Act 3 ending fired early"
    );
    assert_eq!(
        run.starves, 0,
        "fallback default did not prevent starvation"
    );
    assert!(run.act_three_firings > 0, "Act 3 storylets never fired");
    assert_eq!(run.activations.iter().sum::<usize>(), TRAJECTORIES);

    // The same seed reproduces the same histogram, so the run is reproducible.
    assert_eq!(
        run_monte_carlo(0xC0FF_EE11_2233_4455).activations,
        run.activations
    );

    eprintln!("TEST_SALIENCE_MONTE_CARLO: {TRAJECTORIES} trajectories over {POOL} storylets");
    for phase in 0..4 {
        let group: Vec<usize> = (0..POOL).filter(|id| phase_group(*id) == phase).collect();
        let zero: Vec<usize> = group
            .iter()
            .filter(|id| run.activations[**id] == 0)
            .copied()
            .collect();
        eprintln!(
            "  phase {phase}: {} storylets, {} activations, {} orphans {:?}",
            group.len(),
            group.iter().map(|id| run.activations[*id]).sum::<usize>(),
            zero.len(),
            zero
        );
    }
}
