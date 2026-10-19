use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use storylet_engine::chrono_dispatch::{
    EffectKind, IdealMetricTarget, MetricGate, PredicateOperator, PredicateSubject,
    QualityPredicate, StoryletDef, StoryletEffect, UnifiedDispatcher, WaypointTopicEdge,
    WaypointTopicGraph, WaypointTopicNode,
};
use storylet_engine::chrono_runtime::{
    rebase_call_history, CallHistoryEntry, EdgeConnection, FlowGraphAsset, FlowGraphInstance,
    FlowNodeDef, GraphError, NodeCategory, NodeExecutionState, NodeKind, PinDefinition,
    RuntimeContext, SignalToken, DEFAULT_OUTPUT_PIN, TOKEN_ACTIVE,
};
use storylet_engine::chrono_state::{
    AttributeValue, EdgePattern, EdgeTemplate, GameplayTagRegistry, GraphBinding, GraphLayer,
    GraphMutation, GraphPattern, GraphTransformRule, MetricRegistry, MicroPassionDef, MutationOp,
    PassionDirection, PassionMutation, PassionRegistry, PassionThresholdRule, RivalPassionLink,
    StarNpcDef, TagBitset, VertexPattern, VertexTemplate, WorldStateTable, WorldVertex,
};

fn tags(names: &[&str]) -> GameplayTagRegistry {
    GameplayTagRegistry::compile(names.iter().map(|name| (*name).to_owned())).unwrap()
}

fn pin(name: &str, input: bool) -> PinDefinition {
    PinDefinition {
        pin_name: name.to_owned(),
        is_input: input,
    }
}

fn node(guid: u32, kind: NodeKind, category: NodeCategory) -> FlowNodeDef {
    FlowNodeDef {
        node_guid: guid,
        node_type_id: "chrono_acceptance".to_owned(),
        category,
        kind,
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
        ..FlowNodeDef::default()
    }
}

fn asset(nodes: Vec<FlowNodeDef>, start_node_guid: u32) -> FlowGraphAsset {
    FlowGraphAsset {
        asset_id: "chrono-v5-acceptance".to_owned(),
        start_node_guid,
        nodes,
        edges: vec![],
    }
}

fn state_snapshot(state: &WorldStateTable) -> Vec<u8> {
    serde_json::to_vec(state).expect("world state serializes")
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf03_8538_4dc2_d6fc);
    z = (z ^ (z >> 27)).wrapping_mul(0x85eb_ca6b_1b31_10d2);
    z ^ (z >> 31)
}

fn repeatable_storylet(id: u16) -> StoryletDef {
    StoryletDef {
        storylet_id: id,
        is_repeatable: true,
        ..StoryletDef::default()
    }
}

#[test]
fn test_rollback_identity_500_mutations() {
    let registry = tags(&[
        "case.open",
        "clue.alpha",
        "clue.beta",
        "quality.one",
        "quality.two",
        "passion.high",
    ]);
    let metric_registry = MetricRegistry::compile(["pressure".to_owned()]).unwrap();
    let mut storylets = Vec::new();
    for (offset, tag_name) in ["clue.alpha", "clue.beta", "quality.one", "quality.two"]
        .into_iter()
        .enumerate()
    {
        let mut def = repeatable_storylet(200 + offset as u16);
        def.on_complete_mutation
            .add_global_tags
            .set_bit(registry.id(tag_name).unwrap())
            .unwrap();
        storylets.push(def);
    }
    let mut dispatcher = UnifiedDispatcher::new(storylets);
    dispatcher.resolve(&registry, &metric_registry).unwrap();
    let passions = PassionRegistry::compile(
        [StarNpcDef {
            npc_id: 0,
            trust_inertia: 0.0,
            micro_passions: vec![
                MicroPassionDef {
                    npc_id: 0,
                    passion_id: 1,
                    weight: 1.0,
                    is_hidden: false,
                },
                MicroPassionDef {
                    npc_id: 0,
                    passion_id: 2,
                    weight: 1.0,
                    is_hidden: false,
                },
            ],
            macro_bounds: Default::default(),
            zero_sum_links: vec![RivalPassionLink {
                target_character_id: 1,
                source_passion_id: 1,
                target_passion_id: 3,
                weight: -1.0,
            }],
        },
        StarNpcDef {
            npc_id: 1,
            trust_inertia: 0.0,
            micro_passions: vec![MicroPassionDef {
                npc_id: 1,
                passion_id: 3,
                weight: 1.0,
                is_hidden: false,
            }],
            macro_bounds: Default::default(),
            zero_sum_links: vec![],
        }],
        [PassionThresholdRule {
            npc_id: 0,
            threshold_tag_bit: registry.id("passion.high").unwrap(),
            trigger_value: 0.5,
            direction: PassionDirection::Up,
            linked_graph_asset_id: "asset.rollback".to_owned(),
        }],
    )
    .unwrap();
    let mut state = WorldStateTable::new();
    let mut attributes = BTreeMap::new();
    attributes.insert(
        "origin".to_owned(),
        AttributeValue::String("seed".to_owned()),
    );
    state
        .apply_graph_transform(&GraphTransformRule {
            lhs: GraphPattern::default(),
            rhs: GraphMutation {
                add_vertices: vec![VertexTemplate {
                    id: GraphBinding::Literal("v.seed".to_owned()),
                    layer: GraphLayer::Location,
                    attributes,
                }],
                ..GraphMutation::default()
            },
        })
        .unwrap();
    let mut ctx = RuntimeContext::with_registries(&mut state, &dispatcher, &registry, &passions);
    let mut add = MutationOp::default();
    add.add_global_tags
        .set_bit(registry.id("case.open").unwrap())
        .unwrap();
    let add_temp = GraphTransformRule {
        lhs: GraphPattern::default(),
        rhs: GraphMutation {
            add_vertices: vec![VertexTemplate {
                id: GraphBinding::Literal("v.temp".to_owned()),
                layer: GraphLayer::Location,
                attributes: BTreeMap::new(),
            }],
            ..GraphMutation::default()
        },
    };
    let remove_temp = GraphTransformRule {
        lhs: GraphPattern::default(),
        rhs: GraphMutation {
            remove_vertices: vec![GraphBinding::Literal("v.temp".to_owned())],
            ..GraphMutation::default()
        },
    };
    let waypoint = WaypointTopicGraph {
        nodes: vec![
            WaypointTopicNode {
                topic_id: "A".to_owned(),
                revealed_clue_quality_id: Some("quality.one".to_owned()),
                ..WaypointTopicNode::default()
            },
            WaypointTopicNode {
                topic_id: "B".to_owned(),
                revealed_clue_quality_id: Some("quality.two".to_owned()),
                ..WaypointTopicNode::default()
            },
        ],
        edges: vec![
            WaypointTopicEdge {
                edge_id: "a-b".to_owned(),
                from_topic_id: "A".to_owned(),
                to_topic_id: "B".to_owned(),
                base_cost: 1.0,
                ..WaypointTopicEdge::default()
            },
            WaypointTopicEdge {
                edge_id: "b-a".to_owned(),
                from_topic_id: "B".to_owned(),
                to_topic_id: "A".to_owned(),
                base_cost: 1.0,
                ..WaypointTopicEdge::default()
            },
        ],
    };
    let graph = asset(
        vec![
            node(1, NodeKind::Mutation { op: add }, NodeCategory::Mutator),
            node(
                2,
                NodeKind::GraphTransform { rule: add_temp },
                NodeCategory::Mutator,
            ),
            node(
                3,
                NodeKind::GraphTransform { rule: remove_temp },
                NodeCategory::Mutator,
            ),
            node(
                4,
                NodeKind::PassionMutation {
                    op: PassionMutation {
                        micro_deltas: vec![(1, 0.1), (2, 1.0 / 3.0)],
                        active_ops: vec![(1, true), (2, true), (3, true)],
                    },
                },
                NodeCategory::Mutator,
            ),
            node(
                5,
                NodeKind::QbnHub {
                    deck: vec![200, 201, 202, 203],
                    exit_storylet_id: None,
                },
                NodeCategory::DynamicDispatcher,
            ),
            node(
                6,
                NodeKind::SalienceSelector {
                    pool: vec![200, 201, 202, 203],
                    fallback_storylet_id: Some(203),
                    fallback_output_pin: String::new(),
                },
                NodeCategory::DynamicDispatcher,
            ),
            node(
                7,
                NodeKind::WaypointConversation {
                    graph: waypoint,
                    initial_topic_id: "A".to_owned(),
                    npc_goal_topic_id: "B".to_owned(),
                },
                NodeCategory::DynamicDispatcher,
            ),
            node(
                8,
                NodeKind::SetMetric {
                    metric_id: 0,
                    delta: 0.25,
                },
                NodeCategory::Mutator,
            ),
        ],
        1,
    );
    let mut instance = FlowGraphInstance::new(1, graph, 17).unwrap();
    let before = state_snapshot(ctx.state);
    let mut rng = 0x4348_524f_4e4f_u64;
    let mut counts = [0usize; 8];
    let mut completed = 0;
    while completed < 500 {
        let candidate = (splitmix64(&mut rng) % 8) as u32 + 1;
        let guid = match candidate {
            2 if ctx.state.vertex("v.temp").is_some() => continue,
            3 if ctx.state.vertex("v.temp").is_none() => continue,
            7 if counts[6] != 0 => continue,
            _ => candidate,
        };
        match guid {
            5 => {
                let deck = instance.present_deck(&ctx, guid).unwrap();
                let id = deck[(splitmix64(&mut rng) as usize) % deck.len()];
                instance
                    .force_complete(&mut ctx, guid, DEFAULT_OUTPUT_PIN, Some(id))
                    .unwrap();
            }
            6 => instance
                .force_complete(&mut ctx, guid, DEFAULT_OUTPUT_PIN, None)
                .unwrap(),
            _ => instance
                .force_complete(&mut ctx, guid, DEFAULT_OUTPUT_PIN, None)
                .unwrap(),
        }
        counts[(guid - 1) as usize] += 1;
        completed += 1;
    }
    assert_eq!(instance.call_history().len(), 500);
    assert!(counts.iter().all(|count| *count > 0));
    assert_eq!(instance.reconcile_history(&mut ctx, &[]).unwrap(), 0);
    assert!(instance.call_history().is_empty());
    assert!(ctx.state.vertex("v.temp").is_none());
    for guid in 1..=8 {
        assert!(instance.node(guid).unwrap().call_data.is_empty());
    }
    assert_eq!(state_snapshot(ctx.state), before);
}

