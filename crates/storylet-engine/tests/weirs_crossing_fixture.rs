#[path = "support/weirs_crossing.rs"]
mod weirs_crossing;

use storylet_engine::chrono_participants::Controller;
use storylet_engine::chrono_space::OnClose;

#[test]
fn fixture_contains_the_full_cast_world_clock_and_authored_storylets() {
    let content = weirs_crossing::explorer_content();
    let participants = &content.initial_world.participants;
    assert_eq!(participants.keys, ["mara", "teodor", "benedek", "rosa"]);
    assert_eq!(
        participants.location,
        [
            Some(weirs_crossing::INN),
            Some(weirs_crossing::HARBOR),
            Some(weirs_crossing::CUSTOMS),
            Some(weirs_crossing::INN)
        ]
    );
    assert_eq!(participants.controller[0], Controller::Human { seat: 0 });
    assert_eq!(participants.controller[1], Controller::Rule);
    assert_eq!(content.clock.as_ref().unwrap().slot_labels.len(), 6);
    assert_eq!(content.storylet_count(), 17);
    for id in 0..=weirs_crossing::ENDING_LOW_WATER {
        assert!(
            content.dispatcher.storylet(id).is_some(),
            "missing storylet {id}"
        );
        assert!(
            content.storylet_titles.contains_key(&id),
            "missing title for storylet {id}"
        );
    }
    for id in weirs_crossing::ENDING_LIGHT_RESTORED..=weirs_crossing::ENDING_LOW_WATER {
        assert!(content.dispatcher.storylet(id).unwrap().is_ending);
    }
    assert!(content.connections.iter().any(|edge| {
        edge.from == weirs_crossing::LIGHTHOUSE
            && edge.to == weirs_crossing::ROCK
            && edge.open_slots.is_some()
            && edge.requires_fact == content.registry.id("fact.tides")
    }));
    assert_eq!(
        content
            .connections
            .iter()
            .find(|e| e.to == weirs_crossing::ROCK)
            .unwrap()
            .on_close,
        Some(OnClose::Return {
            to: weirs_crossing::LIGHTHOUSE
        })
    );
}

#[test]
fn fixture_variants_support_coop_and_replayable_stranding_cases() {
    let coop = weirs_crossing::content_all_humans();
    assert_eq!(
        coop.initial_world.participants.controller[1],
        Controller::Human { seat: 1 }
    );
    let broken = weirs_crossing::content_without_causeway_close_rule();
    assert_eq!(
        broken
            .connections
            .iter()
            .find(|e| e.to == weirs_crossing::ROCK)
            .unwrap()
            .on_close,
        None
    );
}

#[test]
fn fixture_has_four_canon_entries_and_reveal_nodes() {
    use storylet_engine::chrono_runtime::NodeKind;
    let content = weirs_crossing::explorer_content();
    assert_eq!(
        content
            .chronicle
            .entries
            .iter()
            .map(|e| e.key.as_str())
            .collect::<Vec<_>>(),
        ["P1", "P2", "P3", "P4"]
    );
    for entry in 0..4 {
        assert!(
            content
                .root_asset
                .nodes
                .iter()
                .any(|node| matches!(node.kind, NodeKind::Reveal { entry: id, .. } if id == entry)),
            "missing reveal for entry {entry}"
        );
    }
}

#[test]
fn fixture_paths_match_the_prototype_map_in_both_directions() {
    let content = weirs_crossing::explorer_content();
    for (a, b) in [
        (weirs_crossing::LIGHTHOUSE, weirs_crossing::HARBOR),
        (weirs_crossing::LIGHTHOUSE, weirs_crossing::ROCK),
        (weirs_crossing::HARBOR, weirs_crossing::INN),
        (weirs_crossing::HARBOR, weirs_crossing::CUSTOMS),
        (weirs_crossing::INN, weirs_crossing::CUSTOMS),
    ] {
        assert!(content.connections.iter().any(|e| e.from == a && e.to == b));
        assert!(content.connections.iter().any(|e| e.from == b && e.to == a));
    }
}

