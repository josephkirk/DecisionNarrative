use std::collections::BTreeMap;

use serde_json::json;
use storylet_engine::{
    CharacterDefinition, ClusterType, DecisionResult, Effect, FactRef, LocationDefinition,
    NpcBehaviorRule, Predicate, PredicateOp, PredicateSet, ProjectBundle, QualityDefinition,
    QualityType, RankedStorylet, RankingHead, Repeatability, RoleRequirement, ScenarioDefinition, EventDefinition, EventType,
    SceneContent, SemanticCandidateEmbedding, SemanticRankingRequest, StoryletChoice,
    StoryletDefinition, StoryletRuntime, WorldCommand,
};

fn project() -> ProjectBundle {
    let mut project = ProjectBundle::empty("glass-town", "Glass Town");
    project.qualities.insert(
        "evidence.receipt_found".into(),
        QualityDefinition {
            key: "evidence.receipt_found".into(),
            kind: QualityType::Custom,
            default_value: json!(false),
            description: "Whether the missing receipt has been found".into(),
        },
    );
    project.qualities.insert(
        "evidence.read".into(),
        QualityDefinition {
            key: "evidence.read".into(),
            kind: QualityType::Custom,
            default_value: json!(false),
            description: "Whether the receipt has been read".into(),
        },
    );
    for (id, name) in [("market", "Covered Market"), ("archive", "Night Archive")] {
        project.locations.insert(
            id.into(),
            LocationDefinition {
                id: id.into(),
                name: name.into(),
                description: String::new(),
                connected_locations: vec![],
                map_position: (0.0, 0.0),
            },
        );
    }
    project.characters.insert(
        "nora".into(),
        CharacterDefinition {
            id: "nora".into(),
            name: "Nora".into(),
            description: "A night clerk".into(),
            role_tags: vec!["clerk".into()],
            home_location: "archive".into(),
            goals: vec![],
            red_lines: vec![],
            voice_profile: "Plainspoken".into(),
            schedule: BTreeMap::new(),
        },
    );
    project.storylets.insert(
        "find_receipt".into(),
        StoryletDefinition {
            id: "find_receipt".into(),
            revision: 1,
            title: "The Receipt".into(),
            summary: "A clerk finds the missing receipt".into(),
            approved: true,
            ending: false,
            location_id: Some("archive".into()),
            urgency_weight: 20.0,
            cluster_type: ClusterType::PlayerControlledPassage,
            repeatability: Repeatability::Always,
            required_roles: vec![RoleRequirement {
                role: "clerk".into(),
            }],
            trigger: PredicateSet {
                all_of: vec![Predicate {
                    fact: FactRef::Quality {
                        key: "evidence.receipt_found".into(),
                    },
                    op: PredicateOp::Eq,
                    value: json!(true),
                }],
                any_of: vec![],
            },
            content: SceneContent {
                opening: "Nora unfolds a receipt.".into(),
            },
            choices: vec![StoryletChoice {
                id: "read".into(),
                label: "Read it".into(),
                requirements: PredicateSet::default(),
                effects: vec![Effect::SetQuality {
                    key: "evidence.read".into(),
                    value: json!(true),
                }],
                continues_scene: false,
                divert_target: None,
            }],
        },
    );
    project.npc_behaviors.insert(
        "find_receipt_rule".into(),
        NpcBehaviorRule {
            id: "find_receipt_rule".into(),
            actor_id: "nora".into(),
            priority: 5,
            conditions: PredicateSet {
                all_of: vec![
                    Predicate {
                        fact: FactRef::CurrentLocation,
                        op: PredicateOp::Eq,
                        value: json!("archive"),
                    },
                    Predicate {
                        fact: FactRef::Quality {
                            key: "evidence.receipt_found".into(),
                        },
                        op: PredicateOp::Eq,
                        value: json!(false),
                    },
                ],
                any_of: vec![],
            },
            action: "find_receipt".into(),
            effects: vec![Effect::SetQuality {
                key: "evidence.receipt_found".into(),
                value: json!(true),
            }],
        },
    );
    project.scenarios.insert(
        "opening".into(),
        ScenarioDefinition {
            id: "opening".into(),
            name: "A Missing Receipt".into(),
            player_start_location: "market".into(),
            initial_qualities: BTreeMap::new(),
            actor_locations: BTreeMap::new(),
            storylet_ids: vec!["find_receipt".into()],
            seed: 19,
        event_ids: vec![],
            active_actor_ids: None,
        },
    );
    project
}

