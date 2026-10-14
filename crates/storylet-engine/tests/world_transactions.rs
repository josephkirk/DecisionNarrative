use std::collections::BTreeMap;

use serde_json::json;
use storylet_engine::{Effect, EngineError, WorldCommand, WorldState, apply_command};

fn world() -> WorldState {
    WorldState::default()
}

fn command(id: &str, revision: u64, effects: Vec<Effect>) -> WorldCommand {
    WorldCommand {
        idempotency_key: id.to_owned(),
        expected_revision: revision,
        actor_id: "designer-player".to_owned(),
        action: "test_action".to_owned(),
        effects,
        caused_by: Vec::new(),
        advance_turn: true,
    }
}

#[test]
fn add_and_set_commit_as_one_revisioned_event() {
    let mut state = world();
    let result = apply_command(
        &mut state,
        command(
            "cmd-1",
            0,
            vec![
                Effect::SetQuality {
                    key: "world.open".into(),
                    value: json!(true),
                },
                Effect::AddQuality {
                    key: "resources.food".into(),
                    amount: 3.0,
                },
            ],
        ),
    )
    .unwrap();

    assert_eq!(result.revision, 1);
    assert_eq!(state.turn, 1);
    assert_eq!(state.qualities.get("world.open"), Some(&json!(true)));
    assert_eq!(state.qualities.get("resources.food"), Some(&json!(3.0)));
    assert_eq!(state.events.len(), 1);
    assert_eq!(state.events[0].id, "cmd-1");
}

#[test]
fn duplicate_command_is_idempotent() {
    let mut state = world();
    let cmd = command(
        "cmd-once",
        0,
        vec![Effect::AddQuality {
            key: "stock".into(),
            amount: 4.0,
        }],
    );
    apply_command(&mut state, cmd.clone()).unwrap();
    let result = apply_command(&mut state, cmd).unwrap();

    assert!(result.duplicate);
    assert_eq!(state.qualities.get("stock"), Some(&json!(4.0)));
    assert_eq!(state.revision, 1);
    assert_eq!(state.events.len(), 1);
}

#[test]
fn stale_command_is_rejected_without_mutation() {
    let mut state = world();
    apply_command(&mut state, command("first", 0, vec![])).unwrap();
    let before = state.clone();

    let err = apply_command(
        &mut state,
        command(
            "stale",
            0,
            vec![Effect::SetQuality {
                key: "bad".into(),
                value: json!(1),
            }],
        ),
    )
    .unwrap_err();

    assert_eq!(
        err,
        EngineError::RevisionConflict {
            expected: 0,
            actual: 1
        }
    );
    assert_eq!(state, before);
}

#[test]
fn resource_transfer_conserves_total_and_is_atomic_when_insufficient() {
    let mut state = WorldState {
        qualities: BTreeMap::from([
            ("stock.depot".into(), json!(6.0)),
            ("stock.clinic".into(), json!(2.0)),
        ]),
        ..WorldState::default()
    };
    let transfer = Effect::TransferQuality {
        from: "stock.depot".into(),
        to: "stock.clinic".into(),
        amount: 3.0,
    };
    apply_command(&mut state, command("transfer", 0, vec![transfer])).unwrap();
    assert_eq!(state.qualities.get("stock.depot"), Some(&json!(3.0)));
    assert_eq!(state.qualities.get("stock.clinic"), Some(&json!(5.0)));

    let before = state.clone();
    let err = apply_command(
        &mut state,
        command(
            "too-much",
            1,
            vec![Effect::TransferQuality {
                from: "stock.depot".into(),
                to: "stock.clinic".into(),
                amount: 4.0,
            }],
        ),
    )
    .unwrap_err();
    assert_eq!(
        err,
        EngineError::InsufficientBalance {
            key: "stock.depot".into(),
            available: 3.0,
            requested: 4.0
        }
    );
    assert_eq!(state, before);
}

#[test]
fn invalid_numbers_are_rejected_without_partial_commit() {
    let mut state = world();
    let before = state.clone();
    let err = apply_command(
        &mut state,
        command(
            "nan",
            0,
            vec![
                Effect::AddQuality {
                    key: "first".into(),
                    amount: 2.0,
                },
                Effect::AddQuality {
                    key: "second".into(),
                    amount: f64::NAN,
                },
            ],
        ),
    )
    .unwrap_err();

    assert_eq!(err, EngineError::NonFiniteNumber);
    assert_eq!(state, before);
}

