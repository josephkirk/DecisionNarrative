use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use storylet_engine::{
    ActorState, CandidateRejection, ClusterType, DirectorState, Effect, LifecycleState,
    PredicateSet, Repeatability, RoleRequirement, SceneContent, StoryletChoice, StoryletDefinition,
    WorldState, evaluate_storylets, execute_storylet_choice, start_storylet,
};

fn world() -> WorldState {
    WorldState {
        current_location: Some("archive".into()),
        location_tags: BTreeMap::from([("archive".into(), BTreeSet::from(["archive".into()]))]),
        qualities: BTreeMap::from([
            ("stock.depot".into(), json!(5)),
            ("stock.clinic".into(), json!(1)),
        ]),
        actors: BTreeMap::from([(
            "nora".into(),
            ActorState {
                role_tags: BTreeSet::from(["clerk".into()]),
                current_location: "archive".into(),
                busy: false,
            },
        )]),
        ..WorldState::default()
    }
}

fn definition(repeatability: Repeatability) -> StoryletDefinition {
    StoryletDefinition {
        id: "clinic_record".into(),
        revision: 1,
        title: "A ledger at the clinic".into(),
        summary: "A generic authored incident".into(),
        approved: true,
        ending: false,
        location_tags: BTreeSet::from(["archive".into()]),
        urgency_weight: 60.0,
        cluster_type: ClusterType::Linear,
        repeatability,
        required_roles: vec![RoleRequirement {
            role: "clerk".into(),
        }],
        trigger: PredicateSet::default(),
        preconditions: Default::default(),
        content: SceneContent {
            opening: "Nora turns the ledger around.".into(),
        },
        choices: vec![StoryletChoice {
            id: "send_supplies".into(),
            label: "Send supplies".into(),
            requirements: PredicateSet::default(),
            effects: vec![Effect::TransferQuality {
                from: "stock.depot".into(),
                to: "stock.clinic".into(),
                amount: 2.0,
            }],
            continues_scene: false,
            divert_target: None,
        }],
    }
}

#[test]
fn start_reserves_bound_roles_and_choice_atomically_completes_once_storylet() {
    let definition = definition(Repeatability::OnceEver);
    let mut world = world();
    let mut director = DirectorState::default();
    let candidate = evaluate_storylets(&[definition.clone()], &world, &director, 0.15).remove(0);

    let instance = start_storylet(
        &definition,
        &candidate,
        &mut world,
        &mut director,
        "instance-1",
    )
    .unwrap();
    assert_eq!(instance.bound_roles.get("clerk"), Some(&"nora".to_owned()));
    assert_eq!(
        director.role_reservations.get("nora"),
        Some(&"instance-1".to_owned())
    );

    let other = StoryletDefinition {
        id: "other".into(),
        ..definition.clone()
    };
    let rejected = evaluate_storylets(&[other], &world, &director, 0.15);
    assert_eq!(
        rejected[0].rejection,
        Some(CandidateRejection::RoleReserved {
            role: "clerk".into()
        })
    );

    let result = execute_storylet_choice(
        &definition,
        "instance-1",
        "send_supplies",
        "cmd-choice-1",
        &mut world,
        &mut director,
    )
    .unwrap();
    assert_eq!(result.event.deltas.len(), 2);
    assert_eq!(world.qualities.get("stock.depot"), Some(&json!(3.0)));
    assert_eq!(world.qualities.get("stock.clinic"), Some(&json!(3.0)));
    assert_eq!(
        director.lifecycle.get("clinic_record"),
        Some(&LifecycleState::Retired)
    );
    assert!(!director.role_reservations.contains_key("nora"));
    assert!(director.instances["instance-1"].completed);
}

#[test]
fn direct_choice_retry_reuses_the_original_causal_parents_after_completion() {
    let definition = definition(Repeatability::OnceEver);
    let mut world = world();
    let mut director = DirectorState::default();
    let candidate = evaluate_storylets(&[definition.clone()], &world, &director, 0.15).remove(0);
    start_storylet(
        &definition,
        &candidate,
        &mut world,
        &mut director,
        "instance-1",
    )
    .unwrap();
    let first = execute_storylet_choice(
        &definition,
        "instance-1",
        "send_supplies",
        "cmd-choice-retry",
        &mut world,
        &mut director,
    )
    .unwrap();
    let before_retry = world.clone();

    let retry = execute_storylet_choice(
        &definition,
        "instance-1",
        "send_supplies",
        "cmd-choice-retry",
        &mut world,
        &mut director,
    )
    .unwrap();

    assert!(!first.duplicate);
    assert!(retry.duplicate);
    assert_eq!(retry.event, first.event);
    assert_eq!(world, before_retry);
}

#[test]
fn choice_failure_keeps_scene_active_and_resource_balances_unchanged() {
    let mut definition = definition(Repeatability::Cooldown { turns: 2 });
    definition.choices[0].effects = vec![Effect::TransferQuality {
        from: "stock.depot".into(),
        to: "stock.clinic".into(),
        amount: 10.0,
    }];
    let mut world = world();
    let mut director = DirectorState::default();
    let candidate = evaluate_storylets(&[definition.clone()], &world, &director, 0.15).remove(0);
    start_storylet(
        &definition,
        &candidate,
        &mut world,
        &mut director,
        "instance-1",
    )
    .unwrap();
    let before = world.clone();

    assert!(
        execute_storylet_choice(
            &definition,
            "instance-1",
            "send_supplies",
            "cmd-choice-1",
            &mut world,
            &mut director
        )
        .is_err()
    );
    assert_eq!(world, before);
    assert!(director.role_reservations.contains_key("nora"));
    assert!(!director.instances["instance-1"].completed);
}

#[test]
fn cooldown_is_set_after_choice_and_reopens_at_boundary() {
    let definition = definition(Repeatability::Cooldown { turns: 2 });
    let mut world = world();
    let mut director = DirectorState::default();
    let candidate = evaluate_storylets(&[definition.clone()], &world, &director, 0.15).remove(0);
    start_storylet(
        &definition,
        &candidate,
        &mut world,
        &mut director,
        "instance-1",
    )
    .unwrap();
    execute_storylet_choice(
        &definition,
        "instance-1",
        "send_supplies",
        "cmd-choice-1",
        &mut world,
        &mut director,
    )
    .unwrap();

    assert_eq!(
        director.lifecycle.get("clinic_record"),
        Some(&LifecycleState::CoolingDown { until_turn: 3 })
    );
    assert_eq!(
        evaluate_storylets(&[definition.clone()], &world, &director, 0.15)[0].rejection,
        Some(CandidateRejection::CoolingDown { until_turn: 3 })
    );
    world.turn = 3;
    assert!(evaluate_storylets(&[definition], &world, &director, 0.15)[0].eligible);
}