#[test]
fn test_invoke_equivalence_qbn_chaining() {
    let registry = tags(&[
        "case.open",
        "clue.means",
        "clue.motive",
        "clue.opportunity",
        "case.accused",
    ]);
    let metrics = MetricRegistry::empty();
    let mut definitions = Vec::new();
    for (id, name) in [
        (100, "clue.means"),
        (101, "clue.motive"),
        (102, "clue.opportunity"),
        (103, "case.accused"),
    ] {
        let mut def = repeatable_storylet(id);
        def.display_priority = match id {
            100 => 30,
            101 => 20,
            102 => 10,
            _ => 40,
        };
        def.required_tags
            .set_bit(registry.id("case.open").unwrap())
            .unwrap();
        if id == 103 {
            for clue in ["clue.means", "clue.motive", "clue.opportunity"] {
                def.required_tags
                    .set_bit(registry.id(clue).unwrap())
                    .unwrap();
            }
        }
        let bit = registry.id(name).unwrap();
        def.effects_on_complete.push(StoryletEffect {
            target_id: name.to_owned(),
            value: 1.0,
            kind: EffectKind::AddTag,
            target_index: Some(bit),
            tag_mask: registry.ancestors_mask(bit).unwrap().clone(),
        });
        definitions.push(def);
    }
    let mut dispatcher = UnifiedDispatcher::new(definitions);
    dispatcher.resolve(&registry, &metrics).unwrap();
    let mut open = MutationOp::default();
    open.add_global_tags
        .set_bit(registry.id("case.open").unwrap())
        .unwrap();
    let graph = FlowGraphAsset {
        asset_id: "chrono-v5-acceptance".to_owned(),
        start_node_guid: 1,
        nodes: vec![
            node(1, NodeKind::Mutation { op: open }, NodeCategory::Mutator),
            node(
                2,
                NodeKind::QbnHub {
                    deck: vec![102, 101, 100, 103],
                    exit_storylet_id: Some(103),
                },
                NodeCategory::DynamicDispatcher,
            ),
            node(
                3,
                NodeKind::Poi {
                    poi_id: "POI_End".to_owned(),
                },
                NodeCategory::LogicGate,
            ),
        ],
        edges: vec![
            EdgeConnection {
                source_node_guid: 1,
                source_pin_name: "Out".to_owned(),
                target_node_guid: 2,
                target_pin_name: "In".to_owned(),
            },
            EdgeConnection {
                source_node_guid: 2,
                source_pin_name: "Out".to_owned(),
                target_node_guid: 3,
                target_pin_name: "In".to_owned(),
            },
        ],
    };
    let mut normal_state = WorldStateTable::new();
    let mut normal = FlowGraphInstance::new(1, graph.clone(), 55).unwrap();
    let mut normal_ctx = RuntimeContext::with_registry(&mut normal_state, &dispatcher, &registry);
    normal
        .force_complete(&mut normal_ctx, 1, "Out", None)
        .unwrap();
    for id in [100, 101, 102, 103] {
        normal
            .force_complete(&mut normal_ctx, 2, "Out", Some(id))
            .unwrap();
    }
    normal
        .force_complete(&mut normal_ctx, 3, "Out", None)
        .unwrap();
    let normal_snapshot = state_snapshot(normal_ctx.state);
    let mut invoke_state = WorldStateTable::new();
    let mut invoke = FlowGraphInstance::new(2, graph.clone(), 55).unwrap();
    let mut invoke_ctx = RuntimeContext::with_registry(&mut invoke_state, &dispatcher, &registry);
    let mut warmup_state = WorldStateTable::new();
    let mut warmup = FlowGraphInstance::new(9, graph.clone(), 55).unwrap();
    let mut warmup_ctx = RuntimeContext::with_registry(&mut warmup_state, &dispatcher, &registry);
    warmup
        .invoke_story_to_poi(&mut warmup_ctx, 3, &BTreeMap::new())
        .unwrap();
    let start = Instant::now();
    let path = invoke
        .invoke_story_to_poi(&mut invoke_ctx, 3, &BTreeMap::new())
        .unwrap();
    let elapsed = start.elapsed();
    assert_eq!(path.len(), 5);
    assert_eq!(
        path.iter()
            .filter_map(|entry| entry.chosen_storylet_id)
            .collect::<Vec<_>>(),
        vec![100, 101, 102, 103]
    );
    assert_eq!(state_snapshot(invoke_ctx.state), normal_snapshot);
    // Wall-clock only in optimized builds; debug builds under parallel test
    // load are not a meaningful latency measurement.
    if !cfg!(debug_assertions) {
        assert!(elapsed.as_millis() < 50, "invoke took {elapsed:?}");
    }
}

#[test]
fn invoke_equivalence_with_all_fan_in() {
    let (dispatcher, registry) = runtime();
    let mut graph = asset(
        vec![
            node(1, noop_mutation(), NodeCategory::Mutator),
            node(2, noop_mutation(), NodeCategory::Mutator),
            node(3, noop_mutation(), NodeCategory::Mutator),
            node(5, NodeKind::LogicAll, NodeCategory::LogicGate),
            node(
                6,
                NodeKind::Poi {
                    poi_id: "POI_Merge".to_owned(),
                },
                NodeCategory::LogicGate,
            ),
        ],
        1,
    );
    let mut merge = graph.node(5).cloned().unwrap();
    merge.input_pins = vec![pin("In_0", true), pin("In_1", true)];
    *graph.nodes.iter_mut().find(|node| node.node_guid == 5).unwrap() = merge;
    graph.edges = vec![
        edge(1, "Out", 2, "In"),
        edge(1, "Out", 3, "In"),
        edge(2, "Out", 5, "In_0"),
        edge(3, "Out", 5, "In_1"),
        edge(5, "Out", 6, "In"),
    ];

    let mut live_state = WorldStateTable::new();
    let mut live = FlowGraphInstance::new(1, graph.clone(), 11).unwrap();
    let mut live_ctx = RuntimeContext::with_registry(&mut live_state, &dispatcher, &registry);
    live.start().unwrap();
    live.tick(&mut live_ctx, 16).unwrap();
    assert_eq!(
        live.node(6).unwrap().state,
        NodeExecutionState::Completed
    );
    assert_eq!(
        live.call_history()
            .iter()
            .map(|entry| entry.node_guid)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 5, 6]
    );
    let completed = state_snapshot(live_ctx.state);
    let history = live.call_history().to_vec();

    let mut invoke_state = WorldStateTable::new();
    let mut invoke = FlowGraphInstance::new(2, graph, 11).unwrap();
    let mut invoke_ctx =
        RuntimeContext::with_registry(&mut invoke_state, &dispatcher, &registry);
    invoke
        .invoke_story_to_poi(&mut invoke_ctx, 6, &BTreeMap::new())
        .unwrap();

    assert_eq!(invoke.node(6).unwrap().state, NodeExecutionState::Completed);
    assert_eq!(state_snapshot(invoke_ctx.state), completed);
    assert_eq!(
        invoke
            .call_history()
            .iter()
            .map(|entry| entry.node_guid)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 5, 6]
    );
    assert_eq!(invoke.call_history(), history.as_slice());
}

#[test]
fn invoke_declared_route_selects_one_branch() {
    let (dispatcher, registry) = runtime();
    let mut graph = asset(
        vec![
            node(1, noop_mutation(), NodeCategory::Mutator),
            node(2, noop_mutation(), NodeCategory::Mutator),
            node(3, noop_mutation(), NodeCategory::Mutator),
            node(4, noop_mutation(), NodeCategory::Mutator),
            node(
                6,
                NodeKind::Poi {
                    poi_id: "POI_Route".to_owned(),
                },
                NodeCategory::LogicGate,
            ),
        ],
        1,
    );
    let mut junction = graph.node(4).cloned().unwrap();
    junction.input_pins = vec![pin("In_0", true), pin("In_1", true)];
    *graph
        .nodes
        .iter_mut()
        .find(|node| node.node_guid == 4)
        .unwrap() = junction;
    graph.edges = vec![
        edge(1, "Out", 2, "In"),
        edge(1, "Out", 3, "In"),
        edge(2, "Out", 4, "In_0"),
        edge(3, "Out", 4, "In_1"),
        edge(4, "Out", 6, "In"),
    ];
    let mut state = WorldStateTable::new();
    let mut instance = FlowGraphInstance::new(1, graph, 13).unwrap();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    let routes = BTreeMap::from([(4u32, "In_1".to_owned())]);

    instance
        .invoke_story_to_poi(&mut ctx, 6, &routes)
        .unwrap();

    assert_eq!(
        instance
            .call_history()
            .iter()
            .map(|entry| entry.node_guid)
            .collect::<Vec<_>>(),
        vec![1, 3, 4, 6]
    );
    assert_eq!(
        instance.node(2).unwrap().state,
        NodeExecutionState::Idle,
        "the undeclared branch must not run"
    );
    assert_eq!(
        instance.node(6).unwrap().state,
        NodeExecutionState::Completed
    );
}

