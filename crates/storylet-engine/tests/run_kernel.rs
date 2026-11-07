use std::collections::BTreeMap;
use storylet_engine::chrono_dispatch::{StoryletDef, UnifiedDispatcher};
use storylet_engine::chrono_runtime::{
    EdgeConnection, FlowGraphAsset, FlowNodeDef, NodeCategory, NodeKind, PinDefinition,
};
use storylet_engine::chrono_state::{GameplayTagRegistry, PassionRegistry, WorldStateTable};
use storylet_engine::run::{RunAction, RunContent, RunKernel, RunStart};

fn pin(name: &str, input: bool) -> PinDefinition {
    PinDefinition {
        pin_name: name.into(),
        is_input: input,
    }
}
fn fixture() -> RunContent {
    let choice = FlowNodeDef {
        node_guid: 1,
        node_type_id: "choice".into(),
        category: NodeCategory::LogicGate,
        kind: NodeKind::Choice {
            default_output_pin: String::new(),
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("left", false), pin("right", false)],
        ..Default::default()
    };
    let end = FlowNodeDef {
        node_guid: 2,
        node_type_id: "end".into(),
        category: NodeCategory::Mutator,
        kind: NodeKind::End {
            jump_to_storylet: None,
            precondition_result: true,
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let mut world = WorldStateTable::new();
    world.ensure_participants();
    RunContent {
        registry: GameplayTagRegistry::empty(),
        dispatcher: UnifiedDispatcher::new(vec![StoryletDef {
            storylet_id: 0,
            payload_node_guid: 1,
            ..Default::default()
        }]),
        passion_registry: PassionRegistry::empty(),
        storylet_titles: BTreeMap::from([(0, "Start".into())]),
        initial_world: world,
        root_asset: FlowGraphAsset {
            asset_id: "test".into(),
            start_node_guid: 1,
            nodes: vec![choice, end],
            edges: vec![
                EdgeConnection {
                    source_node_guid: 1,
                    source_pin_name: "left".into(),
                    target_node_guid: 2,
                    target_pin_name: "In".into(),
                },
                EdgeConnection {
                    source_node_guid: 1,
                    source_pin_name: "right".into(),
                    target_node_guid: 2,
                    target_pin_name: "In".into(),
                },
            ],
            data_edges: vec![],
        },
        ..Default::default()
    }
}
#[test]
fn start_auto_advances_to_first_interaction() {
    let content = fixture();
    let run = RunKernel::start(&content, RunStart::Beginning, 42).unwrap();
    let scene = run.view(&content, 0).scene.unwrap();
    assert_eq!(scene.node_guid, 1);
    assert_eq!(scene.choices.len(), 2);
}
#[test]
fn act_choose_appends_step_and_replay_matches() {
    let content = fixture();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 42).unwrap();
    let action = RunAction::Choose {
        node_guid: 1,
        output_pin: 0,
    };
    assert_eq!(run.act(&content, 0, action.clone()).unwrap(), 1);
    let view = run.view(&content, 0);
    assert_eq!(view.steps.len(), 1);
    assert_eq!(view.current_step, 1);
    let (replayed, divergence) =
        RunKernel::replay(&content, RunStart::Beginning, 42, &[(0, action)]);
    assert_eq!(divergence, None);
    assert_eq!(
        serde_json::to_value(view).unwrap(),
        serde_json::to_value(replayed.unwrap().view(&content, 0)).unwrap()
    );
}
#[test]
fn run_actions_have_a_stable_wire_shape() {
    let action = RunAction::Choose {
        node_guid: 7,
        output_pin: 2,
    };
    assert_eq!(
        serde_json::to_value(action).unwrap(),
        serde_json::json!({"kind":"choose","nodeGuid":7,"outputPin":2})
    );
    assert_eq!(
        serde_json::to_value(RunStart::Beginning).unwrap(),
        serde_json::json!({"kind":"beginning"})
    );
}
#[test]
fn act_after_rewind_creates_branch() {
    let content = fixture();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 42).unwrap();
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 1,
            output_pin: 0,
        },
    )
    .unwrap();
    run.rewind(0).unwrap();
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 1,
            output_pin: 1,
        },
    )
    .unwrap();
    let view = run.view(&content, 0);
    assert_eq!(view.branches.len(), 2);
    assert_ne!(
        view.branches[0].steps[0].action,
        view.branches[1].steps[0].action
    );
}
#[test]
fn stalled_graph_returns_stalled() {
    let mut content = fixture();
    content.root_asset.nodes[1].kind = NodeKind::Waiter {
        duration_ms: i64::MAX,
        required_tags: Default::default(),
        wait_event: String::new(),
    };
    content.root_asset.nodes[1].category = NodeCategory::Waiter;
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    assert!(matches!(
        run.act(
            &content,
            0,
            RunAction::Choose {
                node_guid: 1,
                output_pin: 0
            }
        ),
        Err(storylet_engine::run::RunError::Stalled { .. })
    ));
    assert_eq!(run.view(&content, 0).current_step, 0);
}
#[test]
fn why_not_explains_missing_fact() {
    let mut content = fixture();
    content.registry = GameplayTagRegistry::compile(vec!["fact.light_doused".into()]).unwrap();
    let fact = content.registry.id("fact.light_doused").unwrap();
    let mut required = storylet_engine::chrono_state::TagBitset::new();
    content
        .registry
        .add_hierarchical(&mut required, fact)
        .unwrap();
    content.dispatcher.storylets.push(StoryletDef {
        storylet_id: 1,
        required_tags: required,
        ..Default::default()
    });
    content.dispatcher.rebuild_index();
    let run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    assert!(run
        .why_not(&content, 0, 10)
        .iter()
        .any(|m| m.storylet.id == 1 && m.reason == "needs fact \"light doused\""));
}
#[test]
fn content_roundtrip_rebuilds_dispatcher_index() {
    let content = fixture();
    let json = serde_json::to_string(&content).unwrap();
    let decoded: RunContent = serde_json::from_str(&json).unwrap();
    assert!(decoded.dispatcher.storylet(0).is_some());
}
#[test]
fn replay_is_deterministic_over_twenty_choices() {
    let mut content = fixture();
    content.root_asset.edges[0].target_node_guid = 1;
    content.root_asset.edges[1].target_node_guid = 1;
    let mut run = RunKernel::start(&content, RunStart::Beginning, 987).unwrap();
    let actions = (0..20)
        .map(|i| {
            (
                0,
                RunAction::Choose {
                    node_guid: 1,
                    output_pin: (i % 2) as u16,
                },
            )
        })
        .collect::<Vec<_>>();
    for (p, action) in &actions {
        run.act(&content, *p, action.clone()).unwrap();
    }
    let (replay, diverged) = RunKernel::replay(&content, RunStart::Beginning, 987, &actions);
    assert_eq!(diverged, None);
    assert_eq!(
        serde_json::to_value(run.view(&content, 0)).unwrap(),
        serde_json::to_value(replay.unwrap().view(&content, 0)).unwrap()
    );
}
#[test]
fn storylet_start_invokes_its_poi() {
    let mut content = fixture();
    content.root_asset.nodes[0].payload = serde_json::json!({"storyletId":"opening"});
    let poi = FlowNodeDef {
        node_guid: 3,
        node_type_id: "poi".into(),
        category: NodeCategory::LogicGate,
        kind: NodeKind::Poi {
            poi_id: "intro".into(),
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        payload: serde_json::json!({"storyletId":"opening"}),
        ..Default::default()
    };
    content.root_asset.nodes.push(poi);
    content.root_asset.edges.push(EdgeConnection {
        source_node_guid: 3,
        source_pin_name: "Out".into(),
        target_node_guid: 1,
        target_pin_name: "In".into(),
    });
    content.dispatcher.storylets[0].payload_node_guid = 1;
    content.catalog.insert("opening", false, 0, 0.0).unwrap();
    let run = RunKernel::start(&content, RunStart::Storylet { storylet: 0 }, 4).unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().node_guid, 1);
}
#[test]
fn why_not_excludes_storylets_at_other_places() {
    let mut content = fixture();
    content.initial_world = WorldStateTable::with_capacity(2);
    content.initial_world.participants =
        storylet_engine::chrono_participants::ParticipantTable::implicit_player(Some(0));
    content.registry = GameplayTagRegistry::compile(vec![
        "fact.missing".into(),
        "location.docks".into(),
        "location.forest".into(),
    ])
    .unwrap();
    let docks = content.registry.id("location.docks").unwrap();
    let forest = content.registry.id("location.forest").unwrap();
    let fact = content.registry.id("fact.missing").unwrap();
    content
        .registry
        .add_hierarchical(&mut content.initial_world.entity_tags[0], docks)
        .unwrap();
    let mut forest_mask = storylet_engine::chrono_state::TagBitset::new();
    forest_mask.set_bit(forest).unwrap();
    content.storylet_location_tags.insert(1, forest_mask);
    let mut required = storylet_engine::chrono_state::TagBitset::new();
    content
        .registry
        .add_hierarchical(&mut required, fact)
        .unwrap();
    content.dispatcher.storylets.push(StoryletDef {
        storylet_id: 1,
        required_tags: required,
        ..Default::default()
    });
    content.dispatcher.rebuild_index();
    let run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    assert!(run.why_not(&content, 0, 10).is_empty());
}
#[test]
fn nested_mission_choice_is_playable() {
    let mut content = fixture();
    let child = content.root_asset.clone();
    let mission = FlowNodeDef {
        node_guid: 10,
        node_type_id: "mission".into(),
        category: NodeCategory::ScopedContainer,
        kind: NodeKind::MissionSubGraph {
            sub_graph_asset_id: "test".into(),
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let end = FlowNodeDef {
        node_guid: 11,
        node_type_id: "end".into(),
        category: NodeCategory::Mutator,
        kind: NodeKind::End {
            jump_to_storylet: None,
            precondition_result: true,
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    content.root_asset = FlowGraphAsset {
        asset_id: "root".into(),
        start_node_guid: 10,
        nodes: vec![mission, end],
        edges: vec![EdgeConnection {
            source_node_guid: 10,
            source_pin_name: "Out".into(),
            target_node_guid: 11,
            target_pin_name: "In".into(),
        }],
        data_edges: vec![],
    };
    content.root_asset.nodes.push(child.nodes[0].clone());
    content.mission_assets.insert("test".into(), child);
    let mut run = RunKernel::start(&content, RunStart::Beginning, 3).unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().node_guid, 1);
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 1,
            output_pin: 0,
        },
    )
    .unwrap();
    assert!(run.view(&content, 0).scene.is_none());
    run.rewind(0).unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().node_guid, 1);
}

