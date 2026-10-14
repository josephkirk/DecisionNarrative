use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use storylet_engine::{
    ActorState, CandidateRejection, ClusterType, DirectorState, FactRef, LifecycleState, Predicate,
    PredicateOp, PredicateSet, Repeatability, RoleRequirement, SceneContent, StoryletChoice,
    StoryletDefinition, WorldState, evaluate_storylets,
};

fn storylet(id: &str) -> StoryletDefinition {
    StoryletDefinition {
        id: id.into(),
        revision: 1,
        title: format!("{id} title"),
        summary: "A project-owned scene".into(),
        approved: true,
        location_id: Some("archive".into()),
        urgency_weight: 70.0,
        cluster_type: ClusterType::PlayerControlledPassage,
        repeatability: Repeatability::Always,
        required_roles: vec![RoleRequirement {
            role: "clerk".into(),
        }],
        trigger: PredicateSet::default(),
        content: SceneContent {
            opening: "A clerk checks the ledger.".into(),
        },
        choices: vec![StoryletChoice {
            id: "inspect".into(),
            label: "Inspect the ledger".into(),
            requirements: PredicateSet::default(),
            effects: Vec::new(),
            continues_scene: false,
            divert_target: None,
        }],
    }
}

fn world() -> WorldState {
    WorldState {
        current_location: Some("archive".into()),
        qualities: BTreeMap::from([("quality.trust".into(), json!(4))]),
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

#[test]
fn only_approved_matching_storylets_are_eligible_and_bind_generic_cast_roles() {
    let mut approved = storylet("archive_audit");
    approved.trigger.all_of.push(Predicate {
        fact: FactRef::Quality {
            key: "quality.trust".into(),
        },
        op: PredicateOp::Gte,
        value: json!(3),
    });
    let mut draft = storylet("draft_scene");
    draft.approved = false;
    let mut wrong_location = storylet("wrong_location");
    wrong_location.location_id = Some("market".into());

    let results = evaluate_storylets(
        &[approved, draft, wrong_location],
        &world(),
        &DirectorState::default(),
        0.15,
    );

    let selected = results
        .iter()
        .find(|result| result.storylet_id == "archive_audit")
        .unwrap();
    assert!(selected.eligible);
    assert_eq!(selected.bound_roles.get("clerk"), Some(&"nora".to_owned()));
    assert_eq!(
        results
            .iter()
            .find(|result| result.storylet_id == "draft_scene")
            .unwrap()
            .rejection,
        Some(CandidateRejection::NotApproved)
    );
    assert_eq!(
        results
            .iter()
            .find(|result| result.storylet_id == "wrong_location")
            .unwrap()
            .rejection,
        Some(CandidateRejection::WrongLocation)
    );
}

#[test]
fn missing_roles_return_explainable_rejections() {
    let mut definition = storylet("sealed_record");
    definition.required_roles = vec![RoleRequirement {
        role: "archivist".into(),
    }];
    definition.trigger.all_of.push(Predicate {
        fact: FactRef::Quality {
            key: "quality.trust".into(),
        },
        op: PredicateOp::Gte,
        value: json!(9),
    });
    let results = evaluate_storylets(&[definition], &world(), &DirectorState::default(), 0.15);

    assert_eq!(
        results[0].rejection,
        Some(CandidateRejection::MissingRole {
            role: "archivist".into()
        })
    );
}

#[test]
fn cooldown_and_retirement_are_independent_of_content_approval() {
    let definition = storylet("one_shot");
    let director = DirectorState {
        lifecycle: BTreeMap::from([("one_shot".into(), LifecycleState::Retired)]),
        ..DirectorState::default()
    };
    let results = evaluate_storylets(&[definition], &world(), &director, 0.15);
    assert_eq!(results[0].rejection, Some(CandidateRejection::Retired));
}

#[test]
fn relationship_and_witness_knowledge_predicates_use_actor_scoped_state() {
    let mut state = world();
    state.knowledge.insert(
        "actor-b".into(),
        BTreeSet::from(["claim_delivery_seen".into()]),
    );
    state
        .relationships
        .insert("actor-a".into(), BTreeMap::from([("actor-b".into(), 5.0)]));
    let mut definition = storylet("witness_report");
    definition.trigger.all_of.extend([
        Predicate {
            fact: FactRef::Knows {
                actor_id: "actor-b".into(),
                claim_id: "claim_delivery_seen".into(),
            },
            op: PredicateOp::Eq,
            value: json!(true),
        },
        Predicate {
            fact: FactRef::Relationship {
                source_id: "actor-a".into(),
                target_id: "actor-b".into(),
            },
            op: PredicateOp::Gte,
            value: json!(3),
        },
    ]);
    assert!(evaluate_storylets(&[definition], &state, &DirectorState::default(), 0.15)[0].eligible);
}

#[test]
fn seeded_weighted_shuffle_is_reproducible_without_replacement() {
    use storylet_engine::{CandidateEvaluation, seeded_weighted_shuffle};
    let make = || {
        vec![
            CandidateEvaluation {
                storylet_id: "a".into(),
                eligible: true,
                rejection: None,
                bound_roles: Default::default(),
                salience_score: 0.0,
                arc_alignment_score: 0.0,
                final_weight: 1.0,
            },
            CandidateEvaluation {
                storylet_id: "b".into(),
                eligible: true,
                rejection: None,
                bound_roles: Default::default(),
                salience_score: 0.0,
                arc_alignment_score: 0.0,
                final_weight: 5.0,
            },
            CandidateEvaluation {
                storylet_id: "skip".into(),
                eligible: false,
                rejection: Some(storylet_engine::CandidateRejection::NotApproved),
                bound_roles: Default::default(),
                salience_score: 0.0,
                arc_alignment_score: 0.0,
                final_weight: 999.0,
            },
        ]
    };
    let mut left = storylet_engine::WorldState {
        random_seed: 42,
        ..Default::default()
    };
    let mut right = left.clone();
    let a = seeded_weighted_shuffle(&mut left, &make());
    let b = seeded_weighted_shuffle(&mut right, &make());
    assert_eq!(a, b);
    assert_eq!(a.len(), 2);
    assert_ne!(a[0], a[1]);
    assert_eq!(left.random_cursor, right.random_cursor);
}

#[test]
fn failed_trigger_returns_predicate_rejection_after_roles_bind() {
    let mut definition = storylet("not_yet");
    definition.trigger.all_of.push(Predicate {
        fact: FactRef::Quality {
            key: "quality.trust".into(),
        },
        op: PredicateOp::Gte,
        value: json!(9),
    });
    let result = evaluate_storylets(&[definition], &world(), &DirectorState::default(), 0.15);
    assert_eq!(
        result[0].rejection,
        Some(CandidateRejection::TriggerFailed { predicate_index: 0 })
    );
}