#[test]
fn test_salience_phase_gate_and_fallback() {
    const N: usize = 100;
    let phase_bits = [200, 201, 202];
    let mut pool = Vec::new();
    for id in 0..N {
        let mut def = repeatable_storylet(id as u16);
        if id != 99 {
            def.required_tags.set_bit(id as u16).unwrap();
            let phase = if id >= 60 {
                2
            } else if id >= 40 {
                1
            } else {
                0
            };
            def.required_arc_phases.set_bit(phase_bits[phase]).unwrap();
            let mut mask = TagBitset::new();
            mask.set_bit(id as u16).unwrap();
            def.preconditions.push(QualityPredicate {
                target_id: format!("tag.{id}"),
                subject: PredicateSubject::Location,
                operator: PredicateOperator::Eq,
                comparison_value: 0.0,
                specificity_weight: 1000.0 + id as f64,
                entity_index: None,
                target_index: Some(id as u16),
                tag_mask: mask,
            });
        }
        pool.push(def);
    }
    let dispatcher = UnifiedDispatcher::new(pool);
    let graph = asset(
        vec![node(
            10,
            NodeKind::SalienceSelector {
                pool: (0..N as u16).collect(),
                fallback_storylet_id: Some(99),
                fallback_output_pin: String::new(),
            },
            NodeCategory::DynamicDispatcher,
        )],
        10,
    );
    let mut counts = [0usize; N];
    let mut rng = 0x5155_414e_5449_5a45_u64;
    for trajectory in 0..5_000 {
        let target = trajectory % N;
        let phase = if target >= 60 {
            2
        } else if target >= 40 {
            1
        } else {
            0
        };
        let mut state = WorldStateTable::new();
        state.global_tags.set_bit(phase_bits[phase]).unwrap();
        state.global_tags.set_bit(target as u16).unwrap();
        for id in 0..target {
            let other_phase = if id >= 60 {
                2
            } else if id >= 40 {
                1
            } else {
                0
            };
            if other_phase == phase && splitmix64(&mut rng) & 1 == 0 {
                state.global_tags.set_bit(id as u16).unwrap();
            }
        }
        for id in 0..N {
            let other_phase = if id >= 60 {
                2
            } else if id >= 40 {
                1
            } else {
                0
            };
            if other_phase != phase && splitmix64(&mut rng) & 3 == 0 {
                state.global_tags.set_bit(id as u16).unwrap();
            }
        }
        let mut instance =
            FlowGraphInstance::new(trajectory as u64, graph.clone(), trajectory as u64).unwrap();
        let mut ctx = RuntimeContext::new(&mut state, &dispatcher);
        let coverage = instance.salience_coverage(&ctx, 10).unwrap();
        // 99 is the fallback and intentionally has no arc-phase mask, so it is
        // not an eligible candidate.
        if target != 99 {
            assert!(coverage.eligible.contains(&(target as u16)));
        }
        assert!(coverage.has_fallback);
        instance.force_complete(&mut ctx, 10, "Out", None).unwrap();
        let chosen = instance.node(10).unwrap().chosen_storylet_id.unwrap() as usize;
        assert!(
            chosen == 99
                || (chosen < 40 && phase == 0)
                || (40..60).contains(&chosen) && phase == 1
                || (60..99).contains(&chosen) && phase == 2
        );
        counts[chosen] += 1;
    }
    assert_eq!(counts.iter().sum::<usize>(), 5_000);
    assert!(counts[..40].iter().sum::<usize>() > 0);
    assert!(counts[40..60].iter().sum::<usize>() > 0);
    assert!(counts[60..99].iter().sum::<usize>() > 0);

    let orphaned: Vec<usize> = counts
        .iter()
        .enumerate()
        .filter_map(|(storylet_id, activations)| (*activations == 0).then_some(storylet_id))
        .collect();
    println!("Salience activation histogram: {counts:?}");
    println!("Orphaned storylet IDs (zero activations): {orphaned:?}");
    let mut state = WorldStateTable::new();
    let mut instance = FlowGraphInstance::new(1, graph.clone(), 1).unwrap();
    let mut ctx = RuntimeContext::new(&mut state, &dispatcher);
    instance.force_complete(&mut ctx, 10, "Out", None).unwrap();
    assert_eq!(instance.node(10).unwrap().chosen_storylet_id, Some(99));
    let no_fallback = asset(
        vec![node(
            11,
            NodeKind::SalienceSelector {
                pool: (0..99u16).collect(),
                fallback_storylet_id: None,
                fallback_output_pin: String::new(),
            },
            NodeCategory::DynamicDispatcher,
        )],
        11,
    );
    let mut state = WorldStateTable::new();
    let mut instance = FlowGraphInstance::new(2, no_fallback, 1).unwrap();
    let mut ctx = RuntimeContext::new(&mut state, &dispatcher);
    assert!(matches!(
        instance.force_complete(&mut ctx, 11, "Out", None),
        Err(GraphError::NoSalienceMatch { .. })
    ));
}

