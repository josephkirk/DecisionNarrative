use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use storylet_engine::{
    ActorState, Effect, FactRef, NpcBehaviorRule, Predicate, PredicateOp, PredicateSet,
    WorldCommand, WorldState, advance_npcs,
};

#[test]
fn eligible_npc_rule_commits_a_causal_event_without_advancing_an_extra_turn() {
    let mut world = WorldState {
        actors: BTreeMap::from([(
            "mira".into(),
            ActorState {
                role_tags: BTreeSet::from(["porter".into()]),
                current_location: "market".into(),
                busy: false,
            },
        )]),
        qualities: BTreeMap::from([("world.delivery_pending".into(), json!(true))]),
        ..WorldState::default()
    };
    let player_event = storylet_engine::apply_command(
        &mut world,
        WorldCommand {
            idempotency_key: "player:inspect-notice".into(),
            expected_revision: 0,
            actor_id: "player".into(),
            action: "inspect_notice".into(),
            effects: vec![],
            caused_by: vec![],
            advance_turn: true,
        },
    )
    .unwrap()
    .event;
    let rules = vec![NpcBehaviorRule {
        id: "deliver_supplies".into(),
        actor_id: "mira".into(),
        priority: 10,
        conditions: PredicateSet {
            all_of: vec![Predicate {
                fact: FactRef::Quality {
                    key: "world.delivery_pending".into(),
                },
                op: PredicateOp::Eq,
                value: json!(true),
            }],
            any_of: vec![],
        },
        action: "deliver_supplies".into(),
        effects: vec![
            Effect::SetQuality {
                key: "world.delivery_pending".into(),
                value: json!(false),
            },
            Effect::AddQuality {
                key: "stock.clinic".into(),
                amount: 2.0,
            },
        ],
    }];

    let report = advance_npcs(&mut world, &rules, 4, &[player_event.id.clone()]).unwrap();

    assert_eq!(report.events.len(), 1);
    assert_eq!(world.turn, 1);
    assert_eq!(
        world.qualities.get("world.delivery_pending"),
        Some(&json!(false))
    );
    assert_eq!(world.qualities.get("stock.clinic"), Some(&json!(2.0)));
    assert_eq!(report.events[0].caused_by, vec![player_event.id]);
    assert_eq!(report.events[0].actor_id, "mira");
    assert!(!report.events[0].advanced_turn);
}

#[test]
fn one_npc_acts_at_most_once_per_turn_and_rule_order_is_stable() {
    let mut world = WorldState {
        actors: BTreeMap::from([(
            "mira".into(),
            ActorState {
                role_tags: BTreeSet::new(),
                current_location: "market".into(),
                busy: false,
            },
        )]),
        ..WorldState::default()
    };
    let rules = ["z_rule", "a_rule"].map(|id| NpcBehaviorRule {
        id: id.into(),
        actor_id: "mira".into(),
        priority: 1,
        conditions: PredicateSet::default(),
        action: id.into(),
        effects: vec![Effect::AddQuality {
            key: format!("actions.{id}"),
            amount: 1.0,
        }],
    });

    let report = advance_npcs(&mut world, &rules, 4, &[]).unwrap();

    assert_eq!(report.events.len(), 1);
    assert_eq!(report.events[0].action, "a_rule");
    assert_eq!(world.qualities.get("actions.a_rule"), Some(&json!(1.0)));
    assert!(!world.qualities.contains_key("actions.z_rule"));
}

#[test]
fn cascade_limit_is_reported_without_running_more_npc_rules() {
    let mut world = WorldState {
        actors: BTreeMap::from([
            (
                "a".into(),
                ActorState {
                    role_tags: BTreeSet::new(),
                    current_location: "x".into(),
                    busy: false,
                },
            ),
            (
                "b".into(),
                ActorState {
                    role_tags: BTreeSet::new(),
                    current_location: "x".into(),
                    busy: false,
                },
            ),
        ]),
        ..WorldState::default()
    };
    let rules = ["a", "b"].map(|actor| NpcBehaviorRule {
        id: format!("rule_{actor}"),
        actor_id: actor.into(),
        priority: 1,
        conditions: PredicateSet::default(),
        action: "act".into(),
        effects: vec![],
    });

    let report = advance_npcs(&mut world, &rules, 1, &[]).unwrap();

    assert_eq!(report.events.len(), 1);
    assert_eq!(report.limit_reached, true);
    assert_eq!(world.events.len(), 1);
}