#[test]
fn movement_is_logged_and_npc_actions_do_not_advance_player_turn() {
    let mut state = WorldState {
        current_location: Some("location-a".into()),
        actors: std::collections::BTreeMap::from([(
            "actor-a".into(),
            storylet_engine::ActorState {
                role_tags: Default::default(),
                current_location: "location-a".into(),
                busy: false,
            },
        )]),
        ..WorldState::default()
    };
    let mut player_move = command(
        "player-move",
        0,
        vec![Effect::MovePlayer {
            location_id: "location-b".into(),
        }],
    );
    let player = apply_command(&mut state, player_move.clone()).unwrap();
    assert_eq!(state.current_location.as_deref(), Some("location-b"));
    assert_eq!(player.turn, 1);
    player_move.expected_revision = 1;
    let mut npc_move = command(
        "npc-move",
        1,
        vec![Effect::MoveActor {
            actor_id: "actor-a".into(),
            location_id: "location-c".into(),
        }],
    );
    npc_move.advance_turn = false;
    npc_move.caused_by = vec![player.event.id.clone()];
    let npc = apply_command(&mut state, npc_move).unwrap();
    assert_eq!(state.actors["actor-a"].current_location, "location-c");
    assert_eq!(npc.turn, 1);
    assert_eq!(
        npc.event.actor_location_deltas["actor-a"],
        ("location-a".into(), "location-c".into())
    );
    assert_eq!(npc.event.caused_by, vec![player.event.id]);
}

#[test]
fn invalid_actor_move_and_same_quality_transfer_are_atomic() {
    let mut state = world();
    let before = state.clone();
    let error = apply_command(
        &mut state,
        command(
            "missing-actor",
            0,
            vec![Effect::MoveActor {
                actor_id: "ghost".into(),
                location_id: "harbor".into(),
            }],
        ),
    )
    .unwrap_err();
    assert_eq!(
        error,
        EngineError::ActorNotFound {
            actor_id: "ghost".into()
        }
    );
    assert_eq!(state, before);
    let mut state = WorldState {
        qualities: BTreeMap::from([("stock".into(), json!(7.0))]),
        ..WorldState::default()
    };
    let error = apply_command(
        &mut state,
        command(
            "self-transfer",
            0,
            vec![Effect::TransferQuality {
                from: "stock".into(),
                to: "stock".into(),
                amount: 3.0,
            }],
        ),
    )
    .unwrap_err();
    assert_eq!(error, EngineError::InvalidTransferAmount);
    assert_eq!(state.qualities["stock"], json!(7.0));
    assert_eq!(state.revision, 0);
}

#[test]
fn idempotency_key_cannot_be_reused_for_a_different_action() {
    let mut state = world();
    apply_command(
        &mut state,
        command(
            "same",
            0,
            vec![Effect::AddQuality {
                key: "x".into(),
                amount: 1.0,
            }],
        ),
    )
    .unwrap();
    let err = apply_command(
        &mut state,
        command(
            "same",
            0,
            vec![Effect::AddQuality {
                key: "x".into(),
                amount: 100.0,
            }],
        ),
    )
    .unwrap_err();
    assert_eq!(err, EngineError::IdempotencyKeyReuse);
    assert_eq!(state.qualities["x"], json!(1.0));
}

#[test]
fn event_log_replays_and_forks_at_a_causal_prefix() {
    let initial = WorldState {
        current_location: Some("square".into()),
        ..WorldState::default()
    };
    let mut live = initial.clone();
    let first = apply_command(
        &mut live,
        command(
            "move",
            0,
            vec![Effect::MovePlayer {
                location_id: "market".into(),
            }],
        ),
    )
    .unwrap();
    let mut npc = command(
        "npc",
        1,
        vec![Effect::SetQuality {
            key: "rumor.heard".into(),
            value: json!(true),
        }],
    );
    npc.advance_turn = false;
    npc.caused_by = vec![first.event.id.clone()];
    apply_command(&mut live, npc).unwrap();
    let replay = storylet_engine::replay_events(initial.clone(), &live.events).unwrap();
    assert_eq!(replay, live);
    let branch = storylet_engine::fork_at(initial, &live.events, "move").unwrap();
    assert_eq!(branch.current_location.as_deref(), Some("market"));
    assert!(!branch.qualities.contains_key("rumor.heard"));
}