#[test]
fn test_waypoint_diversion_and_rollback() {
    let registry = tags(&[
        "quality.god",
        "quality.people",
        "clue.popular_unrest",
        "quality.revolution",
    ]);
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let topic = |id: &str, quality: &str, player_only: bool| WaypointTopicNode {
        topic_id: id.to_owned(),
        revealed_clue_quality_id: Some(quality.to_owned()),
        is_player_only_trigger: player_only,
        ..WaypointTopicNode::default()
    };
    let edge = |id: &str, from: &str, to: &str, cost: f64| WaypointTopicEdge {
        edge_id: id.to_owned(),
        from_topic_id: from.to_owned(),
        to_topic_id: to.to_owned(),
        base_cost: cost,
        used_penalty_cost: 100.0,
        ..WaypointTopicEdge::default()
    };
    let graph = WaypointTopicGraph {
        nodes: vec![
            topic("ThePeople", "quality.people", false),
            topic("God", "quality.god", false),
            topic("Government", "clue.popular_unrest", false),
            topic("Royalty", "quality.people", false),
            topic("Revolution", "quality.revolution", true),
        ],
        edges: vec![
            edge("e_people_god", "ThePeople", "God", 1.0),
            edge("e_god_royalty", "God", "Royalty", 1.0),
            edge("e_god_people", "God", "ThePeople", 1.0),
            edge("e_people_government", "ThePeople", "Government", 3.0),
            edge("e_government_royalty", "Government", "Royalty", 1.0),
            edge("e_government_revolution", "Government", "Revolution", 1.0),
        ],
    };
    let flow = asset(
        vec![node(
            7,
            NodeKind::WaypointConversation {
                graph,
                initial_topic_id: "ThePeople".to_owned(),
                npc_goal_topic_id: "Royalty".to_owned(),
            },
            NodeCategory::DynamicDispatcher,
        )],
        7,
    );
    let mut state = WorldStateTable::new();
    let mut instance = FlowGraphInstance::new(1, flow, 9).unwrap();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.start().unwrap();
    instance.tick(&mut ctx, 0).unwrap();
    assert_eq!(
        instance.node(7).unwrap().state,
        NodeExecutionState::ActiveLatent
    );
    assert_eq!(instance.node(7).unwrap().current_topic_id.as_str(), "God");
    assert!(matches!(
        instance.divert_topic(&mut ctx, 7, "e_government_revolution"),
        Err(GraphError::TopicEdgeNotAdjacent { .. })
    ));
    instance.divert_topic(&mut ctx, 7, "e_god_people").unwrap();
    assert_eq!(
        instance.next_topic_step(&ctx, 7).unwrap().edge_id,
        "e_people_government"
    );
    instance.force_complete(&mut ctx, 7, "Out", None).unwrap();
    assert_eq!(
        instance.node(7).unwrap().current_topic_id.as_str(),
        "Government"
    );
    assert!(ctx
        .state
        .global_tags
        .bit(registry.id("clue.popular_unrest").unwrap()));
    assert_eq!(
        instance.next_topic_step(&ctx, 7).unwrap().edge_id,
        "e_government_royalty"
    );
    instance
        .divert_topic(&mut ctx, 7, "e_government_revolution")
        .unwrap();
    assert_eq!(
        instance.node(7).unwrap().current_topic_id.as_str(),
        "Revolution"
    );
    for (topic, used_edge) in [
        ("Government", "e_government_revolution"),
        ("ThePeople", "e_people_government"),
        ("God", "e_god_people"),
        ("ThePeople", "e_people_god"),
    ] {
        instance.cancel_and_revert(&mut ctx, 7).unwrap();
        assert_eq!(instance.node(7).unwrap().current_topic_id.as_str(), topic);
        assert_eq!(
            instance
                .node(7)
                .unwrap()
                .edge_use_counts
                .get(used_edge)
                .copied()
                .unwrap_or(0),
            0
        );
    }
    assert!(instance
        .node(7)
        .unwrap()
        .edge_use_counts
        .values()
        .all(|count| *count == 0));
    assert!(!ctx
        .state
        .global_tags
        .bit(registry.id("clue.popular_unrest").unwrap()));
    assert_eq!(
        instance.next_topic_step(&ctx, 7).unwrap().edge_id,
        "e_people_god"
    );
    assert!(matches!(
        instance.cancel_and_revert(&mut ctx, 7),
        Err(GraphError::RevertStackEmpty { .. })
    ));
}
#[test]
fn qbn_completion_supports_storylet_ids_above_256() {
    let completed_id = 300;
    let available_id = 301;
    let dispatcher = UnifiedDispatcher::new(vec![
        StoryletDef {
            storylet_id: completed_id,
            ..StoryletDef::default()
        },
        StoryletDef {
            storylet_id: available_id,
            ..StoryletDef::default()
        },
    ]);
    let completed = BTreeSet::from([completed_id]);

    assert_eq!(
        dispatcher.query_qbn_deck(
            &[completed_id, available_id],
            &WorldStateTable::new(),
            &completed
        ),
        vec![available_id]
    );
}
#[test]
fn passion_threshold_events_activate_and_cancel_on_rollback() {
    let registry = tags(&["passion.high"]);
    let threshold_tag_bit = registry.id("passion.high").unwrap();
    let passions = PassionRegistry::compile(
        [StarNpcDef {
            npc_id: 0,
            trust_inertia: 0.0,
            micro_passions: vec![MicroPassionDef {
                npc_id: 0,
                passion_id: 0,
                weight: 1.0,
                is_hidden: false,
            }],
            macro_bounds: Default::default(),
            zero_sum_links: vec![],
        }],
        [PassionThresholdRule {
            npc_id: 0,
            threshold_tag_bit,
            trigger_value: 0.5,
            direction: PassionDirection::Up,
            linked_graph_asset_id: "asset.romance".to_owned(),
        }],
    )
    .unwrap();
    let dispatcher = UnifiedDispatcher::new(vec![]);
    let mut state = WorldStateTable::new();
    state.passions.micro[0] = 0.0;
    state.passions.macro_values[0] = 0.0;
    state.passions.active[0] = true;
    let mut ctx = RuntimeContext::with_registries(&mut state, &dispatcher, &registry, &passions);
    let graph = asset(
        vec![node(
            1,
            NodeKind::PassionMutation {
                op: PassionMutation {
                    micro_deltas: vec![(0, 1.0)],
                    active_ops: vec![],
                },
            },
            NodeCategory::Mutator,
        )],
        1,
    );
    let mut instance = FlowGraphInstance::new(1, graph, 17).unwrap();

    instance.force_complete(&mut ctx, 1, "Out", None).unwrap();
    assert_eq!(ctx.state.passions.macro_values[0], 1.0);
    assert!(ctx.state.global_tags.bit(threshold_tag_bit));
    let activated = instance.drain_passion_threshold_events();
    assert_eq!(activated.len(), 1);
    assert!(activated[0].activated);
    assert_eq!(activated[0].linked_graph_asset_id, "asset.romance");
    assert!(ctx.state.global_tags.bit(threshold_tag_bit));
    let history = instance.call_history().to_vec();
    let mut replay_state = WorldStateTable::new();
    replay_state.passions.micro[0] = 0.0;
    replay_state.passions.macro_values[0] = 0.0;
    replay_state.passions.active[0] = true;
    let mut replay_ctx =
        RuntimeContext::with_registries(&mut replay_state, &dispatcher, &registry, &passions);
    let mut restored = FlowGraphInstance::new(2, instance.asset.clone(), 17).unwrap();
    restored.replay_history(&mut replay_ctx, &history).unwrap();
    assert!(restored.drain_passion_threshold_events().is_empty());
    assert!(replay_ctx.state.global_tags.bit(threshold_tag_bit));
    restored.cancel_and_revert(&mut replay_ctx, 1).unwrap();
    let replay_rollback = restored.drain_passion_threshold_events();
    assert_eq!(replay_rollback.len(), 1);
    assert!(!replay_rollback[0].activated);

    instance.cancel_and_revert(&mut ctx, 1).unwrap();
    let cancelled = instance.drain_passion_threshold_events();
    assert_eq!(cancelled.len(), 1);
    assert!(!cancelled[0].activated);
    assert_eq!(cancelled[0].node_guid, 1);
    assert!(!ctx.state.global_tags.bit(threshold_tag_bit));
}
#[test]
fn missing_metric_gate_bounds_round_trip_as_finite_unbounded_values() {
    let gate: MetricGate = serde_json::from_str(r#"{"metric_id":7}"#).unwrap();
    assert_eq!(gate.min, f32::MIN);
    assert_eq!(gate.max, f32::MAX);

    let json = serde_json::to_string(&gate).unwrap();
    let restored: MetricGate = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, gate);
}

#[test]
fn graph_transform_assets_and_world_attributes_round_trip() {
    let mut state = WorldStateTable::new();
    state.vertices.push(WorldVertex {
        vertex_id: "v.attribute".to_owned(),
        layer: GraphLayer::Location,
        entity_index: None,
        attributes: BTreeMap::from([
            ("count".to_owned(), AttributeValue::Int(3)),
            (
                "label".to_owned(),
                AttributeValue::String("market".to_owned()),
            ),
            ("open".to_owned(), AttributeValue::Bool(true)),
            ("price".to_owned(), AttributeValue::Float(1.25)),
            ("kind".to_owned(), AttributeValue::Tag(7)),
        ]),
    });
    let state_json = serde_json::to_value(&state).unwrap();
    let restored_state: WorldStateTable = serde_json::from_value(state_json.clone()).unwrap();
    assert_eq!(serde_json::to_value(restored_state).unwrap(), state_json);

    let rule = GraphTransformRule {
        lhs: GraphPattern {
            vertices: vec![Default::default()],
            ..GraphPattern::default()
        },
        rhs: GraphMutation {
            add_vertices: vec![VertexTemplate {
                id: GraphBinding::Literal("v.added".to_owned()),
                layer: GraphLayer::Fact,
                attributes: BTreeMap::from([(
                    "source".to_owned(),
                    AttributeValue::String("transform".to_owned()),
                )]),
            }],
            remove_vertices: vec![GraphBinding::VertexPattern(0)],
            ..GraphMutation::default()
        },
    };
    let graph = asset(
        vec![node(
            1,
            NodeKind::GraphTransform { rule },
            NodeCategory::Mutator,
        )],
        1,
    );
    let graph_json = serde_json::to_value(&graph).unwrap();
    let restored_graph: FlowGraphAsset = serde_json::from_value(graph_json.clone()).unwrap();
    assert_eq!(serde_json::to_value(restored_graph).unwrap(), graph_json);
}

fn edge(from: u32, from_pin: &str, to: u32, to_pin: &str) -> EdgeConnection {
    EdgeConnection {
        source_node_guid: from,
        source_pin_name: from_pin.to_owned(),
        target_node_guid: to,
        target_pin_name: to_pin.to_owned(),
    }
}

fn noop_mutation() -> NodeKind {
    NodeKind::Mutation {
        op: MutationOp::default(),
    }
}

fn runtime() -> (UnifiedDispatcher, GameplayTagRegistry) {
    (
        UnifiedDispatcher::new(Vec::new()),
        GameplayTagRegistry::compile(Vec::new()).unwrap(),
    )
}

#[test]
fn zero_cost_cycle_is_rejected_at_load() {
    let mut graph = asset(
        vec![
            node(1, noop_mutation(), NodeCategory::Mutator),
            node(2, noop_mutation(), NodeCategory::Mutator),
        ],
        1,
    );
    graph.edges = vec![edge(1, "Out", 2, "In"), edge(2, "Out", 1, "In")];

    assert!(matches!(
        FlowGraphInstance::new(1, graph, 7),
        Err(GraphError::ZeroCostCycle { .. })
    ));
}

#[test]
fn costed_node_executes_from_the_start_token() {
    let (dispatcher, registry) = runtime();
    let mut first = node(1, noop_mutation(), NodeCategory::Mutator);
    first.cost_ms = 5;
    let graph = asset(vec![first], 1);
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.start_at_node(1).unwrap();

    let executed = instance.tick(&mut ctx, 16).unwrap();

    assert_eq!(executed.len(), 1);
    assert_eq!(instance.call_history().len(), 1);
}

#[test]
fn node_costlier_than_a_frame_still_makes_progress() {
    let (dispatcher, registry) = runtime();
    let mut first = node(1, noop_mutation(), NodeCategory::Mutator);
    first.cost_ms = 50;
    let graph = asset(vec![first], 1);
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.start_at_node(1).unwrap();

    assert_eq!(instance.tick(&mut ctx, 16).unwrap().len(), 1);
    assert_eq!(instance.call_history().len(), 1);
}

#[test]
fn logic_all_completes_when_every_input_pin_arrives() {
    let (dispatcher, registry) = runtime();
    let mut merge = node(
        2,
        NodeKind::LogicAll,
        NodeCategory::LogicGate,
    );
    merge.input_pins = vec![pin("In_0", true), pin("In_1", true)];
    let mut graph = asset(
        vec![node(1, noop_mutation(), NodeCategory::Mutator), merge],
        1,
    );
    graph.edges = vec![
        edge(1, "Out", 2, "In_0"),
        edge(1, "Out", 2, "In_1"),
    ];
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.start_at_node(1).unwrap();

    instance.tick(&mut ctx, 16).unwrap();

    assert_eq!(
        instance.node(2).unwrap().state,
        NodeExecutionState::Completed
    );
    assert_eq!(instance.call_history().len(), 2);
    assert_eq!(instance.call_history()[0].node_guid, 1);
    assert_eq!(instance.call_history()[1].node_guid, 2);
}