mod support;
#[test]
fn weirs_crossing_explains_why_teodor_cannot_be_pressed_yet() {
    let content = support::weirs_crossing::content();
    let run = RunKernel::start(&content, RunStart::Beginning, 7).unwrap();
    let misses = run.why_not(&content, 0, 10);
    assert!(misses
        .iter()
        .any(|m| m.storylet.id == support::weirs_crossing::PRESS_TEODOR
            && m.reason == "needs fact \"light doused\""));
    assert_eq!(
        content
            .initial_world
            .participants
            .colocated(0)
            .collect::<Vec<_>>(),
        vec![1]
    );
}

#[test]
fn missing_hierarchical_fact_reports_leaf() {
    let mut content = fixture();
    content.registry =
        GameplayTagRegistry::compile(vec!["fact".into(), "fact.light_doused".into()]).unwrap();
    let mut required = storylet_engine::chrono_state::TagBitset::new();
    let fact = content.registry.id("fact.light_doused").unwrap();
    content
        .registry
        .add_hierarchical(&mut required, fact)
        .unwrap();
    content.dispatcher.storylets[0].required_tags = required;
    assert_eq!(
        content
            .dispatcher
            .explain_ineligible(
                &content.dispatcher.storylets[0],
                &content.initial_world,
                &content.registry
            )
            .as_deref(),
        Some("needs fact \"light doused\"")
    );
}
#[test]
fn movement_requires_a_connected_location() {
    use storylet_engine::chrono_state::{GraphLayer, WorldEdge, WorldVertex};
    let mut content = fixture();
    let w = &mut content.initial_world;
    w.entity_tags.resize(4, Default::default());
    w.participants.location[0] = Some(0);
    w.vertices = vec![
        WorldVertex {
            vertex_id: "a".into(),
            layers: GraphLayer::Location.into(),
            entity_index: Some(0),
            ..Default::default()
        },
        WorldVertex {
            vertex_id: "b".into(),
            layers: GraphLayer::Location.into(),
            entity_index: Some(1),
            ..Default::default()
        },
        WorldVertex {
            vertex_id: "c".into(),
            layers: GraphLayer::Location.into(),
            entity_index: Some(2),
            ..Default::default()
        },
        WorldVertex {
            vertex_id: "actor".into(),
            layers: GraphLayer::Character.into(),
            entity_index: Some(3),
            ..Default::default()
        },
    ];
    w.edges = vec![WorldEdge {
        edge_id: "a-b".into(),
        source_id: "a".into(),
        target_id: "b".into(),
        relation_type: "connected".into(),
        weight: 1.0,
    }];
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    assert!(run.act(&content, 0, RunAction::Move { to: 3 }).is_err());
    assert!(run.act(&content, 0, RunAction::Move { to: 2 }).is_err());
    run.act(&content, 0, RunAction::Move { to: 1 }).unwrap();
    assert_eq!(run.world.active_location(0), Some(1));
    run.rewind(0).unwrap();
    assert_eq!(run.world.active_location(0), Some(0));
}
#[test]
fn waypoint_exposes_adjacent_topic_actions() {
    use storylet_engine::chrono_dispatch::{
        WaypointTopicEdge, WaypointTopicGraph, WaypointTopicNode,
    };
    let mut content = fixture();
    content.root_asset.nodes[0].kind = NodeKind::WaypointConversation {
        initial_topic_id: "one".into(),
        npc_goal_topic_id: "two".into(),
        graph: WaypointTopicGraph {
            nodes: vec![
                WaypointTopicNode {
                    topic_id: "one".into(),
                    ..Default::default()
                },
                WaypointTopicNode {
                    topic_id: "two".into(),
                    ..Default::default()
                },
                WaypointTopicNode {
                    topic_id: "three".into(),
                    ..Default::default()
                },
            ],
            edges: vec![
                WaypointTopicEdge {
                    edge_id: "npc".into(),
                    from_topic_id: "one".into(),
                    to_topic_id: "two".into(),
                    base_cost: 1.0,
                    ..Default::default()
                },
                WaypointTopicEdge {
                    edge_id: "player".into(),
                    from_topic_id: "two".into(),
                    to_topic_id: "three".into(),
                    base_cost: 1.0,
                    ..Default::default()
                },
            ],
        },
    };
    content.root_asset.nodes[0].category = NodeCategory::ScopedContainer;
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    let scene = run.view(&content, 0).scene.unwrap();
    assert_eq!(
        scene.choices[0].action,
        RunAction::Topic {
            node_guid: 1,
            edge_id: 1
        }
    );
    run.act(&content, 0, scene.choices[0].action.clone())
        .unwrap();
}
#[test]
fn rewind_restores_storylet_before_new_choice() {
    let mut content = fixture();
    let choice = content.root_asset.nodes[0].clone();
    let hub = FlowNodeDef {
        node_guid: 5,
        node_type_id: "hub".into(),
        category: NodeCategory::DynamicDispatcher,
        kind: NodeKind::QbnHub {
            deck: vec![0, 1],
            exit_storylet_id: Some(1),
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    content.root_asset.nodes.push(hub);
    let mut intro = choice.clone();
    intro.node_guid = 8;
    content.root_asset.nodes.push(intro);
    content.root_asset.start_node_guid = 8;
    content.root_asset.edges.push(EdgeConnection {
        source_node_guid: 8,
        source_pin_name: "left".into(),
        target_node_guid: 5,
        target_pin_name: "In".into(),
    });
    content.root_asset.edges.push(EdgeConnection {
        source_node_guid: 5,
        source_pin_name: "Out".into(),
        target_node_guid: choice.node_guid,
        target_pin_name: "In".into(),
    });
    content.dispatcher.storylets.push(StoryletDef {
        storylet_id: 1,
        payload_node_guid: 1,
        is_repeatable: true,
        ..Default::default()
    });
    content.dispatcher.rebuild_index();
    content.storylet_titles.insert(1, "Next".into());
    let mut run = RunKernel::start(&content, RunStart::Beginning, 2).unwrap();
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 8,
            output_pin: 0,
        },
    )
    .unwrap();
    run.act(
        &content,
        0,
        RunAction::PlayStorylet {
            node_guid: 5,
            storylet: 1,
        },
    )
    .unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().storylet.id, 1);
    run.rewind(0).unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().storylet.id, 0);
}
#[test]
fn selecting_storylet_enters_its_own_scene() {
    let mut content = fixture();
    let mut scene = content.root_asset.nodes[0].clone();
    scene.node_guid = 6;
    let hub = FlowNodeDef {
        node_guid: 5,
        node_type_id: "hub".into(),
        category: NodeCategory::DynamicDispatcher,
        kind: NodeKind::QbnHub {
            deck: vec![1],
            exit_storylet_id: None,
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    content.root_asset.nodes.extend([hub, scene]);
    content.root_asset.start_node_guid = 5;
    content.dispatcher.storylets.push(StoryletDef {
        storylet_id: 1,
        payload_node_guid: 6,
        is_repeatable: true,
        ..Default::default()
    });
    content.dispatcher.rebuild_index();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    run.act(
        &content,
        0,
        RunAction::PlayStorylet {
            node_guid: 5,
            storylet: 1,
        },
    )
    .unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().node_guid, 6);
}
#[test]
fn weirs_crossing_lamp_soot_unlocks_pressing_teodor() {
    let content = support::weirs_crossing::content();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 7).unwrap();
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 1,
            output_pin: 0,
        },
    )
    .unwrap();
    assert!(!run
        .view(&content, 0)
        .also_here
        .iter()
        .any(|s| s.id == support::weirs_crossing::PRESS_TEODOR));
    run.act(
        &content,
        0,
        RunAction::PlayStorylet {
            node_guid: 10,
            storylet: support::weirs_crossing::LAMP_SOOT,
        },
    )
    .unwrap();
    assert_eq!(run.view(&content, 0).scene.as_ref().unwrap().node_guid, 20);
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 20,
            output_pin: 0,
        },
    )
    .unwrap();
    assert!(run
        .view(&content, 0)
        .also_here
        .iter()
        .any(|s| s.id == support::weirs_crossing::PRESS_TEODOR));
    let fact = content.registry.id("fact.light_doused").unwrap();
    assert!(run.world.knowledge.knows(0, fact).is_some());
    assert!(run.world.knowledge.knows(1, fact).is_some());
}
#[test]
fn weirs_crossing_seeded_random_valid_actions_replay() {
    let content = support::weirs_crossing::content();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 99).unwrap();
    let mut seed = 12345u64;
    let mut actions = Vec::new();
    for _ in 0..20 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let view = run.view(&content, 0);
        let action = if let Some(scene) = view.scene {
            let index = (seed as usize) % scene.choices.len();
            scene.choices[index].action.clone()
        } else {
            let index = (seed as usize) % view.also_here.len();
            RunAction::PlayStorylet {
                node_guid: 10,
                storylet: view.also_here[index].id,
            }
        };
        run.act(&content, 0, action.clone()).unwrap();
        actions.push((0, action));
    }
    let (replayed, diverged) = RunKernel::replay(&content, RunStart::Beginning, 99, &actions);
    assert_eq!(diverged, None);
    assert_eq!(
        serde_json::to_value(run.view(&content, 0)).unwrap(),
        serde_json::to_value(replayed.unwrap().view(&content, 0)).unwrap()
    );
}
#[test]
fn exit_storylet_finishes_before_hub_effects_and_continuation() {
    use storylet_engine::chrono_state::{MutationOp, TagBitset};
    let mut content = fixture();
    content.registry =
        GameplayTagRegistry::compile(vec!["effect.storylet".into(), "effect.after".into()])
            .unwrap();
    let effect = content.registry.id("effect.storylet").unwrap();
    let after = content.registry.id("effect.after").unwrap();
    let mut effect_mask = TagBitset::new();
    effect_mask.set_bit(effect).unwrap();
    let mut after_mask = TagBitset::new();
    after_mask.set_bit(after).unwrap();
    let hub = FlowNodeDef {
        node_guid: 5,
        node_type_id: "hub".into(),
        category: NodeCategory::DynamicDispatcher,
        kind: NodeKind::QbnHub {
            deck: vec![1],
            exit_storylet_id: Some(1),
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let mut selected = content.root_asset.nodes[0].clone();
    selected.node_guid = 6;
    let downstream = FlowNodeDef {
        node_guid: 7,
        node_type_id: "mutation".into(),
        category: NodeCategory::Mutator,
        kind: NodeKind::Mutation {
            op: MutationOp {
                add_global_tags: after_mask,
                ..Default::default()
            },
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    content.root_asset.nodes.extend([hub, selected, downstream]);
    content.root_asset.start_node_guid = 5;
    content.root_asset.edges.push(EdgeConnection {
        source_node_guid: 5,
        source_pin_name: "Out".into(),
        target_node_guid: 7,
        target_pin_name: "In".into(),
    });
    content.dispatcher.storylets.push(StoryletDef {
        storylet_id: 1,
        payload_node_guid: 6,
        is_repeatable: false,
        on_complete_mutation: MutationOp {
            add_global_tags: effect_mask,
            ..Default::default()
        },
        ..Default::default()
    });
    content.dispatcher.rebuild_index();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    run.act(
        &content,
        0,
        RunAction::PlayStorylet {
            node_guid: 5,
            storylet: 1,
        },
    )
    .unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().node_guid, 6);
    assert!(!run.world.global_tags.bit(effect));
    assert!(!run.world.global_tags.bit(after));
    assert!(!run.instance.completed_storylets.contains(1));
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 6,
            output_pin: 0,
        },
    )
    .unwrap();
    assert!(run.world.global_tags.bit(effect));
    assert!(run.world.global_tags.bit(after));
    assert!(run.instance.completed_storylets.contains(1));
}

#[test]
fn selected_scene_ignores_unrelated_waiter_on_unchosen_branch() {
    use storylet_engine::chrono_runtime::NodeExecutionState;
    let mut content = fixture();
    let hub = FlowNodeDef {
        node_guid: 5, node_type_id: "hub".into(), category: NodeCategory::DynamicDispatcher,
        kind: NodeKind::QbnHub { deck: vec![1], exit_storylet_id: None },
        input_pins: vec![pin("In", true)], output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let mut choice = content.root_asset.nodes[0].clone();
    choice.node_guid = 6;
    let waiter = FlowNodeDef {
        node_guid: 7, node_type_id: "waiter".into(), category: NodeCategory::Waiter,
        kind: NodeKind::Waiter { duration_ms: 1_000_000, required_tags: Default::default(), wait_event: String::new() },
        input_pins: vec![pin("In", true)], output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    content.root_asset.nodes.extend([hub, choice, waiter]);
    content.root_asset.start_node_guid = 5;
    content.root_asset.edges.extend([
        EdgeConnection { source_node_guid: 6, source_pin_name: "left".into(), target_node_guid: 2, target_pin_name: "In".into() },
        EdgeConnection { source_node_guid: 6, source_pin_name: "right".into(), target_node_guid: 7, target_pin_name: "In".into() },
    ]);
    content.dispatcher.storylets.push(StoryletDef { storylet_id: 1, payload_node_guid: 6, is_repeatable: true, ..Default::default() });
    content.dispatcher.rebuild_index();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    // A separate earlier invocation is still waiting when this scene starts.
    let unrelated = run.instance.nodes.iter_mut().find(|n| n.node_guid == 7).unwrap();
    unrelated.state = NodeExecutionState::ActiveLatent;
    unrelated.waiter_wake_ms = 1_000_000;
    run.act(&content, 0, RunAction::PlayStorylet { node_guid: 5, storylet: 1 }).unwrap();
    // The runtime stores one state per GUID: reject overlapping invocations
    // instead of silently stealing the already-active waiter's ownership.
    assert!(run.act(&content, 0, RunAction::Choose { node_guid: 6, output_pin: 1 }).is_err());
    assert_eq!(run.instance.node(7).unwrap().waiter_wake_ms, 1_000_000);
    run.act(&content, 0, RunAction::Choose { node_guid: 6, output_pin: 0 }).unwrap();
    assert!(run.instance.completed_storylets.contains(1));
}

#[test]
fn invocation_ownership_round_trips_and_old_instances_default_to_unscoped() {
    use storylet_engine::chrono_runtime::FlowGraphInstance;
    let content = fixture();
    let mut instance = FlowGraphInstance::new(1, content.root_asset, 1).unwrap();
    instance.enter_storylet_invocation(1, 42).unwrap();
    let value = serde_json::to_value(&instance).unwrap();
    let restored: FlowGraphInstance = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(restored.token_invocations.get(&restored.tokens[0].token_id), Some(&42));
    let mut old = value;
    old.as_object_mut().unwrap().remove("token_invocations");
    old.as_object_mut().unwrap().remove("node_invocations");
    let restored: FlowGraphInstance = serde_json::from_value(old).unwrap();
    assert!(restored.token_invocations.is_empty());
    assert!(restored.node_invocations.is_empty());
}
#[test]
fn selected_storylet_can_change_its_own_entry_requirement() {
    use storylet_engine::chrono_state::{MutationOp, TagBitset};
    let mut content = fixture();
    content.registry = GameplayTagRegistry::compile(vec!["gate.ready".into()]).unwrap();
    let tag = content.registry.id("gate.ready").unwrap();
    content.initial_world.global_tags.set_bit(tag).unwrap();
    let mut mask = TagBitset::new();
    mask.set_bit(tag).unwrap();
    let hub = FlowNodeDef {
        node_guid: 5,
        node_type_id: "hub".into(),
        category: NodeCategory::DynamicDispatcher,
        kind: NodeKind::QbnHub {
            deck: vec![1],
            exit_storylet_id: None,
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let mut scene = content.root_asset.nodes[0].clone();
    scene.node_guid = 6;
    let change = FlowNodeDef {
        node_guid: 7,
        node_type_id: "consume".into(),
        category: NodeCategory::Mutator,
        kind: NodeKind::Mutation {
            op: MutationOp {
                remove_global_tags: mask,
                ..Default::default()
            },
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    content.root_asset.nodes.extend([hub, scene, change]);
    content.root_asset.start_node_guid = 5;
    content.root_asset.edges.push(EdgeConnection {
        source_node_guid: 6,
        source_pin_name: "left".into(),
        target_node_guid: 7,
        target_pin_name: "In".into(),
    });
    content.dispatcher.storylets.push(StoryletDef {
        storylet_id: 1,
        payload_node_guid: 6,
        required_tags: mask,
        is_repeatable: true,
        ..Default::default()
    });
    content.dispatcher.rebuild_index();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    run.act(
        &content,
        0,
        RunAction::PlayStorylet {
            node_guid: 5,
            storylet: 1,
        },
    )
    .unwrap();
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 6,
            output_pin: 0,
        },
    )
    .unwrap();
    assert!(!run.world.global_tags.bit(tag));
}
#[test]
fn exit_storylet_waits_for_latent_work_before_releasing_hub() {
    use storylet_engine::chrono_state::{MutationOp, TagBitset};
    let mut content = fixture();
    content.registry = GameplayTagRegistry::compile(vec!["effect.done".into()]).unwrap();
    let tag = content.registry.id("effect.done").unwrap();
    let mut mask = TagBitset::new();
    mask.set_bit(tag).unwrap();
    let hub = FlowNodeDef {
        node_guid: 5,
        node_type_id: "hub".into(),
        category: NodeCategory::DynamicDispatcher,
        kind: NodeKind::QbnHub {
            deck: vec![1],
            exit_storylet_id: Some(1),
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let waiter = FlowNodeDef {
        node_guid: 6,
        node_type_id: "waiter".into(),
        category: NodeCategory::Waiter,
        kind: NodeKind::Waiter {
            duration_ms: 3000,
            required_tags: Default::default(),
            wait_event: String::new(),
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let mut choice = content.root_asset.nodes[0].clone();
    choice.node_guid = 7;
    content.root_asset.nodes.extend([hub, waiter, choice]);
    content.root_asset.start_node_guid = 5;
    content.root_asset.edges.push(EdgeConnection {
        source_node_guid: 6,
        source_pin_name: "Out".into(),
        target_node_guid: 7,
        target_pin_name: "In".into(),
    });
    content.dispatcher.storylets.push(StoryletDef {
        storylet_id: 1,
        payload_node_guid: 6,
        is_repeatable: true,
        on_complete_mutation: MutationOp {
            add_global_tags: mask,
            ..Default::default()
        },
        ..Default::default()
    });
    content.dispatcher.rebuild_index();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    run.act(
        &content,
        0,
        RunAction::PlayStorylet {
            node_guid: 5,
            storylet: 1,
        },
    )
    .unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().node_guid, 7);
    assert!(!run.world.global_tags.bit(tag));
    assert!(run.instance.clock_ms >= 3000);
}

#[test]
fn exit_storylet_waits_for_latent_work_after_deferred_jump() {
    use storylet_engine::chrono_state::{MutationOp, TagBitset};
    for jump in [
        NodeKind::End {
            jump_to_storylet: Some(2),
            precondition_result: true,
        },
        NodeKind::Teleport {
            destination_storylet: None,
            destination_node_guid: Some(7),
        },
        NodeKind::Teleport {
            destination_storylet: Some(2),
            destination_node_guid: None,
        },
    ] {
        let mut content = fixture();
        content.registry = GameplayTagRegistry::compile(vec!["effect.done".into()]).unwrap();
        let tag = content.registry.id("effect.done").unwrap();
        let mut mask = TagBitset::new();
        mask.set_bit(tag).unwrap();
        let node = |guid, category, kind| FlowNodeDef {
            node_guid: guid,
            node_type_id: "test".into(),
            category,
            kind,
            input_pins: vec![pin("In", true)],
            output_pins: vec![pin("Out", false)],
            ..Default::default()
        };
        let hub = node(
            5,
            NodeCategory::DynamicDispatcher,
            NodeKind::QbnHub {
                deck: vec![1],
                exit_storylet_id: Some(1),
            },
        );
        let jump_node = node(6, NodeCategory::Mutator, jump);
        let waiter = node(
            7,
            NodeCategory::Waiter,
            NodeKind::Waiter {
                duration_ms: 3000,
                required_tags: Default::default(),
                wait_event: String::new(),
            },
        );
        let mut choice = content.root_asset.nodes[0].clone();
        choice.node_guid = 8;
        content
            .root_asset
            .nodes
            .extend([hub, jump_node, waiter, choice]);
        content.root_asset.start_node_guid = 5;
        content.root_asset.edges.push(EdgeConnection {
            source_node_guid: 7,
            source_pin_name: "Out".into(),
            target_node_guid: 8,
            target_pin_name: "In".into(),
        });
        content.dispatcher.storylets.extend([
            StoryletDef {
                storylet_id: 1,
                payload_node_guid: 6,
                is_repeatable: true,
                on_complete_mutation: MutationOp {
                    add_global_tags: mask,
                    ..Default::default()
                },
                ..Default::default()
            },
            StoryletDef {
                storylet_id: 2,
                payload_node_guid: 7,
                ..Default::default()
            },
        ]);
        content.dispatcher.rebuild_index();
        let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
        run.act(
            &content,
            0,
            RunAction::PlayStorylet {
                node_guid: 5,
                storylet: 1,
            },
        )
        .unwrap();
        assert_eq!(run.view(&content, 0).scene.unwrap().node_guid, 8);
        assert!(!run.world.global_tags.bit(tag));
        assert!(run.instance.clock_ms >= 3000);
    }
}
#[test]
fn mission_hub_can_enter_another_storylet_in_root_asset() {
    let mut content = fixture();
    let scene = content.root_asset.nodes[0].clone();
    let hub = FlowNodeDef {
        node_guid: 5,
        node_type_id: "hub".into(),
        category: NodeCategory::DynamicDispatcher,
        kind: NodeKind::QbnHub {
            deck: vec![1],
            exit_storylet_id: None,
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    let child = FlowGraphAsset {
        asset_id: "child".into(),
        start_node_guid: 5,
        nodes: vec![hub],
        edges: vec![],
        data_edges: vec![],
    };
    let mission = FlowNodeDef {
        node_guid: 10,
        node_type_id: "mission".into(),
        category: NodeCategory::ScopedContainer,
        kind: NodeKind::MissionSubGraph {
            sub_graph_asset_id: "child".into(),
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    };
    content.root_asset.nodes = vec![mission, scene];
    content.root_asset.edges.clear();
    content.root_asset.start_node_guid = 10;
    content.mission_assets.insert("child".into(), child);
    content.dispatcher.storylets.push(StoryletDef {
        storylet_id: 1,
        payload_node_guid: 1,
        is_repeatable: true,
        ..Default::default()
    });
    content.dispatcher.rebuild_index();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    run.act(
        &content,
        0,
        RunAction::PlayStorylet {
            node_guid: 5,
            storylet: 1,
        },
    )
    .unwrap();
    assert_eq!(run.view(&content, 0).scene.unwrap().node_guid, 1);
    run.act(
        &content,
        0,
        RunAction::Choose {
            node_guid: 1,
            output_pin: 0,
        },
    )
    .unwrap();
    assert!(run.view(&content, 0).also_here.iter().any(|s| s.id == 1));
}

#[test]
fn selected_root_choice_is_not_routed_to_dormant_mission_copy() {
    use storylet_engine::chrono_runtime::NodeExecutionState;
    let mut content = fixture();
    let choice = content.root_asset.nodes[0].clone();
    let child = FlowGraphAsset { asset_id: "child".into(), start_node_guid: 1,
        nodes: vec![choice], edges: vec![], data_edges: vec![] };
    let hub = FlowNodeDef { node_guid: 5, node_type_id: "hub".into(),
        category: NodeCategory::DynamicDispatcher,
        kind: NodeKind::QbnHub { deck: vec![1], exit_storylet_id: None },
        input_pins: vec![pin("In", true)], output_pins: vec![pin("Out", false)],
        ..Default::default() };
    let mission = FlowNodeDef { node_guid: 10, node_type_id: "mission".into(),
        category: NodeCategory::ScopedContainer,
        kind: NodeKind::MissionSubGraph { sub_graph_asset_id: "child".into() },
        input_pins: vec![pin("In", true)], output_pins: vec![pin("Out", false)],
        ..Default::default() };
    content.root_asset.nodes.extend([hub, mission]);
    content.root_asset.start_node_guid = 5;
    content.mission_assets.insert("child".into(), child);
    content.dispatcher.storylets.push(StoryletDef { storylet_id: 1, payload_node_guid: 1,
        is_repeatable: true, ..Default::default() });
    content.dispatcher.rebuild_index();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    run.act(&content, 0, RunAction::PlayStorylet { node_guid: 5, storylet: 1 }).unwrap();
    run.act(&content, 0, RunAction::Choose { node_guid: 1, output_pin: 0 }).unwrap();
    assert_eq!(run.instance.mission_instance(10).unwrap().node(1).unwrap().state, NodeExecutionState::Idle);
    assert!(run.instance.completed_storylets.contains(1));
}

// --- Inspect: provenance and chronicle ---------------------------------------

use storylet_engine::chrono_chronicle::{ChronicleEntry, ChronicleTable};
use storylet_engine::chrono_participants::KnowKind;
use storylet_engine::chrono_runtime::Visibility;

fn mutator(guid: u32, kind: NodeKind) -> FlowNodeDef {
    FlowNodeDef {
        node_guid: guid,
        node_type_id: "mutator".into(),
        category: NodeCategory::Mutator,
        kind,
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..Default::default()
    }
}
fn edge(from: u32, from_pin: &str, to: u32) -> EdgeConnection {
    EdgeConnection {
        source_node_guid: from,
        source_pin_name: from_pin.into(),
        target_node_guid: to,
        target_pin_name: "In".into(),
    }
}

/// `choice(left) -> chain of mutators -> end`; `right -> end`.
fn chain_fixture(chain: Vec<NodeKind>) -> RunContent {
    let mut content = fixture();
    content.registry = GameplayTagRegistry::compile(["fact".to_string(), "fact.clue".to_string()]).unwrap();
    content.root_asset.edges.clear();
    let mut previous = (1u32, "left".to_string());
    for (offset, kind) in chain.into_iter().enumerate() {
        let guid = 10 + offset as u32;
        content.root_asset.nodes.push(mutator(guid, kind));
        content.root_asset.edges.push(edge(previous.0, &previous.1, guid));
        previous = (guid, "Out".to_string());
    }
    content.root_asset.edges.push(edge(previous.0, &previous.1, 2));
    content.root_asset.edges.push(edge(1, "right", 2));
    content
}
fn choose(pin: u16) -> RunAction {
    RunAction::Choose { node_guid: 1, output_pin: pin }
}

#[test]
fn inspect_marks_testimony_unconfirmed_and_links_source_step() {
    let content = chain_fixture(vec![NodeKind::Learn {
        fact: 1,
        kind: KnowKind::Testimony,
        visibility: Visibility::Private,
    }]);
    let mut run = RunKernel::start(&content, RunStart::Beginning, 3).unwrap();
    run.act(&content, 0, choose(0)).unwrap();
    let inspect = run.inspect(&content, 0);
    assert_eq!(inspect.knows.len(), 1);
    let row = &inspect.knows[0];
    assert_eq!(row.fact, "fact.clue");
    assert_eq!(row.kind, KnowKind::Testimony);
    assert!(!row.confirmed);
    assert_eq!(row.source_step, Some(1));
}

#[test]
fn inspect_evidence_is_confirmed() {
    let content = chain_fixture(vec![NodeKind::Learn {
        fact: 1,
        kind: KnowKind::Evidence,
        visibility: Visibility::Private,
    }]);
    let mut run = RunKernel::start(&content, RunStart::Beginning, 3).unwrap();
    run.act(&content, 0, choose(0)).unwrap();
    assert!(run.inspect(&content, 0).knows[0].confirmed);
}

fn reveal(entry: u16) -> NodeKind {
    NodeKind::Reveal {
        entry,
        kind: KnowKind::Recollection,
        visibility: Visibility::Private,
        facts: vec![],
    }
}
fn canon_entry(key: &str, story_time: i64) -> ChronicleEntry {
    ChronicleEntry {
        key: key.into(),
        story_time,
        label: key.to_uppercase(),
        location: None,
        facts: vec![],
        text: String::new(),
    }
}

#[test]
fn inspect_chronicle_numbers_told_entries_in_telling_order() {
    // Canon order after sorting: p1 (0), p2 (1), p3 (2). P2 is told first, then P1; P3 is never told.
    let mut content = chain_fixture(vec![reveal(1), reveal(0)]);
    content.chronicle = ChronicleTable::new(vec![canon_entry("p1", 10), canon_entry("p2", 20), canon_entry("p3", 30)]);
    let mut run = RunKernel::start(&content, RunStart::Beginning, 3).unwrap();
    run.act(&content, 0, choose(0)).unwrap();
    let rows = run.inspect(&content, 0).chronicle;
    let told: Vec<_> = rows.iter().map(|r| (r.key.as_str(), r.told)).collect();
    assert_eq!(told, [("p1", Some(2)), ("p2", Some(1)), ("p3", None)]);
    let times: Vec<_> = rows.iter().map(|r| r.story_time).collect();
    assert_eq!(times, [10, 20, 30]);
}

#[test]
fn inspect_chronicle_is_per_participant_and_empty_without_canon() {
    let content = chain_fixture(vec![]);
    let run = RunKernel::start(&content, RunStart::Beginning, 3).unwrap();
    assert!(run.inspect(&content, 0).chronicle.is_empty());
}

#[test]
fn view_exposes_the_hub_node_so_clients_can_play_also_here_storylets() {
    let content = support::weirs_crossing::content();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 7).unwrap();
    run.act(&content, 0, RunAction::Choose { node_guid: 1, output_pin: 0 }).unwrap();
    let view = run.view(&content, 0);
    assert!(!view.also_here.is_empty());
    // The advertised hub plus a listed storylet is a valid action.
    let hub = view.hub.expect("a hub is pending");
    assert_eq!(hub, 10);
    let first = view.also_here[0].id;
    run.act(&content, 0, RunAction::PlayStorylet { node_guid: hub, storylet: first }).unwrap();
    assert!(run.view(&content, 0).hub.is_none() || run.view(&content, 0).scene.is_some());
}

#[test]
fn view_has_no_hub_when_a_plain_choice_is_pending() {
    let content = fixture();
    let run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    assert!(run.view(&content, 0).hub.is_none());
}

#[test]
fn hub_does_not_offer_or_play_storylets_at_another_location() {
    let mut content = support::weirs_crossing::content();
    let mut missing_place = storylet_engine::chrono_state::TagBitset::new();
    let tag = content.registry.id("fact.logbook").unwrap();
    missing_place.set_bit(tag).unwrap();
    let storylet = support::weirs_crossing::LAMP_SOOT;
    content.storylet_location_tags.insert(storylet, missing_place);
    let mut run = RunKernel::start(&content, RunStart::Beginning, 7).unwrap();
    run.act(&content, 0, RunAction::Choose { node_guid: 1, output_pin: 0 }).unwrap();
    assert!(!run.view(&content, 0).also_here.iter().any(|s| s.id == storylet));
    assert!(run.act(&content, 0, RunAction::PlayStorylet { node_guid: 10, storylet }).is_err());
}

#[test]
fn predicting_a_destination_uses_its_location_without_moving_the_player() {
    let mut content = support::weirs_crossing::content();
    let tag = content.registry.id("fact.logbook").unwrap();
    content.initial_world.entity_tags[1].set_bit(tag).unwrap();
    let mut mask = storylet_engine::chrono_state::TagBitset::new();
    mask.set_bit(tag).unwrap();
    let storylet = support::weirs_crossing::LAMP_SOOT;
    content.storylet_location_tags.insert(storylet, mask);
    let mut run = RunKernel::start(&content, RunStart::Beginning, 7).unwrap();
    run.act(&content, 0, RunAction::Choose { node_guid: 1, output_pin: 0 }).unwrap();
    assert!(!run.view(&content, 0).also_here.iter().any(|s| s.id == storylet));
    assert!(run.storylets_at(&content, 0, 1).iter().any(|s| s.id == storylet));
    assert_eq!(run.world.active_location(0), Some(0));
}

#[test]
fn exploration_view_preserves_current_choices_without_copying_the_timeline() {
    let content = support::weirs_crossing::content();
    let mut run = RunKernel::start(&content, RunStart::Beginning, 7).unwrap();
    run.act(&content, 0, RunAction::Choose { node_guid: 1, output_pin: 0 }).unwrap();
    let full = run.view(&content, 0);
    let compact = run.exploration_view(&content, 0);
    assert_eq!(compact.scene, full.scene);
    assert_eq!(compact.also_here, full.also_here);
    assert_eq!(compact.place, full.place);
    assert_eq!(compact.hub, full.hub);
    assert_eq!(compact.current_step, full.current_step);
    assert!(compact.steps.is_empty() && compact.branches.is_empty());
}