#[test]
fn player_movement_triggers_npc_cascade_and_unlocks_a_storylet_in_the_rust_runtime() {
    let mut runtime = StoryletRuntime::default();
    runtime
        .create_session("test-session", project(), "opening")
        .unwrap();

    let result = runtime
        .submit_command(
            "test-session",
            WorldCommand {
                idempotency_key: "player:go-to-archive".into(),
                expected_revision: 0,
                actor_id: "player".into(),
                action: "move".into(),
                effects: vec![Effect::MovePlayer {
                    location_id: "archive".into(),
                }],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .unwrap();

    assert_eq!(result.player.event.turn, 1);
    assert_eq!(result.npc.events.len(), 1);
    assert_eq!(result.npc.events[0].caused_by, vec!["player:go-to-archive"]);
    assert_eq!(result.world.revision, 2);
    assert_eq!(result.world.turn, 1);
    assert_eq!(
        result.world.qualities.get("evidence.receipt_found"),
        Some(&json!(true))
    );

    let evaluation = runtime.evaluate("test-session", 0.15).unwrap();
    assert_eq!(evaluation.selected.as_deref(), Some("find_receipt"));
}

#[test]
fn scenario_canon_and_world_events_fire_after_their_turn_and_are_recorded() {
    let mut authored = project();
    authored.qualities.insert("world.pressure".into(), QualityDefinition {
        key: "world.pressure".into(), kind: QualityType::Metric, default_value: json!(0), description: "Ambient pressure".into(),
    });
    authored.events.insert("canon_revelation".into(), EventDefinition {
        id: "canon_revelation".into(), title: "A canon revelation".into(), description: "Nora confirms the receipt.".into(),
        trigger: PredicateSet { all_of: vec![Predicate { fact: FactRef::Quality { key: "evidence.receipt_found".into() }, op: PredicateOp::Eq, value: json!(true) }], any_of: vec![] },
        effects: vec![Effect::SetQuality { key: "evidence.read".into(), value: json!(true) }], delay_turns: 1, repeatable: false, event_type: EventType::Canon,
    });
    authored.events.insert("weather_shift".into(), EventDefinition {
        id: "weather_shift".into(), title: "Rain reaches the archive".into(), description: "The gutters answer the storm.".into(),
        trigger: PredicateSet::default(), effects: vec![Effect::AddQuality { key: "world.pressure".into(), amount: 1.0 }], delay_turns: 1, repeatable: false, event_type: EventType::World,
    });
    authored.scenarios.get_mut("opening").unwrap().event_ids = vec!["canon_revelation".into(), "weather_shift".into()];
    let mut runtime = StoryletRuntime::default();
    runtime.create_session("scenario-events", authored, "opening").unwrap();
    let result = runtime.submit_command("scenario-events", WorldCommand {
        idempotency_key: "turn-one".into(), expected_revision: 0, actor_id: "player".into(), action: "wait".into(),
        effects: vec![Effect::SetQuality { key: "evidence.receipt_found".into(), value: json!(true) }],
        caused_by: vec![], advance_turn: true, terminal: false,
    }).unwrap();
    let event_actions: Vec<_> = result.world.events.iter().map(|event| event.action.as_str()).collect();
    assert!(event_actions.contains(&"event:canon:canon_revelation"));
    assert!(event_actions.contains(&"event:world:weather_shift"));
    assert_eq!(result.world.qualities.get("evidence.read"), Some(&json!(true)));
    assert_eq!(result.world.qualities.get("world.pressure"), Some(&json!(1.0)));
}

#[test]
fn ending_choice_terminates_session_and_blocks_later_actions() {
    let mut authored = project();
    authored.storylets.get_mut("find_receipt").unwrap().ending = true;
    authored.scenarios.get_mut("opening").unwrap().actor_locations.insert("nora".into(), "archive".into());
    let mut runtime = StoryletRuntime::default();
    runtime.create_session("ending", authored, "opening").unwrap();
    runtime.submit_command("ending", WorldCommand {
        idempotency_key: "reveal".into(), expected_revision: 0, actor_id: "player".into(), action: "reveal".into(),
        effects: vec![
            Effect::SetQuality { key: "evidence.receipt_found".into(), value: json!(true) },
            Effect::MovePlayer { location_id: "archive".into() },
        ], caused_by: vec![], advance_turn: true, terminal: false,
    }).unwrap();
    let completed = runtime.execute_choice("ending", "find_receipt", "read", "ending-choice", 1).unwrap();
    assert!(completed.player.event.terminal);
    assert!(completed.world.is_terminal());
    assert!(runtime.evaluate("ending", 0.15).unwrap().candidates.is_empty());
    let error = runtime.submit_command("ending", WorldCommand {
        idempotency_key: "after-ending".into(), expected_revision: completed.world.revision, actor_id: "player".into(), action: "again".into(),
        effects: vec![], caused_by: vec![], advance_turn: true, terminal: false,
    }).unwrap_err();
    assert!(error.to_string().contains("reached its ending"));
}

#[test]
fn external_decision_is_revalidated_and_can_choose_no_action() {
    let mut authored = project();
    let mut alternate = authored.storylets["find_receipt"].clone();
    alternate.id = "ask_clerk".into();
    alternate.title = "Ask the Clerk".into();
    authored.storylets.insert(alternate.id.clone(), alternate);
    authored.scenarios.get_mut("opening").unwrap().storylet_ids =
        vec!["find_receipt".into(), "ask_clerk".into()];
    let mut runtime = StoryletRuntime::default();
    runtime
        .create_session("decision", authored, "opening")
        .unwrap();
    runtime
        .submit_command(
            "decision",
            WorldCommand {
                idempotency_key: "player:enter-archive".into(),
                expected_revision: 0,
                actor_id: "player".into(),
                action: "move".into(),
                effects: vec![Effect::MovePlayer {
                    location_id: "archive".into(),
                }],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .unwrap();

    let decision = DecisionResult {
        format_version: 1,
        context_revision: 2,
        source: "test-ranker".into(),
        model_version: "fixture-1".into(),
        confidence: 0.8,
        no_action_probability: 0.2,
        ranking: vec![
            RankedStorylet {
                storylet_id: "ask_clerk".into(),
                score: 0.9,
                probability: 0.6,
            },
            RankedStorylet {
                storylet_id: "find_receipt".into(),
                score: 0.4,
                probability: 0.2,
            },
        ],
    };
    let result = runtime
        .apply_decision("decision", 0.15, decision.clone())
        .unwrap();
    assert_eq!(result.selected.as_deref(), Some("ask_clerk"));
    assert_eq!(result.candidates[0].storylet_id, "ask_clerk");
    assert_eq!(result.decision, Some(decision.clone()));

    let mut no_action = decision;
    no_action.no_action_probability = 0.8;
    no_action.ranking[0].probability = 0.1;
    no_action.ranking[1].probability = 0.1;
    let result = runtime.apply_decision("decision", 0.15, no_action).unwrap();
    assert_eq!(result.selected, None);

    let stale = DecisionResult {
        format_version: 1,
        context_revision: 1,
        source: "test-ranker".into(),
        model_version: "fixture-1".into(),
        confidence: 0.5,
        no_action_probability: 1.0,
        ranking: vec![],
    };
    assert!(runtime.apply_decision("decision", 0.15, stale).is_err());
}

#[test]
fn rust_engine_scores_embeddings_and_rejects_incomplete_or_ineligible_vectors() {
    let mut authored = project();
    let mut alternate = authored.storylets["find_receipt"].clone();
    alternate.id = "ask_clerk".into();
    alternate.title = "Ask the Clerk".into();
    authored.storylets.insert(alternate.id.clone(), alternate);
    let mut remote = authored.storylets["find_receipt"].clone();
    remote.id = "weather_elsewhere".into();
    remote.location_id = Some("market".into());
    authored.storylets.insert(remote.id.clone(), remote);
    authored.scenarios.get_mut("opening").unwrap().storylet_ids = vec![
        "find_receipt".into(),
        "ask_clerk".into(),
        "weather_elsewhere".into(),
    ];
    let mut runtime = StoryletRuntime::default();
    runtime
        .create_session("semantic", authored, "opening")
        .unwrap();
    runtime
        .submit_command(
            "semantic",
            WorldCommand {
                idempotency_key: "player:enter-archive".into(),
                expected_revision: 0,
                actor_id: "player".into(),
                action: "move".into(),
                effects: vec![Effect::MovePlayer {
                    location_id: "archive".into(),
                }],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .unwrap();

    let request = SemanticRankingRequest {
        format_version: 1,
        context_revision: 2,
        model_version: "encoder@revision-1".into(),
        query_embedding: vec![1.0, 0.0],
        ranking_head: None,
        candidates: vec![
            SemanticCandidateEmbedding {
                storylet_id: "ask_clerk".into(),
                embedding: vec![0.0, 1.0],
                features: BTreeMap::new(),
            },
            SemanticCandidateEmbedding {
                storylet_id: "find_receipt".into(),
                embedding: vec![1.0, 0.0],
                features: BTreeMap::new(),
            },
        ],
    };
    let result = runtime
        .rank_semantic("semantic", 0.15, request.clone())
        .unwrap();
    assert_eq!(result.selected.as_deref(), Some("find_receipt"));
    assert_eq!(result.candidates[0].storylet_id, "find_receipt");
    assert_eq!(
        result.decision.as_ref().unwrap().source,
        "embedding_baseline_uncalibrated"
    );
    assert!(result.decision.as_ref().unwrap().confidence == 0.0);
    let probabilities = result
        .decision
        .as_ref()
        .unwrap()
        .ranking
        .iter()
        .map(|item| item.probability)
        .sum::<f64>()
        + result.decision.as_ref().unwrap().no_action_probability;
    assert!((probabilities - 1.0).abs() < 1e-10);

    let feature_names = vec![
        "semantic_relevance",
        "urgency",
        "pacing_arc_alignment",
        "novelty",
        "waiting_time",
        "goal_progress",
        "role_binding_quality",
        "predicted_consequence_alignment",
        "no_action",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<Vec<_>>();
    let head = RankingHead {
        schema_version: 1,
        model_version: "reviewed-pairwise-v1".into(),
        feature_names: feature_names.clone(),
        weights: [
            ("semantic_relevance", 0.0),
            ("urgency", 10.0),
            ("pacing_arc_alignment", 0.0),
            ("novelty", 0.0),
            ("waiting_time", 0.0),
            ("goal_progress", 0.0),
            ("role_binding_quality", 0.0),
            ("predicted_consequence_alignment", 0.0),
            ("no_action", 0.0),
        ]
        .into_iter()
        .map(|(name, weight)| (name.to_owned(), weight))
        .collect(),
        bias: 0.0,
        temperature: 1.0,
        no_action_bias: -20.0,
    };
    let mut ranked = request.clone();
    ranked.ranking_head = Some(head);
    for candidate in &mut ranked.candidates {
        candidate.features = feature_names
            .iter()
            .map(|name| (name.clone(), 0.0))
            .collect();
        if candidate.storylet_id == "ask_clerk" {
            candidate.features.insert("urgency".into(), 1.0);
        }
    }
    let trained = runtime.rank_semantic("semantic", 0.15, ranked).unwrap();
    assert_eq!(trained.selected.as_deref(), Some("ask_clerk"));
    assert_eq!(
        trained.decision.as_ref().unwrap().source,
        "trained_ranking_head_uncalibrated"
    );
    assert_eq!(
        trained.decision.as_ref().unwrap().model_version,
        "encoder@revision-1+reviewed-pairwise-v1"
    );
    assert_eq!(trained.decision.as_ref().unwrap().confidence, 0.0);

    let mut incomplete = request.clone();
    incomplete.candidates.pop();
    assert!(runtime.rank_semantic("semantic", 0.15, incomplete).is_err());
    let mut includes_ineligible = request.clone();
    includes_ineligible
        .candidates
        .push(SemanticCandidateEmbedding {
            storylet_id: "weather_elsewhere".into(),
            embedding: vec![1.0, 0.0],
            features: BTreeMap::new(),
        });
    assert!(
        runtime
            .rank_semantic("semantic", 0.15, includes_ineligible)
            .is_err()
    );
}

#[test]
fn replaying_a_player_command_does_not_replay_its_npc_cascade() {
    let mut authored = project();
    authored.npc_behaviors.clear();
    authored.qualities.insert(
        "actions.nora".into(),
        QualityDefinition {
            key: "actions.nora".into(),
            kind: QualityType::Custom,
            default_value: json!(0),
            description: "Number of times Nora acted".into(),
        },
    );
    authored.npc_behaviors.insert(
        "count_action".into(),
        NpcBehaviorRule {
            id: "count_action".into(),
            actor_id: "nora".into(),
            priority: 1,
            conditions: PredicateSet::default(),
            action: "count_action".into(),
            effects: vec![Effect::AddQuality {
                key: "actions.nora".into(),
                amount: 1.0,
            }],
        },
    );
    let mut runtime = StoryletRuntime::default();
    runtime
        .create_session("idempotent", authored, "opening")
        .unwrap();
    let command = WorldCommand {
        idempotency_key: "player:wait".into(),
        expected_revision: 0,
        actor_id: "player".into(),
        action: "wait".into(),
        effects: vec![],
        caused_by: vec![],
        advance_turn: true,
                terminal: false,
    };
    let first = runtime
        .submit_command("idempotent", command.clone())
        .unwrap();
    let retry = runtime.submit_command("idempotent", command).unwrap();

    assert_eq!(first.npc.events.len(), 1);
    assert!(retry.player.duplicate);
    assert!(retry.npc.events.is_empty());
    assert_eq!(retry.world.revision, first.world.revision);
    assert_eq!(retry.world.events.len(), first.world.events.len());
    assert_eq!(retry.world.qualities.get("actions.nora"), Some(&json!(1.0)));
}

#[test]
fn invalid_player_command_keeps_session_snapshot_unchanged() {
    let mut runtime = StoryletRuntime::default();
    runtime
        .create_session("test-session", project(), "opening")
        .unwrap();
    let before = runtime.snapshot("test-session").unwrap();

    let result = runtime.submit_command(
        "test-session",
        WorldCommand {
            idempotency_key: "player:invalid-transfer".into(),
            expected_revision: 0,
            actor_id: "player".into(),
            action: "invalid".into(),
            effects: vec![Effect::MovePlayer {
                location_id: "missing".into(),
            }],
            caused_by: vec![],
            advance_turn: true,
            terminal: false,
        },
    );

    assert!(result.is_err());
    assert_eq!(runtime.snapshot("test-session").unwrap(), before);
}

#[test]
fn clearing_a_playtest_discards_all_session_state_and_allows_a_clean_restart() {
    let mut runtime = StoryletRuntime::default();
    runtime
        .create_session("repeatable-test", project(), "opening")
        .unwrap();
    runtime
        .submit_command(
            "repeatable-test",
            WorldCommand {
                idempotency_key: "player:wait".into(),
                expected_revision: 0,
                actor_id: "player".into(),
                action: "wait".into(),
                effects: vec![],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .unwrap();

    assert!(runtime.delete_session("repeatable-test"));
    assert!(runtime.snapshot("repeatable-test").is_err());
    let fresh = runtime
        .create_session("repeatable-test", project(), "opening")
        .unwrap();
    assert_eq!(fresh.world.turn, 0);
    assert!(fresh.world.events.is_empty());
}

#[test]
fn fork_replays_an_event_prefix_and_keeps_future_branch_actions_isolated() {
    let mut runtime = StoryletRuntime::default();
    let mut authored = project();
    authored
        .storylets
        .get_mut("find_receipt")
        .unwrap()
        .repeatability = Repeatability::OnceEver;
    runtime
        .create_session("original", authored, "opening")
        .unwrap();
    runtime
        .submit_command(
            "original",
            WorldCommand {
                idempotency_key: "player:go-to-archive".into(),
                expected_revision: 0,
                actor_id: "player".into(),
                action: "move".into(),
                effects: vec![Effect::MovePlayer {
                    location_id: "archive".into(),
                }],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .unwrap();
    runtime
        .execute_choice("original", "find_receipt", "read", "player:read-receipt", 2)
        .unwrap();
    let original = runtime.snapshot("original").unwrap();
    let fork = runtime
        .fork_session("original", "branch", "player:read-receipt")
        .unwrap();

    assert_eq!(fork.world, original.world);
    assert_eq!(fork.world.events.len(), 3);
    assert_eq!(
        fork.world.qualities.get("evidence.read"),
        Some(&json!(true))
    );
    assert_eq!(runtime.evaluate("branch", 0.15).unwrap().selected, None);

    runtime
        .submit_command(
            "branch",
            WorldCommand {
                idempotency_key: "branch:wait".into(),
                expected_revision: 3,
                actor_id: "player".into(),
                action: "wait".into(),
                effects: vec![],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .unwrap();
    assert_eq!(runtime.snapshot("original").unwrap(), original);
    assert_eq!(runtime.snapshot("branch").unwrap().world.events.len(), 4);
}

#[test]
fn exported_session_restores_world_and_director_state_after_runtime_restart() {
    let mut runtime = StoryletRuntime::default();
    let mut authored = project();
    authored
        .storylets
        .get_mut("find_receipt")
        .unwrap()
        .repeatability = Repeatability::OnceEver;
    runtime
        .create_session("durable", authored, "opening")
        .unwrap();
    runtime
        .submit_command(
            "durable",
            WorldCommand {
                idempotency_key: "player:go-to-archive".into(),
                expected_revision: 0,
                actor_id: "player".into(),
                action: "move".into(),
                effects: vec![Effect::MovePlayer {
                    location_id: "archive".into(),
                }],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .unwrap();
    runtime
        .execute_choice("durable", "find_receipt", "read", "player:read-receipt", 2)
        .unwrap();
    let before = runtime.snapshot("durable").unwrap();
    let saved = runtime.export_session("durable").unwrap();

    let mut restarted = StoryletRuntime::default();
    let restored = restarted.restore_session(saved.clone()).unwrap();
    assert_eq!(restored, before);
    assert_eq!(restarted.evaluate("durable", 0.15).unwrap().selected, None);

    let mut tampered = saved;
    tampered.session_id = "tampered".into();
    tampered.world.events.pop();
    assert!(restarted.restore_session(tampered).is_err());
}

#[test]
fn eligible_storylet_choice_executes_through_runtime_and_retries_idempotently() {
    let mut runtime = StoryletRuntime::default();
    runtime
        .create_session("choice-session", project(), "opening")
        .unwrap();
    runtime
        .submit_command(
            "choice-session",
            WorldCommand {
                idempotency_key: "player:move".into(),
                expected_revision: 0,
                actor_id: "player".into(),
                action: "move".into(),
                effects: vec![Effect::MovePlayer {
                    location_id: "archive".into(),
                }],
                caused_by: vec![],
                advance_turn: true,
                terminal: false,
            },
        )
        .unwrap();

    let result = runtime
        .execute_choice("choice-session", "find_receipt", "read", "choice:read", 2)
        .unwrap();
    assert_eq!(result.world.turn, 2);
    assert_eq!(
        result.world.qualities.get("evidence.read"),
        Some(&json!(true))
    );
    assert_eq!(result.player.event.action, "storylet:find_receipt:read");

    let duplicate = runtime
        .execute_choice("choice-session", "find_receipt", "read", "choice:read", 2)
        .unwrap();
    assert!(duplicate.player.duplicate);
    assert_eq!(duplicate.world.revision, 3);
    assert_eq!(duplicate.world.events.len(), 3);
}