#[test]
fn replay_preserves_fan_in_input_pins() {
    let (dispatcher, registry) = runtime();
    let mut merge = node(2, NodeKind::LogicAll, NodeCategory::LogicGate);
    merge.input_pins = vec![pin("In_0", true), pin("In_1", true)];
    let mut graph = asset(
        vec![node(1, noop_mutation(), NodeCategory::Mutator), merge],
        1,
    );
    graph.edges = vec![
        edge(1, "Out", 2, "In_0"),
        edge(1, "Out", 2, "In_1"),
    ];
    let mut instance = FlowGraphInstance::new(1, graph.clone(), 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.start_at_node(1).unwrap();
    instance.tick(&mut ctx, 16).unwrap();
    let history = instance.call_history().to_vec();

    let mut restored = FlowGraphInstance::new(2, graph, 7).unwrap();
    let mut restored_state = WorldStateTable::new();
    let mut restored_ctx =
        RuntimeContext::with_registry(&mut restored_state, &dispatcher, &registry);
    restored.replay_history(&mut restored_ctx, &history).unwrap();

    assert_eq!(restored.call_history(), history.as_slice());
    assert_eq!(
        restored.node(2).unwrap().state,
        NodeExecutionState::Completed
    );
}

#[test]
fn reconcile_diverges_at_the_lowest_common_ancestor() {
    let (dispatcher, registry) = runtime();
    let mut graph = asset(
        vec![
            node(1, noop_mutation(), NodeCategory::Mutator),
            node(2, noop_mutation(), NodeCategory::Mutator),
            node(3, noop_mutation(), NodeCategory::Mutator),
        ],
        1,
    );
    graph.edges = vec![edge(1, "Out", 2, "In"), edge(2, "Out", 3, "In")];
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.force_complete(&mut ctx, 1, "Out", None).unwrap();
    instance.force_complete(&mut ctx, 2, "Out", None).unwrap();
    let head = instance.call_history().to_vec();
    let mut target = head.clone();
    target.push(CallHistoryEntry {
        node_guid: 3,
        executed_input_pin: "In".to_owned(),
        chosen_output_pin: "Out".to_owned(),
        chosen_storylet_id: None,
    });

    let new_head = instance.reconcile_history(&mut ctx, &target).unwrap();

    assert_eq!(instance.history.depth(new_head).unwrap(), 3);
    assert_eq!(instance.call_history(), target.as_slice());
}

#[test]
fn checkout_replays_without_duplicating_history_entries() {
    let (dispatcher, registry) = runtime();
    let mut graph = asset(
        vec![
            node(1, noop_mutation(), NodeCategory::Mutator),
            node(2, noop_mutation(), NodeCategory::Mutator),
            node(3, noop_mutation(), NodeCategory::Mutator),
        ],
        1,
    );
    graph.edges = vec![edge(1, "Out", 2, "In"), edge(2, "Out", 3, "In")];
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.force_complete(&mut ctx, 1, "Out", None).unwrap();
    instance.force_complete(&mut ctx, 2, "Out", None).unwrap();
    instance.force_complete(&mut ctx, 3, "Out", None).unwrap();
    let full = instance.call_history().to_vec();
    assert_eq!(full.len(), 3);

    let commits_before = instance.history.commits.len();
    instance.checkout(&mut ctx, 1).unwrap();
    assert_eq!(instance.call_history(), &full[..1]);
    assert_eq!(instance.history.commits.len(), commits_before);

    instance.checkout(&mut ctx, 3).unwrap();
    assert_eq!(instance.call_history(), full.as_slice());
    assert_eq!(instance.history.commits.len(), commits_before);
}

#[test]
fn salience_ideal_weights_change_the_winner() {
    let registry = tags(&["bonus"]);
    let metric_registry = MetricRegistry::compile(["pressure".to_owned()]).unwrap();
    let build = |weight: f32| {
        let mut weighted = repeatable_storylet(1);
        weighted.required_arc_phases.set_bit(0).unwrap();
        weighted.ideal_vector.push(IdealMetricTarget {
            metric_id: 0,
            target: 0.0,
            weight,
        });
        weighted.preconditions.push(QualityPredicate {
            target_id: "bonus".to_owned(),
            subject: PredicateSubject::Location,
            operator: PredicateOperator::Eq,
            comparison_value: 0.0,
            specificity_weight: 2.0,
            entity_index: None,
            target_index: None,
            tag_mask: TagBitset::new(),
        });
        let mut plain = repeatable_storylet(2);
        plain.required_arc_phases.set_bit(0).unwrap();
        plain.ideal_vector.push(IdealMetricTarget {
            metric_id: 0,
            target: 0.0,
            weight: 1.0,
        });
        let mut dispatcher = UnifiedDispatcher::new(vec![weighted, plain]);
        dispatcher.resolve(&registry, &metric_registry).unwrap();
        dispatcher
    };
    let mut table = WorldStateTable::new();
    table.metrics[0] = 100.0;
    table.global_tags.set_bit(0).unwrap();
    table
        .global_tags
        .set_bit(registry.id("bonus").unwrap())
        .unwrap();
    let completed = BTreeSet::new();

    // With weight 4 on the specific candidate, its distance penalty loses.
    let heavy = build(4.0);
    assert_eq!(
        heavy.match_most_salient(&[1, 2], &table, &completed, 7),
        Some(2)
    );

    // With equal weights, the predicate specificity makes candidate 1 win.
    let equal = build(1.0);
    assert_eq!(
        equal.match_most_salient(&[1, 2], &table, &completed, 7),
        Some(1)
    );
}

#[test]
fn signal_token_is_flat() {
    assert!(std::mem::size_of::<SignalToken>() <= 32);
    assert_eq!(std::mem::align_of::<SignalToken>(), 8);

    let mut token = SignalToken {
        token_id: 1,
        graph_instance_id: 2,
        target_node_guid: 3,
        target_pin_index: 4,
        flags: 0,
        time_budget_ms: 16,
        deferred_until_ms: 0,
    };
    assert!(!token.is_active());
    token.set_active(true);
    token.set_deferred(true);
    assert!(token.is_active() && token.is_deferred());
    token.set_active(false);
    token.set_deferred(false);
    assert!(!token.is_active() && !token.is_deferred());
}

#[test]
fn zero_budget_tick_defers_positive_cost_work() {
    let (dispatcher, registry) = runtime();
    let mut first = node(1, noop_mutation(), NodeCategory::Mutator);
    first.cost_ms = 5;
    let graph = asset(vec![first], 1);
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.start_at_node(1).unwrap();

    assert!(instance.tick(&mut ctx, 0).unwrap().is_empty());
    assert!(instance.call_history().is_empty());
    assert_eq!(instance.tick(&mut ctx, 16).unwrap().len(), 1);
}

#[test]
fn scene_reentry_rollback_restores_the_prior_clock() {
    use storylet_engine::chrono_runtime::TimelineTrackEvent;

    let (dispatcher, registry) = runtime();
    let graph = asset(
        vec![node(
            1,
            NodeKind::SceneSection {
                section_duration_ms: 100,
                events: vec![TimelineTrackEvent {
                    event_id: 1,
                    start_time_ms: 10,
                    duration_ms: 10,
                    payload_asset_id: "pose.a".to_owned(),
                    ..TimelineTrackEvent::default()
                }],
            },
            NodeCategory::Mutator,
        )],
        1,
    );
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.process_time_window(1, 100).unwrap();
    assert_eq!(instance.node(1).unwrap().section_clock_ms, 100);

    let token = SignalToken {
        token_id: 0,
        graph_instance_id: instance.instance_id,
        target_node_guid: 1,
        target_pin_index: 0,
        time_budget_ms: 100,
        flags: TOKEN_ACTIVE,
        deferred_until_ms: 0,
    };
    instance.execute_input(&mut ctx, 1, "In", &token).unwrap();
    assert_eq!(instance.node(1).unwrap().section_clock_ms, 0);

    instance.cancel_and_revert(&mut ctx, 1).unwrap();
    assert_eq!(instance.node(1).unwrap().section_clock_ms, 100);
}

#[test]
fn bridge_path_walks_every_hop_and_orders_pins() {
    let mut graph = asset(
        vec![
            node(1, noop_mutation(), NodeCategory::Mutator),
            node(2, noop_mutation(), NodeCategory::Mutator),
            node(3, noop_mutation(), NodeCategory::Mutator),
        ],
        1,
    );
    graph.edges = vec![edge(1, "Out", 2, "In"), edge(2, "Out", 3, "In")];

    let steps = graph.shortest_bridge_path(1, 3).unwrap();
    assert_eq!(
        steps,
        vec![(2, "In".to_owned(), "Out".to_owned())]
    );

    let dispatcher = UnifiedDispatcher::new(Vec::new());
    let saved = vec![
        CallHistoryEntry {
            node_guid: 1,
            executed_input_pin: "In".to_owned(),
            chosen_output_pin: "Out".to_owned(),
            chosen_storylet_id: None,
        },
        CallHistoryEntry {
            node_guid: 3,
            executed_input_pin: "In".to_owned(),
            chosen_output_pin: "Out".to_owned(),
            chosen_storylet_id: None,
        },
    ];
    let rebased = rebase_call_history(&graph, &dispatcher, &saved).unwrap();
    assert_eq!(
        rebased.iter().map(|entry| entry.node_guid).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(rebased[1].executed_input_pin, "In");
    assert_eq!(rebased[1].chosen_output_pin, "Out");
}

#[test]
fn select_storylet_rejects_ids_outside_the_pool() {
    let registry = tags(&["case.open"]);
    let metric_registry = MetricRegistry::empty();
    let mut dispatcher = UnifiedDispatcher::new(vec![
        repeatable_storylet(5),
        repeatable_storylet(6),
    ]);
    dispatcher.resolve(&registry, &metric_registry).unwrap();
    let mut hub = node(
        1,
        NodeKind::QbnHub {
            deck: vec![5],
            exit_storylet_id: None,
        },
        NodeCategory::DynamicDispatcher,
    );
    hub.output_pins = vec![
        pin("choice_5", false),
        pin("choice_6", false),
        pin("Out", false),
    ];
    let graph = asset(vec![hub], 1);
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    state
        .global_tags
        .set_bit(registry.id("case.open").unwrap())
        .unwrap();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);

    assert_eq!(
        instance.select_storylet(&mut ctx, 1, 6),
        Err(GraphError::StoryletNotInDeck {
            node_guid: 1,
            storylet_id: 6,
        })
    );
}

#[test]
fn select_storylet_rejects_ineligible_deck_members() {
    let registry = tags(&["case.open", "case.closed"]);
    let metric_registry = MetricRegistry::empty();
    let mut locked = repeatable_storylet(5);
    locked.required_tags.set_bit(registry.id("case.closed").unwrap()).unwrap();
    let mut dispatcher = UnifiedDispatcher::new(vec![locked]);
    dispatcher.resolve(&registry, &metric_registry).unwrap();
    let mut hub = node(
        1,
        NodeKind::QbnHub {
            deck: vec![5],
            exit_storylet_id: None,
        },
        NodeCategory::DynamicDispatcher,
    );
    hub.output_pins = vec![pin("choice_5", false), pin("Out", false)];
    let graph = asset(vec![hub], 1);
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    state
        .global_tags
        .set_bit(registry.id("case.open").unwrap())
        .unwrap();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);

    assert_eq!(
        instance.select_storylet(&mut ctx, 1, 5),
        Err(GraphError::StoryletNotPlayable {
            node_guid: 1,
            storylet_id: 5,
        })
    );
}