#[test]
fn piece_together_requires_all_three_clues_from_the_actor() {
    use storylet_engine::chrono_participants::{KnowKind, KnowledgeRow};
    use storylet_engine::chrono_runtime::{FlowGraphInstance, RuntimeContext};
    let content = weirs_crossing::explorer_content();
    let graph = content
        .dispatcher
        .storylet(weirs_crossing::PIECE_TOGETHER)
        .unwrap()
        .precondition_graph
        .as_ref()
        .expect("piece together must have a knowledge gate");
    let mut world = content.initial_world.clone();
    let clues = [
        "fact.benedek_suspected",
        "fact.light_doused",
        "fact.crates_landed",
    ];
    for (i, clue) in clues.iter().enumerate() {
        let ctx = RuntimeContext::with_registries(
            &mut world,
            &content.dispatcher,
            &content.registry,
            &content.passion_registry,
        );
        assert!(
            !FlowGraphInstance::evaluate_precondition_graph_for(graph, &ctx, 0).unwrap(),
            "passed with only {i} clues"
        );
        world.knowledge.learn(KnowledgeRow {
            holder: 0,
            fact: content.registry.id(clue).unwrap(),
            kind: KnowKind::Evidence,
            source_event: 0,
        });
    }
    let ctx = RuntimeContext::with_registries(
        &mut world,
        &content.dispatcher,
        &content.registry,
        &content.passion_registry,
    );
    assert!(FlowGraphInstance::evaluate_precondition_graph_for(graph, &ctx, 0).unwrap());
    assert!(!FlowGraphInstance::evaluate_precondition_graph_for(graph, &ctx, 1).unwrap());
    drop(ctx);
    world.knowledge.learn(KnowledgeRow {
        holder: 0,
        fact: content.registry.id("fact.benedek_identified").unwrap(),
        kind: KnowKind::Evidence,
        source_event: 0,
    });
    let ctx = RuntimeContext::with_registries(
        &mut world,
        &content.dispatcher,
        &content.registry,
        &content.passion_registry,
    );
    assert!(
        !FlowGraphInstance::evaluate_precondition_graph_for(graph, &ctx, 0).unwrap(),
        "already identified should disable deduction"
    );
}

#[test]
fn reveal_records_the_matching_past_marker_and_telling() {
    use storylet_engine::chrono_participants::KnowKind;
    use storylet_engine::chrono_runtime::{
        EdgeConnection, FlowGraphAsset, FlowNodeDef, NodeCategory, NodeKind, PinDefinition,
        Visibility,
    };
    use storylet_engine::run::{RunKernel, RunStart};
    let mut content = weirs_crossing::explorer_content();
    let facts = content.chronicle.entries[0].facts.clone();
    let pin = |name: &str, is_input: bool| PinDefinition {
        pin_name: name.into(),
        is_input,
    };
    let reveal = FlowNodeDef {
        node_guid: 1,
        node_type_id: "reveal".into(),
        category: NodeCategory::Mutator,
        kind: NodeKind::Reveal {
            entry: 0,
            kind: KnowKind::Recollection,
            visibility: Visibility::Private,
            facts,
        },
        input_pins: vec![pin("In", true)],
        output_pins: vec![pin("Out", false)],
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
    content.root_asset = FlowGraphAsset {
        asset_id: "past-test".into(),
        start_node_guid: 1,
        nodes: vec![reveal, end],
        edges: vec![EdgeConnection {
            source_node_guid: 1,
            source_pin_name: "Out".into(),
            target_node_guid: 2,
            target_pin_name: "In".into(),
        }],
        data_edges: vec![],
    };
    let run = RunKernel::start(&content, RunStart::Beginning, 1).unwrap();
    let marker = content
        .registry
        .id("fact.past.P1")
        .expect("P1 marker is registered");
    assert!(run.world.knowledge.knows(0, marker).is_some());
    assert!(run.world.telling.is_told(0, 0));
}

#[test]
fn bluff_requires_a_suspect_real_evidence_and_benedek_present() {
    use storylet_engine::chrono_participants::{KnowKind, KnowledgeRow};
    use storylet_engine::chrono_runtime::{FlowGraphInstance, RuntimeContext};
    let content = weirs_crossing::content_all_humans();
    let graph = content
        .dispatcher
        .storylet(weirs_crossing::BLUFF_BENEDEK)
        .unwrap()
        .precondition_graph
        .as_ref()
        .expect("bluff must be gated");
    let mut world = content.initial_world.clone();
    world.participants.location[0] = Some(weirs_crossing::CUSTOMS);
    let check = |world: &mut storylet_engine::chrono_state::WorldStateTable| {
        let ctx = RuntimeContext::with_registries(
            world,
            &content.dispatcher,
            &content.registry,
            &content.passion_registry,
        );
        FlowGraphInstance::evaluate_precondition_graph_for(graph, &ctx, 0).unwrap()
    };
    assert!(!check(&mut world));
    for (fact, expected) in [
        ("fact.benedek_suspected", false),
        ("fact.light_doused", true),
    ] {
        world.knowledge.learn(KnowledgeRow {
            holder: 0,
            fact: content.registry.id(fact).unwrap(),
            kind: KnowKind::Evidence,
            source_event: 0,
        });
        assert_eq!(check(&mut world), expected);
    }
    world.participants.location[weirs_crossing::BENEDEK as usize] = Some(weirs_crossing::INN);
    assert!(!check(&mut world));
    world.participants.location[weirs_crossing::BENEDEK as usize] = Some(weirs_crossing::CUSTOMS);
    world.knowledge.learn(KnowledgeRow {
        holder: 0,
        fact: content.registry.id("fact.benedek_identified").unwrap(),
        kind: KnowKind::Evidence,
        source_event: 0,
    });
    assert!(!check(&mut world));
}