#[test]
fn cancel_keeps_the_mission_scope_for_reconcile_replay() {
    let (dispatcher, registry) = runtime();
    let mut child = asset(vec![node(10, noop_mutation(), NodeCategory::Mutator)], 10);
    child.asset_id = "child-asset".to_owned();
    let mut parent = asset(
        vec![
            node(1, noop_mutation(), NodeCategory::Mutator),
            node(
                2,
                NodeKind::MissionSubGraph {
                    sub_graph_asset_id: "child-asset".to_owned(),
                },
                NodeCategory::ScopedContainer,
            ),
        ],
        1,
    );
    parent.edges = vec![edge(1, "Out", 2, "In")];
    let mut instance = FlowGraphInstance::new(1, parent, 1).unwrap();
    instance
        .attach_mission_asset(2, &child, 1)
        .unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.start().unwrap();
    instance.tick(&mut ctx, 0).unwrap();
    let target = instance.call_history().to_vec();
    assert_eq!(target.len(), 2);
    assert_eq!(instance.node(2).unwrap().local_call_history.len(), 1);

    let reentry = SignalToken {
        token_id: 99,
        graph_instance_id: instance.instance_id,
        target_node_guid: 2,
        target_pin_index: 0,
        flags: TOKEN_ACTIVE,
        time_budget_ms: 16,
        deferred_until_ms: 0,
    };
    assert!(matches!(
        instance.execute_input(&mut ctx, 2, "In", &reentry),
        Err(GraphError::MissionReentry { node_guid: 2 })
    ));

    instance.cancel_and_revert(&mut ctx, 2).unwrap();
    assert!(
        !instance.node(2).unwrap().local_call_history.is_empty(),
        "the scoped array must survive cancel for replay"
    );

    instance.reconcile_history(&mut ctx, &target).unwrap();
    assert_eq!(instance.node(2).unwrap().state, NodeExecutionState::Completed);
    assert_eq!(instance.node(2).unwrap().local_call_history.len(), 1);

    // A fresh start must not carry the old mirror into a new timeline.
    instance.reset();
    assert!(instance.node(2).unwrap().local_call_history.is_empty());
}

#[test]
fn salience_rejects_candidates_without_an_arc_phase() {
    let registry = tags(&["tag.match", "phase.act1"]);
    let metric_registry = MetricRegistry::empty();
    let phase_bit = registry.id("phase.act1").unwrap();
    let mut gated_def = repeatable_storylet(1);
    gated_def
        .required_tags
        .set_bit(registry.id("tag.match").unwrap())
        .unwrap();
    gated_def.required_arc_phases.set_bit(phase_bit).unwrap();
    let mut fallback_dispatcher =
        UnifiedDispatcher::new(vec![gated_def, repeatable_storylet(2)]);
    fallback_dispatcher
        .resolve(&registry, &metric_registry)
        .unwrap();
    let graph = asset(
        vec![node(
            1,
            NodeKind::SalienceSelector {
                pool: vec![1],
                fallback_storylet_id: Some(2),
                fallback_output_pin: String::new(),
            },
            NodeCategory::DynamicDispatcher,
        )],
        1,
    );
    let mut state = WorldStateTable::new();
    state
        .global_tags
        .set_bit(registry.id("tag.match").unwrap())
        .unwrap();
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut ctx = RuntimeContext::with_registry(&mut state, &fallback_dispatcher, &registry);

    // No arc phase: the gated candidate is rejected before scoring.
    instance.force_complete(&mut ctx, 1, "Out", None).unwrap();
    assert_eq!(instance.node(1).unwrap().chosen_storylet_id, Some(2));

    let mut state = WorldStateTable::new();
    state
        .global_tags
        .set_bit(registry.id("tag.match").unwrap())
        .unwrap();
    state.global_tags.set_bit(phase_bit).unwrap();
    let mut instance = FlowGraphInstance::new(2, instance.asset.clone(), 7).unwrap();
    let mut ctx = RuntimeContext::with_registry(&mut state, &fallback_dispatcher, &registry);
    instance.force_complete(&mut ctx, 1, "Out", None).unwrap();
    assert_eq!(instance.node(1).unwrap().chosen_storylet_id, Some(1));
}

#[test]
fn zero_cost_cycle_through_a_choice_is_allowed() {
    let graph = asset(
        vec![
            node(
                1,
                NodeKind::Choice {
                    default_output_pin: "Out".to_owned(),
                },
                NodeCategory::DynamicDispatcher,
            ),
            node(2, noop_mutation(), NodeCategory::Mutator),
        ],
        1,
    );
    let mut graph = graph;
    graph.edges = vec![edge(1, "Out", 2, "In"), edge(2, "Out", 1, "In")];
    assert!(FlowGraphInstance::new(1, graph, 7).is_ok());
}

#[test]
fn signal_wake_leaves_one_inverse_record() {
    let (dispatcher, registry) = runtime();
    let graph = asset(
        vec![node(
            1,
            NodeKind::Waiter {
                duration_ms: 1_000,
                required_tags: TagBitset::new(),
                wait_event: "open".to_owned(),
            },
            NodeCategory::Waiter,
        )],
        1,
    );
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    let token = SignalToken {
        token_id: 0,
        graph_instance_id: instance.instance_id,
        target_node_guid: 1,
        target_pin_index: 0,
        time_budget_ms: 16,
        flags: TOKEN_ACTIVE,
        deferred_until_ms: 0,
    };
    instance.execute_input(&mut ctx, 1, "In", &token).unwrap();
    instance.signal_event(&mut ctx, "open").unwrap();

    assert_eq!(instance.node(1).unwrap().call_data.len(), 1);
    instance.cancel_and_revert(&mut ctx, 1).unwrap();
    assert!(instance.node(1).unwrap().call_data.is_empty());
    assert!(matches!(
        instance.cancel_and_revert(&mut ctx, 1),
        Err(GraphError::RevertStackEmpty { .. })
    ));
}

#[test]
fn execute_input_rejects_a_foreign_token() {
    let (dispatcher, registry) = runtime();
    let graph = asset(vec![node(1, noop_mutation(), NodeCategory::Mutator)], 1);
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    let token = SignalToken {
        token_id: 0,
        graph_instance_id: 999,
        target_node_guid: 1,
        target_pin_index: 0,
        time_budget_ms: 16,
        flags: TOKEN_ACTIVE,
        deferred_until_ms: 0,
    };
    assert_eq!(
        instance.execute_input(&mut ctx, 1, "In", &token),
        Err(GraphError::TokenInstanceMismatch {
            token_instance_id: 999,
            instance_id: 1,
        })
    );
}

#[test]
fn concurrent_sections_arbitrate_in_one_composer_pass() {
    use storylet_engine::chrono_runtime::TimelineTrackEvent;

    let scene = |guid: u32, offset: i32, pose: &str, previous: &str| {
        node(
            guid,
            NodeKind::SceneSection {
                section_duration_ms: 10,
                events: vec![TimelineTrackEvent {
                    event_id: u64::from(guid),
                    action_type: Default::default(),
                    target_entity_path: "actor".to_owned(),
                    start_time_ms: 0,
                    duration_ms: 10,
                    payload_asset_id: pose.to_owned(),
                    cached_previous_pose_id: previous.to_owned(),
                    priority_offset: offset,
                }],
            },
            NodeCategory::Mutator,
        )
    };
    let graph = asset(
        vec![
            scene(10, 2, "pose.10", "pose.prev.10"),
            scene(11, 4, "pose.11", "pose.prev.11"),
        ],
        10,
    );
    let (dispatcher, registry) = runtime();
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    for guid in [10u32, 11u32] {
        let token = SignalToken {
            token_id: u64::from(guid),
            graph_instance_id: instance.instance_id,
            target_node_guid: guid,
            target_pin_index: 0,
            time_budget_ms: 16,
            flags: TOKEN_ACTIVE,
            deferred_until_ms: 0,
        };
        instance.execute_input(&mut ctx, guid, "In", &token).unwrap();
    }

    instance.tick(&mut ctx, 10).unwrap();

    assert_eq!(instance.composer.conflicts.len(), 1);
    let conflict = &instance.composer.conflicts[0];
    assert_eq!(conflict.winner.node_guid, 11);
    assert_eq!(conflict.rejected.len(), 1);
    assert_eq!(conflict.rejected[0].node_guid, 10);
    assert_eq!(
        instance.composer.actor_pose.get("actor").map(String::as_str),
        Some("pose.11")
    );

    // Reverse the frame in descending GUID order; each section restores its
    // own authored prior pose, so skipping either one fails.
    instance.process_time_window(11, -10).unwrap();
    assert_eq!(
        instance.composer.actor_pose.get("actor").map(String::as_str),
        Some("pose.prev.11")
    );
    instance.process_time_window(10, -10).unwrap();
    assert_eq!(
        instance.composer.actor_pose.get("actor").map(String::as_str),
        Some("pose.prev.10")
    );
}

#[test]
fn composer_records_conflicts_for_one_actor() {
    use storylet_engine::chrono_runtime::TimelineTrackEvent;

    let event = |event_id: u64, offset: i32| TimelineTrackEvent {
        event_id,
        action_type: Default::default(),
        target_entity_path: "actor".to_owned(),
        start_time_ms: 0,
        duration_ms: 50,
        payload_asset_id: format!("pose.{event_id}"),
        priority_offset: offset,
        ..TimelineTrackEvent::default()
    };
    let graph = asset(
        vec![node(
            1,
            NodeKind::SceneSection {
                section_duration_ms: 50,
                events: vec![event(1, 0), event(2, 10)],
            },
            NodeCategory::Mutator,
        )],
        1,
    );
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();

    instance.process_time_window(1, 50).unwrap();

    assert_eq!(instance.composer.conflicts.len(), 1);
    assert_eq!(
        instance.composer.conflicts[0].winner.event_id,
        2,
        "the higher priority event wins"
    );
}

#[test]
fn backward_scrub_reverses_the_timeline_window() {
    use storylet_engine::chrono_runtime::{EventOverlapActivity, TimelineTrackEvent};

    let graph = asset(
        vec![node(
            1,
            NodeKind::SceneSection {
                section_duration_ms: 100,
                events: vec![TimelineTrackEvent {
                    event_id: 1,
                    action_type: Default::default(),
                    target_entity_path: "actor".to_owned(),
                    start_time_ms: 10,
                    duration_ms: 10,
                    payload_asset_id: "pose.a".to_owned(),
                    ..TimelineTrackEvent::default()
                }],
            },
            NodeCategory::Mutator,
        )],
        1,
    );
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();

    let forward = instance.process_time_window(1, 100).unwrap();
    assert_eq!(forward.len(), 1);
    assert_eq!(forward[0].activity, EventOverlapActivity::StartActivity);
    assert_eq!(instance.node(1).unwrap().section_clock_ms, 100);
    assert_eq!(instance.call_history().len(), 1);

    let backward = instance.process_time_window(1, -100).unwrap();
    assert_eq!(backward.len(), 1);
    assert_eq!(backward[0].activity, EventOverlapActivity::EndActivity);
    assert_eq!(backward[0].local_cursor_time_ms, -10);
    assert_eq!(instance.node(1).unwrap().section_clock_ms, 0);
    assert_eq!(
        instance.node(1).unwrap().state,
        NodeExecutionState::ActiveLatent
    );

    assert_eq!(instance.call_history().len(), 0);

    let replay = instance.process_time_window(1, 100).unwrap();
    assert_eq!(replay.len(), 1);
    assert_eq!(
        instance.node(1).unwrap().state,
        NodeExecutionState::Completed
    );
    assert_eq!(instance.call_history().len(), 1);
}

#[test]
fn present_deck_hides_storylets_on_millisecond_cooldown() {
    let registry = tags(&["case.open"]);
    let metric_registry = MetricRegistry::empty();
    let mut storylet = repeatable_storylet(5);
    storylet.cooldown_ms = 100;
    let mut dispatcher = UnifiedDispatcher::new(vec![storylet]);
    dispatcher.resolve(&registry, &metric_registry).unwrap();
    let mut hub = node(
        1,
        NodeKind::QbnHub {
            deck: vec![5],
            exit_storylet_id: None,
        },
        NodeCategory::DynamicDispatcher,
    );
    hub.output_pins = vec![pin("choice_5", false), pin("Out", false)];
    let graph = asset(vec![hub], 1);
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    state
        .global_tags
        .set_bit(registry.id("case.open").unwrap())
        .unwrap();
    let ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);

    instance.last_completed_ms.insert(5, 0);
    instance.clock_ms = 50;
    assert!(instance.present_deck(&ctx, 1).unwrap().is_empty());

    instance.clock_ms = 100;
    assert_eq!(instance.present_deck(&ctx, 1).unwrap(), vec![5]);
}

#[test]
fn once_only_completion_lock_is_global_across_hubs() {
    let registry = tags(&["case.open"]);
    let metric_registry = MetricRegistry::empty();
    let mut def = repeatable_storylet(5);
    def.is_repeatable = false;
    let mut dispatcher = UnifiedDispatcher::new(vec![def]);
    dispatcher.resolve(&registry, &metric_registry).unwrap();
    let mut first = node(
        1,
        NodeKind::QbnHub {
            deck: vec![5],
            exit_storylet_id: None,
        },
        NodeCategory::DynamicDispatcher,
    );
    first.output_pins = vec![pin("choice_5", false), pin("Out", false)];
    let mut second = node(
        2,
        NodeKind::QbnHub {
            deck: vec![5],
            exit_storylet_id: None,
        },
        NodeCategory::DynamicDispatcher,
    );
    second.output_pins = vec![pin("choice_5", false), pin("Out", false)];
    let graph = asset(vec![first, second], 1);
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    state
        .global_tags
        .set_bit(registry.id("case.open").unwrap())
        .unwrap();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);

    instance.select_storylet(&mut ctx, 1, 5).unwrap();

    assert!(instance.present_deck(&ctx, 2).unwrap().is_empty());
    assert_eq!(
        instance.select_storylet(&mut ctx, 2, 5),
        Err(GraphError::StoryletNotPlayable {
            node_guid: 2,
            storylet_id: 5,
        })
    );
}

#[test]
fn pattern_matching_backtracks_to_a_valid_assignment() {
    let mut state = WorldStateTable::new();
    state.vertices.push(WorldVertex {
        vertex_id: "v.a".to_owned(),
        layer: GraphLayer::Location,
        entity_index: None,
        attributes: BTreeMap::new(),
    });
    state.vertices.push(WorldVertex {
        vertex_id: "v.b".to_owned(),
        layer: GraphLayer::Location,
        entity_index: None,
        attributes: BTreeMap::new(),
    });
    state
        .edges
        .push(storylet_engine::chrono_state::WorldEdge {
            edge_id: "e.b.a".to_owned(),
            source_id: "v.b".to_owned(),
            target_id: "v.a".to_owned(),
            relation_type: "path".to_owned(),
            weight: 1.0,
        });
    let pattern = GraphPattern {
        vertices: vec![
            VertexPattern {
                layer: GraphLayer::Location,
                ..VertexPattern::default()
            },
            VertexPattern {
                layer: GraphLayer::Location,
                ..VertexPattern::default()
            },
        ],
        edges: vec![EdgePattern {
            source: Some(GraphBinding::VertexPattern(0)),
            target: Some(GraphBinding::VertexPattern(1)),
            ..EdgePattern::default()
        }],
    };

    let matched = state.match_graph_pattern(&pattern).unwrap();

    assert_eq!(matched.vertices, vec!["v.b".to_owned(), "v.a".to_owned()]);
    assert_eq!(matched.edges, vec!["e.b.a".to_owned()]);
}

#[test]
fn pattern_matching_backtracks_across_a_chain() {
    let mut state = WorldStateTable::new();
    for id in ["v.a", "v.b", "v.c"] {
        state.vertices.push(WorldVertex {
            vertex_id: id.to_owned(),
            layer: GraphLayer::Location,
            entity_index: None,
            attributes: BTreeMap::new(),
        });
    }
    for (edge_id, source, target) in [
        ("e.b.c", "v.b", "v.c"),
        ("e.c.a", "v.c", "v.a"),
    ] {
        state
            .edges
            .push(storylet_engine::chrono_state::WorldEdge {
                edge_id: edge_id.to_owned(),
                source_id: source.to_owned(),
                target_id: target.to_owned(),
                relation_type: "path".to_owned(),
                weight: 1.0,
            });
    }
    let vertex = || VertexPattern {
        layer: GraphLayer::Location,
        ..VertexPattern::default()
    };
    let chain = |from: usize, to: usize| EdgePattern {
        source: Some(GraphBinding::VertexPattern(from)),
        target: Some(GraphBinding::VertexPattern(to)),
        ..EdgePattern::default()
    };
    let pattern = GraphPattern {
        vertices: vec![vertex(), vertex(), vertex()],
        edges: vec![chain(0, 1), chain(1, 2)],
    };

    let matched = state.match_graph_pattern(&pattern).unwrap();

    assert_eq!(
        matched.vertices,
        vec!["v.b".to_owned(), "v.c".to_owned(), "v.a".to_owned()]
    );
    assert_eq!(
        matched.edges,
        vec!["e.b.c".to_owned(), "e.c.a".to_owned()]
    );
}

#[test]
fn empty_passion_pool_is_rejected() {
    let result = PassionRegistry::compile(
        [StarNpcDef {
            npc_id: 0,
            trust_inertia: 0.0,
            micro_passions: vec![],
            macro_bounds: Default::default(),
            zero_sum_links: vec![],
        }],
        [],
    );

    assert!(matches!(
        result,
        Err(storylet_engine::chrono_state::ChronoStateError::EmptyPassionPool { npc_id: 0 })
    ));
}

#[test]
fn macro_passions_clamp_to_the_spec_range() {
    let registry = PassionRegistry::compile(
        [StarNpcDef {
            npc_id: 0,
            // Micro values clamp at 100; inertia pushes the raw macro to 150,
            // so only the spec clamp keeps the result at 100.
            trust_inertia: 50.0,
            micro_passions: vec![MicroPassionDef {
                npc_id: 0,
                passion_id: 1,
                weight: 1.0,
                is_hidden: false,
            }],
            macro_bounds: Default::default(),
            zero_sum_links: vec![],
        }],
        [],
    )
    .unwrap();
    let mut state = WorldStateTable::new();
    state.passions.active[1] = true;
    let mut flags = TagBitset::new();

    storylet_engine::chrono_state::apply_passion_mutation(
        &mut state.passions,
        &PassionMutation {
            micro_deltas: vec![(1, 400.0)],
            active_ops: vec![],
        },
        &registry,
        &mut flags,
    )
    .unwrap();

    assert_eq!(state.passions.micro[1], 100.0);
    assert_eq!(state.passions.macro_values[0], 100.0);
}

#[test]
fn threshold_events_follow_the_value_transition() {
    let registry = tags(&["passion.high"]);
    let bit = registry.id("passion.high").unwrap();
    let passions = PassionRegistry::compile(
        [StarNpcDef {
            npc_id: 0,
            trust_inertia: 0.0,
            micro_passions: vec![MicroPassionDef {
                npc_id: 0,
                passion_id: 1,
                weight: 1.0,
                is_hidden: false,
            }],
            macro_bounds: Default::default(),
            zero_sum_links: vec![],
        }],
        [PassionThresholdRule {
            npc_id: 0,
            threshold_tag_bit: bit,
            trigger_value: 0.5,
            direction: PassionDirection::Up,
            linked_graph_asset_id: "asset.threshold".to_owned(),
        }],
    )
    .unwrap();
    let mut state = WorldStateTable::new();
    state.passions.active[1] = true;
    state.global_tags.set_bit(bit).unwrap();
    let mut flags = state.global_tags;

    let delta = storylet_engine::chrono_state::apply_passion_mutation(
        &mut state.passions,
        &PassionMutation {
            micro_deltas: vec![(1, 1.0)],
            active_ops: vec![],
        },
        &passions,
        &mut flags,
    )
    .unwrap();

    assert_eq!(delta.threshold_events.len(), 1);
    assert!(delta.threshold_events[0].activated);
    assert!(flags.bit(bit));
}

#[test]
fn transform_spawns_vertices_and_edges_in_one_rule() {
    let mut state = WorldStateTable::new();
    let rule = GraphTransformRule {
        lhs: GraphPattern::default(),
        rhs: GraphMutation {
            add_vertices: vec![
                VertexTemplate {
                    id: GraphBinding::Literal("v.spawn_a".to_owned()),
                    layer: GraphLayer::Location,
                    attributes: BTreeMap::new(),
                },
                VertexTemplate {
                    id: GraphBinding::Literal("v.spawn_b".to_owned()),
                    layer: GraphLayer::Location,
                    attributes: BTreeMap::new(),
                },
            ],
            add_edges: vec![EdgeTemplate {
                id: GraphBinding::Literal("e.spawn".to_owned()),
                source: GraphBinding::Literal("v.spawn_a".to_owned()),
                target: GraphBinding::Literal("v.spawn_b".to_owned()),
                relation_type: "path".to_owned(),
                weight: 1.0,
            }],
            ..GraphMutation::default()
        },
    };
    let before = state_snapshot(&state);
    let diff = state.apply_graph_transform(&rule).unwrap();

    assert!(state.vertex("v.spawn_a").is_some());
    assert!(state.edge("e.spawn").is_some());

    state.revert_graph_transform(&diff).unwrap();
    assert_eq!(state_snapshot(&state), before);
}

#[test]
fn transform_removes_the_edge_bound_by_the_pattern() {
    let mut state = WorldStateTable::new();
    let seed = GraphTransformRule {
        lhs: GraphPattern::default(),
        rhs: GraphMutation {
            add_vertices: vec![
                VertexTemplate {
                    id: GraphBinding::Literal("v.a".to_owned()),
                    layer: GraphLayer::Location,
                    attributes: BTreeMap::new(),
                },
                VertexTemplate {
                    id: GraphBinding::Literal("v.b".to_owned()),
                    layer: GraphLayer::Location,
                    attributes: BTreeMap::new(),
                },
            ],
            add_edges: vec![EdgeTemplate {
                id: GraphBinding::Literal("e.match".to_owned()),
                source: GraphBinding::Literal("v.a".to_owned()),
                target: GraphBinding::Literal("v.b".to_owned()),
                relation_type: "path".to_owned(),
                weight: 1.0,
            }],
            ..GraphMutation::default()
        },
    };
    state.apply_graph_transform(&seed).unwrap();
    let before = state_snapshot(&state);
    let remove = GraphTransformRule {
        lhs: GraphPattern {
            vertices: vec![
                VertexPattern {
                    vertex_id: Some("v.a".to_owned()),
                    ..VertexPattern::default()
                },
                VertexPattern {
                    vertex_id: Some("v.b".to_owned()),
                    ..VertexPattern::default()
                },
            ],
            edges: vec![EdgePattern {
                edge_id: Some("e.match".to_owned()),
                source: Some(GraphBinding::VertexPattern(0)),
                target: Some(GraphBinding::VertexPattern(1)),
                ..EdgePattern::default()
            }],
        },
        rhs: GraphMutation {
            remove_edges: vec![GraphBinding::EdgePattern(0)],
            ..GraphMutation::default()
        },
    };
    let diff = state.apply_graph_transform(&remove).unwrap();

    assert!(state.edge("e.match").is_none());

    state.revert_graph_transform(&diff).unwrap();
    assert_eq!(state_snapshot(&state), before);
}

#[test]
fn transform_can_replace_a_vertex_in_one_rule() {
    let mut state = WorldStateTable::new();
    state
        .apply_graph_transform(&GraphTransformRule {
            lhs: GraphPattern::default(),
            rhs: GraphMutation {
                add_vertices: vec![VertexTemplate {
                    id: GraphBinding::Literal("v.actor".to_owned()),
                    layer: GraphLayer::Location,
                    attributes: BTreeMap::from([(
                        "room".to_owned(),
                        AttributeValue::String("hall".to_owned()),
                    )]),
                }],
                ..GraphMutation::default()
            },
        })
        .unwrap();
    let before = state_snapshot(&state);
    let replace = GraphTransformRule {
        lhs: GraphPattern {
            vertices: vec![VertexPattern {
                vertex_id: Some("v.actor".to_owned()),
                ..VertexPattern::default()
            }],
            edges: Vec::new(),
        },
        rhs: GraphMutation {
            remove_vertices: vec![GraphBinding::VertexPattern(0)],
            add_vertices: vec![VertexTemplate {
                id: GraphBinding::Literal("v.actor".to_owned()),
                layer: GraphLayer::Location,
                attributes: BTreeMap::from([(
                    "room".to_owned(),
                    AttributeValue::String("garden".to_owned()),
                )]),
            }],
            ..GraphMutation::default()
        },
    };
    let diff = state.apply_graph_transform(&replace).unwrap();

    assert_eq!(
        state
            .vertex("v.actor")
            .unwrap()
            .attributes
            .get("room"),
        Some(&AttributeValue::String("garden".to_owned()))
    );

    state.revert_graph_transform(&diff).unwrap();
    assert_eq!(state_snapshot(&state), before);
}

#[test]
fn zero_cost_fan_out_hits_the_per_tick_execution_cap() {
    let (dispatcher, registry) = runtime();
    let layers = 14u32;
    let mut nodes = Vec::new();
    for layer in 0..layers {
        nodes.push(node(layer + 1, noop_mutation(), NodeCategory::Mutator));
    }
    let mut edges = Vec::new();
    for layer in 1..layers {
        edges.push(edge(layer, "Out", layer + 1, "In"));
        edges.push(edge(layer, "Out", layer + 1, "In"));
    }
    let mut graph = asset(nodes, 1);
    graph.edges = edges;
    let mut instance = FlowGraphInstance::new(1, graph, 7).unwrap();
    let mut state = WorldStateTable::new();
    let mut ctx = RuntimeContext::with_registry(&mut state, &dispatcher, &registry);
    instance.start_at_node(1).unwrap();

    assert!(matches!(
        instance.tick(&mut ctx, 16),
        Err(GraphError::ExecutionLimitExceeded { .. })
    ));
}
